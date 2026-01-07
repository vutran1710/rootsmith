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
use kanal::{unbounded_async, AsyncReceiver, AsyncSender};
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

/// Signal sent from storage writer to committer indicating a namespace has new data.
#[derive(Debug, Clone, Copy)]
struct NamespaceUpdate {
    namespace: Namespace,
}

/// Commitment result to be delivered by the delivery handler.
#[derive(Debug, Clone)]
struct CommitmentResult {
    meta: BatchCommitmentMeta,
    proofs: Vec<StoredProof>,
    committed_records: Vec<CommittedRecord>,
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

        // Channel 1: Upstream -> Storage Writer
        let (data_tx, data_rx) = unbounded_async::<IncomingRecord>();
        
        // Channel 2: Storage Writer -> Committer (namespace updates)
        let (namespace_tx, namespace_rx) = unbounded_async::<NamespaceUpdate>();
        
        // Channel 3: Committer -> Delivery Handler (commitment results)
        let (commit_result_tx, commit_result_rx) = unbounded_async::<CommitmentResult>();

        info!("Starting upstream connector: {}", self.upstream.name());

        // Task 1: Upstream pulling
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

        // Task 2: Storage writer
        let storage_writer_handle = {
            let storage = Arc::clone(&self.storage);
            let namespace_tx = namespace_tx.clone();
            
            tokio::task::spawn(async move {
                let span = span!(Level::INFO, "storage_writer_task");
                let _enter = span.enter();

                Self::storage_writer_loop(data_rx, storage, namespace_tx).await
            })
        };

        // Task 3: Periodic committer
        let committer_handle = {
            let storage = Arc::clone(&self.storage);
            let epoch_start_ts = Arc::clone(&self.epoch_start_ts);
            let active_namespaces = Arc::clone(&self.active_namespaces);
            let config = self.config.clone();
            let commit_result_tx_clone = commit_result_tx.clone();
            
            tokio::task::spawn(async move {
                let span = span!(Level::INFO, "periodic_committer_task");
                let _enter = span.enter();

                Self::periodic_committer_loop(
                    namespace_rx,
                    storage,
                    epoch_start_ts,
                    active_namespaces,
                    commit_result_tx_clone,
                    config,
                )
                .await
            })
        };

        // Task 4: Delivery handler
        let delivery_handle = {
            let storage = Arc::clone(&self.storage);
            let commitment_registry = self.commitment_registry;
            let proof_registry = self.proof_registry;
            
            tokio::task::spawn(async move {
                let span = span!(Level::INFO, "delivery_handler_task");
                let _enter = span.enter();

                Self::delivery_handler_loop(
                    commit_result_rx,
                    storage,
                    commitment_registry,
                    proof_registry,
                )
                .await
            })
        };

        // Wait for all tasks to complete
        let upstream_result = upstream_handle
            .await
            .map_err(|_| anyhow::anyhow!("Upstream task panicked"))?;

        drop(data_tx);

        let storage_writer_result = storage_writer_handle
            .await
            .map_err(|_| anyhow::anyhow!("Storage writer task panicked"))?;

        drop(namespace_tx);

        let committer_result = committer_handle
            .await
            .map_err(|_| anyhow::anyhow!("Committer task panicked"))?;

        drop(commit_result_tx);

        let delivery_result = delivery_handle
            .await
            .map_err(|_| anyhow::anyhow!("Delivery handler task panicked"))?;

        upstream_result?;
        storage_writer_result?;
        committer_result?;
        delivery_result?;

        info!("RootSmith run completed successfully");
        Ok(())
    }

    /// Task 2: Storage writer loop - consumes records from upstream and writes to storage
    async fn storage_writer_loop(
        data_rx: AsyncReceiver<IncomingRecord>,
        storage: Arc<Mutex<Storage>>,
        namespace_tx: AsyncSender<NamespaceUpdate>,
    ) -> Result<()> {
        loop {
            match data_rx.recv().await {
                Ok(record) => {
                    let span = span!(Level::DEBUG, "handle_record", 
                        namespace = ?record.namespace);
                    let _enter = span.enter();

                    {
                        let storage_guard = storage.lock().unwrap();
                        storage_guard.put(&record)?;
                        debug!("Record stored for namespace {:?}", record.namespace);
                    }

                    // Notify committer about namespace update
                    let update = NamespaceUpdate {
                        namespace: record.namespace,
                    };
                    if namespace_tx.send(update).await.is_err() {
                        error!(
                            "Failed to send namespace update - committer task has stopped. \
                            Stopping storage writer to prevent unbounded storage growth."
                        );
                        break;
                    }
                }
                Err(_) => {
                    info!("Data channel closed, storage writer finishing");
                    break;
                }
            }
        }

        info!("Storage writer loop finished");
        Ok(())
    }

    /// Task 3: Periodic committer loop - commits data on interval
    async fn periodic_committer_loop(
        namespace_rx: AsyncReceiver<NamespaceUpdate>,
        storage: Arc<Mutex<Storage>>,
        epoch_start_ts: Arc<Mutex<u64>>,
        active_namespaces: Arc<Mutex<HashMap<Namespace, bool>>>,
        commit_result_tx: AsyncSender<CommitmentResult>,
        config: BaseConfig,
    ) -> Result<()> {
        loop {
            // Check if it's time to commit
            let should_commit = {
                let epoch_start = *epoch_start_ts.lock().unwrap();
                let now = Self::now_secs();
                let elapsed = now.saturating_sub(epoch_start);
                elapsed >= config.batch_interval_secs
            };

            if should_commit {
                info!("Commit phase starting");

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
                    if let Some(result) = Self::commit_namespace(
                        &storage,
                        &namespace,
                        committed_at,
                        config.accumulator_type,
                    )
                    .await?
                    {
                        // Send result to delivery handler
                        if commit_result_tx.send(result).await.is_err() {
                            return Err(anyhow::anyhow!(
                                "Delivery handler has stopped - cannot deliver commitments. \
                                Stopping committer to prevent data loss."
                            ));
                        }
                    }
                }

                // Reset epoch
                {
                    let mut epoch_start = epoch_start_ts.lock().unwrap();
                    *epoch_start = Self::now_secs();
                }
                {
                    let mut ns = active_namespaces.lock().unwrap();
                    ns.clear();
                }

                info!("Commit phase completed");
            }

            // Process namespace updates or timeout
            match tokio::time::timeout(Duration::from_millis(100), namespace_rx.recv()).await {
                Ok(Ok(update)) => {
                    let mut namespaces = active_namespaces.lock().unwrap();
                    namespaces.insert(update.namespace, true);
                    debug!("Namespace {:?} marked as active", update.namespace);
                }
                Ok(Err(_)) => {
                    info!("Namespace channel closed, performing final commit if needed");

                    let should_final_commit = {
                        let namespaces = active_namespaces.lock().unwrap();
                        !namespaces.is_empty()
                    };

                    if should_final_commit {
                        // Align final commit timestamp with regular epoch-based commits:
                        // committed_at = epoch_start + batch_interval_secs
                        let now = Self::now_secs();
                        let interval = config.batch_interval_secs;
                        let epoch_start = now - (now % interval);
                        let committed_at = epoch_start + interval;

                        let namespaces: Vec<Namespace> = {
                            let ns = active_namespaces.lock().unwrap();
                            ns.keys().copied().collect()
                        };

                        for namespace in namespaces {
                            if let Some(result) = Self::commit_namespace(
                                &storage,
                                &namespace,
                                committed_at,
                                config.accumulator_type,
                            )
                            .await?
                            {
                                if commit_result_tx.send(result).await.is_err() {
                                    return Err(anyhow::anyhow!(
                                        "Delivery handler has stopped - cannot deliver final commitments. \
                                        Data loss prevented by failing."
                                    ));
                                }
                            }
                        }
                    }

                    break;
                }
                Err(_) => {
                    // Timeout, continue checking for commit
                    continue;
                }
            }
        }

        info!("Periodic committer loop finished");
        Ok(())
    }

    /// Task 4: Delivery handler loop - handles commitment and proof delivery
    async fn delivery_handler_loop(
        commit_result_rx: AsyncReceiver<CommitmentResult>,
        storage: Arc<Mutex<Storage>>,
        commitment_registry: CommitmentRegistryVariant,
        proof_registry: ProofRegistryVariant,
    ) -> Result<()> {
        loop {
            match commit_result_rx.recv().await {
                Ok(result) => {
                    let span = span!(Level::DEBUG, "deliver_commitment", 
                        namespace = ?result.meta.commitment.namespace);
                    let _enter = span.enter();

                    // Deliver commitment
                    commitment_registry.commit(&result.meta).await?;
                    info!(
                        "Delivered commitment for namespace {:?}: {} records",
                        result.meta.commitment.namespace, result.meta.leaf_count
                    );

                    // Deliver proofs if any
                    if !result.proofs.is_empty() {
                        proof_registry.save_proofs(&result.proofs).await?;
                        debug!("Delivered {} proofs", result.proofs.len());
                    }

                    // Prune committed data
                    if !result.committed_records.is_empty() {
                        Self::prune_records(&storage, &result.committed_records)?;
                    }
                }
                Err(_) => {
                    info!("Commit result channel closed, delivery handler finishing");
                    break;
                }
            }
        }

        info!("Delivery handler loop finished");
        Ok(())
    }

    /// Commit a single namespace and return the result
    async fn commit_namespace(
        storage: &Arc<Mutex<Storage>>,
        namespace: &Namespace,
        committed_at: u64,
        accumulator_type: AccumulatorType,
    ) -> Result<Option<CommitmentResult>> {
        let (root, record_count, records_to_track) = {
            let storage_guard = storage.lock().unwrap();

            let filter = crate::storage::StorageScanFilter {
                namespace: *namespace,
                timestamp: Some(committed_at),
            };
            let records = storage_guard.scan(&filter)?;

            if records.is_empty() {
                debug!("No records to commit for namespace {:?}", namespace);
                return Ok(None);
            }

            info!(
                "Preparing commit for {} records in namespace {:?}",
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

        // TODO: Generate and attach proofs for the committed records based on the accumulator root.
        // For now, proofs are intentionally left empty and may be populated by a later stage.
        let proofs: Vec<StoredProof> = Vec::new();

        let result = CommitmentResult {
            meta,
            proofs,
            committed_records: records_to_track,
        };

        info!(
            "Namespace {:?} commit prepared: {} records, root len={}",
            namespace,
            record_count,
            root.len()
        );

        Ok(Some(result))
    }

    /// Prune committed records from storage
    fn prune_records(
        storage: &Arc<Mutex<Storage>>,
        records: &[CommittedRecord],
    ) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        info!("Pruning {} committed records from storage", records.len());

        let storage_guard = storage.lock().unwrap();

        let mut by_namespace: HashMap<Namespace, Vec<&CommittedRecord>> = HashMap::new();
        for record in records {
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
