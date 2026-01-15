//! Async task orchestration with tokio::spawn and stateless business logic functions.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use futures_util::future;
use kanal::unbounded_async;
use kanal::AsyncReceiver;
use kanal::AsyncSender;
use tokio::task::JoinHandle;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::span;
use tracing::Level;

use super::core::CommittedRecord;
use super::core::RootSmith;
use crate::archive::ArchiveStorageVariant;
use crate::config::AccumulatorType;
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

// ==================== TYPE ALIASES ====================

type NamespaceMap = Arc<tokio::sync::Mutex<HashMap<Namespace, bool>>>;
type StorageArc = Arc<tokio::sync::Mutex<Storage>>;
type ArchiveStorage = Arc<tokio::sync::Mutex<ArchiveStorageVariant>>;

// ==================== STATELESS BUSINESS LOGIC FUNCTIONS ====================

/// Get current unix timestamp in seconds.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time is before UNIX_EPOCH - please check your system clock")
        .as_secs()
}

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
        let now = now_secs();
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
            match commit_namespace(
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
            *epoch_start = now_secs();
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
            *epoch_start = now_secs();
        }
    }

    Ok(true)
}

/// Helper function to commit a namespace: scan records, build accumulator, commit, and notify proof task.
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

                let stored_proof = StoredProof {
                    root: root.clone(),
                    proof: proof_bytes.clone(),
                    key: *key,
                    meta: serde_json::json!({
                        "namespace": hex::encode(namespace),
                        "committed_at": committed_at,
                    }),
                };
                stored_proofs.push(stored_proof);
                proof_map.insert(*key, proof_bytes);
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

// ==================== GENERIC TASK UTILITIES ====================

/// Spawn an interval-based task that runs periodically or continuously.
///
/// If `interval` is `Some(duration)`, sleeps between iterations.
/// If `interval` is `None`, yields with `tokio::task::yield_now()`.
fn spawn_interval_task<F, Fut>(
    name: &'static str,
    interval: Option<Duration>,
    task_fn: F,
) -> JoinHandle<Result<()>>
where
    F: Fn() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send,
{
    tokio::spawn(async move {
        let span = span!(Level::INFO, "task", name = name);
        let _enter = span.enter();

        loop {
            match task_fn().await {
                Ok(_) => {
                    // Task iteration successful
                }
                Err(e) => {
                    error!("Error in task {}: {}", name, e);
                }
            }

            // Sleep or yield based on interval
            match interval {
                Some(duration) => tokio::time::sleep(duration).await,
                None => tokio::task::yield_now().await,
            }
        }
    })
}

/// Spawn a channel consumer task that processes messages from an AsyncReceiver.
///
/// Consumes messages from the receiver and processes them with the provided function.
/// Handles channel closure gracefully.
fn spawn_channel_consumer_task<T, F, Fut>(
    name: &'static str,
    receiver: AsyncReceiver<T>,
    process_fn: F,
) -> JoinHandle<Result<()>>
where
    T: Send + 'static,
    F: Fn(T) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send,
{
    tokio::spawn(async move {
        let span = span!(Level::INFO, "task", name = name);
        let _enter = span.enter();

        info!("Starting {} task", name);

        while let Ok(message) = receiver.recv().await {
            if let Err(e) = process_fn(message).await {
                error!("Error processing message in {}: {}", name, e);
            }
        }

        info!("{} task finished (channel closed)", name);
        Ok(())
    })
}

/// Spawn a one-time task that runs once and completes.
///
/// Runs the provided async function once and returns its result.
fn spawn_oneshot_task<F, Fut>(name: &'static str, task_fn: F) -> JoinHandle<Result<()>>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send,
{
    tokio::spawn(async move {
        let span = span!(Level::INFO, "task", name = name);
        let _enter = span.enter();

        debug!("{} task running", name);
        task_fn().await
    })
}

// ==================== STATELESS BUSINESS LOGIC FUNCTIONS ====================

/// Process archiving cycle: archive old records and commitments.
async fn process_archiving_cycle(
    archive_storage: &ArchiveStorage,
    storage: &StorageArc,
    active_namespaces: &NamespaceMap,
    archive_threshold_secs: u64,
) -> Result<()> {
    let now = now_secs();
    let archive_cutoff = now.saturating_sub(archive_threshold_secs);

    info!(
        "Running archiving cycle (cutoff: {} days ago, timestamp: {})",
        archive_threshold_secs / (24 * 3600),
        archive_cutoff
    );

    // Get list of active namespaces
    let namespaces: Vec<Namespace> = {
        let active = active_namespaces.lock().await;
        active.keys().copied().collect()
    };

    if namespaces.is_empty() {
        debug!("No active namespaces to archive");
        return Ok(());
    }

    info!(
        "Checking {} namespaces for archivable data",
        namespaces.len()
    );

    // Archive old records
    let archived_count =
        archive_old_records(archive_storage, storage, &namespaces, archive_cutoff).await?;

    // Note: Commitment archiving disabled as commitment_registry was replaced with downstream

    // Summary logging
    if archived_count > 0 {
        info!(
            "Archiving cycle: {} records archived",
            archived_count
        );
    } else {
        debug!("No old data to archive");
    }

    Ok(())
}

/// Archive old records from storage.
async fn archive_old_records(
    archive_storage: &ArchiveStorage,
    storage: &StorageArc,
    namespaces: &[Namespace],
    archive_cutoff: u64,
) -> Result<usize> {
    use crate::storage::StorageScanFilter;
    use crate::traits::ArchiveData;
    use crate::traits::ArchiveStorage as ArchiveStorageTrait;

    let mut archived_count = 0;

    for namespace in namespaces {
        let filter = StorageScanFilter {
            namespace: *namespace,
            timestamp: Some(archive_cutoff),
        };

        let records = {
            let db = storage.lock().await;
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

                let archive = archive_storage.lock().await;
                match (*archive).archive_batch(&archive_data).await {
                    Ok(ids) => {
                        archived_count += ids.len();
                        debug!(
                            "Archived {} records, IDs: {:?}",
                            ids.len(),
                            &ids[..ids.len().min(3)]
                        );
                    }
                    Err(e) => {
                        error!("Failed to archive batch: {}", e);
                    }
                }
            }
        }
    }

    Ok(archived_count)
}

// Note: archive_old_commitments function disabled as commitment_registry was replaced with downstream
// The archiving of commitments would need to be reimplemented using a different mechanism if needed.
/*
/// Archive old commitments for namespaces.
async fn archive_old_commitments(
    archive_storage: &ArchiveStorage,
    commitment_registry: &CommitmentRegistry,
    namespaces: &[Namespace],
    archive_cutoff: u64,
) -> Result<usize> {
    use crate::traits::ArchiveData;
    use crate::traits::ArchiveStorage as ArchiveStorageTrait;
    use crate::traits::CommitmentRegistry as CommitmentRegistryTrait;
    use crate::types::BatchCommitmentMeta;
    use crate::types::CommitmentFilterOptions;

    let mut archived_commitments = 0;

    for namespace in namespaces {
        let filter = CommitmentFilterOptions {
            namespace: *namespace,
            time: archive_cutoff,
        };

        let registry = commitment_registry.lock().await;
        let result = (*registry).get_prev_commitment(&filter).await;
        drop(registry);

        match result {
            Ok(Some(commitment)) if commitment.committed_at <= archive_cutoff => {
                let meta = BatchCommitmentMeta {
                    commitment: commitment.clone(),
                    leaf_count: 0, // Unknown - could be tracked separately
                };

                let mut archive = archive_storage.lock().await;
                match (*archive).archive(&ArchiveData::Commitment(meta)).await {
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

    Ok(archived_commitments)
}
*/

impl RootSmith {
    /// Run the application: spawn all tasks and orchestrate the system.
    pub async fn run(self) -> Result<()> {
        let span = span!(Level::INFO, "app_run");
        let _enter = span.enter();

        info!(
            "Starting RootSmith with batch_interval_secs={}",
            self.config.batch_interval_secs
        );

        let (data_tx, data_rx) = unbounded_async::<IncomingRecord>();
        // Channel for commit task to notify proof task about new commitments
        let (commit_tx, commit_rx) =
            unbounded_async::<(Namespace, Vec<u8>, u64, Vec<(Key32, crate::types::Value32)>)>();
        // Channel for proof registry task to notify delivery task about saved proofs
        let (proof_delivery_tx, proof_delivery_rx) = unbounded_async::<Vec<StoredProof>>();

        // Destructure self so we can move individual fields into tasks.
        let RootSmith {
            upstream,
            downstream,
            proof_delivery,
            archive_storage,
            config,
            storage,
            epoch_start_ts,
            active_namespaces,
            committed_records,
        } = self;

        // Wrap shared components in Arc<Mutex> for sharing across tasks
        let downstream = Arc::new(tokio::sync::Mutex::new(downstream));
        let proof_delivery = Arc::new(tokio::sync::Mutex::new(proof_delivery));
        let archive_storage = Arc::new(tokio::sync::Mutex::new(archive_storage));

        // === Upstream task: pull data from the configured upstream and send into the channel ===
        let upstream_handle = spawn_oneshot_task("upstream", || async move {
            match run_upstream_task(upstream, data_tx.clone()).await {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!("Upstream connector failed: {}", e);
                    Err(e)
                }
            }
        });

        // === Storage writer: consume records from channel and persist to RocksDB ===
        let storage_handle = {
            let storage = Arc::clone(&storage);
            let active_namespaces = Arc::clone(&active_namespaces);

            spawn_channel_consumer_task("storage_write", data_rx, move |record| {
                let storage = Arc::clone(&storage);
                let active_namespaces = Arc::clone(&active_namespaces);
                async move { storage_write_once(&storage, &active_namespaces, &record).await }
            })
        };

        // === Commit task: periodically commit batches to downstream ===
        let commit_handle = {
            let storage = Arc::clone(&storage);
            let active_namespaces = Arc::clone(&active_namespaces);
            let epoch_start_ts = Arc::clone(&epoch_start_ts);
            let committed_records = Arc::clone(&committed_records);
            let downstream = Arc::clone(&downstream);
            let commit_tx = commit_tx.clone();
            let config_clone = config.clone();

            spawn_interval_task("commit", Some(Duration::from_millis(100)), move || {
                let storage = Arc::clone(&storage);
                let active_namespaces = Arc::clone(&active_namespaces);
                let epoch_start_ts = Arc::clone(&epoch_start_ts);
                let committed_records = Arc::clone(&committed_records);
                let downstream = Arc::clone(&downstream);
                let commit_tx = commit_tx.clone();
                let config_clone = config_clone.clone();

                async move {
                    process_commit_cycle(
                        &epoch_start_ts,
                        &active_namespaces,
                        &storage,
                        &committed_records,
                        &downstream,
                        &commit_tx,
                        config_clone.batch_interval_secs,
                        config_clone.accumulator_type,
                    )
                    .await
                    .map(|_| ())
                }
            })
        };

        // === Proof generation task: generate proofs for committed batches ===
        let proof_registry_handle = {
            let downstream = Arc::clone(&downstream);
            let config_clone = config.clone();
            let proof_delivery_tx = proof_delivery_tx.clone();

            spawn_channel_consumer_task(
                "proof_registry",
                commit_rx,
                move |(namespace, root, committed_at, records)| {
                    let downstream = Arc::clone(&downstream);
                    let proof_delivery_tx = proof_delivery_tx.clone();
                    let config_clone = config_clone.clone();

                    async move {
                        process_proof_generation(
                            namespace,
                            root,
                            committed_at,
                            records,
                            &downstream,
                            &proof_delivery_tx,
                            config_clone.accumulator_type,
                        )
                        .await
                        .map(|_| ())
                    }
                },
            )
        };

        // === Proof delivery task: deliver proofs to downstream consumers ===
        let proof_delivery_handle = {
            let proof_delivery = Arc::clone(&proof_delivery);

            spawn_channel_consumer_task("proof_delivery", proof_delivery_rx, move |proofs| {
                let proof_delivery = Arc::clone(&proof_delivery);

                async move {
                    if proofs.is_empty() {
                        return Ok(());
                    }

                    info!(
                        "Delivering batch of {} proofs (first key: {:?})",
                        proofs.len(),
                        proofs.first().map(|p| hex::encode(&p.key))
                    );

                    deliver_once(&proof_delivery, &proofs).await
                }
            })
        };

        // === DB prune task: periodically prune committed data from storage ===
        let db_prune_handle = {
            let storage = Arc::clone(&storage);
            let committed_records = Arc::clone(&committed_records);

            spawn_interval_task("db_prune", Some(Duration::from_secs(5)), move || {
                let storage = Arc::clone(&storage);
                let committed_records = Arc::clone(&committed_records);

                async move {
                    match prune_once(&storage, &committed_records).await {
                        Ok(count) if count > 0 => {
                            info!("Pruned {} records", count);
                        }
                        Ok(_) => {
                            // No records pruned
                        }
                        Err(e) => {
                            error!("Error during pruning: {}", e);
                        }
                    }
                    Ok(())
                }
            })
        };

        // === Archiving task: archive old records to cold storage ===
        let archiving_handle = {
            let archive_storage = Arc::clone(&archive_storage);
            let storage = Arc::clone(&storage);
            let active_namespaces = Arc::clone(&active_namespaces);

            // Archive retention threshold: data older than this will be archived
            let archive_threshold_secs = 30 * 24 * 3600; // 30 days

            spawn_interval_task("archiving", Some(Duration::from_secs(3600)), move || {
                let archive_storage = Arc::clone(&archive_storage);
                let storage = Arc::clone(&storage);
                let active_namespaces = Arc::clone(&active_namespaces);

                async move {
                    process_archiving_cycle(
                        &archive_storage,
                        &storage,
                        &active_namespaces,
                        archive_threshold_secs,
                    )
                    .await
                }
            })
        };

        // Skeleton tasks for future work
        let query_layer_handle = spawn_oneshot_task("query_layer", || async {
            debug!("Query layer task skeleton running");
            // Future work: provide a query interface over storage.
            Ok(())
        });

        let metrics_layer_handle = spawn_oneshot_task("metrics_layer", || async {
            debug!("Metrics layer task skeleton running");
            // Future work: expose metrics for monitoring/observability.
            Ok(())
        });

        let admin_server_handle = spawn_oneshot_task("admin_server", || async {
            debug!("Admin server task skeleton running");
            // Future work: admin HTTP/gRPC server for health, metrics, etc.
            Ok(())
        });

        // Wait for all tasks to complete with fail-fast behavior
        let handles = vec![
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
        ];

        future::try_join_all(handles).await?;

        info!("RootSmith run completed successfully");
        Ok(())
    }
}
