//! Async task orchestration with tokio::spawn - calls business logic from core.rs

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::types::{IncomingRecord, Key32, Namespace, StoredProof};
use anyhow::Result;
use kanal::{unbounded_async, AsyncSender};
use tracing::{debug, error, info, span, Level};

use super::core::{CommittedRecord, RootSmith};

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
        let upstream_handle = {
            let async_tx = data_tx.clone();
            tokio::spawn(async move {
                let span = span!(Level::INFO, "upstream_task");
                let _enter = span.enter();

                match RootSmith::run_upstream_task(upstream, async_tx).await {
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
                    if let Err(e) =
                        RootSmith::storage_write_once(&storage, &active_namespaces, &record)
                    {
                        error!("Failed to write record: {}", e);
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

                info!(
                    "Commit task started (batch_interval_secs={})",
                    config_clone.batch_interval_secs
                );

                loop {
                    // Process commit cycle
                    match RootSmith::process_commit_cycle(
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
                    {
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
            let proof_registry = Arc::clone(&proof_registry);
            let config_clone = config.clone();
            let proof_delivery_tx = proof_delivery_tx.clone();

            tokio::spawn(async move {
                let span = span!(Level::INFO, "proof_registry_task");
                let _enter = span.enter();

                let registry_name = {
                    use crate::traits::ProofRegistry;
                    let registry = proof_registry.lock().await;
                    (*registry).name().to_string()
                };
                info!("Proof registry task started (registry={})", registry_name);

                // Listen for new commitments from commit task
                while let Ok((namespace, root, committed_at, records)) = commit_rx.recv().await {
                    match RootSmith::process_proof_generation(
                        namespace,
                        root,
                        committed_at,
                        records,
                        &proof_registry,
                        &proof_delivery_tx,
                        config_clone.accumulator_type,
                    )
                    .await
                    {
                        Ok(count) => {
                            debug!("Generated {} proofs for namespace {:?}", count, namespace);
                        }
                        Err(e) => {
                            error!(
                                "Failed to process proof generation for namespace {:?}: {}",
                                namespace, e
                            );
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
                    use crate::traits::ProofDelivery;
                    let delivery = proof_delivery.lock().await;
                    (*delivery).name().to_string()
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

                    // Use the deliver_once helper
                    if let Err(e) = RootSmith::deliver_once(&proof_delivery, &proofs).await {
                        error!("Failed to deliver proofs: {}", e);
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

                    match RootSmith::prune_once(&storage, &committed_records) {
                        Ok(count) if count > 0 => {
                            info!("Pruned {} records", count);
                        }
                        Ok(_) => {
                            // No records pruned, continue
                        }
                        Err(e) => {
                            error!("Error during pruning: {}", e);
                        }
                    }
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

                info!(
                    "Archiving task started with backend: {}",
                    {
                        use crate::traits::ArchiveStorage;
                        let storage = archive_storage.lock().await;
                        (*storage).name()
                    }
                );

                // Archive retention threshold: data older than this will be archived
                // In production, this could be configurable (e.g., 30 days)
                let archive_threshold_secs = 30 * 24 * 3600; // 30 days

                loop {
                    // Run archiving check every hour
                    tokio::time::sleep(Duration::from_secs(3600)).await;

                    let now = RootSmith::now_secs();
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

                    info!(
                        "Checking {} namespaces for archivable data",
                        namespaces.len()
                    );

                    // Archive old records from storage that are older than cutoff
                    let mut archived_count = 0;
                    for namespace in &namespaces {
                        // Scan storage for old records in this namespace
                        use crate::storage::StorageScanFilter;
                        use crate::traits::ArchiveData;

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
                                let archive_data: Vec<ArchiveData> =
                                    chunk.iter().map(|r| ArchiveData::Record(r.clone())).collect();

                                use crate::traits::ArchiveStorage;
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

                    // Archive old commitments for each namespace
                    use crate::types::{BatchCommitmentMeta, CommitmentFilterOptions};

                    let mut archived_commitments = 0;
                    for namespace in &namespaces {
                        let filter = CommitmentFilterOptions {
                            namespace: *namespace,
                            time: archive_cutoff,
                        };

                        use crate::traits::CommitmentRegistry;
                        let registry = commitment_registry.lock().await;
                        let result = (*registry).get_prev_commitment(&filter).await;
                        drop(registry);

                        match result {
                            Ok(Some(commitment)) if commitment.committed_at <= archive_cutoff => {
                                let meta = BatchCommitmentMeta {
                                    commitment: commitment.clone(),
                                    leaf_count: 0, // Unknown - could be tracked separately
                                };

                                use crate::traits::{ArchiveData, ArchiveStorage};
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

        // Skeleton tasks for future work
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
}

