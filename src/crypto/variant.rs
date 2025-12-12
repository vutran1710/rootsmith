use anyhow::Result;
use crate::types::Key32;
use crate::traits::Accumulator;
use super::{
    merkle_accumulator::MerkleAccumulator,
    sparse_merkle_accumulator::SparseMerkleAccumulator,
    simple_accumulator::SimpleAccumulator,
    mock_accumulator::MockAccumulator,
};

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


