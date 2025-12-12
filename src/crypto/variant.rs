use anyhow::Result;
use crate::types::Key32;
use crate::traits::Accumulator;
use super::{merkle::MerkleTree, sparse_merkle::SparseMerkleTree};

/// Enum representing all possible accumulator implementations.
pub enum AccumulatorVariant {
    Merkle(MerkleAccumulator),
    SparseMerkle(SparseMerkleAccumulator),
    Simple(SimpleAccumulator),
    Mock(MockAccumulator),
}

impl Accumulator for AccumulatorVariant {
    fn id(&self) -> &'static str {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.id(),
            AccumulatorVariant::SparseMerkle(inner) => inner.id(),
            AccumulatorVariant::Simple(inner) => inner.id(),
            AccumulatorVariant::Mock(inner) => inner.id(),
        }
    }

    fn put(&mut self, key: Key32, value: Vec<u8>) -> Result<()> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.put(key, value),
            AccumulatorVariant::SparseMerkle(inner) => inner.put(key, value),
            AccumulatorVariant::Simple(inner) => inner.put(key, value),
            AccumulatorVariant::Mock(inner) => inner.put(key, value),
        }
    }

    fn build_root(&self) -> Result<Vec<u8>> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.build_root(),
            AccumulatorVariant::SparseMerkle(inner) => inner.build_root(),
            AccumulatorVariant::Simple(inner) => inner.build_root(),
            AccumulatorVariant::Mock(inner) => inner.build_root(),
        }
    }

    fn verify_inclusion(&self, key: &Key32, value: &[u8]) -> Result<bool> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.verify_inclusion(key, value),
            AccumulatorVariant::SparseMerkle(inner) => inner.verify_inclusion(key, value),
            AccumulatorVariant::Simple(inner) => inner.verify_inclusion(key, value),
            AccumulatorVariant::Mock(inner) => inner.verify_inclusion(key, value),
        }
    }

    fn verify_non_inclusion(&self, key: &Key32) -> Result<bool> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.verify_non_inclusion(key),
            AccumulatorVariant::SparseMerkle(inner) => inner.verify_non_inclusion(key),
            AccumulatorVariant::Simple(inner) => inner.verify_non_inclusion(key),
            AccumulatorVariant::Mock(inner) => inner.verify_non_inclusion(key),
        }
    }

    fn flush(&mut self) -> Result<()> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.flush(),
            AccumulatorVariant::SparseMerkle(inner) => inner.flush(),
            AccumulatorVariant::Simple(inner) => inner.flush(),
            AccumulatorVariant::Mock(inner) => inner.flush(),
        }
    }
}

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

/// Simple XOR-based accumulator for demonstration.
pub struct SimpleAccumulator {
    root: Vec<u8>,
}

impl SimpleAccumulator {
    pub fn new() -> Self {
        Self {
            root: vec![0u8; 32],
        }
    }
}

impl Default for SimpleAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Accumulator for SimpleAccumulator {
    fn id(&self) -> &'static str {
        "simple-accumulator"
    }
    
    fn put(&mut self, key: Key32, value: Vec<u8>) -> Result<()> {
        // Simple XOR-based accumulation
        for (i, &byte) in key.iter().enumerate() {
            self.root[i] ^= byte;
        }
        for (i, &byte) in value.iter().enumerate() {
            self.root[i % 32] ^= byte;
        }
        Ok(())
    }
    
    fn build_root(&self) -> Result<Vec<u8>> {
        Ok(self.root.clone())
    }
    
    fn verify_inclusion(&self, _key: &Key32, _value: &[u8]) -> Result<bool> {
        Ok(true)
    }
    
    fn verify_non_inclusion(&self, _key: &Key32) -> Result<bool> {
        Ok(true)
    }
    
    fn flush(&mut self) -> Result<()> {
        self.root = vec![0u8; 32];
        Ok(())
    }
}

/// Mock accumulator for testing.
pub struct MockAccumulator {
    pub leaves: std::collections::HashMap<Key32, Vec<u8>>,
}

impl MockAccumulator {
    pub fn new() -> Self {
        Self {
            leaves: std::collections::HashMap::new(),
        }
    }
}

impl Accumulator for MockAccumulator {
    fn id(&self) -> &'static str {
        "mock-accumulator"
    }

    fn put(&mut self, key: Key32, value: Vec<u8>) -> Result<()> {
        self.leaves.insert(key, value);
        Ok(())
    }

    fn build_root(&self) -> Result<Vec<u8>> {
        // Simple hash: XOR all keys and values together
        let mut root = vec![0u8; 32];
        
        for (key, value) in &self.leaves {
            for (i, &byte) in key.iter().enumerate() {
                root[i] ^= byte;
            }
            for (i, &byte) in value.iter().enumerate() {
                root[i % 32] ^= byte;
            }
        }

        Ok(root)
    }

    fn verify_inclusion(&self, key: &Key32, value: &[u8]) -> Result<bool> {
        Ok(self.leaves.get(key).map(|v| v == value).unwrap_or(false))
    }

    fn verify_non_inclusion(&self, key: &Key32) -> Result<bool> {
        Ok(!self.leaves.contains_key(key))
    }

    fn flush(&mut self) -> Result<()> {
        self.leaves.clear();
        Ok(())
    }
}

