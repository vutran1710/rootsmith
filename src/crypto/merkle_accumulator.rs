use anyhow::Result;
use crate::types::{Key32, Value32};
use crate::traits::Accumulator;
use rs_merkle::{MerkleTree as RsMerkleTree, algorithms::Sha256, Hasher};

/// Merkle tree based accumulator using rs-merkle library.
pub struct MerkleAccumulator {
    leaves: Vec<[u8; 32]>,
    tree: Option<RsMerkleTree<Sha256>>,
}

impl MerkleAccumulator {
    pub fn new() -> Self {
        Self {
            leaves: Vec::new(),
            tree: None,
        }
    }
    
    fn rebuild_tree(&mut self) {
        if self.leaves.is_empty() {
            self.tree = None;
        } else {
            self.tree = Some(RsMerkleTree::<Sha256>::from_leaves(&self.leaves));
        }
    }
}

impl Default for MerkleAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Accumulator for MerkleAccumulator {
    fn id(&self) -> &'static str {
        "merkle"
    }

    fn put(&mut self, key: Key32, value: Value32) -> Result<()> {
        // Combine key and value, then hash
        let mut data = Vec::with_capacity(64);
        data.extend_from_slice(&key);
        data.extend_from_slice(&value);
        let leaf = Sha256::hash(&data);
        self.leaves.push(leaf);
        // Tree will be rebuilt on next build_root() call
        self.tree = None;
        Ok(())
    }

    fn build_root(&self) -> Result<Vec<u8>> {
        if self.leaves.is_empty() {
            return Ok(vec![0u8; 32]);
        }
        
        // Build tree if not already built
        let tree = RsMerkleTree::<Sha256>::from_leaves(&self.leaves);
        
        match tree.root() {
            Some(root) => Ok(root.to_vec()),
            None => Ok(vec![0u8; 32]),
        }
    }

    fn verify_inclusion(&self, key: &Key32, value: &[u8]) -> Result<bool> {
        // Combine key and value to create the leaf we're looking for
        let mut data = Vec::with_capacity(64);
        data.extend_from_slice(key);
        data.extend_from_slice(value);
        let target_leaf = Sha256::hash(&data);
        
        // Check if this leaf exists in our leaves
        Ok(self.leaves.contains(&target_leaf))
    }

    fn verify_non_inclusion(&self, key: &Key32) -> Result<bool> {
        // For a standard Merkle tree, we can't prove non-inclusion easily
        // We check if no leaf starts with this key
        let key_prefix = Sha256::hash(key);
        Ok(!self.leaves.iter().any(|leaf| leaf[..16] == key_prefix[..16]))
    }

    fn flush(&mut self) -> Result<()> {
        self.leaves.clear();
        self.tree = None;
        Ok(())
    }
}
