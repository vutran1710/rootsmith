use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver};
use tracing::{info, error, debug, span, Level};
use crate::storage::Storage;
use crate::types::{BatchCommitmentMeta, Commitment, IncomingRecord, Namespace, StoredProof};
use crate::config::{BaseConfig, AccumulatorType};
use crate::traits::{Accumulator, CommitmentRegistry, ProofRegistry, UpstreamConnector};
use crate::upstream::UpstreamVariant;
use crate::commitment_registry::CommitmentRegistryVariant;
use crate::proof_registry::ProofRegistryVariant;
use crate::crypto::AccumulatorVariant;

/// Epoch phase for the commit cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpochPhase {
    /// Waiting for the next commit window.
    Pending,
    /// Actively committing batches.
    Commit,
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

    /// Initialize RootSmith with default/noop components for demonstration.
    /// In production, this would be replaced with feature-gated concrete implementations.
    pub fn initialize(config: BaseConfig) -> Result<Self> {
        use crate::upstream::NoopUpstream;
        use crate::commitment_registry::CommitmentRegistryVariant;
        use crate::commitment_registry::commitment_noop::CommitmentNoop;
        use crate::proof_registry::{ProofRegistryVariant, NoopProofRegistry};

        // Initialize storage
        let storage = Storage::open(&config.storage_path)?;
        info!("Storage opened at: {}", config.storage_path);
        
        // Create upstream connector
        let upstream = UpstreamVariant::Noop(NoopUpstream);
        
        // Create registries
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

    /// Main run loop with parallel tasks:
    /// - Task 1: Upstream receiving data, forwarding to data channel
    /// - Task 2: Clock & commit with proper synchronization
    #[tracing::instrument(skip(self), name = "app_run")]
    pub async fn run(self) -> Result<()> {
        let span = span!(Level::INFO, "app_run");
        let _enter = span.enter();
        
        info!("Starting RootSmith with batch_interval_secs={}", self.config.batch_interval_secs);
        
        // Create unbounded channel for data flow
        let (data_tx, data_rx) = unbounded::<IncomingRecord>();
        
        info!("Starting upstream connector: {}", self.upstream.name());
        
        // Task 1: Upstream data ingestion
        let upstream_handle = {
            let data_tx_clone = data_tx.clone();
            let mut upstream = self.upstream;
            tokio::task::spawn(async move {
                let span = span!(Level::INFO, "upstream_task");
                let _enter = span.enter();
                
                if let Err(e) = upstream.open(data_tx_clone).await {
                    error!("Upstream connector failed: {}", e);
                    return Err(e);
                }
                
                info!("Upstream connector finished");
                Ok(())
            })
        };
        
        // Task 2: Data processing and commit cycle
        let process_handle = {
            let storage = Arc::clone(&self.storage);
            let epoch_start_ts = Arc::clone(&self.epoch_start_ts);
            let active_namespaces = Arc::clone(&self.active_namespaces);
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
                    commitment_registry,
                    proof_registry,
                    config,
                ).await
            })
        };
        
        // Wait for both tasks to complete
        let upstream_result = upstream_handle.await
            .map_err(|_| anyhow::anyhow!("Upstream task panicked"))?;
        
        // Drop the sender to signal data processing to finish
        drop(data_tx);
        
        let process_result = process_handle.await
            .map_err(|_| anyhow::anyhow!("Process task panicked"))?;
        
        upstream_result?;
        process_result?;
        
        info!("RootSmith run completed successfully");
        Ok(())
    }

    /// Combined data processing and commit loop.
    /// This runs in a separate thread and handles both incoming data and periodic commits.
    #[tracing::instrument(skip_all, name = "process_and_commit_loop")]
    async fn process_and_commit_loop(
        data_rx: Receiver<IncomingRecord>,
        storage: Arc<Mutex<Storage>>,
        epoch_start_ts: Arc<Mutex<u64>>,
        active_namespaces: Arc<Mutex<HashMap<Namespace, bool>>>,
        commitment_registry: CommitmentRegistryVariant,
        proof_registry: ProofRegistryVariant,
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
                info!("Commit phase starting - pausing data ingestion");
                
                // Perform commit with storage lock held (pauses DB inserts)
                Self::perform_commit_locked(
                    &storage,
                    &epoch_start_ts,
                    &active_namespaces,
                    &commitment_registry,
                    &proof_registry,
                    &config,
                ).await?;
                
                info!("Commit phase completed - resuming data ingestion");
            }
            
            // Process incoming data with timeout to allow periodic commit checks
            match data_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(record) => {
                    let span = span!(Level::DEBUG, "handle_record", 
                        namespace = ?record.namespace);
                    let _enter = span.enter();
                    
                    // Lock storage for writing
                    let storage_guard = storage.lock().unwrap();
                    
                    // Track active namespace
                    {
                        let mut namespaces = active_namespaces.lock().unwrap();
                        namespaces.insert(record.namespace, true);
                    }
                    
                    // Write to storage
                    storage_guard.put(&record)?;
                    debug!("Record stored for namespace {:?}", record.namespace);
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    // Timeout - continue to check for commit
                    continue;
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    // Channel closed - perform final commit if needed
                    info!("Data channel closed, performing final commit if needed");
                    
                    let should_final_commit = {
                        let namespaces = active_namespaces.lock().unwrap();
                        !namespaces.is_empty()
                    };
                    
                    if should_final_commit {
                        Self::perform_commit_locked(
                            &storage,
                            &epoch_start_ts,
                            &active_namespaces,
                            &commitment_registry,
                            &proof_registry,
                            &config,
                        ).await?;
                    }
                    
                    break;
                }
            }
        }
        
        info!("Process and commit loop finished");
        Ok(())
    }

    /// Perform commit with locks held (this pauses DB inserts).
    #[tracing::instrument(skip_all, name = "commit")]
    async fn perform_commit_locked(
        storage: &Arc<Mutex<Storage>>,
        epoch_start_ts: &Arc<Mutex<u64>>,
        active_namespaces: &Arc<Mutex<HashMap<Namespace, bool>>>,
        commitment_registry: &CommitmentRegistryVariant,
        proof_registry: &ProofRegistryVariant,
        config: &BaseConfig,
    ) -> Result<()> {
        info!("Starting commit phase");
        
        let committed_at = {
            let epoch_start = *epoch_start_ts.lock().unwrap();
            epoch_start + config.batch_interval_secs
        };
        
        // Get list of active namespaces
        let namespaces: Vec<Namespace> = {
            let ns = active_namespaces.lock().unwrap();
            ns.keys().copied().collect()
        };
        
        info!("Committing {} namespaces", namespaces.len());
        
        // Commit each namespace
        for namespace in namespaces {
            Self::commit_namespace_locked(
                storage,
                &namespace,
                committed_at,
                commitment_registry,
                proof_registry,
                config.accumulator_type,
            ).await?;
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
        Ok(())
    }

    /// Commit a single namespace with storage lock.
    #[tracing::instrument(skip_all, name = "commit_namespace", fields(namespace = ?namespace))]
    async fn commit_namespace_locked(
        storage: &Arc<Mutex<Storage>>,
        namespace: &Namespace,
        committed_at: u64,
        commitment_registry: &CommitmentRegistryVariant,
        proof_registry: &ProofRegistryVariant,
        accumulator_type: AccumulatorType,
    ) -> Result<()> {
        // Scan storage and prepare data (release lock before async operations)
        let (root, record_count) = {
            let storage_guard = storage.lock().unwrap();
            
            // Scan storage for this namespace up to committed_at
            let filter = crate::storage::StorageScanFilter {
                namespace: *namespace,
                timestamp: Some(committed_at),
            };
            let records = storage_guard.scan(&filter)?;
            
            if records.is_empty() {
                debug!("No records to commit for namespace {:?}", namespace);
                return Ok(());
            }
            
            info!("Committing {} records for namespace {:?}", records.len(), namespace);
            
            // Create accumulator
            let mut accumulator = AccumulatorVariant::new(accumulator_type);
            
            // Add all records to accumulator
            for record in &records {
                accumulator.put(record.key, record.value.clone())?;
            }
            
            // Build root
            let root = accumulator.build_root()?;
            let record_count = records.len();
            
            (root, record_count)
            // storage_guard is dropped here
        };
        
        // Create commitment
        let commitment = Commitment {
            namespace: *namespace,
            root: root.clone(),
            committed_at,
        };
        
        let meta = BatchCommitmentMeta {
            commitment,
            leaf_count: record_count as u64,
        };
        
        // Save commitment (async operation)
        commitment_registry.commit(&meta).await?;
        
        // Generate and save proofs (placeholder - would generate real proofs here)
        let proofs: Vec<StoredProof> = Vec::new();
        if !proofs.is_empty() {
            proof_registry.save_proofs(&proofs).await?;
        }
        
        // Remove committed data from storage (re-acquire lock)
        {
            let storage_guard = storage.lock().unwrap();
            let delete_filter = crate::storage::StorageDeleteFilter {
                namespace: *namespace,
                timestamp: committed_at,
            };
            storage_guard.delete(&delete_filter)?;
        }
        
        info!(
            "Committed namespace {:?}: {} records, root len={}",
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
        
        // Test the RootSmith::now_secs function (it's public and associated with RootSmith)
        let app_now = RootSmith::now_secs();
        assert!(app_now > 0);
        assert!(app_now < u64::MAX);
        // Should be very close to the previous timestamp
        assert!((app_now as i64 - now as i64).abs() < 2);
    }
}

