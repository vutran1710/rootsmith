use anyhow::{Ok, Result};
use rootsmith::crypto::merkle_accumulator::MerkleAccumulator;
use rootsmith::crypto::sparse_merkle_accumulator::SparseMerkleAccumulator;
use rootsmith::proof_registry::{proof_to_json, InclusionProofJson, to_0x_hex};
use rootsmith::traits::Accumulator;
use rootsmith::types::{Proof, ProofNode};
use serde_json;
use sha2::{Digest, Sha256};
use hex;

// ===== Test Helper Functions =====

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

// ===== Basic Prover/Verifier Tests =====

#[test]
fn test_single_inclusion_proof() -> Result<()> {
    println!("\n=== Test: Single Inclusion Proof ===");

    let mut acc = SparseMerkleAccumulator::new();
    let key = test_key(1);
    let value = test_value(10);

    // Prover: Insert data and build root
    acc.put(key, value)?;
    let root = acc.build_root()?;

    println!("Root: {:?}", &root[..8]);

    // Verifier: Verify the inclusion proof
    let is_included = acc.verify_inclusion(&key, &value)?;
    assert!(is_included, "Inclusion proof should verify successfully");

    println!("✓ Single inclusion proof verified\n");
    Ok(())
}

#[test]
fn test_single_non_inclusion_proof() -> Result<()> {
    println!("\n=== Test: Single Non-Inclusion Proof ===");

    let mut acc = SparseMerkleAccumulator::new();
    let key1 = test_key(1);
    let value1 = test_value(10);
    let key2 = test_key(2); // Not inserted

    // Prover: Insert only key1
    acc.put(key1, value1)?;
    let root = acc.build_root()?;

    println!("Root: {:?}", &root[..8]);

    // Verifier: Verify key2 is NOT included
    let is_not_included = acc.verify_non_inclusion(&key2)?;
    assert!(
        is_not_included,
        "Non-inclusion proof should verify that key2 is not in the tree"
    );

    // Verify that key1 CANNOT pass non-inclusion
    let key1_not_included = acc.verify_non_inclusion(&key1)?;
    assert!(
        !key1_not_included,
        "Inserted key should fail non-inclusion verification"
    );

    println!("✓ Non-inclusion proof verified\n");
    Ok(())
}

#[test]
fn test_multiple_inclusion_proofs() -> Result<()> {
    println!("\n=== Test: Multiple Inclusion Proofs ===");

    let mut acc = SparseMerkleAccumulator::new();

    // Prover: Insert multiple key-value pairs
    let pairs = vec![
        (test_key(1), test_value(10)),
        (test_key(2), test_value(20)),
        (test_key(3), test_value(30)),
        (test_key(4), test_value(40)),
        (test_key(5), test_value(50)),
    ];

    for (key, value) in &pairs {
        acc.put(*key, *value)?;
    }

    let root = acc.build_root()?;
    println!("Root after {} insertions: {:?}", pairs.len(), &root[..8]);

    // Verifier: Verify all inclusion proofs
    for (i, (key, value)) in pairs.iter().enumerate() {
        let is_included = acc.verify_inclusion(key, value)?;
        assert!(
            is_included,
            "Inclusion proof for pair {} should verify",
            i + 1
        );
    }

    println!("✓ All {} inclusion proofs verified\n", pairs.len());
    Ok(())
}

#[test]
fn test_proof_with_wrong_value() -> Result<()> {
    println!("\n=== Test: Proof Verification Fails with Wrong Value ===");

    let mut acc = SparseMerkleAccumulator::new();
    let key = test_key(1);
    let correct_value = test_value(10);
    let wrong_value = test_value(99);

    // Prover: Insert with correct value
    acc.put(key, correct_value)?;
    acc.build_root()?;

    // Verifier: Try to verify with wrong value
    let is_included_wrong = acc.verify_inclusion(&key, &wrong_value)?;
    assert!(
        !is_included_wrong,
        "Verification should fail with wrong value"
    );

    // Verify with correct value works
    let is_included_correct = acc.verify_inclusion(&key, &correct_value)?;
    assert!(
        is_included_correct,
        "Verification should succeed with correct value"
    );

    println!("✓ Proof correctly rejects wrong value\n");
    Ok(())
}

#[test]
fn test_proof_with_non_existent_key() -> Result<()> {
    println!("\n=== Test: Proof Verification Fails for Non-Existent Key ===");

    let mut acc = SparseMerkleAccumulator::new();
    let existing_key = test_key(1);
    let existing_value = test_value(10);
    let non_existent_key = test_key(99);
    let non_existent_value = test_value(99);

    // Prover: Insert only one key
    acc.put(existing_key, existing_value)?;
    acc.build_root()?;

    // Verifier: Try to verify inclusion of non-existent key
    let is_included = acc.verify_inclusion(&non_existent_key, &non_existent_value)?;
    assert!(
        !is_included,
        "Non-existent key should fail inclusion verification"
    );

    println!("✓ Proof correctly rejects non-existent key\n");
    Ok(())
}

// ===== Advanced Prover/Verifier Tests =====

#[test]
fn test_proof_after_key_update() -> Result<()> {
    println!("\n=== Test: Proofs After Key Update ===");

    let mut acc = SparseMerkleAccumulator::new();
    let key = test_key(1);
    let old_value = test_value(10);
    let new_value = test_value(20);

    // Prover: Insert initial value
    acc.put(key, old_value)?;
    let root1 = acc.build_root()?;
    println!("Root after initial insert: {:?}", &root1[..8]);

    // Verify old value is included
    assert!(acc.verify_inclusion(&key, &old_value)?);

    // Prover: Update the key with new value
    acc.put(key, new_value)?;
    let root2 = acc.build_root()?;
    println!("Root after update: {:?}", &root2[..8]);

    assert_ne!(root1, root2, "Root should change after update");

    // Verifier: Old value should no longer verify
    let old_verifies = acc.verify_inclusion(&key, &old_value)?;
    assert!(!old_verifies, "Old value should not verify after update");

    // Verifier: New value should verify
    let new_verifies = acc.verify_inclusion(&key, &new_value)?;
    assert!(new_verifies, "New value should verify after update");

    println!("✓ Proofs correctly updated after key modification\n");
    Ok(())
}

#[test]
fn test_batch_proof_verification() -> Result<()> {
    println!("\n=== Test: Batch Proof Verification ===");

    let mut acc = SparseMerkleAccumulator::new();

    // Prover: Insert a large batch
    let batch_size = 50;
    let mut pairs = Vec::new();

    for i in 0..batch_size {
        let key = test_key(i);
        let value = test_value(i.wrapping_mul(2));
        acc.put(key, value)?;
        pairs.push((key, value));
    }

    let root = acc.build_root()?;
    println!("Root for batch of {}: {:?}", batch_size, &root[..8]);

    // Verifier: Verify all proofs in batch
    let mut verified_count = 0;
    for (key, value) in &pairs {
        if acc.verify_inclusion(key, value)? {
            verified_count += 1;
        }
    }

    assert_eq!(
        verified_count, batch_size as usize,
        "All {} proofs should verify",
        batch_size
    );

    println!(
        "✓ Batch of {} proofs verified successfully\n",
        verified_count
    );
    Ok(())
}

#[test]
fn test_sparse_keyspace_proofs() -> Result<()> {
    println!("\n=== Test: Proofs in Sparse Keyspace ===");

    let mut acc = SparseMerkleAccumulator::new();

    // Prover: Insert keys that are far apart in the keyspace
    let sparse_pairs = vec![
        ([0u8; 32], test_value(1)),   // All zeros
        ([255u8; 32], test_value(2)), // All 0xFF
        (
            {
                let mut k = [0u8; 32];
                k[0] = 1;
                k
            },
            test_value(3),
        ), // 1 at start
        (
            {
                let mut k = [0u8; 32];
                k[31] = 1;
                k
            },
            test_value(4),
        ), // 1 at end
        (
            {
                let mut k = [0u8; 32];
                k[15] = 128;
                k
            },
            test_value(5),
        ), // Middle bit set
    ];

    for (key, value) in &sparse_pairs {
        acc.put(*key, *value)?;
    }

    let root = acc.build_root()?;
    println!("Root for sparse keyspace: {:?}", &root[..8]);

    // Verifier: Verify all sparse proofs
    for (key, value) in &sparse_pairs {
        let is_included = acc.verify_inclusion(key, value)?;
        assert!(is_included, "Sparse key should verify");
    }

    // Verify non-inclusion for keys in between
    let empty_key = test_key(10);
    assert!(
        acc.verify_non_inclusion(&empty_key)?,
        "Empty positions should prove non-inclusion"
    );

    println!("✓ Sparse keyspace proofs verified\n");
    Ok(())
}

#[test]
fn test_empty_tree_proofs() -> Result<()> {
    println!("\n=== Test: Proofs on Empty Tree ===");

    let acc = SparseMerkleAccumulator::new();
    let root = acc.build_root()?;

    // Empty tree should have zero root
    assert_eq!(root, vec![0u8; 32], "Empty tree should have zero root");

    // Any key should prove non-inclusion
    for i in 0..10u8 {
        let key = test_key(i);
        let is_not_included = acc.verify_non_inclusion(&key)?;
        assert!(
            is_not_included,
            "All keys should prove non-inclusion in empty tree"
        );
    }

    println!("✓ Empty tree proofs verified\n");
    Ok(())
}

#[test]
fn test_proof_determinism() -> Result<()> {
    println!("\n=== Test: Proof Determinism ===");

    // Create two accumulators with same data
    let mut acc1 = SparseMerkleAccumulator::new();
    let mut acc2 = SparseMerkleAccumulator::new();

    let pairs = vec![
        (test_key(1), test_value(10)),
        (test_key(2), test_value(20)),
        (test_key(3), test_value(30)),
    ];

    // Insert in same order
    for (key, value) in &pairs {
        acc1.put(*key, *value)?;
        acc2.put(*key, *value)?;
    }

    let root1 = acc1.build_root()?;
    let root2 = acc2.build_root()?;

    assert_eq!(root1, root2, "Roots should be identical");

    // Verify same proofs on both trees
    for (key, value) in &pairs {
        let verified1 = acc1.verify_inclusion(key, value)?;
        let verified2 = acc2.verify_inclusion(key, value)?;
        assert_eq!(
            verified1, verified2,
            "Proof verification should be deterministic"
        );
    }

    println!("✓ Proofs are deterministic\n");
    Ok(())
}

#[test]
fn test_proof_order_independence() -> Result<()> {
    println!("\n=== Test: Proof Order Independence ===");

    let mut acc1 = SparseMerkleAccumulator::new();
    let mut acc2 = SparseMerkleAccumulator::new();

    let key1 = test_key(1);
    let value1 = test_value(10);
    let key2 = test_key(2);
    let value2 = test_value(20);
    let key3 = test_key(3);
    let value3 = test_value(30);

    // Insert in order: 1, 2, 3
    acc1.put(key1, value1)?;
    acc1.put(key2, value2)?;
    acc1.put(key3, value3)?;

    // Insert in order: 3, 1, 2
    acc2.put(key3, value3)?;
    acc2.put(key1, value1)?;
    acc2.put(key2, value2)?;

    let root1 = acc1.build_root()?;
    let root2 = acc2.build_root()?;

    assert_eq!(
        root1, root2,
        "Sparse Merkle roots should be order-independent"
    );

    // Verify all proofs work the same on both trees
    assert!(acc1.verify_inclusion(&key1, &value1)?);
    assert!(acc2.verify_inclusion(&key1, &value1)?);
    assert!(acc1.verify_inclusion(&key2, &value2)?);
    assert!(acc2.verify_inclusion(&key2, &value2)?);

    println!("✓ Proofs are order-independent\n");
    Ok(())
}

// ===== Edge Case Tests =====

#[test]
fn test_proof_with_identical_keys() -> Result<()> {
    println!("\n=== Test: Proof with Identical Keys (Update Scenario) ===");

    let mut acc = SparseMerkleAccumulator::new();
    let key = test_key(1);
    let value1 = test_value(10);
    let value2 = test_value(20);

    // Insert same key twice with different values
    acc.put(key, value1)?;
    acc.put(key, value2)?; // Update

    // Only the latest value should verify
    assert!(
        acc.verify_inclusion(&key, &value2)?,
        "Latest value should verify"
    );
    assert!(
        !acc.verify_inclusion(&key, &value1)?,
        "Old value should not verify after update"
    );

    println!("✓ Key update proof behavior verified\n");
    Ok(())
}

#[test]
fn test_proof_after_flush() -> Result<()> {
    println!("\n=== Test: Proofs After Flush ===");

    let mut acc = SparseMerkleAccumulator::new();
    let key = test_key(1);
    let value = test_value(10);

    // Insert and verify
    acc.put(key, value)?;
    let _root_before = acc.build_root()?;
    assert!(acc.verify_inclusion(&key, &value)?);

    // Flush the tree
    acc.flush()?;
    let root_after = acc.build_root()?;

    assert_eq!(root_after, vec![0u8; 32], "Root should be zero after flush");

    // Old proofs should no longer verify
    assert!(
        !acc.verify_inclusion(&key, &value)?,
        "Inclusion should fail after flush"
    );

    // Non-inclusion should work
    assert!(
        acc.verify_non_inclusion(&key)?,
        "Non-inclusion should succeed after flush"
    );

    println!("✓ Proof behavior after flush verified\n");
    Ok(())
}

#[test]
fn test_proof_with_all_zero_key() -> Result<()> {
    println!("\n=== Test: Proof with All-Zero Key ===");

    let mut acc = SparseMerkleAccumulator::new();
    let zero_key = [0u8; 32];
    let value = test_value(10);

    acc.put(zero_key, value)?;
    acc.build_root()?;

    assert!(
        acc.verify_inclusion(&zero_key, &value)?,
        "All-zero key should verify"
    );

    println!("✓ All-zero key proof verified\n");
    Ok(())
}

#[test]
fn test_proof_with_all_ff_key() -> Result<()> {
    println!("\n=== Test: Proof with All-0xFF Key ===");

    let mut acc = SparseMerkleAccumulator::new();
    let ff_key = [0xFFu8; 32];
    let value = test_value(10);

    acc.put(ff_key, value)?;
    acc.build_root()?;

    assert!(
        acc.verify_inclusion(&ff_key, &value)?,
        "All-0xFF key should verify"
    );

    println!("✓ All-0xFF key proof verified\n");
    Ok(())
}

#[test]
fn test_mixed_inclusion_non_inclusion_proofs() -> Result<()> {
    println!("\n=== Test: Mixed Inclusion and Non-Inclusion Proofs ===");

    let mut acc = SparseMerkleAccumulator::new();

    // Insert only even keys
    let inserted_keys: Vec<_> = (0..10u8).step_by(2).map(test_key).collect();
    let inserted_values: Vec<_> = (0..10u8).step_by(2).map(|i| test_value(i * 10)).collect();

    for (key, value) in inserted_keys.iter().zip(inserted_values.iter()) {
        acc.put(*key, *value)?;
    }

    acc.build_root()?;

    // Verify inclusion for even keys
    for (key, value) in inserted_keys.iter().zip(inserted_values.iter()) {
        assert!(
            acc.verify_inclusion(key, value)?,
            "Even keys should prove inclusion"
        );
        assert!(
            !acc.verify_non_inclusion(key)?,
            "Even keys should not prove non-inclusion"
        );
    }

    // Verify non-inclusion for odd keys
    for i in (1..10u8).step_by(2) {
        let odd_key = test_key(i);
        assert!(
            acc.verify_non_inclusion(&odd_key)?,
            "Odd keys should prove non-inclusion"
        );
        assert!(
            !acc.verify_inclusion(&odd_key, &test_value(i * 10))?,
            "Odd keys should not prove inclusion"
        );
    }

    println!("✓ Mixed inclusion/non-inclusion proofs verified\n");
    Ok(())
}

#[test]
fn test_large_scale_proof_verification() -> Result<()> {
    println!("\n=== Test: Large Scale Proof Verification ===");

    let mut acc = SparseMerkleAccumulator::new();
    let num_entries = 1000;

    println!("Inserting {} entries...", num_entries);

    // Insert large number of entries
    for i in 0..num_entries {
        let mut key = [0u8; 32];
        key[0] = (i % 256) as u8;
        key[1] = ((i / 256) % 256) as u8;
        key[2] = ((i / 65536) % 256) as u8;

        let mut value = [0u8; 32];
        value[0] = ((i * 2) % 256) as u8;

        acc.put(key, value)?;
    }

    let root = acc.build_root()?;
    println!("Root for {} entries: {:?}", num_entries, &root[..8]);

    // Sample verification - verify every 100th entry
    let mut verified_count = 0;
    for i in (0..num_entries).step_by(100) {
        let mut key = [0u8; 32];
        key[0] = (i % 256) as u8;
        key[1] = ((i / 256) % 256) as u8;
        key[2] = ((i / 65536) % 256) as u8;

        let mut value = [0u8; 32];
        value[0] = ((i * 2) % 256) as u8;

        if acc.verify_inclusion(&key, &value)? {
            verified_count += 1;
        }
    }

    let expected_count = (num_entries + 99) / 100;
    assert_eq!(
        verified_count, expected_count,
        "All sampled proofs should verify"
    );

    println!(
        "✓ Verified {} sampled proofs from {} total entries\n",
        verified_count, num_entries
    );
    Ok(())
}

#[test]
fn test_proof_consistency_across_builds() -> Result<()> {
    println!("\n=== Test: Proof Consistency Across Multiple Root Builds ===");

    let mut acc = SparseMerkleAccumulator::new();
    let key = test_key(1);
    let value = test_value(10);

    acc.put(key, value)?;

    // Build root multiple times
    let root1 = acc.build_root()?;
    let root2 = acc.build_root()?;
    let root3 = acc.build_root()?;

    // All roots should be identical
    assert_eq!(root1, root2);
    assert_eq!(root2, root3);

    // Verification should work consistently
    assert!(acc.verify_inclusion(&key, &value)?);
    assert!(acc.verify_inclusion(&key, &value)?);
    assert!(acc.verify_inclusion(&key, &value)?);

    println!("✓ Proofs remain consistent across multiple root builds\n");
    Ok(())
}

#[test]
fn test_accumulator_create_proof() -> Result<()> {
    println!("\n=== Test: Create Proof with JSON Data ===");

    // Read and load kv-data.json
    let json_path = "tests/data/kv-data.json";
    let json_content =
        std::fs::read_to_string(json_path).expect(&format!("Failed to read {}", json_path));

    // Parse JSON
    let data: serde_json::Value =
        serde_json::from_str(&json_content).expect("Failed to parse JSON");

    // Print the data
    let array = data.as_array().expect("Expected JSON array");
    println!(
        "\n📦 Loaded {} records from kv-data.json\n",
        array.len()
    );

    // Initialize accumulator
    let mut acc = SparseMerkleAccumulator::new();

    // Insert each element into the accumulator
    for (index, element) in array.iter().enumerate() {
        // Extract key (number) and value (string) from JSON
        let key_num = element["key"]
            .as_u64()
            .expect(&format!("Expected numeric key at index {}", index))
            as u8;
        let value_str = element["value"]
            .as_str()
            .expect(&format!("Expected string value at index {}", index));

        // Convert key number to [u8; 32] (put number in first byte)
        let mut key = [0u8; 32];
        key[0] = key_num;

        // Convert value string to [u8; 32] by hashing with SHA256
        let mut hasher = Sha256::new();
        hasher.update(value_str.as_bytes());
        let value = hasher.finalize();
        let value_array: [u8; 32] = value.into();

        // Insert into accumulator
        acc.put(key, value_array)?;

        println!(
            "Inserted record {}: key={}, value={}",
            index + 1,
            key_num,
            value_str
        );
    }
    let root = acc.build_root()?;
    println!("Root: {:?}", root);

    // Prove the first key
    let first_element = array.first().expect("Array should not be empty");
    let first_key_num = first_element["key"].as_u64().expect("Expected numeric key") as u8;

    let mut first_key = [0u8; 32];
    first_key[0] = first_key_num;

    // Get the value hash for the first element
    let first_value_str = first_element["value"]
        .as_str()
        .expect("Expected string value");
    let mut hasher = Sha256::new();
    hasher.update(first_value_str.as_bytes());
    let value_hash = hasher.finalize();
    let value_hash_array: [u8; 32] = value_hash.into();

    let proof = acc.prove(&first_key)?;

    match proof {
        Some(proof) => {
            println!(
                "\n✓ Proof generated for key {} (first record)",
                first_key_num
            );
            println!("Proof contains {} nodes", proof.nodes.len());
            for (i, node) in proof.nodes.iter().enumerate() {
                // hash is Vec<u8> with variable length, safely slice first 8 bytes for display
                let hash_preview = if node.sibling.len() >= 8 {
                    &node.sibling[..8]
                } else {
                    &node.sibling[..]
                };
                println!(
                    "  Node {}: direction={}, hash={:?} (length={})",
                    i + 1,
                    if node.is_left { "left" } else { "right" },
                    hash_preview,
                    node.sibling.len()
                );
            }

            // Create payload with proof
            let proof_json = proof_to_json(proof);
            let payload = InclusionProofJson {
                root: to_0x_hex(&root),
                key: to_0x_hex(&first_key),
                value_hash: to_0x_hex(&value_hash_array),
                proof: proof_json,
            };

            // Print payload as JSON
            println!("\n📄 Inclusion Payload (JSON):");
            println!("{}", serde_json::to_string_pretty(&payload)?);

            // Verify the proof with JSON payload
            // Parse hex strings back to bytes
            let root_hex = payload.root.strip_prefix("0x").unwrap_or(&payload.root);
            let root_bytes: [u8; 32] = hex::decode(root_hex)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("Root must be 32 bytes"))?;

            let value_hash_hex = payload.value_hash.strip_prefix("0x").unwrap_or(&payload.value_hash);
            let value_hash_bytes: [u8; 32] = hex::decode(value_hash_hex)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("Value hash must be 32 bytes"))?;

            // Reconstruct custom Proof from JSON
            let proof_nodes: Vec<ProofNode> = payload
                .proof
                .into_iter()
                .map(|node| {
                    let is_left = node.direction == "left";
                    let sibling_hex = node.sibling.strip_prefix("0x").unwrap_or(&node.sibling);
                    let sibling_bytes = hex::decode(sibling_hex)
                        .map_err(|e| anyhow::anyhow!("Failed to decode sibling hex: {}", e))?;
                    Ok(ProofNode {
                        is_left,
                        sibling: sibling_bytes,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let reconstructed_proof = Proof {
                nodes: proof_nodes,
            };

            // Verify the proof
            let is_valid = acc.verify_proof(
                &root_bytes,
                &value_hash_bytes,
                Some(&reconstructed_proof),
            )?;

            if is_valid {
                println!("\n✅ Proof verification SUCCESSFUL!");
                println!("   Root: {}", payload.root);
                println!("   Key: {}", payload.key);
                println!("   Value Hash: {}", payload.value_hash);
            } else {
                println!("\n❌ Proof verification FAILED!");
            } 
        }
        None => {
            println!(
                "\n✗ No proof available for key {} (key may not exist)",
                first_key_num
            );
        }
    }

    Ok(())
}

#[test]
fn test_merkle_accumulator_create_proof() -> Result<()> {
    println!("\n=== Test: Merkle Accumulator Create Proof with JSON Data ===");

    // Read and load kv-data.json
    let json_path = "tests/data/kv-data.json";
    let json_content =
        std::fs::read_to_string(json_path).expect(&format!("Failed to read {}", json_path));

    // Parse JSON
    let data: serde_json::Value =
        serde_json::from_str(&json_content).expect("Failed to parse JSON");

    // Print the data
    let array = data.as_array().expect("Expected JSON array");
    println!(
        "\n📦 Loaded {} records from kv-data.json\n",
        array.len()
    );

    // Initialize Merkle accumulator
    let mut acc = MerkleAccumulator::new();

    // Insert each element into the accumulator
    for (index, element) in array.iter().enumerate() {
        // Extract key (number) and value (string) from JSON
        let key_num = element["key"]
            .as_u64()
            .expect(&format!("Expected numeric key at index {}", index))
            as u8;
        let value_str = element["value"]
            .as_str()
            .expect(&format!("Expected string value at index {}", index));

        // Convert key number to [u8; 32] (put number in first byte)
        let mut key = [0u8; 32];
        key[0] = key_num;

        // Convert value string to [u8; 32] by hashing with SHA256
        let mut hasher = Sha256::new();
        hasher.update(value_str.as_bytes());
        let value = hasher.finalize();
        let value_array: [u8; 32] = value.into();

        // Insert into accumulator
        acc.put(key, value_array)?;

        println!(
            "Inserted record {}: key={}, value={}",
            index + 1,
            key_num,
            value_str
        );
    }
    let root = acc.build_root()?;
    println!("Root: {:?}", root);

    // Prove the first key
    let first_element = array.first().expect("Array should not be empty");
    let first_key_num = first_element["key"].as_u64().expect("Expected numeric key") as u8;

    let mut first_key = [0u8; 32];
    first_key[0] = first_key_num;

    // Get the value hash for the first element
    let first_value_str = first_element["value"]
        .as_str()
        .expect("Expected string value");
    let mut hasher = Sha256::new();
    hasher.update(first_value_str.as_bytes());
    let value_hash = hasher.finalize();
    let value_hash_array: [u8; 32] = value_hash.into();

    let proof = acc.prove(&first_key)?;

    match proof {
        Some(proof) => {
            println!(
                "\n✓ Proof generated for key {} (first record)",
                first_key_num
            );
            println!("Proof contains {} nodes", proof.nodes.len());
            for (i, node) in proof.nodes.iter().enumerate() {
                // hash is Vec<u8> with variable length, safely slice first 8 bytes for display
                let hash_preview = if node.sibling.len() >= 8 {
                    &node.sibling[..8]
                } else {
                    &node.sibling[..]
                };
                println!(
                    "  Node {}: direction={}, hash={:?} (length={})",
                    i + 1,
                    if node.is_left { "left" } else { "right" },
                    hash_preview,
                    node.sibling.len()
                );
            }

            // Create payload with proof
            let proof_json = proof_to_json(proof);
            let payload = InclusionProofJson {
                root: to_0x_hex(&root),
                key: to_0x_hex(&first_key),
                value_hash: to_0x_hex(&value_hash_array),
                proof: proof_json,
            };

            // Print payload as JSON
            println!("\n📄 Inclusion Payload (JSON):");
            println!("{}", serde_json::to_string_pretty(&payload)?);

            // Verify the proof with JSON payload
            // Parse hex strings back to bytes
            let root_hex = payload.root.strip_prefix("0x").unwrap_or(&payload.root);
            let root_bytes: [u8; 32] = hex::decode(root_hex)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("Root must be 32 bytes"))?;

            let value_hash_hex = payload.value_hash.strip_prefix("0x").unwrap_or(&payload.value_hash);
            let value_hash_bytes: [u8; 32] = hex::decode(value_hash_hex)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("Value hash must be 32 bytes"))?;

            // Reconstruct custom Proof from JSON
            let proof_nodes: Vec<ProofNode> = payload
                .proof
                .into_iter()
                .map(|node| {
                    let is_left = node.direction == "left";
                    let sibling_hex = node.sibling.strip_prefix("0x").unwrap_or(&node.sibling);
                    let sibling_bytes = hex::decode(sibling_hex)
                        .map_err(|e| anyhow::anyhow!("Failed to decode sibling hex: {}", e))?;
                    Ok(ProofNode {
                        is_left,
                        sibling: sibling_bytes,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let reconstructed_proof = Proof {
                nodes: proof_nodes,
            };

            // Verify the proof
            let is_valid = acc.verify_proof(
                &root_bytes,
                &value_hash_bytes,
                Some(&reconstructed_proof),
            )?;

            if is_valid {
                println!("\n✅ Proof verification SUCCESSFUL!");
                println!("   Root: {}", payload.root);
                println!("   Key: {}", payload.key);
                println!("   Value Hash: {}", payload.value_hash);
            } else {
                println!("\n❌ Proof verification FAILED!");
            } 
        }
        None => {
            println!(
                "\n✗ No proof available for key {} (key may not exist)",
                first_key_num
            );
        }
    }

    Ok(())
}