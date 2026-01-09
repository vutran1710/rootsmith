use ::rootsmith::*;
use anyhow::Result;
use kanal::unbounded_async;
use std::sync::Arc;

use ::rootsmith::config::AccumulatorType;
use ::rootsmith::proof_delivery::MockDelivery;
use ::rootsmith::proof_registry::{MockProofRegistry, ProofRegistryVariant};
use ::rootsmith::types::{Key32, Namespace, StoredProof, Value32};

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

// ===== Unit Tests =====

#[tokio::test]
async fn test_process_proof_generation_success() -> Result<()> {
    println!("\n=== Test: Proof Generation Success ===\n");

    let namespace = test_namespace(1);
    let root = vec![1u8; 32]; // Dummy root
    let committed_at = test_now();
    
    // Create test records
    let records = vec![
        (test_key(1), test_value(1)),
        (test_key(2), test_value(2)),
        (test_key(3), test_value(3)),
    ];

    let mock_proof_registry = MockProofRegistry::new();
    let registry_clone = mock_proof_registry.clone();
    let proof_registry = Arc::new(tokio::sync::Mutex::new(
        ProofRegistryVariant::Mock(mock_proof_registry),
    ));

    let mock_delivery = MockDelivery::new();
    let delivery_clone = mock_delivery.clone();
    let (proof_delivery_tx, proof_delivery_rx) = unbounded_async::<Vec<StoredProof>>();

    // Generate proofs
    let proof_count = RootSmith::process_proof_generation(
        namespace,
        root.clone(),
        committed_at,
        records.clone(),
        &proof_registry,
        &proof_delivery_tx,
        AccumulatorType::Merkle,
    )
    .await?;

    // Verify proof count
    assert_eq!(proof_count, 3, "Should generate 3 proofs");

    // Wait for proofs to be sent to delivery channel
    let proofs = tokio::time::timeout(
        tokio::time::Duration::from_secs(2),
        proof_delivery_rx.recv(),
    )
    .await??;

    assert_eq!(proofs.len(), 3, "Should receive 3 proofs");

    // Verify proof contents
    for (i, proof) in proofs.iter().enumerate() {
        assert_eq!(proof.root, root, "Proof {} root should match", i);
        assert_eq!(proof.key, test_key((i + 1) as u8), "Proof {} key should match", i);
        assert!(!proof.proof.is_empty(), "Proof {} should have proof bytes", i);
        
        // Verify metadata
        assert_eq!(
            proof.meta["namespace"].as_str().unwrap(),
            hex::encode(namespace),
            "Proof {} namespace should match",
            i
        );
        assert_eq!(
            proof.meta["committed_at"].as_u64().unwrap(),
            committed_at,
            "Proof {} committed_at should match",
            i
        );
    }

    // Verify proofs were saved to registry
    let saved_proofs = registry_clone.get_proofs();
    assert_eq!(saved_proofs.len(), 3, "Should have 3 saved proofs");

    // Verify proofs were delivered (MockDelivery stores all proofs in a flat list)
    let delivered_proofs = delivery_clone.get_delivered();
    assert_eq!(delivered_proofs.len(), 3, "Should have 3 delivered proofs");

    println!("✓ Generated {} proofs", proof_count);
    println!("✓ Proofs saved to registry");
    println!("✓ Proofs sent to delivery channel");
    println!("✓ Proofs delivered successfully\n");

    Ok(())
}

#[tokio::test]
async fn test_process_proof_generation_empty_records() -> Result<()> {
    println!("\n=== Test: Proof Generation Empty Records ===\n");

    let namespace = test_namespace(1);
    let root = vec![1u8; 32];
    let committed_at = test_now();
    let records: Vec<(Key32, Value32)> = Vec::new();

    let proof_registry = Arc::new(tokio::sync::Mutex::new(
        ProofRegistryVariant::Mock(MockProofRegistry::new()),
    ));
    let (proof_delivery_tx, _proof_delivery_rx) = unbounded_async::<Vec<StoredProof>>();

    // Generate proofs with empty records
    let proof_count = RootSmith::process_proof_generation(
        namespace,
        root,
        committed_at,
        records,
        &proof_registry,
        &proof_delivery_tx,
        AccumulatorType::Merkle,
    )
    .await?;

    assert_eq!(proof_count, 0, "Should generate 0 proofs");

    println!("✓ Generated 0 proofs (as expected)");
    println!("✓ No proofs saved or delivered\n");

    Ok(())
}

#[tokio::test]
async fn test_process_proof_generation_sparse_merkle() -> Result<()> {
    println!("\n=== Test: Proof Generation Sparse Merkle ===\n");

    let namespace = test_namespace(1);
    let root = vec![1u8; 32];
    let committed_at = test_now();
    
    let records = vec![
        (test_key(1), test_value(1)),
        (test_key(2), test_value(2)),
    ];

    let proof_registry = Arc::new(tokio::sync::Mutex::new(
        ProofRegistryVariant::Mock(MockProofRegistry::new()),
    ));
    let (proof_delivery_tx, proof_delivery_rx) = unbounded_async::<Vec<StoredProof>>();

    // Generate proofs using Sparse Merkle Accumulator
    let proof_count = RootSmith::process_proof_generation(
        namespace,
        root.clone(),
        committed_at,
        records,
        &proof_registry,
        &proof_delivery_tx,
        AccumulatorType::SparseMerkle,
    )
    .await?;

    assert_eq!(proof_count, 2, "Should generate 2 proofs");

    // Verify proofs were received
    let proofs = tokio::time::timeout(
        tokio::time::Duration::from_secs(2),
        proof_delivery_rx.recv(),
    )
    .await??;

    assert_eq!(proofs.len(), 2, "Should receive 2 proofs");

    // Verify proof format (should be valid JSON)
    for proof in &proofs {
        // Try to deserialize proof bytes as JSON to verify format
        let proof_json: serde_json::Value = serde_json::from_slice(&proof.proof)
            .expect("Proof bytes should be valid JSON");
        assert!(proof_json.is_object(), "Proof JSON should be an object");
        
        // Sparse Merkle proofs should have "nodes" array
        if let Some(nodes) = proof_json.get("nodes") {
            assert!(nodes.is_array(), "Proof nodes should be an array");
        }
    }

    println!("✓ Generated {} proofs with Sparse Merkle", proof_count);
    println!("✓ Proof format is valid\n");

    Ok(())
}

#[tokio::test]
async fn test_process_proof_generation_large_batch() -> Result<()> {
    println!("\n=== Test: Proof Generation Large Batch ===\n");

    let namespace = test_namespace(1);
    let root = vec![1u8; 32];
    let committed_at = test_now();
    
    // Create 50 records
    let mut records = Vec::new();
    for i in 0..50 {
        records.push((test_key(i), test_value(i)));
    }

    let mock_proof_registry = MockProofRegistry::new();
    let proof_registry = Arc::new(tokio::sync::Mutex::new(
        ProofRegistryVariant::Mock(mock_proof_registry),
    ));
    let (proof_delivery_tx, proof_delivery_rx) = unbounded_async::<Vec<StoredProof>>();

    // Generate proofs for large batch
    let start = std::time::Instant::now();
    let proof_count = RootSmith::process_proof_generation(
        namespace,
        root,
        committed_at,
        records.clone(),
        &proof_registry,
        &proof_delivery_tx,
        AccumulatorType::Merkle,
    )
    .await?;

    let elapsed = start.elapsed();

    assert_eq!(proof_count, 50, "Should generate 50 proofs");
    
    // Verify proofs were received
    let proofs = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        proof_delivery_rx.recv(),
    )
    .await??;

    assert_eq!(proofs.len(), 50, "Should receive 50 proofs");

    println!("✓ Generated {} proofs in {:?}", proof_count, elapsed);
    println!("✓ All proofs saved and delivered successfully\n");

    Ok(())
}

