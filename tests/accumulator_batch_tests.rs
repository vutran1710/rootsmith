use anyhow::Result;
use rootsmith::accumulator::AccumulatorVariant;
use rootsmith::config::AccumulatorType;
use rootsmith::types::RawRecord;
use rootsmith::Accumulator;

fn test_key(id: u8) -> [u8; 32] {
    let mut key = [0u8; 32];
    key[0] = id;
    key
}

fn test_value(id: u8) -> Vec<u8> {
    vec![id; 32]
}

#[tokio::test]
async fn test_merkle_accumulator_commit() -> Result<()> {
    println!("\n=== Test: Merkle Accumulator Commit ===\n");

    let mut accumulator = AccumulatorVariant::new(AccumulatorType::Merkle);

    // Create test records
    let records: Vec<RawRecord> = (0..5)
        .map(|i| RawRecord {
            key: test_key(i),
            value: test_value(i),
        })
        .collect();

    // Create channel for receiving results
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Commit batch
    accumulator.commit(&records, tx).await?;

    // Receive the result
    let result = rx.recv().await;
    assert!(result.is_some(), "Should receive a result");

    let (root, proofs) = result.unwrap();

    println!("Root hash: {:?}", hex::encode(&root));
    println!("Number of proofs generated: {}", proofs.as_ref().map(|p| p.len()).unwrap_or(0));

    // Verify root is not empty
    assert!(!root.is_empty(), "Root should not be empty");
    assert_eq!(root.len(), 32, "Root should be 32 bytes");

    // Verify proofs were generated
    assert!(proofs.is_some(), "Proofs should be generated");
    let proofs = proofs.unwrap();
    assert_eq!(proofs.len(), 5, "Should have 5 proofs");

    // Verify each proof exists
    for i in 0..5 {
        let key = test_key(i);
        let proof = proofs.get(&key);
        assert!(proof.is_some(), "Proof should exist for key {}", i);
        assert!(!proof.unwrap().nodes.is_empty(), "Proof should have nodes");
    }

    println!("✓ Commit with proofs completed successfully\n");

    Ok(())
}

#[tokio::test]
async fn test_sparse_merkle_accumulator_commit() -> Result<()> {
    println!("\n=== Test: Sparse Merkle Accumulator Commit ===\n");

    let mut accumulator = AccumulatorVariant::new(AccumulatorType::SparseMerkle);

    // Create test records
    let records: Vec<RawRecord> = (0..5)
        .map(|i| RawRecord {
            key: test_key(i),
            value: test_value(i),
        })
        .collect();

    // Create channel for receiving results
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Commit batch
    accumulator.commit(&records, tx).await?;

    // Receive the result
    let result = rx.recv().await;
    assert!(result.is_some(), "Should receive a result");

    let (root, proofs) = result.unwrap();

    println!("Root hash: {:?}", hex::encode(&root));
    println!("Number of proofs generated: {}", proofs.as_ref().map(|p| p.len()).unwrap_or(0));

    // Verify root is not empty
    assert!(!root.is_empty(), "Root should not be empty");
    assert_eq!(root.len(), 32, "Root should be 32 bytes");

    // Verify proofs were generated
    assert!(proofs.is_some(), "Proofs should be generated");
    let proofs = proofs.unwrap();
    assert_eq!(proofs.len(), 5, "Should have 5 proofs");

    // Verify each proof exists
    for i in 0..5 {
        let key = test_key(i);
        let proof = proofs.get(&key);
        assert!(proof.is_some(), "Proof should exist for key {}", i);
        assert!(!proof.unwrap().nodes.is_empty(), "Proof should have nodes");
    }

    println!("✓ Commit with proofs completed successfully\n");

    Ok(())
}

#[tokio::test]
async fn test_empty_batch() -> Result<()> {
    println!("\n=== Test: Empty Batch Commit ===\n");

    let mut accumulator = AccumulatorVariant::new(AccumulatorType::Merkle);

    // Empty records
    let records: Vec<RawRecord> = vec![];

    // Create channel for receiving results
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Commit empty batch
    accumulator.commit(&records, tx).await?;

    // Receive the result
    let result = rx.recv().await;
    assert!(result.is_some(), "Should receive a result");

    let (root, proofs) = result.unwrap();

    println!("Root hash for empty batch: {:?}", hex::encode(&root));

    // Verify root exists (should be default/empty root)
    assert!(!root.is_empty(), "Root should not be empty");

    // Verify no proofs were generated
    assert!(proofs.is_some(), "Proofs map should exist");
    let proofs = proofs.unwrap();
    assert_eq!(proofs.len(), 0, "Should have no proofs");

    println!("✓ Empty batch handled correctly\n");

    Ok(())
}

#[tokio::test]
async fn test_multiple_batches() -> Result<()> {
    println!("\n=== Test: Multiple Sequential Batches ===\n");

    let mut accumulator = AccumulatorVariant::new(AccumulatorType::Merkle);

    // First batch
    let records1: Vec<RawRecord> = (0..3)
        .map(|i| RawRecord {
            key: test_key(i),
            value: test_value(i),
        })
        .collect();

    let (tx1, mut rx1) = tokio::sync::mpsc::unbounded_channel();
    accumulator.commit(&records1, tx1).await?;
    let (root1, _proofs1) = rx1.recv().await.unwrap();
    println!("Batch 1 root: {:?}", hex::encode(&root1));

    // Second batch (should flush first batch)
    let records2: Vec<RawRecord> = (10..13)
        .map(|i| RawRecord {
            key: test_key(i),
            value: test_value(i),
        })
        .collect();

    let (tx2, mut rx2) = tokio::sync::mpsc::unbounded_channel();
    accumulator.commit(&records2, tx2).await?;
    let (root2, proofs2) = rx2.recv().await.unwrap();
    println!("Batch 2 root: {:?}", hex::encode(&root2));

    // Roots should be different (different data)
    assert_ne!(root1, root2, "Roots should be different");

    // Verify proofs for second batch
    let proofs2 = proofs2.unwrap();
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
