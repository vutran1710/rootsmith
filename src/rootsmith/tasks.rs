//! Async task orchestration with tokio::spawn - calls business logic from core.rs

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::archive::ArchiveStorageVariant;
use crate::commitment_registry::CommitmentRegistryVariant;
use crate::config::AccumulatorType;
use crate::proof_delivery::ProofDeliveryVariant;
use crate::proof_registry::ProofRegistryVariant;
use crate::storage::Storage;
use crate::types::{IncomingRecord, Key32, Namespace, StoredProof, Value32};
use crate::upstream::UpstreamVariant;
use anyhow::Result;
use kanal::{unbounded_async, AsyncReceiver, AsyncSender};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, span, Level};

use super::core::{CommittedRecord, RootSmith};

// ==================== TYPE ALIASES ====================

type NamespaceMap = Arc<tokio::sync::Mutex<HashMap<Namespace, bool>>>;
type StorageArc = Arc<tokio::sync::Mutex<Storage>>;
type CommittedRecordsList = Arc<tokio::sync::Mutex<Vec<CommittedRecord>>>;
type EpochStartTs = Arc<tokio::sync::Mutex<u64>>;
type CommitmentRegistry = Arc<tokio::sync::Mutex<CommitmentRegistryVariant>>;
type ProofRegistry = Arc<tokio::sync::Mutex<ProofRegistryVariant>>;
type ProofDelivery = Arc<tokio::sync::Mutex<ProofDeliveryVariant>>;
type ArchiveStorage = Arc<tokio::sync::Mutex<ArchiveStorageVariant>>;

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
fn spawn_oneshot_task<F, Fut>(
    name: &'static str,
    task_fn: F,
) -> JoinHandle<Result<()>>
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
    commitment_registry: &CommitmentRegistry,
    archive_threshold_secs: u64,
) -> Result<()> {
    let now = RootSmith::now_secs();
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
    let archived_count = archive_old_records(
        archive_storage,
        storage,
        &namespaces,
        archive_cutoff,
    )
    .await?;

    // Archive old commitments
    let archived_commitments = archive_old_commitments(
        archive_storage,
        commitment_registry,
        &namespaces,
        archive_cutoff,
    )
    .await?;

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
    use crate::traits::{ArchiveData, ArchiveStorage as ArchiveStorageTrait};

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
                let archive_data: Vec<ArchiveData> =
                    chunk.iter().map(|r| ArchiveData::Record(r.clone())).collect();

                let mut archive = archive_storage.lock().await;
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

/// Archive old commitments for namespaces.
async fn archive_old_commitments(
    archive_storage: &ArchiveStorage,
    commitment_registry: &CommitmentRegistry,
    namespaces: &[Namespace],
    archive_cutoff: u64,
) -> Result<usize> {
    use crate::traits::{ArchiveData, ArchiveStorage as ArchiveStorageTrait, CommitmentRegistry as CommitmentRegistryTrait};
    use crate::types::{BatchCommitmentMeta, CommitmentFilterOptions};

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
        let upstream_handle = spawn_oneshot_task("upstream", || async move {
            match RootSmith::run_upstream_task(upstream, data_tx.clone()).await {
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
                async move {
                    RootSmith::storage_write_once(&storage, &active_namespaces, &record).await
                }
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

            spawn_interval_task("commit", Some(Duration::from_millis(100)), move || {
                let storage = Arc::clone(&storage);
                let active_namespaces = Arc::clone(&active_namespaces);
                let epoch_start_ts = Arc::clone(&epoch_start_ts);
                let committed_records = Arc::clone(&committed_records);
                let commitment_registry = Arc::clone(&commitment_registry);
                let commit_tx = commit_tx.clone();
                let config_clone = config_clone.clone();
                
                async move {
                    RootSmith::process_commit_cycle(
                        &epoch_start_ts,
                        &active_namespaces,
                        &storage,
                        &committed_records,
                        &commitment_registry,
                        &commit_tx,
                        config_clone.batch_interval_secs,
                        config_clone.accumulator_type,
                    )
                    .await
                    .map(|_| ())
                }
            })
        };

        // === Proof registry task: generate and persist proofs for committed batches ===
        let proof_registry_handle = {
            let proof_registry = Arc::clone(&proof_registry);
            let config_clone = config.clone();
            let proof_delivery_tx = proof_delivery_tx.clone();

            spawn_channel_consumer_task("proof_registry", commit_rx, move |(namespace, root, committed_at, records)| {
                let proof_registry = Arc::clone(&proof_registry);
                let proof_delivery_tx = proof_delivery_tx.clone();
                let config_clone = config_clone.clone();
                
                async move {
                    RootSmith::process_proof_generation(
                        namespace,
                        root,
                        committed_at,
                        records,
                        &proof_registry,
                        &proof_delivery_tx,
                        config_clone.accumulator_type,
                    )
                    .await
                    .map(|_| ())
                }
            })
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

                    RootSmith::deliver_once(&proof_delivery, &proofs).await
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
                    match RootSmith::prune_once(&storage, &committed_records).await {
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

        // === Archiving task: archive old commitments and proofs to cold storage ===
        let archiving_handle = {
            let archive_storage = Arc::clone(&archive_storage);
            let storage = Arc::clone(&storage);
            let active_namespaces = Arc::clone(&active_namespaces);
            let commitment_registry = Arc::clone(&commitment_registry);

            // Archive retention threshold: data older than this will be archived
            let archive_threshold_secs = 30 * 24 * 3600; // 30 days

            spawn_interval_task("archiving", Some(Duration::from_secs(3600)), move || {
                let archive_storage = Arc::clone(&archive_storage);
                let storage = Arc::clone(&storage);
                let active_namespaces = Arc::clone(&active_namespaces);
                let commitment_registry = Arc::clone(&commitment_registry);
                
                async move {
                    process_archiving_cycle(
                        &archive_storage,
                        &storage,
                        &active_namespaces,
                        &commitment_registry,
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
}

