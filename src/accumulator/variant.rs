use anyhow::Result;
use async_trait::async_trait;

use super::merkle_accumulator::MerkleAccumulator;
use super::sparse_merkle_accumulator::SparseMerkleAccumulator;
use crate::config::AccumulatorType;
use crate::traits::accumulator::AccumulatorRecord;
use crate::traits::accumulator::CommitmentResult;
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

#[async_trait]
impl Accumulator for AccumulatorVariant {
    fn id(&self) -> &'static str {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.id(),
            AccumulatorVariant::SparseMerkle(inner) => inner.id(),
        }
    }

    fn commit_batch(&mut self, records: &[AccumulatorRecord]) -> Result<CommitmentResult> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.commit_batch(records),
            AccumulatorVariant::SparseMerkle(inner) => inner.commit_batch(records),
        }
    }

    async fn commit_batch_async(
        &mut self,
        records: &[AccumulatorRecord],
        result_tx: tokio::sync::mpsc::UnboundedSender<CommitmentResult>,
    ) -> Result<()> {
        match self {
            AccumulatorVariant::Merkle(inner) => {
                inner.commit_batch_async(records, result_tx).await
            }
            AccumulatorVariant::SparseMerkle(inner) => {
                inner.commit_batch_async(records, result_tx).await
            }
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

    // ===== Legacy Methods =====

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
}
