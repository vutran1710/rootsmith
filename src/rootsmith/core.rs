//! Core business logic for RootSmith - testable functions without tokio::spawn.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use kanal::AsyncSender;
use tracing::debug;
use tracing::error;
use tracing::info;

use crate::archive::ArchiveStorageVariant;
use crate::config::AccumulatorType;
use crate::config::BaseConfig;
use crate::crypto::AccumulatorVariant;
use crate::downstream::DownstreamVariant;
use crate::proof_delivery::ProofDeliveryVariant;
use crate::storage::Storage;
use crate::storage::StorageDeleteFilter;
use crate::storage::StorageScanFilter;
use crate::traits::Accumulator;
use crate::traits::Downstream;
use crate::traits::ProofDelivery;
use crate::traits::UpstreamConnector;
use crate::types::CommitmentResult;
use crate::types::IncomingRecord;
use crate::types::Key32;
use crate::types::Namespace;
use crate::types::StoredProof;
use crate::types::Value32;
use crate::upstream::UpstreamVariant;

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
pub struct CommittedRecord {
    pub namespace: Namespace,
    pub key: [u8; 32],
    pub value: Value32,
    pub timestamp: u64,
}

/// Main application orchestrator with epoch-based architecture.
pub struct RootSmith {
    /// Upstream connector.
    pub upstream: UpstreamVariant,

    /// Downstream handler for commitment results.
    pub downstream: DownstreamVariant,

    /// Proof delivery implementation.
    pub proof_delivery: ProofDeliveryVariant,

    /// Archive storage implementation.
    pub archive_storage: ArchiveStorageVariant,

    /// Global/base configuration.
    pub config: BaseConfig,

    /// Persistent storage (RocksDB).
    pub storage: Arc<tokio::sync::Mutex<Storage>>,

    /// Start time of the current epoch (unix seconds).
    pub epoch_start_ts: Arc<tokio::sync::Mutex<u64>>,

    /// Track active namespaces for efficient commit.
    pub active_namespaces: Arc<tokio::sync::Mutex<HashMap<Namespace, bool>>>,

    /// Track committed records for pruning in pending phase.
    pub committed_records: Arc<tokio::sync::Mutex<Vec<CommittedRecord>>>,
}

impl RootSmith {
    /// Create a new RootSmith.
    pub fn new(
        upstream: UpstreamVariant,
        downstream: DownstreamVariant,
        proof_delivery: ProofDeliveryVariant,
        archive_storage: ArchiveStorageVariant,
        config: BaseConfig,
        storage: Storage,
    ) -> Self {
        let now = Self::now_secs();

        Self {
            upstream,
            downstream,
            proof_delivery,
            archive_storage,
            config,
            storage: Arc::new(tokio::sync::Mutex::new(storage)),
            epoch_start_ts: Arc::new(tokio::sync::Mutex::new(now)),
            active_namespaces: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            committed_records: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    /// Initialize RootSmith with default Noop implementations.
    pub async fn initialize(config: BaseConfig) -> Result<Self> {
        use crate::archive::ArchiveStorageVariant;
        use crate::archive::NoopArchive;
        use crate::downstream::BlackholeDownstream;
        use crate::downstream::DownstreamVariant;
        use crate::proof_delivery::NoopDelivery;
        use crate::proof_delivery::ProofDeliveryVariant;
        use crate::upstream::NoopUpstream;

        let storage = Storage::open(&config.storage_path)?;
        info!("Storage opened at: {}", config.storage_path);

        let upstream = UpstreamVariant::Noop(NoopUpstream);
        let downstream = DownstreamVariant::Blackhole(BlackholeDownstream::new());
        let proof_delivery = ProofDeliveryVariant::Noop(NoopDelivery);
        let archive_storage = ArchiveStorageVariant::Noop(NoopArchive);

        Ok(Self::new(
            upstream,
            downstream,
            proof_delivery,
            archive_storage,
            config,
            storage,
        ))
    }

    /// Get current unix timestamp in seconds.
    pub fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time is before UNIX_EPOCH - please check your system clock")
            .as_secs()
    }

    // ==================== TESTABLE BUSINESS LOGIC ("*_once" functions) ====================

    /// Process one storage write: persist record and mark namespace as active.
    ///
    /// Returns `Ok(())` on success.
    pub async fn storage_write_once(
        storage: &Arc<tokio::sync::Mutex<Storage>>,
        active_namespaces: &Arc<tokio::sync::Mutex<HashMap<Namespace, bool>>>,
        record: &IncomingRecord,
    ) -> Result<()> {
        // Persist to storage
        {
            let db = storage.lock().await;
            db.put(record)?;
        }

        // Mark namespace as active
        {
            let mut active = active_namespaces.lock().await;
            active.insert(record.namespace, true);
        }

        Ok(())
    }

    /// Run the upstream task: open upstream connector and forward records to the data channel.
    ///
    /// Returns `Ok(())` on successful completion, or an error if the upstream connector fails.
    pub async fn run_upstream_task(
        mut upstream: UpstreamVariant,
        data_tx: AsyncSender<IncomingRecord>,
    ) -> Result<()> {
        info!("Starting upstream connector: {}", upstream.name());
        upstream.open(data_tx).await?;
        info!("Upstream connector finished");
        Ok(())
    }

    /// Process a single commit cycle: check if commit is needed, commit all active namespaces, and reset epoch.
    ///
    /// Returns `Ok(true)` if a commit was performed, `Ok(false)` if not yet time.
    pub async fn process_commit_cycle(
        epoch_start_ts: &Arc<tokio::sync::Mutex<u64>>,
        active_namespaces: &Arc<tokio::sync::Mutex<HashMap<Namespace, bool>>>,
        storage: &Arc<tokio::sync::Mutex<Storage>>,
        committed_records: &Arc<tokio::sync::Mutex<Vec<CommittedRecord>>>,
        downstream: &Arc<tokio::sync::Mutex<DownstreamVariant>>,
        commit_tx: &AsyncSender<(Namespace, Vec<u8>, u64, Vec<(Key32, Value32)>)>,
        batch_interval_secs: u64,
        accumulator_type: AccumulatorType,
    ) -> Result<bool> {
        // Check if it's time to commit
        let should_commit = {
            let epoch_start = *epoch_start_ts.lock().await;
            let now = Self::now_secs();
            let elapsed = now.saturating_sub(epoch_start);
            elapsed >= batch_interval_secs
        };

        if !should_commit {
            return Ok(false);
        }

        info!("Commit phase starting");

        // Get list of active namespaces
        let namespaces: Vec<Namespace> = {
            let ns = active_namespaces.lock().await;
            ns.keys().copied().collect()
        };

        if !namespaces.is_empty() {
            let committed_at = {
                let epoch_start = *epoch_start_ts.lock().await;
                epoch_start + batch_interval_secs
            };

            info!("Committing {} namespaces", namespaces.len());

            // Commit each namespace
            for namespace in namespaces {
                match Self::commit_namespace(
                    storage,
                    &namespace,
                    committed_at,
                    committed_records,
                    downstream,
                    commit_tx,
                    accumulator_type,
                )
                .await
                {
                    Ok(_) => {
                        debug!("Committed namespace {:?}", namespace);
                    }
                    Err(e) => {
                        error!("Failed to commit namespace {:?}: {}", namespace, e);
                    }
                }
            }

            // Reset epoch and clear active namespaces
            {
                let mut epoch_start = epoch_start_ts.lock().await;
                *epoch_start = Self::now_secs();
            }
            {
                let mut ns = active_namespaces.lock().await;
                ns.clear();
            }

            info!("Commit phase completed");
        } else {
            debug!("No active namespaces to commit");
            // Still update epoch start to avoid spinning
            {
                let mut epoch_start = epoch_start_ts.lock().await;
                *epoch_start = Self::now_secs();
            }
        }

        Ok(true)
    }

    /// Process proof generation for a committed batch: build accumulator, generate proofs, save to registry, and notify delivery.
    ///
    /// Returns the number of proofs generated.
    pub async fn process_proof_generation(
        namespace: Namespace,
        root: Vec<u8>,
        committed_at: u64,
        records: Vec<(Key32, Value32)>,
        downstream: &Arc<tokio::sync::Mutex<DownstreamVariant>>,
        proof_delivery_tx: &AsyncSender<Vec<StoredProof>>,
        accumulator_type: AccumulatorType,
    ) -> Result<usize> {
        info!(
            "Generating proofs for namespace {:?} with {} records",
            namespace,
            records.len()
        );

        // Build accumulator from committed records
        let mut accumulator = AccumulatorVariant::new(accumulator_type);
        for (key, value) in &records {
            if let Err(e) = accumulator.put(*key, *value) {
                error!("Failed to insert key into accumulator: {}", e);
                continue;
            }
        }

        // Generate proofs for all records
        let mut stored_proofs = Vec::new();
        let mut proof_map = HashMap::new();
        for (key, _value) in &records {
            match accumulator.prove(key) {
                Ok(Some(proof)) => {
                    // Serialize proof to bytes
                    let proof_bytes = match serde_json::to_vec(&proof) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            error!("Failed to serialize proof: {}", e);
                            continue;
                        }
                    };

                    proof_map.insert(*key, proof_bytes.clone());

                    let stored_proof = StoredProof {
                        root: root.clone(),
                        proof: proof_bytes,
                        key: *key,
                        meta: serde_json::json!({
                            "namespace": hex::encode(namespace),
                            "committed_at": committed_at,
                        }),
                    };
                    stored_proofs.push(stored_proof);
                }
                Ok(None) => {
                    debug!("No proof available for key {:?}", key);
                }
                Err(e) => {
                    error!("Failed to generate proof for key {:?}: {}", key, e);
                }
            }
        }

        // Create CommitmentResult with proofs
        let proof_count = proof_map.len();
        if !proof_map.is_empty() {
            let result = CommitmentResult {
                commitment: root.clone(),
                proofs: Some(proof_map),
                committed_at,
            };

            // Send to downstream
            let ds = downstream.lock().await;
            ds.handle(&result).await?;
            info!(
                "Sent {} proofs for namespace {:?} to downstream",
                proof_count, namespace
            );

            // Notify delivery task about saved proofs
            proof_delivery_tx
                .send(stored_proofs)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send proofs to delivery task: {}", e))?;
        } else {
            debug!("No proofs to save for namespace {:?}", namespace);
        }

        Ok(proof_count)
    }

    /// Deliver proofs once using the proof delivery implementation.
    ///
    /// Returns `Ok(())` on success.
    pub async fn deliver_once(
        proof_delivery: &Arc<tokio::sync::Mutex<ProofDeliveryVariant>>,
        proofs: &[StoredProof],
    ) -> Result<()> {
        let delivery = proof_delivery.lock().await;
        delivery.deliver_batch(proofs).await?;
        info!("Delivered batch of {} proofs", proofs.len());
        Ok(())
    }

    /// Prune old committed records from storage once.
    ///
    /// Groups committed records by namespace, finds max timestamp per namespace,
    /// and deletes records up to that timestamp. Clears the committed_records list.
    ///
    /// Returns the number of records pruned.
    pub async fn prune_once(
        storage: &Arc<tokio::sync::Mutex<Storage>>,
        committed_records: &Arc<tokio::sync::Mutex<Vec<CommittedRecord>>>,
    ) -> Result<usize> {
        // Extract and clear committed records
        let records = {
            let mut committed = committed_records.lock().await;
            let records = committed.clone();
            committed.clear();
            records
        };

        if records.is_empty() {
            return Ok(0);
        }

        // Group by namespace and find max timestamp
        let mut namespace_max_ts: HashMap<Namespace, u64> = HashMap::new();
        for record in &records {
            namespace_max_ts
                .entry(record.namespace)
                .and_modify(|ts| *ts = (*ts).max(record.timestamp))
                .or_insert(record.timestamp);
        }

        // Delete records for each namespace
        let db = storage.lock().await;
        let mut total_pruned = 0;

        for (namespace, max_ts) in namespace_max_ts {
            let filter = StorageDeleteFilter {
                namespace,
                timestamp: max_ts,
            };

            match db.delete(&filter) {
                Ok(count) => {
                    debug!(
                        "Pruned {} records for namespace {:?} up to ts={}",
                        count, namespace, max_ts
                    );
                    total_pruned += count;
                }
                Err(e) => {
                    error!(
                        "Failed to prune records for namespace {:?}: {}",
                        namespace, e
                    );
                }
            }
        }

        if total_pruned > 0 {
            info!("Pruned {} total records", total_pruned);
        }

        Ok(total_pruned as usize)
    }

    // ==================== PRIVATE HELPERS ====================

    /// Helper method to commit a namespace: scan records, build accumulator, commit, and notify proof task.
    async fn commit_namespace(
        storage: &Arc<tokio::sync::Mutex<Storage>>,
        namespace: &Namespace,
        committed_at: u64,
        committed_records: &Arc<tokio::sync::Mutex<Vec<CommittedRecord>>>,
        downstream: &Arc<tokio::sync::Mutex<DownstreamVariant>>,
        commit_tx: &AsyncSender<(Namespace, Vec<u8>, u64, Vec<(Key32, Value32)>)>,
        accumulator_type: AccumulatorType,
    ) -> Result<()> {
        // Scan storage for records in this namespace up to committed_at
        let (root, record_count, records_to_track, records_for_proof) = {
            let storage_guard = storage.lock().await;

            let filter = StorageScanFilter {
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

            // Build accumulator from records
            let mut accumulator = AccumulatorVariant::new(accumulator_type);

            let mut records_to_track = Vec::new();
            let mut records_for_proof = Vec::new();

            for record in &records {
                accumulator.put(record.key, record.value)?;
                records_to_track.push(CommittedRecord {
                    namespace: record.namespace,
                    key: record.key,
                    value: record.value,
                    timestamp: record.timestamp,
                });
                records_for_proof.push((record.key, record.value));
            }

            let root = accumulator.build_root()?;
            let record_count = records.len();

            (root, record_count, records_to_track, records_for_proof)
        };

        // Create CommitmentResult and send to downstream
        let result = CommitmentResult {
            commitment: root.clone(),
            proofs: None,
            committed_at,
        };

        let ds = downstream.lock().await;
        ds.handle(&result).await?;
        info!(
            "Committed namespace {:?}: {} records, root len={}",
            namespace,
            record_count,
            root.len()
        );

        // Track records for pruning
        {
            let mut committed = committed_records.lock().await;
            committed.extend(records_to_track);
        }

        // Notify proof task about new commitment
        if let Err(e) = commit_tx
            .send((*namespace, root.clone(), committed_at, records_for_proof))
            .await
        {
            error!("Failed to notify proof task about commitment: {}", e);
            // Don't fail the commit if proof notification fails
        }

        Ok(())
    }
}
