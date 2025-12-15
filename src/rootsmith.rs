use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::commitment_registry::CommitmentRegistryVariant;
use crate::config::{AccumulatorType, BaseConfig};
use crate::crypto::AccumulatorVariant;
use crate::proof_registry::ProofRegistryVariant;
use crate::storage::Storage;
use crate::traits::{Accumulator, CommitmentRegistry, ProofRegistry, UpstreamConnector};
use crate::types::{
    BatchCommitmentMeta, Commitment, IncomingRecord, Namespace, StoredProof, Value32,
};
use crate::upstream::UpstreamVariant;
use anyhow::Result;
use kanal::{unbounded_async, AsyncReceiver};
use tracing::{debug, error, info, span, Level};

/// Epoch phase for the commit cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpochPhase {
    /// Waiting for the next commit window.
    Pending,
    /// Actively committing batches.
    Commit,
}

/// Record tracked in memory for pruning after commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CommittedRecord {
    namespace: Namespace,
    key: [u8; 32],
    value: Value32,
    timestamp: u64,
}

/// Main application orchestrator with epoch-based architecture.
pub struct RootSmith {
    /// Upstream connector.
    pub upstream: UpstreamVariant,

    /// Commitment registry implementation.
    pub commitment_registry: CommitmentRegistryVariant,

    /// Proof registry implementation.
    pub proof_registry: ProofRegistryVariant,

    /// Global/base configuration.
    pub config: BaseConfig,

    /// Persistent storage (RocksDB).
    pub storage: Arc<Mutex<Storage>>,

    /// Start time of the current epoch (unix seconds).
    epoch_start_ts: Arc<Mutex<u64>>,

    /// Track active namespaces for efficient commit.
    active_namespaces: Arc<Mutex<HashMap<Namespace, bool>>>,

    /// Track committed records for pruning in pending phase.
    committed_records: Arc<Mutex<Vec<CommittedRecord>>>,
}

impl RootSmith {
    /// Create a new RootSmith.
    pub fn new(
        upstream: UpstreamVariant,
        commitment_registry: CommitmentRegistryVariant,
        proof_registry: ProofRegistryVariant,
        config: BaseConfig,
        storage: Storage,
    ) -> Self {
        let now = Self::now_secs();

        Self {
            upstream,
            commitment_registry,
            proof_registry,
            config,
            storage: Arc::new(Mutex::new(storage)),
            epoch_start_ts: Arc::new(Mutex::new(now)),
            active_namespaces: Arc::new(Mutex::new(HashMap::new())),
            committed_records: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn initialize(config: BaseConfig) -> Result<Self> {
        use crate::commitment_registry::commitment_noop::CommitmentNoop;
        use crate::commitment_registry::CommitmentRegistryVariant;
        use crate::proof_registry::{NoopProofRegistry, ProofRegistryVariant};
        use crate::upstream::NoopUpstream;

        let storage = Storage::open(&config.storage_path)?;
        info!("Storage opened at: {}", config.storage_path);

        let upstream = UpstreamVariant::Noop(NoopUpstream);
        let commitment_registry = CommitmentRegistryVariant::Noop(CommitmentNoop::new());
        let proof_registry = ProofRegistryVariant::Noop(NoopProofRegistry);

        Ok(Self::new(
            upstream,
            commitment_registry,
            proof_registry,
            config,
            storage,
        ))
    }

    pub async fn run(self) -> Result<()> {
        let span = span!(Level::INFO, "app_run");
        let _enter = span.enter();

        info!(
            "Starting RootSmith with batch_interval_secs={}",
            self.config.batch_interval_secs
        );

        let (data_tx, data_rx) = unbounded_async::<IncomingRecord>();

        info!("Starting upstream connector: {}", self.upstream.name());

        let upstream_handle = {
            let async_tx = data_tx.clone();
            let mut upstream = self.upstream;
            tokio::task::spawn(async move {
                let span = span!(Level::INFO, "upstream_task");
                let _enter = span.enter();

                if let Err(e) = upstream.open(async_tx).await {
                    error!("Upstream connector failed: {}", e);
                    return Err(e);
                }

                info!("Upstream connector finished");
                Ok(())
            })
        };

        let process_handle = {
            let storage = Arc::clone(&self.storage);
            let epoch_start_ts = Arc::clone(&self.epoch_start_ts);
            let active_namespaces = Arc::clone(&self.active_namespaces);
            let committed_records = Arc::clone(&self.committed_records);
            let commitment_registry = self.commitment_registry;
            let proof_registry = self.proof_registry;
            let config = self.config.clone();

            tokio::task::spawn(async move {
                let span = span!(Level::INFO, "process_and_commit_task");
                let _enter = span.enter();

                Self::process_and_commit_loop(
                    data_rx,
                    storage,
                    epoch_start_ts,
                    active_namespaces,
                    committed_records,
                    commitment_registry,
                    proof_registry,
                    config,
                )
                .await
            })
        };

        let upstream_result = upstream_handle
            .await
            .map_err(|_| anyhow::anyhow!("Upstream task panicked"))?;

        drop(data_tx);

        let process_result = process_handle
            .await
            .map_err(|_| anyhow::anyhow!("Process task panicked"))?;

        upstream_result?;
        process_result?;

        info!("RootSmith run completed successfully");
        Ok(())
    }

    async fn process_and_commit_loop(
        data_rx: AsyncReceiver<IncomingRecord>,
        storage: Arc<Mutex<Storage>>,
        epoch_start_ts: Arc<Mutex<u64>>,
        active_namespaces: Arc<Mutex<HashMap<Namespace, bool>>>,
        committed_records: Arc<Mutex<Vec<CommittedRecord>>>,
        commitment_registry: CommitmentRegistryVariant,
        proof_registry: ProofRegistryVariant,
        config: BaseConfig,
    ) -> Result<()> {
        loop {
            let should_commit = {
                let epoch_start = *epoch_start_ts.lock().unwrap();
                let now = Self::now_secs();
                let elapsed = now.saturating_sub(epoch_start);
                elapsed >= config.batch_interval_secs
            };

            if should_commit {
                info!("Commit phase starting - DB writes continue freely");

                Self::perform_commit_nonblocking(
                    &storage,
                    &epoch_start_ts,
                    &active_namespaces,
                    &committed_records,
                    &commitment_registry,
                    &proof_registry,
                    &config,
                )
                .await?;

                info!("Commit phase completed - entering pending phase");

                Self::prune_committed_data(&storage, &committed_records)?;
            }

            match tokio::time::timeout(Duration::from_millis(100), data_rx.recv()).await {
                Ok(Ok(record)) => {
                    let span = span!(Level::DEBUG, "handle_record", 
                        namespace = ?record.namespace);
                    let _enter = span.enter();

                    let storage_guard = storage.lock().unwrap();

                    {
                        let mut namespaces = active_namespaces.lock().unwrap();
                        namespaces.insert(record.namespace, true);
                    }

                    storage_guard.put(&record)?;
                    debug!("Record stored for namespace {:?}", record.namespace);
                }
                Ok(Err(_)) => {
                    info!("Data channel closed, performing final commit if needed");

                    let should_final_commit = {
                        let namespaces = active_namespaces.lock().unwrap();
                        !namespaces.is_empty()
                    };

                    if should_final_commit {
                        Self::perform_commit_nonblocking(
                            &storage,
                            &epoch_start_ts,
                            &active_namespaces,
                            &committed_records,
                            &commitment_registry,
                            &proof_registry,
                            &config,
                        )
                        .await?;

                        Self::prune_committed_data(&storage, &committed_records)?;
                    }

                    break;
                }
                Err(_) => {
                    continue;
                }
            }
        }

        info!("Process and commit loop finished");
        Ok(())
    }

    async fn perform_commit_nonblocking(
        storage: &Arc<Mutex<Storage>>,
        epoch_start_ts: &Arc<Mutex<u64>>,
        active_namespaces: &Arc<Mutex<HashMap<Namespace, bool>>>,
        committed_records: &Arc<Mutex<Vec<CommittedRecord>>>,
        commitment_registry: &CommitmentRegistryVariant,
        proof_registry: &ProofRegistryVariant,
        config: &BaseConfig,
    ) -> Result<()> {
        info!("Starting commit phase");

        let committed_at = {
            let epoch_start = *epoch_start_ts.lock().unwrap();
            epoch_start + config.batch_interval_secs
        };

        let namespaces: Vec<Namespace> = {
            let ns = active_namespaces.lock().unwrap();
            ns.keys().copied().collect()
        };

        info!("Committing {} namespaces", namespaces.len());

        for namespace in namespaces {
            Self::commit_namespace_nonblocking(
                storage,
                &namespace,
                committed_at,
                committed_records,
                commitment_registry,
                proof_registry,
                config.accumulator_type,
            )
            .await?;
        }

        {
            let mut epoch_start = epoch_start_ts.lock().unwrap();
            *epoch_start = Self::now_secs();
        }
        {
            let mut ns = active_namespaces.lock().unwrap();
            ns.clear();
        }

        info!("Commit phase completed - downstream pipelines finished");
        Ok(())
    }

    fn prune_committed_data(
        storage: &Arc<Mutex<Storage>>,
        committed_records: &Arc<Mutex<Vec<CommittedRecord>>>,
    ) -> Result<()> {
        let records_to_prune = {
            let mut records = committed_records.lock().unwrap();
            let to_prune = records.clone();
            records.clear();
            to_prune
        };

        if records_to_prune.is_empty() {
            return Ok(());
        }

        info!(
            "Pruning {} committed records from storage",
            records_to_prune.len()
        );

        let storage_guard = storage.lock().unwrap();

        let mut by_namespace: HashMap<Namespace, Vec<&CommittedRecord>> = HashMap::new();
        for record in &records_to_prune {
            by_namespace
                .entry(record.namespace)
                .or_insert_with(Vec::new)
                .push(record);
        }

        for (namespace, records) in by_namespace {
            if let Some(max_ts) = records.iter().map(|r| r.timestamp).max() {
                let delete_filter = crate::storage::StorageDeleteFilter {
                    namespace,
                    timestamp: max_ts,
                };
                storage_guard.delete(&delete_filter)?;
                debug!(
                    "Pruned {} records for namespace {:?}",
                    records.len(),
                    namespace
                );
            }
        }

        info!("Pruning completed");
        Ok(())
    }

    async fn commit_namespace_nonblocking(
        storage: &Arc<Mutex<Storage>>,
        namespace: &Namespace,
        committed_at: u64,
        committed_records: &Arc<Mutex<Vec<CommittedRecord>>>,
        commitment_registry: &CommitmentRegistryVariant,
        proof_registry: &ProofRegistryVariant,
        accumulator_type: AccumulatorType,
    ) -> Result<()> {
        let (root, record_count, records_to_track) = {
            let storage_guard = storage.lock().unwrap();

            let filter = crate::storage::StorageScanFilter {
                namespace: *namespace,
                timestamp: Some(committed_at),
            };
            let records = storage_guard.scan(&filter)?;

            if records.is_empty() {
                debug!("No records to commit for namespace {:?}", namespace);
                return Ok(());
            }

            info!(
                "Committing {} records for namespace {:?}",
                records.len(),
                namespace
            );

            let mut accumulator = AccumulatorVariant::new(accumulator_type);

            let mut records_to_track = Vec::new();
            for record in &records {
                accumulator.put(record.key, record.value)?;
                records_to_track.push(CommittedRecord {
                    namespace: record.namespace,
                    key: record.key,
                    value: record.value,
                    timestamp: record.timestamp,
                });
            }

            let root = accumulator.build_root()?;
            let record_count = records.len();

            (root, record_count, records_to_track)
        };

        let commitment = Commitment {
            namespace: *namespace,
            root: root.clone(),
            committed_at,
        };

        let meta = BatchCommitmentMeta {
            commitment,
            leaf_count: record_count as u64,
        };

        commitment_registry.commit(&meta).await?;

        let proofs: Vec<StoredProof> = Vec::new();
        if !proofs.is_empty() {
            proof_registry.save_proofs(&proofs).await?;
        }

        {
            let mut committed = committed_records.lock().unwrap();
            committed.extend(records_to_track);
        }

        info!(
            "Committed namespace {:?}: {} records, root len={} - tracked for pruning",
            namespace,
            record_count,
            root.len()
        );

        Ok(())
    }

    /// Get current unix timestamp in seconds.
    pub fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time is before UNIX_EPOCH - please check your system clock")
            .as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now_secs() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before UNIX_EPOCH")
            .as_secs();
        assert!(now > 0);
        assert!(now < u64::MAX);

        let app_now = RootSmith::now_secs();
        assert!(app_now > 0);
        assert!(app_now < u64::MAX);
        assert!((app_now as i64 - now as i64).abs() < 2);
    }
}
