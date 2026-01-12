use ::rootsmith::*;
use anyhow::Result;
use sha2::{Digest, Sha256};

use ::rootsmith::archive::{ArchiveStorageVariant, MockArchive};
use ::rootsmith::commitment_registry::{CommitmentRegistryVariant, MockCommitmentRegistry};
use ::rootsmith::config::AccumulatorType;
use ::rootsmith::proof_delivery::{MockDelivery, ProofDeliveryVariant};
use ::rootsmith::proof_registry::{MockProofRegistry, ProofRegistryVariant};
use ::rootsmith::types::Value32;
use ::rootsmith::upstream::{MockUpstream, UpstreamVariant};

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

fn test_value(id: u8) -> Value32 {
    let mut value = [0u8; 32];
    value[0] = id;
    value
}

fn test_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time before UNIX_EPOCH")
        .as_secs()
}

// ===== E2E Tests =====

#[tokio::test]
async fn test_rootsmith_e2e_full_cycle() -> Result<()> {
    println!("\n=== E2E Test: RootSmith Full Cycle ===\n");

    // Initialize telemetry like main.rs does
    ::rootsmith::telemetry::init();

    // Create temporary storage directory
    let temp_dir = tempfile::tempdir()?;
    let storage_path = temp_dir.path().join("test_e2e_db");

    // Configure with 1-second batch interval for faster testing
    let config = BaseConfig {
        storage_path: storage_path.to_str().unwrap().to_string(),
        batch_interval_secs: 1,
        auto_commit: true,
        accumulator_type: AccumulatorType::Merkle,
    };

    // Load test data from kv-data.json
    let namespace1 = test_namespace(1);
    let base_timestamp = test_now();

    let json_path = "tests/data/kv-data.json";
    let json_content = std::fs::read_to_string(json_path)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", json_path, e))?;

    // Parse JSON
    let data: serde_json::Value = serde_json::from_str(&json_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e))?;

    let array = data.as_array().ok_or_else(|| anyhow::anyhow!("Expected JSON array"))?;

    let mut test_records = vec![];

    // Convert each JSON entry to IncomingRecord
    for (index, element) in array.iter().enumerate() {
        // Extract key (number) and value (string) from JSON
        let key_num = element["key"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Expected numeric key at index {}", index))?
            as u8;
        let value_str = element["value"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Expected string value at index {}", index))?;

        // Convert key number to [u8; 32] (put number in first byte)
        let mut key = [0u8; 32];
        key[0] = key_num;

        // Convert value string to [u8; 32] by hashing with SHA256
        let mut hasher = Sha256::new();
        hasher.update(value_str.as_bytes());
        let value = hasher.finalize();
        let value_array: [u8; 32] = value.into();

        test_records.push(IncomingRecord {
            namespace: namespace1,
            key,
            value: value_array,
            timestamp: base_timestamp + index as u64,
        });
    }

    println!("Loaded {} test records from kv-data.json", test_records.len());

    // Initialize RootSmith like main.rs does
    let mut app = RootSmith::initialize(config).await?;
    
    // Replace Noop components with Mocks for testing (keeping the main.rs pattern)
    let commitment_registry = MockCommitmentRegistry::new();
    let commitment_registry_clone = commitment_registry.clone();
    app.commitment_registry = CommitmentRegistryVariant::Mock(commitment_registry);
    
    let mock_proof_registry = MockProofRegistry::new();
    let proof_registry_clone = mock_proof_registry.clone();
    app.proof_registry = ProofRegistryVariant::Mock(mock_proof_registry);
    
    let mock_delivery = MockDelivery::new();
    let delivery_clone = mock_delivery.clone();
    app.proof_delivery = ProofDeliveryVariant::Mock(mock_delivery);
    
    let mock_archive = MockArchive::new();
    let archive_clone = mock_archive.clone();
    app.archive_storage = ArchiveStorageVariant::Mock(mock_archive);
    
    app.upstream = UpstreamVariant::Mock(MockUpstream::new(test_records.clone(), 50));

    println!("RootSmith initialized, starting run loop...");

    // Run app like main.rs does
    let app_handle = tokio::spawn(async move { app.run().await });

    // Wait for app to process records and commit
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Wait for app task to finish
    let result = app_handle
        .await
        .map_err(|_| anyhow::anyhow!("RootSmith task panicked"))?;

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
    let ns1_commitments: Vec<_> = commitments
        .iter()
        .filter(|c| c.commitment.namespace == namespace1)
        .collect();
    assert!(
        !ns1_commitments.is_empty(),
        "Missing commitment for namespace1"
    );

    // Sum up all leaves committed for this namespace
    let total_leaves: u64 = ns1_commitments.iter().map(|c| c.leaf_count).sum();
    assert_eq!(
        total_leaves,
        test_records.len() as u64,
        "Expected {} total leaves committed, got {}",
        test_records.len(),
        total_leaves
    );
    println!(
        "✓ Namespace1: {} leaves committed across {} commit(s)",
        total_leaves,
        ns1_commitments.len()
    );

    // Verify proofs were generated
    let proofs = proof_registry_clone.proofs.lock().unwrap();
    println!("✓ Generated {} proofs", proofs.len());
    assert!(
        proofs.len() >= test_records.len(),
        "Expected at least {} proofs, got {}",
        test_records.len(),
        proofs.len()
    );

    // Verify each proof has required fields
    for proof in proofs.iter() {
        assert!(!proof.root.is_empty(), "Proof root should not be empty");
        assert!(!proof.proof.is_empty(), "Proof data should not be empty");
    }
    println!("✓ All proofs have valid root and proof data");

    // Verify proofs were delivered
    let delivered = delivery_clone.get_delivered();
    println!("✓ Delivered {} proofs", delivered.len());
    assert!(
        delivered.len() >= test_records.len(),
        "Expected at least {} delivered proofs, got {}",
        test_records.len(),
        delivered.len()
    );
    println!("✓ All proofs were successfully delivered");

    println!("\n=== E2E Test Passed! ===\n");

    Ok(())
}
