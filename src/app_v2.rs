use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use crossbeam_channel::Receiver;
use crate::storage::Storage;
use crate::types::{BatchCommitmentMeta, Commitment, IncomingRecord, Namespace, StoredProof};
use crate::config::BaseConfig;
use crate::traits::{Accumulator, CommitmentRegistry, ProofRegistry, UpstreamConnector};

/// Epoch phase for the commit cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpochPhase {
    /// Waiting for the next commit window.
    Pending,
    /// Actively committing batches.
    Commit,
}

/// Main application orchestrator (V2 with epoch-based architecture).
pub struct AppV2<U, CR, PR>
where
    U: UpstreamConnector,
    CR: CommitmentRegistry,
    PR: ProofRegistry,
{
    /// Upstream connector.
    pub upstream: Option<U>,

    /// Commitment registry implementation.
    pub commitment_registry: CR,

    /// Proof registry implementation.
    pub proof_registry: PR,

    /// Global/base configuration.
    pub config: BaseConfig,

    /// Factory to create new accumulator instances per namespace.
    pub accumulator_factory: Arc<dyn Fn() -> Box<dyn Accumulator> + Send + Sync>,

    /// Persistent storage (RocksDB).
    pub storage: Storage,

    /// Start time of the current epoch (unix seconds).
    epoch_start_ts: u64,

    /// Track active namespaces for efficient commit.
    active_namespaces: HashMap<Namespace, bool>,
}

impl<U, CR, PR> AppV2<U, CR, PR>
where
    U: UpstreamConnector,
    CR: CommitmentRegistry,
    PR: ProofRegistry,
{
    /// Create a new AppV2.
    pub fn new(
        upstream: U,
        commitment_registry: CR,
        proof_registry: PR,
        config: BaseConfig,
        accumulator_factory: Arc<dyn Fn() -> Box<dyn Accumulator> + Send + Sync>,
        storage: Storage,
    ) -> Self {
        let now = Self::now_secs();
        
        Self {
            upstream: Some(upstream),
            commitment_registry,
            proof_registry,
            config,
            accumulator_factory,
            storage,
            epoch_start_ts: now,
            active_namespaces: HashMap::new(),
        }
    }

    /// Get current unix timestamp in seconds.
    pub fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time is before UNIX_EPOCH - please check your system clock")
            .as_secs()
    }

    /// Handle a single incoming record by writing it to storage.
    pub fn handle_incoming_record(&mut self, record: IncomingRecord) -> Result<()> {
        // Track active namespace
        self.active_namespaces.insert(record.namespace, true);
        
        // Write to storage
        self.storage.put(&record)?;
        Ok(())
    }

    /// Check if we should commit based on elapsed time.
    pub fn should_commit(&self) -> bool {
        let now = Self::now_secs();
        let elapsed = now.saturating_sub(self.epoch_start_ts);
        elapsed >= self.config.batch_interval_secs
    }

    /// Perform commit phase:
    /// - Retrieve all data from storage for each active namespace
    /// - Build accumulator and generate commitment
    /// - Save proofs and commitment
    /// - Remove committed data from storage
    pub fn perform_commit(&mut self) -> Result<()> {
        tracing::info!("Starting commit phase");

        let committed_at = self.epoch_start_ts + self.config.batch_interval_secs;

        // Commit each active namespace
        let namespaces: Vec<Namespace> = self.active_namespaces.keys().copied().collect();
        
        for namespace in namespaces {
            self.commit_namespace(&namespace, committed_at)?;
        }

        // Reset epoch
        self.epoch_start_ts = Self::now_secs();
        self.active_namespaces.clear();

        tracing::info!("Commit phase completed");
        Ok(())
    }

    /// Commit a single namespace's data.
    pub fn commit_namespace(
        &mut self,
        namespace: &Namespace,
        committed_at: u64,
    ) -> Result<()> {
        // Scan storage for this namespace up to committed_at
        let filter = crate::storage::StorageScanFilter {
            namespace: *namespace,
            timestamp: Some(committed_at),
        };
        let records = self.storage.scan(&filter)?;

        if records.is_empty() {
            return Ok(());
        }

        // Create accumulator
        let mut accumulator = (self.accumulator_factory)();

        // Add all records to accumulator
        for record in &records {
            accumulator.put(record.key, record.value.clone())?;
        }

        // Build root
        let root = accumulator.build_root()?;

        // Create commitment
        let commitment = Commitment {
            namespace: *namespace,
            root: root.clone(),
            committed_at,
        };

        let meta = BatchCommitmentMeta {
            commitment,
            leaf_count: records.len() as u64,
        };

        // Save commitment
        self.commitment_registry.commit(&meta)?;

        // Generate and save proofs (placeholder - would generate real proofs here)
        let proofs: Vec<StoredProof> = Vec::new();
        if !proofs.is_empty() {
            self.proof_registry.save_proofs(&proofs)?;
        }

        // Remove committed data from storage
        let delete_filter = crate::storage::StorageDeleteFilter {
            namespace: *namespace,
            timestamp: committed_at,
        };
        self.storage.delete(&delete_filter)?;

        tracing::info!(
            "Committed namespace {:?}: {} records, root len={}",
            namespace,
            records.len(),
            root.len()
        );

        Ok(())
    }

    /// Process incoming records from a channel for a duration.
    /// Returns when the receiver is closed or an error occurs.
    pub fn process_incoming(&mut self, rx: Receiver<IncomingRecord>) -> Result<()> {
        while let Ok(record) = rx.recv() {
            self.handle_incoming_record(record)?;

            // Check if we should commit
            if self.should_commit() {
                self.perform_commit()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CommitmentFilterOptions, Key32, StoredProof};
    use crossbeam_channel::Sender;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    // ===== Mock Implementations =====

    /// Mock upstream connector that generates test data.
    struct MockUpstreamConnector {
        records: Vec<IncomingRecord>,
        delay_ms: u64,
    }

    impl MockUpstreamConnector {
        fn new(records: Vec<IncomingRecord>, delay_ms: u64) -> Self {
            Self { records, delay_ms }
        }
    }

    impl UpstreamConnector for MockUpstreamConnector {
        fn name(&self) -> &'static str {
            "mock-upstream"
        }

        fn open(&mut self, tx: Sender<IncomingRecord>) -> Result<()> {
            let records = self.records.clone();
            let delay = self.delay_ms;

            thread::spawn(move || {
                for record in records {
                    if delay > 0 {
                        thread::sleep(Duration::from_millis(delay));
                    }
                    if tx.send(record).is_err() {
                        break;
                    }
                }
            });

            Ok(())
        }

        fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }

    /// Mock commitment registry that stores commitments in memory.
    #[derive(Clone)]
    struct MockCommitmentRegistry {
        commitments: Arc<Mutex<Vec<BatchCommitmentMeta>>>,
    }

    impl MockCommitmentRegistry {
        fn new() -> Self {
            Self {
                commitments: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_commitments(&self) -> Vec<BatchCommitmentMeta> {
            self.commitments.lock().unwrap().clone()
        }
    }

    impl CommitmentRegistry for MockCommitmentRegistry {
        fn name(&self) -> &'static str {
            "mock-commitment-registry"
        }

        fn commit(&self, meta: &BatchCommitmentMeta) -> Result<()> {
            self.commitments.lock().unwrap().push(meta.clone());
            println!(
                "MockCommitmentRegistry: committed {} leaves for namespace {:?}",
                meta.leaf_count,
                meta.commitment.namespace
            );
            Ok(())
        }

        fn get_prev_commitment(
            &self,
            filter: &CommitmentFilterOptions,
        ) -> Result<Option<Commitment>> {
            let commitments = self.commitments.lock().unwrap();
            
            let result = commitments
                .iter()
                .filter(|m| {
                    m.commitment.namespace == filter.namespace
                        && m.commitment.committed_at <= filter.time
                })
                .max_by_key(|m| m.commitment.committed_at)
                .map(|m| m.commitment.clone());

            Ok(result)
        }
    }

    /// Mock proof registry that stores proofs in memory.
    #[derive(Clone)]
    struct MockProofRegistry {
        proofs: Arc<Mutex<Vec<StoredProof>>>,
    }

    impl MockProofRegistry {
        fn new() -> Self {
            Self {
                proofs: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl ProofRegistry for MockProofRegistry {
        fn name(&self) -> &'static str {
            "mock-proof-registry"
        }

        fn save_proof(&self, proof: &StoredProof) -> Result<()> {
            self.proofs.lock().unwrap().push(proof.clone());
            Ok(())
        }

        fn save_proofs(&self, proofs: &[StoredProof]) -> Result<()> {
            for proof in proofs {
                self.save_proof(proof)?;
            }
            Ok(())
        }
    }

    /// Simple mock accumulator for testing.
    struct MockAccumulator {
        leaves: HashMap<Key32, Vec<u8>>,
    }

    impl MockAccumulator {
        fn new() -> Self {
            Self {
                leaves: HashMap::new(),
            }
        }
    }

    impl Accumulator for MockAccumulator {
        fn id(&self) -> &'static str {
            "mock-accumulator"
        }

        fn put(&mut self, key: Key32, value: Vec<u8>) -> Result<()> {
            self.leaves.insert(key, value);
            Ok(())
        }

        fn build_root(&self) -> Result<Vec<u8>> {
            // Simple hash: XOR all keys and values together
            let mut root = vec![0u8; 32];
            
            for (key, value) in &self.leaves {
                for (i, &byte) in key.iter().enumerate() {
                    root[i] ^= byte;
                }
                for (i, &byte) in value.iter().enumerate() {
                    root[i % 32] ^= byte;
                }
            }

            Ok(root)
        }

        fn verify_inclusion(&self, key: &Key32, value: &[u8]) -> Result<bool> {
            Ok(self.leaves.get(key).map(|v| v == value).unwrap_or(false))
        }

        fn verify_non_inclusion(&self, key: &Key32) -> Result<bool> {
            Ok(!self.leaves.contains_key(key))
        }

        fn flush(&mut self) -> Result<()> {
            self.leaves.clear();
            Ok(())
        }
    }

    // ===== Test Helper Functions =====

    fn test_namespace(id: u8) -> Namespace {
        let mut ns = [0u8; 32];
        ns[0] = id;
        ns
    }

    fn test_key(id: u8) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[0] = id;
        key
    }

    fn test_now() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before UNIX_EPOCH")
            .as_secs()
    }

    // ===== E2E Tests =====

    #[test]
    fn test_e2e_with_2s_commit_phase() -> Result<()> {
        println!("\n=== Starting E2E test with 2s commit phase ===\n");

        // Create temporary storage directory
        let temp_dir = tempfile::tempdir()?;
        let storage_path = temp_dir.path().join("test_db");
        let storage = Storage::open(storage_path.to_str().unwrap())?;

        // Configure with 2-second batch interval
        let config = BaseConfig {
            storage_path: storage_path.to_str().unwrap().to_string(),
            batch_interval_secs: 2,
            auto_commit: true,
            max_batch_leaves: None,
        };

        // Create test data
        let namespace1 = test_namespace(1);
        let namespace2 = test_namespace(2);
        let base_timestamp = test_now();

        let mut test_records = vec![];
        
        // Add 5 records for namespace1
        for i in 0..5 {
            test_records.push(IncomingRecord {
                namespace: namespace1,
                key: test_key(i),
                value: vec![i, i + 1, i + 2],
                timestamp: base_timestamp,
            });
        }

        // Add 3 records for namespace2
        for i in 0..3 {
            test_records.push(IncomingRecord {
                namespace: namespace2,
                key: test_key(i + 10),
                value: vec![i + 10, i + 11, i + 12],
                timestamp: base_timestamp,
            });
        }

        println!("Created {} test records across 2 namespaces", test_records.len());

        // Create mock components
        let upstream = MockUpstreamConnector::new(test_records.clone(), 100);
        let commitment_registry = MockCommitmentRegistry::new();
        let proof_registry = MockProofRegistry::new();
        let commitment_registry_clone = commitment_registry.clone();

        // Create accumulator factory
        let accumulator_factory: Arc<dyn Fn() -> Box<dyn Accumulator> + Send + Sync> =
            Arc::new(|| Box::new(MockAccumulator::new()));

        // Create App
        let mut app = AppV2::new(
            upstream,
            commitment_registry,
            proof_registry,
            config,
            accumulator_factory,
            storage,
        );

        println!("App created, ingesting test records...");

        // Ingest all records
        for record in test_records.iter() {
            app.handle_incoming_record(record.clone())?;
        }

        println!("All records ingested, waiting for commit phase (2s)...");

        // Wait for the commit phase to trigger
        thread::sleep(Duration::from_secs(3));

        // Perform commit
        if app.should_commit() {
            println!("Commit phase triggered, performing commit...");
            app.perform_commit()?;
        }

        // Verify results
        println!("\n=== Verifying Results ===\n");

        let commitments = commitment_registry_clone.get_commitments();
        println!("Total commitments: {}", commitments.len());

        assert!(
            commitments.len() >= 2,
            "Expected at least 2 commitments (one per namespace), got {}",
            commitments.len()
        );

        // Verify namespace1 commitment
        let ns1_commitment = commitments
            .iter()
            .find(|c| c.commitment.namespace == namespace1);
        assert!(ns1_commitment.is_some(), "Missing commitment for namespace1");
        let ns1_commitment = ns1_commitment.unwrap();
        assert_eq!(ns1_commitment.leaf_count, 5);
        println!("✓ Namespace1: {} leaves committed", ns1_commitment.leaf_count);

        // Verify namespace2 commitment
        let ns2_commitment = commitments
            .iter()
            .find(|c| c.commitment.namespace == namespace2);
        assert!(ns2_commitment.is_some(), "Missing commitment for namespace2");
        let ns2_commitment = ns2_commitment.unwrap();
        assert_eq!(ns2_commitment.leaf_count, 3);
        println!("✓ Namespace2: {} leaves committed", ns2_commitment.leaf_count);

        println!("\n=== E2E Test Passed Successfully! ===\n");

        Ok(())
    }

    #[test]
    fn test_data_flow() -> Result<()> {
        println!("\n=== Testing Data Flow ===\n");

        let temp_dir = tempfile::tempdir()?;
        let storage_path = temp_dir.path().join("test_flow_db");
        let storage = Storage::open(storage_path.to_str().unwrap())?;

        let config = BaseConfig {
            storage_path: storage_path.to_str().unwrap().to_string(),
            batch_interval_secs: 1,
            auto_commit: true,
            max_batch_leaves: None,
        };

        let namespace = test_namespace(42);
        let base_timestamp = test_now();

        let record = IncomingRecord {
            namespace,
            key: test_key(1),
            value: vec![1, 2, 3, 4, 5],
            timestamp: base_timestamp,
        };

        let upstream = MockUpstreamConnector::new(vec![record.clone()], 0);
        let commitment_registry = MockCommitmentRegistry::new();
        let proof_registry = MockProofRegistry::new();
        let commitment_registry_clone = commitment_registry.clone();

        let accumulator_factory: Arc<dyn Fn() -> Box<dyn Accumulator> + Send + Sync> =
            Arc::new(|| Box::new(MockAccumulator::new()));

        let mut app = AppV2::new(
            upstream,
            commitment_registry,
            proof_registry,
            config,
            accumulator_factory,
            storage,
        );

        // Step 1: Ingest record
        println!("Step 1: Ingesting record...");
        app.handle_incoming_record(record.clone())?;

        // Verify it's in storage
        let stored = app.storage.get(&namespace, &record.key, None)?;
        assert!(stored.is_some(), "Record should be in storage");
        println!("✓ Record stored successfully");

        // Step 2: Wait for commit window
        println!("Step 2: Waiting for commit window...");
        thread::sleep(Duration::from_millis(1100));

        // Step 3: Perform commit
        println!("Step 3: Performing commit...");
        app.perform_commit()?;

        // Step 4: Verify commitment was created
        let commitments = commitment_registry_clone.get_commitments();
        assert_eq!(commitments.len(), 1, "Should have 1 commitment");
        assert_eq!(commitments[0].leaf_count, 1, "Should have 1 leaf");
        println!("✓ Commitment created with {} leaves", commitments[0].leaf_count);

        println!("\n=== Data Flow Test Passed! ===\n");

        Ok(())
    }

    #[test]
    fn test_multiple_commit_cycles() -> Result<()> {
        println!("\n=== Testing Multiple Commit Cycles ===\n");

        let temp_dir = tempfile::tempdir()?;
        let storage_path = temp_dir.path().join("test_multi_db");
        let storage = Storage::open(storage_path.to_str().unwrap())?;

        let config = BaseConfig {
            storage_path: storage_path.to_str().unwrap().to_string(),
            batch_interval_secs: 1,
            auto_commit: true,
            max_batch_leaves: None,
        };

        let namespace = test_namespace(99);
        let base_timestamp = test_now();

        let upstream = MockUpstreamConnector::new(vec![], 0);
        let commitment_registry = MockCommitmentRegistry::new();
        let proof_registry = MockProofRegistry::new();
        let commitment_registry_clone = commitment_registry.clone();

        let accumulator_factory: Arc<dyn Fn() -> Box<dyn Accumulator> + Send + Sync> =
            Arc::new(|| Box::new(MockAccumulator::new()));

        let mut app = AppV2::new(
            upstream,
            commitment_registry,
            proof_registry,
            config,
            accumulator_factory,
            storage,
        );

        // Cycle 1
        println!("Cycle 1: Adding 2 records");
        for i in 0..2 {
            app.handle_incoming_record(IncomingRecord {
                namespace,
                key: test_key(i),
                value: vec![i],
                timestamp: base_timestamp,
            })?;
        }
        thread::sleep(Duration::from_millis(1100));
        app.perform_commit()?;

        // Cycle 2
        println!("Cycle 2: Adding 3 records");
        let cycle2_timestamp = test_now();
        for i in 10..13 {
            app.handle_incoming_record(IncomingRecord {
                namespace,
                key: test_key(i),
                value: vec![i],
                timestamp: cycle2_timestamp,
            })?;
        }
        thread::sleep(Duration::from_millis(1100));
        app.perform_commit()?;

        // Verify
        let commitments = commitment_registry_clone.get_commitments();
        assert_eq!(commitments.len(), 2, "Should have 2 commitments (one per cycle)");
        assert_eq!(commitments[0].leaf_count, 2, "Cycle 1 should have 2 leaves");
        assert_eq!(commitments[1].leaf_count, 3, "Cycle 2 should have 3 leaves");

        println!("✓ Cycle 1: {} leaves", commitments[0].leaf_count);
        println!("✓ Cycle 2: {} leaves", commitments[1].leaf_count);

        println!("\n=== Multiple Commit Cycles Test Passed! ===\n");

        Ok(())
    }
}

