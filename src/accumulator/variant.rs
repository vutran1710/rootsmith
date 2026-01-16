use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;

use super::merkle_accumulator::MerkleAccumulator;
use super::sparse_merkle_accumulator::SparseMerkleAccumulator;
use crate::config::AccumulatorType;
use crate::traits::Accumulator;
use crate::types::CommitmentResult;
use crate::types::Key32;
use crate::types::Proof;
use crate::types::RawRecord;

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

#[async_trait]
impl Accumulator for AccumulatorVariant {
    fn id(&self) -> &'static str {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.id(),
            AccumulatorVariant::SparseMerkle(inner) => inner.id(),
        }
    }

    async fn commit(
        &mut self,
        records: &[RawRecord],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.commit(records, result_tx).await,
            AccumulatorVariant::SparseMerkle(inner) => inner.commit(records, result_tx).await,
        }
    }

    fn verify_proof(
        &self,
        root: &[u8; 32],
        key: &Key32,
        value: &[u8],
        proof: Option<&Proof>,
    ) -> Result<bool> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.verify_proof(root, key, value, proof),
            AccumulatorVariant::SparseMerkle(inner) => inner.verify_proof(root, key, value, proof),
        }
    }
}
