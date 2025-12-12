use anyhow::Result;
use crate::types::Key32;
use crate::traits::Accumulator;
use super::sparse_merkle::SparseMerkleTree;

/// Sparse Merkle tree based accumulator.
pub struct SparseMerkleAccumulator {
    tree: SparseMerkleTree,
}

impl SparseMerkleAccumulator {
    pub fn new() -> Self {
        Self {
            tree: SparseMerkleTree::new(256),
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

    fn put(&mut self, key: Key32, value: Vec<u8>) -> Result<()> {
        self.tree.update(key.to_vec(), value)
    }

    fn build_root(&self) -> Result<Vec<u8>> {
        self.tree.root()
    }

    fn verify_inclusion(&self, key: &Key32, value: &[u8]) -> Result<bool> {
        Ok(self.tree.get(key).map(|v| v.as_slice() == value).unwrap_or(false))
    }

    fn verify_non_inclusion(&self, key: &Key32) -> Result<bool> {
        Ok(self.tree.get(key).is_none())
    }

    fn flush(&mut self) -> Result<()> {
        self.tree = SparseMerkleTree::new(256);
        Ok(())
    }
}
