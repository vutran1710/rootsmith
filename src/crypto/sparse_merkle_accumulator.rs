use crate::traits::Accumulator;
use crate::types::{Key32, Proof, ProofNode, Value32};
use anyhow::Result;
use monotree::database::MemoryDB;
use monotree::hasher::Blake3;
use monotree::Hasher;
use monotree::{verify_proof, Hash, Monotree};
use std::sync::Mutex;

/// Sparse Merkle tree based accumulator using monotree library.
pub struct SparseMerkleAccumulator {
    tree: Mutex<Monotree<MemoryDB>>,
    root: Mutex<Option<Hash>>,
}

impl SparseMerkleAccumulator {
    pub fn new() -> Self {
        Self {
            tree: Mutex::new(Monotree::default()),
            root: Mutex::new(None),
        }
    }
}

impl Default for SparseMerkleAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Accumulator for SparseMerkleAccumulator {
    fn id(&self) -> &'static str {
        "sparse-merkle"
    }

    fn put(&mut self, key: Key32, value: Value32) -> Result<()> {
        // Convert key and value to Hash type
        let key_hash = Hash::from(key);
        let value_hash = Hash::from(value);

        // Insert into sparse merkle tree and update root
        let mut tree = self.tree.lock().unwrap();
        let mut root = self.root.lock().unwrap();

        let new_root = tree
            .insert(root.as_ref(), &key_hash, &value_hash)
            .map_err(|e| anyhow::anyhow!("Failed to insert into tree: {:?}", e))?;

        *root = new_root;

        Ok(())
    }

    fn build_root(&self) -> Result<Vec<u8>> {
        let root = self.root.lock().unwrap();
        match &*root {
            Some(root_hash) => Ok(root_hash.as_ref().to_vec()),
            None => Ok(vec![0u8; 32]),
        }
    }

    fn prove(&self, key: &Key32) -> Result<Option<Proof>> {
        let key_hash = Hash::from(*key);

        let mut tree = self.tree.lock().unwrap();
        let root = self.root.lock().unwrap();

        if root.is_none() {
            return Ok(None);
        }

        let monotree_proof = tree
            .get_merkle_proof(root.as_ref(), &key_hash)
            .ok()
            .flatten();

        match monotree_proof {
            Some(monotree_proof) => {
                // Convert monotree::Proof to our custom Proof type
                let nodes: Vec<ProofNode> = monotree_proof
                    .into_iter()
                    .map(|(is_left, sibling)| ProofNode {
                        is_left,
                        sibling,
                    })
                    .collect();
                Ok(Some(Proof { nodes }))
            }
            None => Ok(None),
        }
    }

    fn verify_proof(
        &self,
        root: &[u8; 32],
        value: &[u8; 32],
        proof: Option<&Proof>,
    ) -> Result<bool> {
        let hasher = Blake3::new();
        let root_hash = if root.iter().all(|&b| b == 0) {
            None
        } else {
            Some(Hash::from(*root))
        };

        let leaf_hash = Hash::from(*value);

        // Convert our custom Proof to monotree::Proof for verification
        let monotree_proof = proof.map(|p| {
            p.nodes
                .iter()
                .map(|node| (node.is_left, node.sibling.clone()))
                .collect()
        });

        let ok = verify_proof(
            &hasher,
            root_hash.as_ref(),
            &leaf_hash,
            monotree_proof.as_ref(),
        );
        Ok(ok)
    }

    fn verify_inclusion(&self, key: &Key32, value: &[u8]) -> Result<bool> {
        let key_hash = Hash::from(*key);

        // Get the value from the tree
        let mut tree = self.tree.lock().unwrap();
        let root = self.root.lock().unwrap();

        let stored_value = tree
            .get(root.as_ref(), &key_hash)
            .map_err(|e| anyhow::anyhow!("Failed to get from tree: {:?}", e))?;

        match stored_value {
            Some(stored_hash) => {
                // Compare stored value with provided value
                if value.len() == 32 {
                    let value_array: [u8; 32] = value
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("Invalid value length"))?;
                    let value_hash = Hash::from(value_array);
                    Ok(stored_hash == value_hash)
                } else {
                    Ok(false)
                }
            }
            None => Ok(false),
        }
    }

    fn verify_non_inclusion(&self, key: &Key32) -> Result<bool> {
        let key_hash = Hash::from(*key);

        let mut tree = self.tree.lock().unwrap();
        let root = self.root.lock().unwrap();

        let stored_value = tree
            .get(root.as_ref(), &key_hash)
            .map_err(|e| anyhow::anyhow!("Failed to get from tree: {:?}", e))?;
        Ok(stored_value.is_none())
    }

    fn flush(&mut self) -> Result<()> {
        // Create a new tree and reset root
        let mut tree = self.tree.lock().unwrap();
        let mut root = self.root.lock().unwrap();

        *tree = Monotree::default();
        *root = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_accumulator_has_zero_root() {
        let acc = SparseMerkleAccumulator::new();
        let root = acc.build_root().unwrap();
        assert_eq!(root, vec![0u8; 32]);
    }

    #[test]
    fn test_id_returns_sparse_merkle() {
        let acc = SparseMerkleAccumulator::new();
        assert_eq!(acc.id(), "sparse-merkle");
    }

    #[test]
    fn test_single_insertion() {
        let mut acc = SparseMerkleAccumulator::new();
        let key = [1u8; 32];
        let value = [2u8; 32];

        acc.put(key, value).unwrap();
        let root = acc.build_root().unwrap();

        // Root should not be all zeros after insertion
        assert_ne!(root, vec![0u8; 32]);
        assert_eq!(root.len(), 32);
    }

    #[test]
    fn test_multiple_insertions() {
        let mut acc = SparseMerkleAccumulator::new();

        // Insert multiple key-value pairs
        for i in 0..5u8 {
            let key = [i; 32];
            let value = [i + 10; 32];
            acc.put(key, value).unwrap();
        }

        let root = acc.build_root().unwrap();
        assert_ne!(root, vec![0u8; 32]);
        assert_eq!(root.len(), 32);
    }

    #[test]
    fn test_deterministic_root() {
        let mut acc1 = SparseMerkleAccumulator::new();
        let mut acc2 = SparseMerkleAccumulator::new();

        // Insert same data
        let key1 = [1u8; 32];
        let value1 = [2u8; 32];
        let key2 = [3u8; 32];
        let value2 = [4u8; 32];

        acc1.put(key1, value1).unwrap();
        acc1.put(key2, value2).unwrap();

        acc2.put(key1, value1).unwrap();
        acc2.put(key2, value2).unwrap();

        let root1 = acc1.build_root().unwrap();
        let root2 = acc2.build_root().unwrap();

        assert_eq!(root1, root2);
    }

    #[test]
    fn test_order_independent() {
        let mut acc1 = SparseMerkleAccumulator::new();
        let mut acc2 = SparseMerkleAccumulator::new();

        let key1 = [1u8; 32];
        let value1 = [2u8; 32];
        let key2 = [3u8; 32];
        let value2 = [4u8; 32];

        // Insert in different order
        acc1.put(key1, value1).unwrap();
        acc1.put(key2, value2).unwrap();

        acc2.put(key2, value2).unwrap();
        acc2.put(key1, value1).unwrap();

        let root1 = acc1.build_root().unwrap();
        let root2 = acc2.build_root().unwrap();

        // Sparse Merkle trees are order-independent
        assert_eq!(root1, root2);
    }

    #[test]
    fn test_update_existing_key() {
        let mut acc = SparseMerkleAccumulator::new();
        let key = [5u8; 32];
        let value1 = [10u8; 32];
        let value2 = [20u8; 32];

        // Insert initial value
        acc.put(key, value1).unwrap();
        let root1 = acc.build_root().unwrap();

        // Update with new value
        acc.put(key, value2).unwrap();
        let root2 = acc.build_root().unwrap();

        // Roots should be different
        assert_ne!(root1, root2);

        // Old value should not verify
        assert!(!acc.verify_inclusion(&key, &value1).unwrap());
        // New value should verify
        assert!(acc.verify_inclusion(&key, &value2).unwrap());
    }

    #[test]
    fn test_verify_inclusion_positive() {
        let mut acc = SparseMerkleAccumulator::new();
        let key = [5u8; 32];
        let value = [10u8; 32];

        acc.put(key, value).unwrap();

        // Verify the inserted key-value pair is included
        assert!(acc.verify_inclusion(&key, &value).unwrap());
    }

    #[test]
    fn test_verify_inclusion_negative() {
        let mut acc = SparseMerkleAccumulator::new();
        let key1 = [5u8; 32];
        let value1 = [10u8; 32];

        acc.put(key1, value1).unwrap();

        // Try to verify a different key-value pair
        let key2 = [6u8; 32];
        let value2 = [11u8; 32];

        assert!(!acc.verify_inclusion(&key2, &value2).unwrap());
    }

    #[test]
    fn test_verify_inclusion_multiple_items() {
        let mut acc = SparseMerkleAccumulator::new();

        let pairs = vec![
            ([1u8; 32], [10u8; 32]),
            ([2u8; 32], [20u8; 32]),
            ([3u8; 32], [30u8; 32]),
        ];

        for (key, value) in &pairs {
            acc.put(*key, *value).unwrap();
        }

        // All inserted pairs should be verifiable
        for (key, value) in &pairs {
            assert!(acc.verify_inclusion(key, value).unwrap());
        }
    }

    #[test]
    fn test_verify_non_inclusion_empty_tree() {
        let acc = SparseMerkleAccumulator::new();
        let key = [99u8; 32];

        // Key should not be in empty tree
        assert!(acc.verify_non_inclusion(&key).unwrap());
    }

    #[test]
    fn test_verify_non_inclusion_after_insertions() {
        let mut acc = SparseMerkleAccumulator::new();
        let key1 = [1u8; 32];
        let value1 = [10u8; 32];

        acc.put(key1, value1).unwrap();

        // Key that was inserted should NOT pass non-inclusion
        assert!(!acc.verify_non_inclusion(&key1).unwrap());

        // Key that was not inserted should pass non-inclusion
        let key2 = [2u8; 32];
        assert!(acc.verify_non_inclusion(&key2).unwrap());
    }

    #[test]
    fn test_verify_inclusion_wrong_value() {
        let mut acc = SparseMerkleAccumulator::new();
        let key = [5u8; 32];
        let value = [10u8; 32];
        let wrong_value = [11u8; 32];

        acc.put(key, value).unwrap();

        // Verify with correct key but wrong value should fail
        assert!(!acc.verify_inclusion(&key, &wrong_value).unwrap());
    }

    #[test]
    fn test_flush_clears_state() {
        let mut acc = SparseMerkleAccumulator::new();
        let key = [5u8; 32];
        let value = [10u8; 32];

        acc.put(key, value).unwrap();
        let root_before = acc.build_root().unwrap();
        assert_ne!(root_before, vec![0u8; 32]);

        // Flush the accumulator
        acc.flush().unwrap();

        // After flush, root should be zero
        let root_after = acc.build_root().unwrap();
        assert_eq!(root_after, vec![0u8; 32]);

        // Verify inclusion should fail after flush
        assert!(!acc.verify_inclusion(&key, &value).unwrap());

        // Verify non-inclusion should succeed after flush
        assert!(acc.verify_non_inclusion(&key).unwrap());
    }

    #[test]
    fn test_flush_and_reuse() {
        let mut acc = SparseMerkleAccumulator::new();

        // First batch
        let key1 = [1u8; 32];
        let value1 = [10u8; 32];
        acc.put(key1, value1).unwrap();
        let root1 = acc.build_root().unwrap();

        // Flush
        acc.flush().unwrap();

        // Second batch
        let key2 = [2u8; 32];
        let value2 = [20u8; 32];
        acc.put(key2, value2).unwrap();
        let root2 = acc.build_root().unwrap();

        // Roots should be different
        assert_ne!(root1, root2);

        // First key should not be in second batch
        assert!(!acc.verify_inclusion(&key1, &value1).unwrap());
        assert!(acc.verify_non_inclusion(&key1).unwrap());

        // Second key should be in second batch
        assert!(acc.verify_inclusion(&key2, &value2).unwrap());
        assert!(!acc.verify_non_inclusion(&key2).unwrap());
    }

    #[test]
    fn test_default_constructor() {
        let acc = SparseMerkleAccumulator::default();
        let root = acc.build_root().unwrap();
        assert_eq!(root, vec![0u8; 32]);
    }

    #[test]
    fn test_build_root_multiple_times() {
        let mut acc = SparseMerkleAccumulator::new();
        let key = [1u8; 32];
        let value = [2u8; 32];

        acc.put(key, value).unwrap();

        // Build root multiple times - should be consistent
        let root1 = acc.build_root().unwrap();
        let root2 = acc.build_root().unwrap();
        let root3 = acc.build_root().unwrap();

        assert_eq!(root1, root2);
        assert_eq!(root2, root3);
    }

    #[test]
    fn test_large_batch() {
        let mut acc = SparseMerkleAccumulator::new();

        // Insert 100 items
        for i in 0..100u8 {
            let mut key = [0u8; 32];
            key[0] = i;
            let mut value = [0u8; 32];
            value[0] = i.wrapping_mul(2);
            acc.put(key, value).unwrap();
        }

        let root = acc.build_root().unwrap();
        assert_ne!(root, vec![0u8; 32]);

        // Verify all items
        for i in 0..100u8 {
            let mut key = [0u8; 32];
            key[0] = i;
            let mut value = [0u8; 32];
            value[0] = i.wrapping_mul(2);
            assert!(acc.verify_inclusion(&key, &value).unwrap());
        }
    }

    #[test]
    fn test_verify_inclusion_with_invalid_value_length() {
        let mut acc = SparseMerkleAccumulator::new();
        let key = [5u8; 32];
        let value = [10u8; 32];

        acc.put(key, value).unwrap();

        // Try to verify with incorrect length value
        let wrong_length_value = vec![10u8; 16];
        assert!(!acc.verify_inclusion(&key, &wrong_length_value).unwrap());
    }
}
