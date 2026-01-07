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
use tokio::sync::broadcast;
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

/// Request to commit a namespace at a specific time.
#[derive(Debug, Clone)]
struct CommitRequest {
    namespace: Namespace,
    committed_at: u64,
}

/// Result from reading storage and building merkle root.
#[derive(Debug, Clone)]
struct CommitData {
    namespace: Namespace,
    root: Vec<u8>,
    committed_at: u64,
    leaf_count: u64,
    records_to_track: Vec<CommittedRecord>,
}

/// Shutdown signal message.
#[derive(Debug, Clone)]
struct ShutdownSignal;

/// Result type for task operations that may be interrupted by shutdown.
type TaskResult<T> = Result<T>;

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

        // Create channels for task communication
        let (upstream_tx, upstream_rx) = unbounded_async::<IncomingRecord>();
        let (commit_tx, commit_rx) = unbounded_async::<CommitRequest>();
        let (delivery_tx, delivery_rx) = unbounded_async::<CommitData>();
        let (pruning_tx, pruning_rx) = unbounded_async::<Vec<CommittedRecord>>();
        let (shutdown_tx, _) = broadcast::channel::<ShutdownSignal>(5);

        info!("Starting upstream connector: {}", self.upstream.name());

        // Task 1: Pull from upstream
        let upstream_handle = Self::spawn_with_shutdown(
            "upstream_task",
            self.upstream,
            upstream_tx.clone(),
            shutdown_tx.clone(),
            |mut upstream, tx, mut shutdown_rx| async move {
                tokio::select! {
                    result = upstream.open(tx) => {
                        if let Err(e) = result {
                            error!("Upstream connector failed: {}", e);
                            return Err(e);
                        }
                        info!("Upstream connector finished");
                        Ok(())
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Upstream task received shutdown signal");
                        Ok(())
                    }
                }
            },
        );

        // Task 2: Write to storage
        let storage_writer_handle = Self::spawn_with_shutdown(
            "storage_writer_task",
            (
                Arc::clone(&self.storage),
                Arc::clone(&self.active_namespaces),
            ),
            upstream_rx,
            shutdown_tx.clone(),
            |(storage, active_namespaces), rx, shutdown_rx| async move {
                Self::storage_writer_task(storage, active_namespaces, rx, shutdown_rx).await
            },
        );

        // Task 3: Read storage and commit (build merkle roots)
        let commit_reader_handle = Self::spawn_with_shutdown(
            "commit_reader_task",
            (Arc::clone(&self.storage), self.config.accumulator_type, delivery_tx.clone()),
            commit_rx,
            shutdown_tx.clone(),
            |(storage, accumulator_type, delivery_tx), rx, shutdown_rx| async move {
                Self::commit_reader_task(storage, accumulator_type, rx, delivery_tx, shutdown_rx)
                    .await
            },
        );

        // Task 4: Delivery handling (commitment/proof registry)
        let delivery_handle = Self::spawn_with_shutdown(
            "delivery_task",
            (self.commitment_registry, self.proof_registry, pruning_tx.clone()),
            delivery_rx,
            shutdown_tx.clone(),
            |(commitment_registry, proof_registry, pruning_tx), rx, shutdown_rx| async move {
                Self::delivery_task(commitment_registry, proof_registry, rx, pruning_tx, shutdown_rx)
                    .await
            },
        );

        // Task 5: DB pruning
        let pruning_handle = Self::spawn_with_shutdown(
            "pruning_task",
            Arc::clone(&self.storage),
            pruning_rx,
            shutdown_tx.clone(),
            |storage, rx, shutdown_rx| async move {
                Self::pruning_task(storage, rx, shutdown_rx).await
            },
        );

        // Coordinator task: manages commit timing
        let coordinator_handle = {
            let epoch_start_ts = Arc::clone(&self.epoch_start_ts);
            let active_namespaces = Arc::clone(&self.active_namespaces);
            let config = self.config.clone();
            let mut shutdown_rx = shutdown_tx.subscribe();
            let commit_tx_clone = commit_tx.clone();

            tokio::task::spawn(async move {
                let span = span!(Level::INFO, "coordinator_task");
                let _enter = span.enter();

                loop {
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(100)) => {
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
                                    if commit_tx_clone
                                        .send(CommitRequest {
                                            namespace,
                                            committed_at,
                                        })
                                        .await
                                        .is_err()
                                    {
                                        error!("Failed to send commit request, commit channel closed");
                                        break;
                                    }
                                }

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
                        }
                        _ = shutdown_rx.recv() => {
                            info!("Coordinator task received shutdown signal");
                            
                            // Perform final commit if there are any active namespaces
                            let should_final_commit = {
                                let namespaces = active_namespaces.lock().unwrap();
                                !namespaces.is_empty()
                            };

                            if should_final_commit {
                                info!("Performing final commit before shutdown");
                                
                                let committed_at = {
                                    let epoch_start = *epoch_start_ts.lock().unwrap();
                                    Self::now_secs().max(epoch_start + config.batch_interval_secs)
                                };

                                let namespaces: Vec<Namespace> = {
                                    let ns = active_namespaces.lock().unwrap();
                                    ns.keys().copied().collect()
                                };

                                info!("Final commit: {} namespaces", namespaces.len());

                                for namespace in namespaces {
                                    if commit_tx_clone
                                        .send(CommitRequest {
                                            namespace,
                                            committed_at,
                                        })
                                        .await
                                        .is_err()
                                    {
                                        error!("Failed to send final commit request");
                                        break;
                                    }
                                }

                                {
                                    let mut ns = active_namespaces.lock().unwrap();
                                    ns.clear();
                                }
                            }
                            
                            break;
                        }
                    }
                }

                info!("Coordinator task finished");
                Ok::<(), anyhow::Error>(())
            })
        };

        // Wait for upstream to complete or fail
        let upstream_result = upstream_handle.await
            .map_err(|_| anyhow::anyhow!("Upstream task panicked"))?;

        // Drop upstream sender to signal no more data
        drop(upstream_tx);

        info!("Upstream completed, waiting for storage writer to finish");
        
        // Wait for storage writer to drain
        let storage_writer_result = storage_writer_handle.await
            .map_err(|_| anyhow::anyhow!("Storage writer task panicked"))?;

        info!("Storage writer completed, triggering coordinator shutdown");

        // Check upstream result and trigger shutdown
        match upstream_result {
            Err(e) => {
                error!("Upstream task failed: {}", e);
                let _ = shutdown_tx.send(ShutdownSignal);
            }
            Ok(()) => {
                // Trigger graceful shutdown
                let _ = shutdown_tx.send(ShutdownSignal);
            }
        }

        info!("Waiting for coordinator to complete final commit");
        
        // Wait for coordinator to finish (will do final commit)
        let coordinator_result = coordinator_handle.await
            .map_err(|_| anyhow::anyhow!("Coordinator task panicked"))?;

        info!("Coordinator completed, closing commit channel");
        
        // Drop commit sender to signal no more commits
        drop(commit_tx);

        info!("Waiting for remaining tasks to complete");
        
        // Wait for remaining tasks to complete
        let commit_reader_result = commit_reader_handle.await
            .map_err(|_| anyhow::anyhow!("Commit reader task panicked"))?;
        
        // Drop delivery sender
        drop(delivery_tx);
        
        let delivery_result = delivery_handle.await
            .map_err(|_| anyhow::anyhow!("Delivery task panicked"))?;
        
        // Drop pruning sender
        drop(pruning_tx);
        
        let pruning_result = pruning_handle.await
            .map_err(|_| anyhow::anyhow!("Pruning task panicked"))?;

        storage_writer_result?;
        commit_reader_result?;
        delivery_result?;
        pruning_result?;
        coordinator_result?;

        info!("RootSmith run completed successfully");
        Ok(())
    }

    /// Helper method to spawn a task with graceful shutdown support.
    fn spawn_with_shutdown<T, R, F, Fut>(
        _task_name: &'static str,
        context: T,
        input: R,
        shutdown_tx: broadcast::Sender<ShutdownSignal>,
        task_fn: F,
    ) -> tokio::task::JoinHandle<TaskResult<()>>
    where
        T: Send + 'static,
        R: Send + 'static,
        F: FnOnce(T, R, broadcast::Receiver<ShutdownSignal>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = TaskResult<()>> + Send + 'static,
    {
        let shutdown_rx = shutdown_tx.subscribe();
        tokio::task::spawn(async move {
            task_fn(context, input, shutdown_rx).await
        })
    }

    /// Task 2: Write incoming records to storage.
    async fn storage_writer_task(
        storage: Arc<Mutex<Storage>>,
        active_namespaces: Arc<Mutex<HashMap<Namespace, bool>>>,
        rx: AsyncReceiver<IncomingRecord>,
        mut shutdown_rx: broadcast::Receiver<ShutdownSignal>,
    ) -> TaskResult<()> {
        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(record) => {
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
                        Err(_) => {
                            info!("Storage writer: upstream channel closed");
                            break;
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Storage writer task received shutdown signal");
                    break;
                }
            }
        }

        info!("Storage writer task finished");
        Ok(())
    }

    /// Task 3: Read storage and build merkle roots for commits.
    async fn commit_reader_task(
        storage: Arc<Mutex<Storage>>,
        accumulator_type: AccumulatorType,
        rx: AsyncReceiver<CommitRequest>,
        tx: AsyncSender<CommitData>,
        mut _shutdown_rx: broadcast::Receiver<ShutdownSignal>,
    ) -> TaskResult<()> {
        // This task processes until the commit channel is closed
        // Shutdown signal is only used if we're blocked indefinitely
        loop {
            match rx.recv().await {
                Ok(request) => {
                    let commit_data = Self::read_and_build_commit(
                        &storage,
                        request.namespace,
                        request.committed_at,
                        accumulator_type,
                    )?;

                    if let Some(data) = commit_data {
                        if tx.send(data).await.is_err() {
                            error!("Failed to send commit data, delivery channel closed");
                            break;
                        }
                    }
                }
                Err(_) => {
                    info!("Commit reader: commit channel closed");
                    break;
                }
            }
        }

        info!("Commit reader task finished");
        Ok(())
    }

    /// Task 4: Handle delivery to commitment and proof registries.
    async fn delivery_task(
        commitment_registry: CommitmentRegistryVariant,
        proof_registry: ProofRegistryVariant,
        rx: AsyncReceiver<CommitData>,
        tx: AsyncSender<Vec<CommittedRecord>>,
        mut _shutdown_rx: broadcast::Receiver<ShutdownSignal>,
    ) -> TaskResult<()> {
        // This task processes until the delivery channel is closed
        loop {
            match rx.recv().await {
                Ok(data) => {
                    Self::deliver_commitment(
                        &commitment_registry,
                        &proof_registry,
                        &data,
                    ).await?;

                    // Send records for pruning
                    if tx.send(data.records_to_track).await.is_err() {
                        error!("Failed to send pruning data, pruning channel closed");
                        break;
                    }
                }
                Err(_) => {
                    info!("Delivery task: delivery channel closed");
                    break;
                }
            }
        }

        info!("Delivery task finished");
        Ok(())
    }

    /// Task 5: Prune committed data from storage.
    async fn pruning_task(
        storage: Arc<Mutex<Storage>>,
        rx: AsyncReceiver<Vec<CommittedRecord>>,
        mut _shutdown_rx: broadcast::Receiver<ShutdownSignal>,
    ) -> TaskResult<()> {
        // This task processes until the pruning channel is closed
        loop {
            match rx.recv().await {
                Ok(records) => {
                    Self::prune_records(&storage, records)?;
                }
                Err(_) => {
                    info!("Pruning task: pruning channel closed");
                    break;
                }
            }
        }

        info!("Pruning task finished");
        Ok(())
    }

    /// Read storage and build merkle root for a namespace.
    fn read_and_build_commit(
        storage: &Arc<Mutex<Storage>>,
        namespace: Namespace,
        committed_at: u64,
        accumulator_type: AccumulatorType,
    ) -> Result<Option<CommitData>> {
        let storage_guard = storage.lock().unwrap();

        let filter = crate::storage::StorageScanFilter {
            namespace,
            timestamp: Some(committed_at),
        };
        let records = storage_guard.scan(&filter)?;

        if records.is_empty() {
            debug!("No records to commit for namespace {:?}", namespace);
            return Ok(None);
        }

        info!(
            "Building commit for {} records in namespace {:?}",
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
        let leaf_count = records.len() as u64;

        Ok(Some(CommitData {
            namespace,
            root,
            committed_at,
            leaf_count,
            records_to_track,
        }))
    }

    /// Deliver commitment to registries.
    async fn deliver_commitment(
        commitment_registry: &CommitmentRegistryVariant,
        proof_registry: &ProofRegistryVariant,
        data: &CommitData,
    ) -> Result<()> {
        let commitment = Commitment {
            namespace: data.namespace,
            root: data.root.clone(),
            committed_at: data.committed_at,
        };

        let meta = BatchCommitmentMeta {
            commitment,
            leaf_count: data.leaf_count,
        };

        commitment_registry.commit(&meta).await?;

        let proofs: Vec<StoredProof> = Vec::new();
        if !proofs.is_empty() {
            proof_registry.save_proofs(&proofs).await?;
        }

        info!(
            "Delivered commitment for namespace {:?}: {} records, root len={}",
            data.namespace,
            data.leaf_count,
            data.root.len()
        );

        Ok(())
    }

    /// Prune records from storage.
    fn prune_records(
        storage: &Arc<Mutex<Storage>>,
        records: Vec<CommittedRecord>,
    ) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        info!(
            "Pruning {} committed records from storage",
            records.len()
        );

        let storage_guard = storage.lock().unwrap();

        let mut by_namespace: HashMap<Namespace, Vec<&CommittedRecord>> = HashMap::new();
        for record in &records {
            by_namespace
                .entry(record.namespace)
                .or_insert_with(Vec::new)
                .push(record);
        }

        for (namespace, recs) in by_namespace {
            if let Some(max_ts) = recs.iter().map(|r| r.timestamp).max() {
                let delete_filter = crate::storage::StorageDeleteFilter {
                    namespace,
                    timestamp: max_ts,
                };
                storage_guard.delete(&delete_filter)?;
                debug!(
                    "Pruned {} records for namespace {:?}",
                    recs.len(),
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
    use crate::commitment_registry::{CommitmentRegistryVariant, MockCommitmentRegistry};
    use crate::config::AccumulatorType;
    use crate::proof_registry::{MockProofRegistry, ProofRegistryVariant};
    use tempfile::tempdir;

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

    #[tokio::test]
    async fn test_storage_writer_task() {
        let temp_dir = tempdir().unwrap();
        let storage_path = temp_dir.path().join("test_storage_writer");
        let storage = Storage::open(storage_path.to_str().unwrap()).unwrap();
        let storage = Arc::new(Mutex::new(storage));
        let active_namespaces = Arc::new(Mutex::new(HashMap::new()));

        let (tx, rx) = unbounded_async::<IncomingRecord>();
        let (_shutdown_tx, shutdown_rx) = broadcast::channel::<ShutdownSignal>(1);

        // Spawn storage writer task
        let storage_clone = Arc::clone(&storage);
        let namespaces_clone = Arc::clone(&active_namespaces);
        let task = tokio::spawn(async move {
            RootSmith::storage_writer_task(storage_clone, namespaces_clone, rx, shutdown_rx).await
        });

        // Send test records
        let namespace = [1u8; 32];
        let record = IncomingRecord {
            namespace,
            key: [2u8; 32],
            value: [3u8; 32],
            timestamp: 100,
        };

        tx.send(record.clone()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Verify record was stored
        {
            let storage_guard = storage.lock().unwrap();
            let filter = crate::storage::StorageScanFilter {
                namespace,
                timestamp: None,
            };
            let records = storage_guard.scan(&filter).unwrap();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].key, record.key);
        }

        // Verify namespace was marked active
        {
            let namespaces = active_namespaces.lock().unwrap();
            assert!(namespaces.contains_key(&namespace));
        }

        // Close channel to signal completion
        drop(tx);
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_read_and_build_commit() {
        let temp_dir = tempdir().unwrap();
        let storage_path = temp_dir.path().join("test_read_build");
        let storage = Storage::open(storage_path.to_str().unwrap()).unwrap();

        // Add test records
        let namespace = [1u8; 32];
        let timestamp = 100;
        for i in 0..3 {
            let record = IncomingRecord {
                namespace,
                key: [i; 32],
                value: [i + 10; 32],
                timestamp,
            };
            storage.put(&record).unwrap();
        }

        let storage = Arc::new(Mutex::new(storage));

        // Test reading and building commit
        let result = RootSmith::read_and_build_commit(
            &storage,
            namespace,
            timestamp + 1,
            AccumulatorType::Merkle,
        )
        .unwrap();

        assert!(result.is_some());
        let commit_data = result.unwrap();
        assert_eq!(commit_data.namespace, namespace);
        assert_eq!(commit_data.leaf_count, 3);
        assert_eq!(commit_data.records_to_track.len(), 3);
        assert!(!commit_data.root.is_empty());
    }

    #[tokio::test]
    async fn test_read_and_build_commit_empty() {
        let temp_dir = tempdir().unwrap();
        let storage_path = temp_dir.path().join("test_read_build_empty");
        let storage = Storage::open(storage_path.to_str().unwrap()).unwrap();
        let storage = Arc::new(Mutex::new(storage));

        // Test with no records
        let namespace = [1u8; 32];
        let result = RootSmith::read_and_build_commit(
            &storage,
            namespace,
            100,
            AccumulatorType::Merkle,
        )
        .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_deliver_commitment() {
        let commitment_registry = MockCommitmentRegistry::new();
        let commitment_registry_clone = commitment_registry.clone();
        let commitment_registry_variant = CommitmentRegistryVariant::Mock(commitment_registry);
        let proof_registry = ProofRegistryVariant::Mock(MockProofRegistry::new());

        let commit_data = CommitData {
            namespace: [1u8; 32],
            root: vec![0xaa, 0xbb, 0xcc],
            committed_at: 100,
            leaf_count: 5,
            records_to_track: vec![],
        };

        RootSmith::deliver_commitment(
            &commitment_registry_variant,
            &proof_registry,
            &commit_data,
        )
        .await
        .unwrap();

        // Verify commitment was registered
        let commitments = commitment_registry_clone.get_commitments();
        assert_eq!(commitments.len(), 1);
        assert_eq!(commitments[0].commitment.namespace, commit_data.namespace);
        assert_eq!(commitments[0].commitment.root, commit_data.root);
        assert_eq!(commitments[0].leaf_count, commit_data.leaf_count);
    }

    #[tokio::test]
    async fn test_prune_records() {
        let temp_dir = tempdir().unwrap();
        let storage_path = temp_dir.path().join("test_prune");
        let storage = Storage::open(storage_path.to_str().unwrap()).unwrap();

        // Add test records
        let namespace = [1u8; 32];
        let timestamp = 100;
        for i in 0..3 {
            let record = IncomingRecord {
                namespace,
                key: [i; 32],
                value: [i + 10; 32],
                timestamp,
            };
            storage.put(&record).unwrap();
        }

        let storage = Arc::new(Mutex::new(storage));

        // Verify records exist
        {
            let storage_guard = storage.lock().unwrap();
            let filter = crate::storage::StorageScanFilter {
                namespace,
                timestamp: None,
            };
            let records = storage_guard.scan(&filter).unwrap();
            assert_eq!(records.len(), 3);
        }

        // Prune records
        let records_to_prune = vec![
            CommittedRecord {
                namespace,
                key: [0; 32],
                value: [10; 32],
                timestamp,
            },
            CommittedRecord {
                namespace,
                key: [1; 32],
                value: [11; 32],
                timestamp,
            },
            CommittedRecord {
                namespace,
                key: [2; 32],
                value: [12; 32],
                timestamp,
            },
        ];

        RootSmith::prune_records(&storage, records_to_prune).unwrap();

        // Verify records were pruned
        {
            let storage_guard = storage.lock().unwrap();
            let filter = crate::storage::StorageScanFilter {
                namespace,
                timestamp: Some(timestamp),
            };
            let records = storage_guard.scan(&filter).unwrap();
            assert_eq!(records.len(), 0);
        }
    }

    #[tokio::test]
    async fn test_graceful_shutdown_signal() {
        // Test that shutdown signal propagates correctly
        let (shutdown_tx, mut shutdown_rx1) = broadcast::channel::<ShutdownSignal>(2);
        let mut shutdown_rx2 = shutdown_tx.subscribe();

        // Send shutdown signal
        shutdown_tx.send(ShutdownSignal).unwrap();

        // Both receivers should get the signal
        tokio::select! {
            result = shutdown_rx1.recv() => {
                assert!(result.is_ok());
            }
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                panic!("Shutdown signal not received by receiver 1");
            }
        }

        tokio::select! {
            result = shutdown_rx2.recv() => {
                assert!(result.is_ok());
            }
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                panic!("Shutdown signal not received by receiver 2");
            }
        }
    }

    #[tokio::test]
    async fn test_task_communication_chain() {
        // Test the full chain of task communication
        let temp_dir = tempdir().unwrap();
        let storage_path = temp_dir.path().join("test_chain");
        let storage = Storage::open(storage_path.to_str().unwrap()).unwrap();
        let storage = Arc::new(Mutex::new(storage));

        let (commit_tx, commit_rx) = unbounded_async::<CommitRequest>();
        let (delivery_tx, delivery_rx) = unbounded_async::<CommitData>();
        let (pruning_tx, pruning_rx) = unbounded_async::<Vec<CommittedRecord>>();

        // Add test records to storage
        let namespace = [1u8; 32];
        let timestamp = 100;
        for i in 0..2 {
            let record = IncomingRecord {
                namespace,
                key: [i; 32],
                value: [i + 10; 32],
                timestamp,
            };
            storage.lock().unwrap().put(&record).unwrap();
        }

        // Spawn commit reader task
        let storage_clone = Arc::clone(&storage);
        let (_, shutdown_rx1) = broadcast::channel::<ShutdownSignal>(1);
        let commit_reader = tokio::spawn(async move {
            RootSmith::commit_reader_task(
                storage_clone,
                AccumulatorType::Merkle,
                commit_rx,
                delivery_tx,
                shutdown_rx1,
            )
            .await
        });

        // Spawn delivery task
        let commitment_registry = MockCommitmentRegistry::new();
        let commitment_registry_clone = commitment_registry.clone();
        let commitment_registry_variant = CommitmentRegistryVariant::Mock(commitment_registry);
        let proof_registry = ProofRegistryVariant::Mock(MockProofRegistry::new());
        let (_, shutdown_rx2) = broadcast::channel::<ShutdownSignal>(1);
        let delivery = tokio::spawn(async move {
            RootSmith::delivery_task(
                commitment_registry_variant,
                proof_registry,
                delivery_rx,
                pruning_tx,
                shutdown_rx2,
            )
            .await
        });

        // Spawn pruning task
        let storage_clone = Arc::clone(&storage);
        let (_, shutdown_rx3) = broadcast::channel::<ShutdownSignal>(1);
        let pruning = tokio::spawn(async move {
            RootSmith::pruning_task(storage_clone, pruning_rx, shutdown_rx3).await
        });

        // Send commit request
        commit_tx
            .send(CommitRequest {
                namespace,
                committed_at: timestamp + 1,
            })
            .await
            .unwrap();

        // Give tasks time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Close channels to signal completion
        drop(commit_tx);

        // Wait for all tasks
        commit_reader.await.unwrap().unwrap();
        delivery.await.unwrap().unwrap();
        pruning.await.unwrap().unwrap();

        // Verify commitment was created
        let commitments = commitment_registry_clone.get_commitments();
        assert_eq!(commitments.len(), 1);
        assert_eq!(commitments[0].commitment.namespace, namespace);
        assert_eq!(commitments[0].leaf_count, 2);
    }
}
