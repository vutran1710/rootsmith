use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::archive::ArchiveStorageVariant;
use crate::commitment_registry::CommitmentRegistryVariant;
use crate::config::{AccumulatorType, BaseConfig};
use crate::crypto::AccumulatorVariant;
use crate::proof_delivery::ProofDeliveryVariant;
use crate::proof_registry::ProofRegistryVariant;
use crate::storage::{Storage, StorageDeleteFilter, StorageScanFilter};
use crate::traits::{
    Accumulator, ArchiveData, ArchiveStorage, CommitmentRegistry, ProofDelivery, ProofRegistry,
    UpstreamConnector,
};
use crate::types::{
    BatchCommitmentMeta, Commitment, IncomingRecord, Key32, Namespace, StoredProof, Value32,
};
use crate::upstream::UpstreamVariant;
use anyhow::Result;
use kanal::{unbounded_async, AsyncSender};
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

    /// Proof delivery implementation.
    pub proof_delivery: ProofDeliveryVariant,

    /// Archive storage implementation.
    pub archive_storage: ArchiveStorageVariant,

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
        proof_delivery: ProofDeliveryVariant,
        archive_storage: ArchiveStorageVariant,
        config: BaseConfig,
        storage: Storage,
    ) -> Self {
        let now = Self::now_secs();

        Self {
            upstream,
            commitment_registry,
            proof_registry,
            proof_delivery,
            archive_storage,
            config,
            storage: Arc::new(Mutex::new(storage)),
            epoch_start_ts: Arc::new(Mutex::new(now)),
            active_namespaces: Arc::new(Mutex::new(HashMap::new())),
            committed_records: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn initialize(config: BaseConfig) -> Result<Self> {
        use crate::archive::{ArchiveStorageVariant, NoopArchive};
        use crate::commitment_registry::commitment_noop::CommitmentNoop;
        use crate::commitment_registry::CommitmentRegistryVariant;
        use crate::proof_delivery::{NoopDelivery, ProofDeliveryVariant};
        use crate::proof_registry::{NoopProofRegistry, ProofRegistryVariant};
        use crate::upstream::NoopUpstream;

        let storage = Storage::open(&config.storage_path)?;
        info!("Storage opened at: {}", config.storage_path);

        let upstream = UpstreamVariant::Noop(NoopUpstream);
        let commitment_registry = CommitmentRegistryVariant::Noop(CommitmentNoop::new());
        let proof_registry = ProofRegistryVariant::Noop(NoopProofRegistry);
        let proof_delivery = ProofDeliveryVariant::Noop(NoopDelivery);
        let archive_storage = ArchiveStorageVariant::Noop(NoopArchive);

        Ok(Self::new(
            upstream,
            commitment_registry,
            proof_registry,
            proof_delivery,
            archive_storage,
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
        // Channel for commit task to notify proof task about new commitments
        let (commit_tx, commit_rx) = unbounded_async::<(Namespace, Vec<u8>, u64, Vec<(Key32, Value32)>)>();
        // Channel for proof registry task to notify delivery task about saved proofs
        let (proof_delivery_tx, proof_delivery_rx) = unbounded_async::<Vec<StoredProof>>();

        // Destructure self so we can move individual fields into tasks.
        let RootSmith {
            mut upstream,
            commitment_registry,
            proof_registry,
            proof_delivery,
            archive_storage,
            config,
            storage,
            epoch_start_ts,
            active_namespaces,
            committed_records,
        } = self;

        // Wrap shared components in Arc<Mutex> for sharing across tasks
        let commitment_registry = Arc::new(tokio::sync::Mutex::new(commitment_registry));
        let proof_registry = Arc::new(tokio::sync::Mutex::new(proof_registry));
        let proof_delivery = Arc::new(tokio::sync::Mutex::new(proof_delivery));
        let archive_storage = Arc::new(tokio::sync::Mutex::new(archive_storage));

        // === Upstream task: pull data from the configured upstream and send into the channel ===
        let upstream_handle = {
            let async_tx = data_tx.clone();
            tokio::spawn(async move {
                let span = span!(Level::INFO, "upstream_task");
                let _enter = span.enter();

                match Self::run_upstream_task(upstream, async_tx).await {
                    Ok(_) => Ok::<(), anyhow::Error>(()),
                    Err(e) => {
                    error!("Upstream connector failed: {}", e);
                        Err(e)
                    }
                }
            })
        };

        // === Storage writer: consume records from channel and persist to RocksDB ===
        let storage_handle = {
            let storage = Arc::clone(&storage);
            let active_namespaces = Arc::clone(&active_namespaces);
            tokio::spawn(async move {
                let span = span!(Level::INFO, "storage_write_task");
                let _enter = span.enter();
                info!("Starting storage write loop");
                while let Ok(record) = data_rx.recv().await {
                    {
                        let db = storage.lock().unwrap();
                        db.put(&record)?;
                    }
                    {
                        let mut active = active_namespaces.lock().unwrap();
                        active.insert(record.namespace, true);
                    }
                }
                info!("Storage write loop finished (channel closed)");
                Ok::<(), anyhow::Error>(())
            })
        };

        // === Commit task: periodically commit batches to commitment registry ===
        let commit_handle = {
            let storage = Arc::clone(&storage);
            let active_namespaces = Arc::clone(&active_namespaces);
            let epoch_start_ts = Arc::clone(&epoch_start_ts);
            let committed_records = Arc::clone(&committed_records);
            let commitment_registry = Arc::clone(&commitment_registry);
            let commit_tx = commit_tx.clone();
            let config_clone = config.clone();
            
            tokio::spawn(async move {
                let span = span!(Level::INFO, "commit_task");
                let _enter = span.enter();

                info!("Commit task started (batch_interval_secs={})", config_clone.batch_interval_secs);

        loop {
                    // Process commit cycle
                    match Self::process_commit_cycle(
                    &epoch_start_ts,
                    &active_namespaces,
                        &storage,
                    &committed_records,
                    &commitment_registry,
                        &commit_tx,
                        config_clone.batch_interval_secs,
                        config_clone.accumulator_type,
                    ).await {
                        Ok(_) => {
                            // Commit cycle processed successfully
                        }
                        Err(e) => {
                            error!("Error in commit cycle: {}", e);
                        }
                    }

                    // Sleep for a short interval before checking again
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                // This will never be reached but satisfies return type
                #[allow(unreachable_code)]
                Ok::<(), anyhow::Error>(())
            })
        };

        // === Proof registry task: generate and persist proofs for committed batches ===
        let proof_registry_handle = {
            let storage = Arc::clone(&storage);
            let proof_registry = Arc::clone(&proof_registry);
            let config_clone = config.clone();
            let proof_delivery_tx = proof_delivery_tx.clone();

            tokio::spawn(async move {
                let span = span!(Level::INFO, "proof_registry_task");
                let _enter = span.enter();

                let registry_name = {
                    let registry = proof_registry.lock().await;
                    registry.name().to_string()
                };
                info!("Proof registry task started (registry={})", registry_name);

                // Listen for new commitments from commit task
                while let Ok((namespace, root, committed_at, records)) = commit_rx.recv().await {
                    match Self::process_proof_generation(
                        namespace,
                        root,
                        committed_at,
                        records,
                    &proof_registry,
                        &proof_delivery_tx,
                        config_clone.accumulator_type,
                    ).await {
                        Ok(count) => {
                            debug!("Generated {} proofs for namespace {:?}", count, namespace);
                        }
                        Err(e) => {
                            error!("Failed to process proof generation for namespace {:?}: {}", namespace, e);
                        }
                    }
                }

                info!("Proof registry task finished (channel closed)");
                Ok::<(), anyhow::Error>(())
            })
        };

        // === Proof delivery task: deliver proofs to downstream consumers ===
        let proof_delivery_handle = {
            let proof_delivery = Arc::clone(&proof_delivery);
            
            tokio::spawn(async move {
                let span = span!(Level::INFO, "proof_delivery_task");
                let _enter = span.enter();

                let delivery_name = {
                    let delivery = proof_delivery.lock().await;
                    delivery.name().to_string()
                };
                info!("Proof delivery task started (delivery={})", delivery_name);

                // Listen for saved proofs from proof registry task
                while let Ok(proofs) = proof_delivery_rx.recv().await {
                    if proofs.is_empty() {
                        continue;
                    }

                    info!(
                        "Delivering batch of {} proofs (first key: {:?})",
                        proofs.len(),
                        proofs.first().map(|p| hex::encode(&p.key))
                    );

                    // Use the proof delivery abstraction to deliver proofs
                    let delivery = proof_delivery.lock().await;
                    match delivery.deliver_batch(&proofs).await {
                        Ok(_) => {
                            info!(
                                "Successfully delivered batch of {} proofs via {}",
                                proofs.len(),
                                delivery.name()
                            );
                        }
                        Err(e) => {
                            error!(
                                "Failed to deliver batch of {} proofs via {}: {}",
                                proofs.len(),
                                delivery.name(),
                                e
                            );
                        }
                    }
                }

                info!("Proof delivery task finished (channel closed)");
                Ok::<(), anyhow::Error>(())
            })
        };

        // === DB prune task: periodically prune committed data from storage ===
        let db_prune_handle = {
            let storage = Arc::clone(&storage);
            let committed_records = Arc::clone(&committed_records);
            
            tokio::spawn(async move {
                let span = span!(Level::INFO, "db_prune_task");
                    let _enter = span.enter();

                info!("DB prune task started");

                loop {
                    // Check for committed records to prune every 5 seconds
                    tokio::time::sleep(Duration::from_secs(5)).await;

                    // Get committed records that need to be pruned
                    let records_to_prune = {
                        let mut records = committed_records.lock().unwrap();
                        if records.is_empty() {
                            continue;
                        }
                        let to_prune = records.clone();
                        records.clear();
                        to_prune
                    };

                    if records_to_prune.is_empty() {
                        continue;
                    }

                    info!(
                        "Pruning {} committed records from storage",
                        records_to_prune.len()
                    );

                    // Group records by namespace and find max timestamp for each
                    let mut by_namespace: HashMap<Namespace, Vec<&CommittedRecord>> = HashMap::new();
                    for record in &records_to_prune {
                        by_namespace
                            .entry(record.namespace)
                            .or_insert_with(Vec::new)
                            .push(record);
                    }

                    // Delete records for each namespace up to the max timestamp
                    let storage_guard = storage.lock().unwrap();
                    for (namespace, records) in by_namespace {
                        if let Some(max_ts) = records.iter().map(|r| r.timestamp).max() {
                            let delete_filter = StorageDeleteFilter {
                                namespace,
                                timestamp: max_ts,
                            };
                            
                            match storage_guard.delete(&delete_filter) {
                                Ok(count) => {
                                    info!(
                                        "Pruned {} records for namespace {:?} (up to timestamp {})",
                                        count,
                                        namespace,
                                        max_ts
                                    );
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to prune records for namespace {:?}: {}",
                                        namespace, e
                                    );
                                }
                            }
                        }
                    }

                    info!("Pruning cycle completed");
                }
                // This will never be reached but satisfies return type
                #[allow(unreachable_code)]
                Ok::<(), anyhow::Error>(())
            })
        };

        // === Archiving task: archive old commitments and proofs to cold storage ===
        let archiving_handle = {
            let archive_storage = Arc::clone(&archive_storage);
            let storage = Arc::clone(&storage);
            let active_namespaces = Arc::clone(&active_namespaces);
            let commitment_registry = Arc::clone(&commitment_registry);
            
            tokio::spawn(async move {
                let span = span!(Level::INFO, "archiving_task");
                let _enter = span.enter();

                info!("Archiving task started with backend: {}", {
                    let storage = archive_storage.lock().await;
                    storage.name()
                });

                // Archive retention threshold: data older than this will be archived
                // In production, this could be configurable (e.g., 30 days)
                let archive_threshold_secs = 30 * 24 * 3600; // 30 days

                loop {
                    // Run archiving check every hour
                    tokio::time::sleep(Duration::from_secs(3600)).await;

                    let now = Self::now_secs();
                    let archive_cutoff = now.saturating_sub(archive_threshold_secs);

                    info!(
                        "Running archiving cycle (cutoff: {} days ago, timestamp: {})",
                        archive_threshold_secs / (24 * 3600),
                        archive_cutoff
                    );

                    // Get list of active namespaces to check for archivable data
                    let namespaces: Vec<Namespace> = {
                        let active = active_namespaces.lock().unwrap();
                        active.keys().copied().collect()
                    };

                    if namespaces.is_empty() {
                        debug!("No active namespaces to archive");
                        continue;
                    }

                    info!("Checking {} namespaces for archivable data", namespaces.len());

                    // Archive old records from storage that are older than cutoff
                    let mut archived_count = 0;
                    for namespace in &namespaces {
                        // Scan storage for old records in this namespace
                        let filter = StorageScanFilter {
                            namespace: *namespace,
                            timestamp: Some(archive_cutoff),
                        };

                        let records = {
                            let db = storage.lock().unwrap();
                            match db.scan(&filter) {
                                Ok(recs) => recs,
                                Err(e) => {
                                    error!("Failed to scan storage for namespace: {}", e);
                    continue;
                }
                            }
                        };

                        if !records.is_empty() {
                            debug!(
                                "Found {} old records in namespace to archive",
                                records.len()
                            );

                            // Archive records in batches
                            let batch_size = 100;
                            for chunk in records.chunks(batch_size) {
                                let archive_data: Vec<ArchiveData> = chunk
                                    .iter()
                                    .map(|r| ArchiveData::Record(r.clone()))
                                    .collect();

                                let mut archive = archive_storage.lock().await;
                                match archive.archive_batch(&archive_data).await {
                                    Ok(ids) => {
                                        archived_count += ids.len();
                                        debug!("Archived {} records, IDs: {:?}", ids.len(), &ids[..ids.len().min(3)]);
                                    }
                                    Err(e) => {
                                        error!("Failed to archive batch: {}", e);
                                    }
                                }
                            }
                        }
                    }

                    // Archive old commitments for each namespace
                    let mut archived_commitments = 0;
                    for namespace in &namespaces {
                        use crate::types::CommitmentFilterOptions;
                        
                        let filter = CommitmentFilterOptions {
                            namespace: *namespace,
                            time: archive_cutoff,
                        };

                        let registry = commitment_registry.lock().await;
                        let result = registry.get_prev_commitment(&filter).await;
                        drop(registry);

                        match result {
                            Ok(Some(commitment)) if commitment.committed_at <= archive_cutoff => {
                                let meta = BatchCommitmentMeta {
                                    commitment: commitment.clone(),
                                    leaf_count: 0, // Unknown - could be tracked separately
                                };

                                let mut archive = archive_storage.lock().await;
                                match archive.archive(&ArchiveData::Commitment(meta)).await {
                                    Ok(id) => {
                                        archived_commitments += 1;
                                        debug!(
                                            "Archived commitment ID: {}, timestamp: {}",
                                            id, commitment.committed_at
                                        );
                                    }
                                    Err(e) => error!("Failed to archive commitment: {}", e),
                                }
                            }
                            Ok(Some(_)) => debug!("Commitment too recent to archive"),
                            Ok(None) => debug!("No commitment found for namespace"),
                            Err(e) => error!("Failed to query commitment: {}", e),
                        }
                    }

                    // Summary logging
                    let total = archived_count + archived_commitments;
                    if total > 0 {
                        info!(
                            "Archiving cycle: {} records, {} commitments",
                            archived_count, archived_commitments
                        );
                    } else {
                        debug!("No old data to archive");
                    }
                }
                // This will never be reached but satisfies return type
                #[allow(unreachable_code)]
                Ok::<(), anyhow::Error>(())
            })
        };

        let query_layer_handle = tokio::spawn(async move {
            let span = span!(Level::INFO, "query_layer_task");
            let _enter = span.enter();

            debug!("Query layer task skeleton running");
            // Future work: provide a query interface over storage.
            Ok::<(), anyhow::Error>(())
        });

        let metrics_layer_handle = tokio::spawn(async move {
            let span = span!(Level::INFO, "metrics_layer_task");
            let _enter = span.enter();

            debug!("Metrics layer task skeleton running");
            // Future work: expose metrics for monitoring/observability.
            Ok::<(), anyhow::Error>(())
        });

        let admin_server_handle = tokio::spawn(async move {
            let span = span!(Level::INFO, "admin_server_task");
            let _enter = span.enter();

            debug!("Admin server task skeleton running");
            // Future work: admin HTTP/gRPC server for health, metrics, etc.
            Ok::<(), anyhow::Error>(())
        });

        // Wait for all tasks to complete.
        let (
            upstream_res,
            storage_res,
            commit_res,
            proof_registry_res,
            proof_delivery_res,
            db_prune_res,
            archiving_res,
            query_layer_res,
            metrics_layer_res,
            admin_server_res,
        ) = tokio::join!(
            upstream_handle,
            storage_handle,
            commit_handle,
            proof_registry_handle,
            proof_delivery_handle,
            db_prune_handle,
            archiving_handle,
            query_layer_handle,
            metrics_layer_handle,
            admin_server_handle,
        );

        upstream_res??;
        storage_res??;
        commit_res??;
        proof_registry_res??;
        proof_delivery_res??;
        db_prune_res??;
        archiving_res??;
        query_layer_res??;
        metrics_layer_res??;
        admin_server_res??;

        info!("RootSmith run completed successfully");
        Ok(())
    }

    /// Process a single commit cycle: check if commit is needed, commit all active namespaces, and reset epoch.
    /// Returns true if a commit was performed, false otherwise.
    ///
    /// This method is public for testing purposes. In production, it's called from `run()`.
    pub async fn process_commit_cycle(
        epoch_start_ts: &Arc<Mutex<u64>>,
        active_namespaces: &Arc<Mutex<HashMap<Namespace, bool>>>,
        storage: &Arc<Mutex<Storage>>,
        committed_records: &Arc<Mutex<Vec<CommittedRecord>>>,
        commitment_registry: &Arc<tokio::sync::Mutex<CommitmentRegistryVariant>>,
        commit_tx: &AsyncSender<(Namespace, Vec<u8>, u64, Vec<(Key32, Value32)>)>,
        batch_interval_secs: u64,
        accumulator_type: AccumulatorType,
    ) -> Result<bool> {
        // Check if it's time to commit
        let should_commit = {
            let epoch_start = *epoch_start_ts.lock().unwrap();
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
            let ns = active_namespaces.lock().unwrap();
            ns.keys().copied().collect()
        };

        if !namespaces.is_empty() {
            let committed_at = {
                let epoch_start = *epoch_start_ts.lock().unwrap();
                epoch_start + batch_interval_secs
        };

        info!("Committing {} namespaces", namespaces.len());

            // Commit each namespace
        for namespace in namespaces {
                let registry = commitment_registry.lock().await;
                match Self::commit_namespace(
                storage,
                &namespace,
                committed_at,
                committed_records,
                    &*registry,
                    commit_tx,
                    accumulator_type,
                ).await {
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
            let mut epoch_start = epoch_start_ts.lock().unwrap();
            *epoch_start = Self::now_secs();
        }
        {
            let mut ns = active_namespaces.lock().unwrap();
            ns.clear();
        }

            info!("Commit phase completed");
        } else {
            debug!("No active namespaces to commit");
            // Still update epoch start to avoid spinning
            {
                let mut epoch_start = epoch_start_ts.lock().unwrap();
                *epoch_start = Self::now_secs();
            }
        }

        Ok(true)
    }

    /// Run the upstream task: open upstream connector and forward records to the data channel.
    /// Returns Ok(()) on successful completion, or an error if the upstream connector fails.
    ///
    /// This method is public for testing purposes. In production, it's called from `run()`.
    pub async fn run_upstream_task(
        mut upstream: UpstreamVariant,
        data_tx: AsyncSender<IncomingRecord>,
    ) -> Result<()> {
        info!("Starting upstream connector: {}", upstream.name());

        upstream.open(data_tx).await?;

        info!("Upstream connector finished");
        Ok(())
    }

    /// Process proof generation for a committed batch: build accumulator, generate proofs, save to registry, and notify delivery.
    /// Returns the number of proofs generated.
    ///
    /// This method is public for testing purposes. In production, it's called from `run()`.
    pub async fn process_proof_generation(
        namespace: Namespace,
        root: Vec<u8>,
        committed_at: u64,
        records: Vec<(Key32, Value32)>,
        proof_registry: &Arc<tokio::sync::Mutex<ProofRegistryVariant>>,
        proof_delivery_tx: &AsyncSender<Vec<StoredProof>>,
        accumulator_type: AccumulatorType,
    ) -> Result<usize> {
        info!("Generating proofs for namespace {:?} with {} records", namespace, records.len());

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

        // Save proofs to proof registry
        let proof_count = stored_proofs.len();
        if !stored_proofs.is_empty() {
            let registry = proof_registry.lock().await;
            registry.save_proofs(&stored_proofs).await?;
            info!("Saved {} proofs for namespace {:?}", stored_proofs.len(), namespace);
            
            // Notify delivery task about saved proofs
            proof_delivery_tx.send(stored_proofs).await
                .map_err(|e| anyhow::anyhow!("Failed to send proofs to delivery task: {}", e))?;
        } else {
            debug!("No proofs to save for namespace {:?}", namespace);
        }

        Ok(proof_count)
    }

    /// Helper method to commit a namespace: scan records, build accumulator, commit, and notify proof task.
    async fn commit_namespace(
        storage: &Arc<Mutex<Storage>>,
        namespace: &Namespace,
        committed_at: u64,
        committed_records: &Arc<Mutex<Vec<CommittedRecord>>>,
        commitment_registry: &CommitmentRegistryVariant,
        commit_tx: &AsyncSender<(Namespace, Vec<u8>, u64, Vec<(Key32, Value32)>)>,
        accumulator_type: AccumulatorType,
    ) -> Result<()> {
        // Scan storage for records in this namespace up to committed_at
        let (root, record_count, records_to_track, records_for_proof) = {
            let storage_guard = storage.lock().unwrap();

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

        // Create commitment and send to commitment registry
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
        info!("Committed namespace {:?}: {} records, root len={}", namespace, record_count, root.len());

        // Track records for pruning
        {
            let mut committed = committed_records.lock().unwrap();
            committed.extend(records_to_track);
        }

        // Notify proof task about new commitment
        if let Err(e) = commit_tx.send((*namespace, root.clone(), committed_at, records_for_proof)).await {
            error!("Failed to notify proof task about commitment: {}", e);
            // Don't fail the commit if proof notification fails
        }

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
