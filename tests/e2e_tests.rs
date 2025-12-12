use ::rootsmith::*;
use anyhow::Result;

use ::rootsmith::commitment_registry::{CommitmentRegistryVariant, MockCommitmentRegistry};
use ::rootsmith::config::AccumulatorType;
use ::rootsmith::proof_registry::{MockProofRegistry, ProofRegistryVariant};
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

fn test_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time before UNIX_EPOCH")
        .as_secs()
}

// ===== E2E Tests =====

#[tokio::test]
async fn test_e2e_app_run() -> Result<()> {
    println!("\n=== E2E Test: RootSmith.run() with commit cycle ===\n");

    // Create temporary storage directory
    let temp_dir = tempfile::tempdir()?;
    let storage_path = temp_dir.path().join("test_e2e_db");
    let storage = Storage::open(storage_path.to_str().unwrap())?;

    // Configure with 1-second batch interval for faster testing
    let config = BaseConfig {
        storage_path: storage_path.to_str().unwrap().to_string(),
        batch_interval_secs: 1,
        auto_commit: true,
        accumulator_type: AccumulatorType::Merkle,
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
            value: test_key(i + 10), // Use 32-byte hash as value
            timestamp: base_timestamp,
        });
    }

    println!("Created {} test records", test_records.len());

    // Create mock components using the enum variants
    let upstream = UpstreamVariant::Mock(MockUpstream::new(test_records.clone(), 50));
    let commitment_registry = MockCommitmentRegistry::new();
    let commitment_registry_clone = commitment_registry.clone();
    let commitment_registry_variant = CommitmentRegistryVariant::Mock(commitment_registry);
    let proof_registry = ProofRegistryVariant::Mock(MockProofRegistry::new());

    // Create RootSmith
    let app = RootSmith::new(
        upstream,
        commitment_registry_variant,
        proof_registry,
        config,
        storage,
    );

    println!("RootSmith created, starting run loop in async task...");

    // Run app in a separate async task with timeout
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

    // Verify namespace1 commitment - get the last one as there may be multiple commits
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
    assert_eq!(total_leaves, 3, "Expected 3 total leaves committed");
    println!(
        "✓ Namespace1: {} leaves committed across {} commit(s)",
        total_leaves,
        ns1_commitments.len()
    );

    println!("\n=== E2E Test Passed! ===\n");

    Ok(())
}

#[tokio::test]
async fn test_accumulator_type_configuration() -> Result<()> {
    println!("\n=== Test: Different accumulator types configuration ===\n");

    // Test that each accumulator type can be configured and creates the correct variant
    let accumulator_types = vec![AccumulatorType::Merkle, AccumulatorType::SparseMerkle];

    for acc_type in accumulator_types {
        println!("Testing accumulator type: {:?}", acc_type);

        // Create temporary storage directory
        let temp_dir = tempfile::tempdir()?;
        let storage_path = temp_dir.path().join("test_accumulator_type_db");
        let storage = Storage::open(storage_path.to_str().unwrap())?;

        // Configure with specific accumulator type
        let config = BaseConfig {
            storage_path: storage_path.to_str().unwrap().to_string(),
            batch_interval_secs: 1,
            auto_commit: true,
            accumulator_type: acc_type,
        };

        // Create test data
        let namespace1 = test_namespace(1);
        let base_timestamp = test_now();

        let test_records = vec![IncomingRecord {
            namespace: namespace1,
            key: test_key(1),
            value: test_key(100), // Use 32-byte hash as value
            timestamp: base_timestamp,
        }];

        // Create mock components
        let upstream = UpstreamVariant::Mock(MockUpstream::new(test_records.clone(), 50));
        let commitment_registry = MockCommitmentRegistry::new();
        let commitment_registry_clone = commitment_registry.clone();
        let commitment_registry_variant = CommitmentRegistryVariant::Mock(commitment_registry);
        let proof_registry = ProofRegistryVariant::Mock(MockProofRegistry::new());

        // Create RootSmith with specific accumulator type
        let app = RootSmith::new(
            upstream,
            commitment_registry_variant,
            proof_registry,
            config.clone(),
            storage,
        );

        // Verify accumulator type is stored in config
        assert_eq!(app.config.accumulator_type, acc_type);

        // Run app in a separate async task
        let app_handle = tokio::spawn(async move { app.run().await });

        // Wait for processing
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Wait for app task to finish
        let result = app_handle
            .await
            .map_err(|_| anyhow::anyhow!("RootSmith task panicked"))?;

        result?;

        // Verify commitment was created (accumulator worked)
        let commitments = commitment_registry_clone.get_commitments();
        assert!(
            !commitments.is_empty(),
            "Expected commitment for accumulator type {:?}",
            acc_type
        );

        println!("✓ Accumulator type {:?} works correctly", acc_type);
    }

    println!("\n=== All accumulator types test passed! ===\n");

    Ok(())
}
