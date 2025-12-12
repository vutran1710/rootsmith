use anyhow::Result;
use crossbeam_channel::Sender;
use rootsmith::*;
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
    leaves: std::collections::HashMap<Key32, Vec<u8>>,
}

impl MockAccumulator {
    fn new() -> Self {
        Self {
            leaves: std::collections::HashMap::new(),
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
fn test_e2e_app_run() -> Result<()> {
    println!("\n=== E2E Test: App.run() with commit cycle ===\n");

    // Create temporary storage directory
    let temp_dir = tempfile::tempdir()?;
    let storage_path = temp_dir.path().join("test_e2e_db");
    let storage = Storage::open(storage_path.to_str().unwrap())?;

    // Configure with 1-second batch interval for faster testing
    let config = BaseConfig {
        storage_path: storage_path.to_str().unwrap().to_string(),
        batch_interval_secs: 1,
        auto_commit: true,
    };

    // Create test data
    let namespace1 = test_namespace(1);
    let base_timestamp = test_now();

    let mut test_records = vec![];
    
    // Add 3 records
    for i in 0..3 {
        test_records.push(IncomingRecord {
            namespace: namespace1,
            key: test_key(i),
            value: vec![i, i + 1, i + 2],
            timestamp: base_timestamp,
        });
    }

    println!("Created {} test records", test_records.len());

    // Create mock components
    let upstream = MockUpstreamConnector::new(test_records.clone(), 50);
    let commitment_registry = MockCommitmentRegistry::new();
    let proof_registry = MockProofRegistry::new();
    let commitment_registry_clone = commitment_registry.clone();

    // Create accumulator factory
    let accumulator_factory: Arc<dyn Fn() -> Box<dyn Accumulator> + Send + Sync> =
        Arc::new(|| Box::new(MockAccumulator::new()));

    // Create App
    let app = App::new(
        upstream,
        commitment_registry,
        proof_registry,
        config,
        accumulator_factory,
        storage,
    );

    println!("App created, starting run loop in thread...");

    // Run app in a separate thread with timeout
    let app_handle = thread::spawn(move || app.run());

    // Wait for app to process records and commit
    thread::sleep(Duration::from_secs(2));

    // Wait for app thread to finish
    let result = app_handle.join()
        .map_err(|_| anyhow::anyhow!("App thread panicked"))?;
    
    result?;

    // Verify results
    println!("\n=== Verifying Results ===\n");

    let commitments = commitment_registry_clone.get_commitments();
    println!("Total commitments: {}", commitments.len());

    assert!(
        commitments.len() >= 1,
        "Expected at least 1 commitment, got {}",
        commitments.len()
    );

    // Verify namespace1 commitment
    let ns1_commitment = commitments
        .iter()
        .find(|c| c.commitment.namespace == namespace1);
    assert!(ns1_commitment.is_some(), "Missing commitment for namespace1");
    let ns1_commitment = ns1_commitment.unwrap();
    assert_eq!(ns1_commitment.leaf_count, 3);
    println!("✓ Namespace1: {} leaves committed", ns1_commitment.leaf_count);

    println!("\n=== E2E Test Passed! ===\n");

    Ok(())
}
