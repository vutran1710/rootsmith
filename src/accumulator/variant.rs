use anyhow::Result;

use super::merkle_accumulator::MerkleAccumulator;
use super::sparse_merkle_accumulator::SparseMerkleAccumulator;
use crate::config::AccumulatorType;
use crate::traits::Accumulator;
use crate::types::Key32;
use crate::types::Proof;
use crate::types::Value32;

/// Enum representing all possible accumulator implementations.
pub enum AccumulatorVariant {
    Merkle(MerkleAccumulator),
    SparseMerkle(SparseMerkleAccumulator),
}

impl AccumulatorVariant {
    /// Create a new accumulator instance based on the specified type.
    pub fn new(accumulator_type: AccumulatorType) -> Self {
        match accumulator_type {
            AccumulatorType::Merkle => AccumulatorVariant::Merkle(MerkleAccumulator::new()),
            AccumulatorType::SparseMerkle => {
                AccumulatorVariant::SparseMerkle(SparseMerkleAccumulator::new())
            }
        }
    }
}

impl Accumulator for AccumulatorVariant {
    fn id(&self) -> &'static str {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.id(),
            AccumulatorVariant::SparseMerkle(inner) => inner.id(),
        }
    }

    fn put(&mut self, key: Key32, value: Value32) -> Result<()> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.put(key, value),
            AccumulatorVariant::SparseMerkle(inner) => inner.put(key, value),
        }
    }

    fn build_root(&self) -> Result<Vec<u8>> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.build_root(),
            AccumulatorVariant::SparseMerkle(inner) => inner.build_root(),
        }
    }

    fn verify_inclusion(&self, key: &Key32, value: &[u8]) -> Result<bool> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.verify_inclusion(key, value),
            AccumulatorVariant::SparseMerkle(inner) => inner.verify_inclusion(key, value),
        }
    }

    fn verify_non_inclusion(&self, key: &Key32) -> Result<bool> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.verify_non_inclusion(key),
            AccumulatorVariant::SparseMerkle(inner) => inner.verify_non_inclusion(key),
        }
    }

    fn flush(&mut self) -> Result<()> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.flush(),
            AccumulatorVariant::SparseMerkle(inner) => inner.flush(),
        }
    }

    fn prove(&self, key: &Key32) -> Result<Option<Proof>> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.prove(key),
            AccumulatorVariant::SparseMerkle(inner) => inner.prove(key),
        }
    }

    fn verify_proof(
        &self,
        root: &[u8; 32],
        key: &Key32,
        value: &Value32,
        proof: Option<&Proof>,
    ) -> Result<bool> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.verify_proof(root, key, value, proof),
            AccumulatorVariant::SparseMerkle(inner) => inner.verify_proof(root, key, value, proof),
        }
    }
}
