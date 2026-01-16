use anyhow::Result;
use rootsmith::accumulator::AccumulatorVariant;
use rootsmith::config::AccumulatorType;
use rootsmith::traits::accumulator::AccumulatorRecord;
use rootsmith::Accumulator;

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

#[test]
fn test_merkle_accumulator_commit_batch() -> Result<()> {
    println!("\n=== Test: Merkle Accumulator Batch Commit ===\n");

    let mut accumulator = AccumulatorVariant::new(AccumulatorType::Merkle);

    // Create test records
    let records: Vec<AccumulatorRecord> = (0..5)
        .map(|i| AccumulatorRecord {
            key: test_key(i),
            value: test_value(i),
        })
        .collect();

    // Commit batch
    let result = accumulator.commit_batch(&records)?;

    println!("Root hash: {:?}", hex::encode(&result.root));
    println!("Commitment timestamp: {}", result.committed_at);
    println!("Number of proofs generated: {}", result.proofs.as_ref().map(|p| p.len()).unwrap_or(0));

    // Verify root is not empty
    assert!(!result.root.is_empty(), "Root should not be empty");
    assert_eq!(result.root.len(), 32, "Root should be 32 bytes");

    // Verify proofs were generated
    assert!(result.proofs.is_some(), "Proofs should be generated");
    let proofs = result.proofs.unwrap();
    assert_eq!(proofs.len(), 5, "Should have 5 proofs");

    // Verify each proof exists
    for i in 0..5 {
        let key = test_key(i);
        let proof = proofs.get(&key);
        assert!(proof.is_some(), "Proof should exist for key {}", i);
        assert!(!proof.unwrap().nodes.is_empty(), "Proof should have nodes");
    }

    println!("✓ Batch commit with proofs completed successfully\n");

    Ok(())
}

#[test]
fn test_sparse_merkle_accumulator_commit_batch() -> Result<()> {
    println!("\n=== Test: Sparse Merkle Accumulator Batch Commit ===\n");

    let mut accumulator = AccumulatorVariant::new(AccumulatorType::SparseMerkle);

    // Create test records
    let records: Vec<AccumulatorRecord> = (0..5)
        .map(|i| AccumulatorRecord {
            key: test_key(i),
            value: test_value(i),
        })
        .collect();

    // Commit batch
    let result = accumulator.commit_batch(&records)?;

    println!("Root hash: {:?}", hex::encode(&result.root));
    println!("Commitment timestamp: {}", result.committed_at);
    println!("Number of proofs generated: {}", result.proofs.as_ref().map(|p| p.len()).unwrap_or(0));

    // Verify root is not empty
    assert!(!result.root.is_empty(), "Root should not be empty");
    assert_eq!(result.root.len(), 32, "Root should be 32 bytes");

    // Verify proofs were generated
    assert!(result.proofs.is_some(), "Proofs should be generated");
    let proofs = result.proofs.unwrap();
    assert_eq!(proofs.len(), 5, "Should have 5 proofs");

    // Verify each proof exists
    for i in 0..5 {
        let key = test_key(i);
        let proof = proofs.get(&key);
        assert!(proof.is_some(), "Proof should exist for key {}", i);
        assert!(!proof.unwrap().nodes.is_empty(), "Proof should have nodes");
    }

    println!("✓ Batch commit with proofs completed successfully\n");

    Ok(())
}

#[tokio::test]
async fn test_merkle_accumulator_commit_batch_async() -> Result<()> {
    println!("\n=== Test: Merkle Accumulator Async Batch Commit ===\n");

    let mut accumulator = AccumulatorVariant::new(AccumulatorType::Merkle);

    // Create test records
    let records: Vec<AccumulatorRecord> = (0..3)
        .map(|i| AccumulatorRecord {
            key: test_key(i),
            value: test_value(i),
        })
        .collect();

    // Create channel for receiving results
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Commit batch asynchronously
    accumulator.commit_batch_async(&records, tx).await?;

    // Receive the result
    let result = rx.recv().await;
    assert!(result.is_some(), "Should receive a result");

    let result = result.unwrap();

    println!("Root hash: {:?}", hex::encode(&result.root));
    println!("Commitment timestamp: {}", result.committed_at);

    // Verify root is not empty
    assert!(!result.root.is_empty(), "Root should not be empty");
    assert_eq!(result.root.len(), 32, "Root should be 32 bytes");

    // Verify proofs were generated
    assert!(result.proofs.is_some(), "Proofs should be generated");
    let proofs = result.proofs.unwrap();
    assert_eq!(proofs.len(), 3, "Should have 3 proofs");

    println!("✓ Async commit completed successfully\n");

    Ok(())
}

#[test]
fn test_empty_batch() -> Result<()> {
    println!("\n=== Test: Empty Batch Commit ===\n");

    let mut accumulator = AccumulatorVariant::new(AccumulatorType::Merkle);

    // Empty records
    let records: Vec<AccumulatorRecord> = vec![];

    // Commit empty batch
    let result = accumulator.commit_batch(&records)?;

    println!("Root hash for empty batch: {:?}", hex::encode(&result.root));

    // Verify root exists (should be default/empty root)
    assert!(!result.root.is_empty(), "Root should not be empty");

    // Verify no proofs were generated
    assert!(result.proofs.is_some(), "Proofs map should exist");
    let proofs = result.proofs.unwrap();
    assert_eq!(proofs.len(), 0, "Should have no proofs");

    println!("✓ Empty batch handled correctly\n");

    Ok(())
}

#[test]
fn test_multiple_batches() -> Result<()> {
    println!("\n=== Test: Multiple Sequential Batches ===\n");

    let mut accumulator = AccumulatorVariant::new(AccumulatorType::Merkle);

    // First batch
    let records1: Vec<AccumulatorRecord> = (0..3)
        .map(|i| AccumulatorRecord {
            key: test_key(i),
            value: test_value(i),
        })
        .collect();

    let result1 = accumulator.commit_batch(&records1)?;
    println!("Batch 1 root: {:?}", hex::encode(&result1.root));

    // Second batch (should flush first batch)
    let records2: Vec<AccumulatorRecord> = (10..13)
        .map(|i| AccumulatorRecord {
            key: test_key(i),
            value: test_value(i),
        })
        .collect();

    let result2 = accumulator.commit_batch(&records2)?;
    println!("Batch 2 root: {:?}", hex::encode(&result2.root));

    // Roots should be different (different data)
    assert_ne!(result1.root, result2.root, "Roots should be different");

    // Verify proofs for second batch
    let proofs2 = result2.proofs.unwrap();
    assert_eq!(proofs2.len(), 3, "Should have 3 proofs in batch 2");

    for i in 10..13 {
        let key = test_key(i);
        let proof = proofs2.get(&key);
        assert!(proof.is_some(), "Proof should exist for key {}", i);
        assert!(!proof.unwrap().nodes.is_empty(), "Proof should have nodes");
    }

    println!("✓ Multiple batches handled correctly\n");

    Ok(())
}
