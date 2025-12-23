use crate::traits::Accumulator;
use crate::types::{Key32, Proof, Value32};
use anyhow::{Ok, Result};
use rs_merkle::{algorithms::Sha256, Hasher, MerkleTree as RsMerkleTree};

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

    fn verify_non_inclusion(&self, _key: &Key32) -> Result<bool> {
        // Note: Standard Merkle trees do not support non-inclusion proofs.
        // This method returns false as a safe default, indicating we cannot
        // prove non-inclusion. For proper non-inclusion support, use SparseMerkleAccumulator.
        Ok(false)
    }

    fn flush(&mut self) -> Result<()> {
        self.leaves.clear();
        self.tree = None;
        Ok(())
    }

    fn prove(&self, _key: &Key32) -> Result<Option<Proof>> {
        // Standard Merkle trees do not support efficient proof generation.
        // This implementation returns None to indicate proofs are not available.
        // For proof generation, use SparseMerkleAccumulator instead.
        Ok(None)
    }

    fn verify_proof(
        &self,
        _root: &[u8; 32],
        _value: &[u8; 32],
        _proof: Option<&Proof>,
    ) -> Result<bool> {
        // Standard Merkle trees do not support proof verification.
        // This implementation returns false as a safe default.
        Ok(false)
    }
}
