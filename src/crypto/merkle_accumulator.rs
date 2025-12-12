use anyhow::Result;
use crate::types::Key32;
use crate::traits::Accumulator;
use super::merkle::MerkleTree;

/// Merkle tree based accumulator.
pub struct MerkleAccumulator {
    tree: MerkleTree,
}

impl MerkleAccumulator {
    pub fn new() -> Self {
        Self {
            tree: MerkleTree::new(),
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

    fn put(&mut self, key: Key32, value: Vec<u8>) -> Result<()> {
        let mut data = key.to_vec();
        data.extend_from_slice(&value);
        self.tree.add_leaf(data);
        Ok(())
    }

    fn build_root(&self) -> Result<Vec<u8>> {
        self.tree.root()
    }

    fn verify_inclusion(&self, _key: &Key32, _value: &[u8]) -> Result<bool> {
        // Simplified - real implementation would use proofs
        Ok(true)
    }

    fn verify_non_inclusion(&self, _key: &Key32) -> Result<bool> {
        // Simplified - real implementation would use proofs
        Ok(true)
    }

    fn flush(&mut self) -> Result<()> {
        self.tree = MerkleTree::new();
        Ok(())
    }
}
