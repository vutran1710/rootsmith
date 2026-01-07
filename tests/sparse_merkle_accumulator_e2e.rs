use ::rootsmith::*;
use anyhow::Result;

use ::rootsmith::commitment_registry::{CommitmentRegistryVariant, MockCommitmentRegistry};
use ::rootsmith::config::AccumulatorType;
use ::rootsmith::crypto::AccumulatorVariant;
use ::rootsmith::proof_registry::{MockProofRegistry, ProofRegistryVariant};
use ::rootsmith::traits::Accumulator;
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

fn test_value(id: u8) -> [u8; 32] {
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

// ===== E2E Tests for Sparse Merkle Accumulator =====

#[tokio::test]
async fn test_sparse_merkle_e2e_basic_flow() -> Result<()> {
    println!("\n=== E2E Test: Sparse Merkle Accumulator Basic Flow ===\n");

    // Create temporary storage directory
    let temp_dir = tempfile::tempdir()?;
    let storage_path = temp_dir.path().join("test_sparse_merkle_db");
    let storage = Storage::open(storage_path.to_str().unwrap())?;

    // Configure with Sparse Merkle accumulator
    let config = BaseConfig {
        storage_path: storage_path.to_str().unwrap().to_string(),
        batch_interval_secs: 1,
        auto_commit: true,
        accumulator_type: AccumulatorType::SparseMerkle,
    };

    // Create test data
    let namespace1 = test_namespace(1);
    let base_timestamp = test_now();

    let test_records = vec![
        IncomingRecord {
            namespace: namespace1,
            key: test_key(1),
            value: test_value(10),
            timestamp: base_timestamp,
        },
        IncomingRecord {
            namespace: namespace1,
            key: test_key(2),
            value: test_value(20),
            timestamp: base_timestamp,
        },
        IncomingRecord {
            namespace: namespace1,
            key: test_key(3),
            value: test_value(30),
            timestamp: base_timestamp,
        },
    ];

    println!("Created {} test records", test_records.len());

    // Create mock components
    let upstream = UpstreamVariant::Mock(MockUpstream::new(test_records.clone(), 50));
    let commitment_registry = MockCommitmentRegistry::new();
    let commitment_registry_clone = commitment_registry.clone();
    let commitment_registry_variant = CommitmentRegistryVariant::Mock(commitment_registry);
    let proof_registry = ProofRegistryVariant::Mock(MockProofRegistry::new());

    // Create RootSmith with Sparse Merkle accumulator
    let app = RootSmith::new(
        upstream,
        commitment_registry_variant,
        proof_registry,
        config,
        storage,
    );

    println!("RootSmith created with Sparse Merkle accumulator...");

    // Run app
    let app_handle = tokio::spawn(async move { app.run().await });

    // Wait for processing
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

    assert!(!commitments.is_empty(), "Expected at least 1 commitment");

    // Verify namespace commitment
    let ns1_commitments: Vec<_> = commitments
        .iter()
        .filter(|c| c.commitment.namespace == namespace1)
        .collect();
    assert!(
        !ns1_commitments.is_empty(),
        "Missing commitment for namespace1"
    );

    let total_leaves: u64 = ns1_commitments.iter().map(|c| c.leaf_count).sum();
    assert_eq!(total_leaves, 3, "Expected 3 total leaves committed");
    println!(
        "✓ Sparse Merkle: {} leaves committed across {} commit(s)",
        total_leaves,
        ns1_commitments.len()
    );

    // Verify root is not zero
    for commitment in ns1_commitments {
        assert_ne!(
            commitment.commitment.root,
            vec![0u8; 32],
            "Root should not be zero after insertions"
        );
    }

    println!("\n=== E2E Test Passed! ===\n");

    Ok(())
}

#[tokio::test]
async fn test_sparse_merkle_order_independence() -> Result<()> {
    println!("\n=== E2E Test: Sparse Merkle Order Independence ===\n");

    // Test order independence using direct accumulator API
    // This avoids timing issues with the full RootSmith E2E flow

    let mut acc1 = AccumulatorVariant::new(AccumulatorType::SparseMerkle);
    let mut acc2 = AccumulatorVariant::new(AccumulatorType::SparseMerkle);

    // Insert same data in different orders
    println!("Testing order 1: keys 1, 2, 3");
    acc1.put(test_key(1), test_value(10))?;
    acc1.put(test_key(2), test_value(20))?;
    acc1.put(test_key(3), test_value(30))?;

    println!("Testing order 2: keys 3, 1, 2");
    acc2.put(test_key(3), test_value(30))?;
    acc2.put(test_key(1), test_value(10))?;
    acc2.put(test_key(2), test_value(20))?;

    let root1 = acc1.build_root()?;
    let root2 = acc2.build_root()?;

    // Verify that both orders produce the same root (order independence)
    assert_eq!(
        root1, root2,
        "Sparse Merkle trees should produce the same root regardless of insertion order"
    );

    println!("✓ Both insertion orders produced identical roots");
    println!("Root: {:?}", &root1[..8]);

    // Verify inclusion works the same for both
    assert!(acc1.verify_inclusion(&test_key(1), &test_value(10))?);
    assert!(acc2.verify_inclusion(&test_key(1), &test_value(10))?);

    println!("✓ Inclusion verification works identically for both");
    println!("\n=== Order Independence Test Passed! ===\n");

    Ok(())
}

#[tokio::test]
async fn test_sparse_merkle_key_updates() -> Result<()> {
    println!("\n=== E2E Test: Sparse Merkle Key Updates ===\n");

    let temp_dir = tempfile::tempdir()?;
    let storage_path = temp_dir.path().join("test_updates_db");
    let storage = Storage::open(storage_path.to_str().unwrap())?;

    let config = BaseConfig {
        storage_path: storage_path.to_str().unwrap().to_string(),
        batch_interval_secs: 1,
        auto_commit: true,
        accumulator_type: AccumulatorType::SparseMerkle,
    };

    let namespace1 = test_namespace(1);
    let base_timestamp = test_now();

    // First batch: insert initial values
    let test_records_batch1 = vec![
        IncomingRecord {
            namespace: namespace1,
            key: test_key(1),
            value: test_value(10),
            timestamp: base_timestamp,
        },
        IncomingRecord {
            namespace: namespace1,
            key: test_key(2),
            value: test_value(20),
            timestamp: base_timestamp,
        },
    ];

    let upstream = UpstreamVariant::Mock(MockUpstream::new(test_records_batch1.clone(), 50));
    let commitment_registry = MockCommitmentRegistry::new();
    let commitment_registry_clone = commitment_registry.clone();
    let commitment_registry_variant = CommitmentRegistryVariant::Mock(commitment_registry);
    let proof_registry = ProofRegistryVariant::Mock(MockProofRegistry::new());

    let app = RootSmith::new(
        upstream,
        commitment_registry_variant,
        proof_registry,
        config,
        storage,
    );

    let app_handle = tokio::spawn(async move { app.run().await });

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let result = app_handle
        .await
        .map_err(|_| anyhow::anyhow!("RootSmith task panicked"))?;

    result?;

    let commitments_batch1 = commitment_registry_clone.get_commitments();
    let _root1 = &commitments_batch1
        .iter()
        .find(|c| c.commitment.namespace == namespace1)
        .expect("Expected commitment for batch 1")
        .commitment
        .root;

    println!("✓ Batch 1 committed");

    // Now test that updating a key (same key, different value) produces different root
    // This is a unit-level test since RootSmith processes batches independently
    let mut acc1 = AccumulatorVariant::new(AccumulatorType::SparseMerkle);
    let mut acc2 = AccumulatorVariant::new(AccumulatorType::SparseMerkle);

    // Accumulator 1: original values
    acc1.put(test_key(1), test_value(10))?;
    acc1.put(test_key(2), test_value(20))?;

    // Accumulator 2: update key 1's value
    acc2.put(test_key(1), test_value(100))?; // Updated value
    acc2.put(test_key(2), test_value(20))?;

    let root_original = acc1.build_root()?;
    let root_updated = acc2.build_root()?;

    assert_ne!(
        root_original, root_updated,
        "Roots should differ when a key's value is updated"
    );

    // Verify inclusion after update
    assert!(
        acc2.verify_inclusion(&test_key(1), &test_value(100))?,
        "New value should be included"
    );
    assert!(
        !acc2.verify_inclusion(&test_key(1), &test_value(10))?,
        "Old value should not be included after update"
    );

    println!("✓ Key updates produce different roots as expected");
    println!("\n=== Key Updates Test Passed! ===\n");

    Ok(())
}

#[tokio::test]
async fn test_sparse_merkle_non_inclusion_proofs() -> Result<()> {
    println!("\n=== E2E Test: Sparse Merkle Non-Inclusion Proofs ===\n");

    let mut acc = AccumulatorVariant::new(AccumulatorType::SparseMerkle);

    // Insert some keys
    let keys_inserted = vec![test_key(1), test_key(2), test_key(3)];
    let values = vec![test_value(10), test_value(20), test_value(30)];

    for (key, value) in keys_inserted.iter().zip(values.iter()) {
        acc.put(*key, *value)?;
    }

    let root = acc.build_root()?;
    assert_ne!(root, vec![0u8; 32], "Root should not be zero");

    println!("✓ Inserted {} keys", keys_inserted.len());

    // Test non-inclusion for keys that were NOT inserted
    let non_inserted_keys = vec![test_key(10), test_key(20), test_key(30)];

    for key in non_inserted_keys.iter() {
        let is_non_included = acc.verify_non_inclusion(key)?;
        assert!(is_non_included, "Key {:?} should not be included", key);
    }

    println!(
        "✓ Non-inclusion verified for {} keys",
        non_inserted_keys.len()
    );

    // Test that inserted keys CANNOT pass non-inclusion check
    for key in keys_inserted.iter() {
        let is_non_included = acc.verify_non_inclusion(key)?;
        assert!(
            !is_non_included,
            "Inserted key {:?} should not pass non-inclusion check",
            key
        );
    }

    println!("✓ Inserted keys correctly fail non-inclusion checks");
    println!("\n=== Non-Inclusion Proofs Test Passed! ===\n");

    Ok(())
}

#[tokio::test]
async fn test_sparse_merkle_large_sparse_dataset() -> Result<()> {
    println!("\n=== E2E Test: Sparse Merkle Large Sparse Dataset ===\n");

    let temp_dir = tempfile::tempdir()?;
    let storage_path = temp_dir.path().join("test_large_sparse_db");
    let storage = Storage::open(storage_path.to_str().unwrap())?;

    let config = BaseConfig {
        storage_path: storage_path.to_str().unwrap().to_string(),
        batch_interval_secs: 1,
        auto_commit: true,
        accumulator_type: AccumulatorType::SparseMerkle,
    };

    let namespace1 = test_namespace(1);
    let base_timestamp = test_now();

    // Create 100 records with sparse keys (simulating a sparse address space)
    let mut test_records = Vec::new();
    for i in 0..100u8 {
        let mut key = [0u8; 32];
        // Spread keys across the space
        key[0] = i;
        key[31] = i.wrapping_mul(3);

        let mut value = [0u8; 32];
        value[0] = i.wrapping_mul(2);

        test_records.push(IncomingRecord {
            namespace: namespace1,
            key,
            value,
            timestamp: base_timestamp,
        });
    }

    println!("Created {} test records", test_records.len());

    let upstream = UpstreamVariant::Mock(MockUpstream::new(test_records.clone(), 50));
    let commitment_registry = MockCommitmentRegistry::new();
    let commitment_registry_clone = commitment_registry.clone();
    let commitment_registry_variant = CommitmentRegistryVariant::Mock(commitment_registry);
    let proof_registry = ProofRegistryVariant::Mock(MockProofRegistry::new());

    let app = RootSmith::new(
        upstream,
        commitment_registry_variant,
        proof_registry,
        config,
        storage,
    );

    println!("Running RootSmith with large sparse dataset...");

    let app_handle = tokio::spawn(async move { app.run().await });

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let result = app_handle
        .await
        .map_err(|_| anyhow::anyhow!("RootSmith task panicked"))?;

    result?;

    // Verify results
    let commitments = commitment_registry_clone.get_commitments();
    assert!(!commitments.is_empty(), "Expected at least 1 commitment");

    let ns1_commitments: Vec<_> = commitments
        .iter()
        .filter(|c| c.commitment.namespace == namespace1)
        .collect();

    let total_leaves: u64 = ns1_commitments.iter().map(|c| c.leaf_count).sum();
    assert_eq!(total_leaves, 100, "Expected 100 total leaves committed");

    println!(
        "✓ Successfully processed {} leaves with sparse Merkle tree",
        total_leaves
    );
    println!("\n=== Large Sparse Dataset Test Passed! ===\n");

    Ok(())
}

#[tokio::test]
async fn test_sparse_merkle_multiple_namespaces() -> Result<()> {
    println!("\n=== E2E Test: Sparse Merkle Multiple Namespaces ===\n");

    let temp_dir = tempfile::tempdir()?;
    let storage_path = temp_dir.path().join("test_multi_ns_db");
    let storage = Storage::open(storage_path.to_str().unwrap())?;

    let config = BaseConfig {
        storage_path: storage_path.to_str().unwrap().to_string(),
        batch_interval_secs: 1,
        auto_commit: true,
        accumulator_type: AccumulatorType::SparseMerkle,
    };

    let namespace1 = test_namespace(1);
    let namespace2 = test_namespace(2);
    let namespace3 = test_namespace(3);
    let base_timestamp = test_now();

    // Create records for multiple namespaces
    let mut test_records = Vec::new();

    // Namespace 1: 3 records
    for i in 0..3u8 {
        test_records.push(IncomingRecord {
            namespace: namespace1,
            key: test_key(i),
            value: test_value(i + 10),
            timestamp: base_timestamp,
        });
    }

    // Namespace 2: 5 records
    for i in 0..5u8 {
        test_records.push(IncomingRecord {
            namespace: namespace2,
            key: test_key(i),
            value: test_value(i + 20),
            timestamp: base_timestamp,
        });
    }

    // Namespace 3: 2 records
    for i in 0..2u8 {
        test_records.push(IncomingRecord {
            namespace: namespace3,
            key: test_key(i),
            value: test_value(i + 30),
            timestamp: base_timestamp,
        });
    }

    println!(
        "Created {} test records across 3 namespaces",
        test_records.len()
    );

    let upstream = UpstreamVariant::Mock(MockUpstream::new(test_records.clone(), 50));
    let commitment_registry = MockCommitmentRegistry::new();
    let commitment_registry_clone = commitment_registry.clone();
    let commitment_registry_variant = CommitmentRegistryVariant::Mock(commitment_registry);
    let proof_registry = ProofRegistryVariant::Mock(MockProofRegistry::new());

    let app = RootSmith::new(
        upstream,
        commitment_registry_variant,
        proof_registry,
        config,
        storage,
    );

    let app_handle = tokio::spawn(async move { app.run().await });

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let result = app_handle
        .await
        .map_err(|_| anyhow::anyhow!("RootSmith task panicked"))?;

    result?;

    // Verify results
    let commitments = commitment_registry_clone.get_commitments();

    // Check each namespace
    let ns1_commits: Vec<_> = commitments
        .iter()
        .filter(|c| c.commitment.namespace == namespace1)
        .collect();
    let ns2_commits: Vec<_> = commitments
        .iter()
        .filter(|c| c.commitment.namespace == namespace2)
        .collect();
    let ns3_commits: Vec<_> = commitments
        .iter()
        .filter(|c| c.commitment.namespace == namespace3)
        .collect();

    assert!(!ns1_commits.is_empty(), "Expected commits for namespace 1");
    assert!(!ns2_commits.is_empty(), "Expected commits for namespace 2");
    assert!(!ns3_commits.is_empty(), "Expected commits for namespace 3");

    let ns1_leaves: u64 = ns1_commits.iter().map(|c| c.leaf_count).sum();
    let ns2_leaves: u64 = ns2_commits.iter().map(|c| c.leaf_count).sum();
    let ns3_leaves: u64 = ns3_commits.iter().map(|c| c.leaf_count).sum();

    assert_eq!(ns1_leaves, 3, "Expected 3 leaves for namespace 1");
    assert_eq!(ns2_leaves, 5, "Expected 5 leaves for namespace 2");
    assert_eq!(ns3_leaves, 2, "Expected 2 leaves for namespace 3");

    println!("✓ Namespace 1: {} leaves", ns1_leaves);
    println!("✓ Namespace 2: {} leaves", ns2_leaves);
    println!("✓ Namespace 3: {} leaves", ns3_leaves);

    // Verify all roots are different
    let roots: Vec<_> = commitments.iter().map(|c| &c.commitment.root).collect();
    let unique_roots: std::collections::HashSet<_> = roots.iter().collect();
    assert_eq!(
        unique_roots.len(),
        commitments.len(),
        "All namespace roots should be unique"
    );

    println!("\n=== Multiple Namespaces Test Passed! ===\n");

    Ok(())
}

#[tokio::test]
async fn test_sparse_merkle_empty_namespace() -> Result<()> {
    println!("\n=== E2E Test: Sparse Merkle Empty Namespace ===\n");

    let acc = AccumulatorVariant::new(AccumulatorType::SparseMerkle);

    // Don't insert anything - test empty tree
    let root = acc.build_root()?;

    assert_eq!(root, vec![0u8; 32], "Empty tree should have zero root");

    // Non-inclusion should work for any key in empty tree
    let test_keys = vec![test_key(1), test_key(2), test_key(100)];
    for key in test_keys.iter() {
        assert!(
            acc.verify_non_inclusion(key)?,
            "Any key should have non-inclusion proof in empty tree"
        );
    }

    println!("✓ Empty tree has zero root");
    println!("✓ Non-inclusion proofs work for empty tree");
    println!("\n=== Empty Namespace Test Passed! ===\n");

    Ok(())
}
