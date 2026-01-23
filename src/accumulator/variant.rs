use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;

use super::merkle_accumulator::MerkleAccumulator;
use super::sparse_merkle_accumulator::SparseMerkleAccumulator;
use super::zk_adapter::ZkAccumulatorAdapter;
use super::zk_accumulator::ZkAccumulator;
use crate::config::{AccumulatorType, BaseConfig};
use crate::traits::Accumulator;
use crate::types::CommitmentResult;
use crate::types::RawRecord;

/// Enum representing all possible accumulator implementations.
pub enum AccumulatorVariant {
    Merkle(MerkleAccumulator),
    SparseMerkle(SparseMerkleAccumulator),
    Zk(ZkAccumulatorAdapter),
}

impl AccumulatorVariant {
    /// Create a new accumulator instance based on the specified type.
    pub fn new(accumulator_type: AccumulatorType, config: &BaseConfig) -> Self {
        match accumulator_type {
            AccumulatorType::Merkle => AccumulatorVariant::Merkle(MerkleAccumulator::new()),
            AccumulatorType::SparseMerkle => {
                AccumulatorVariant::SparseMerkle(SparseMerkleAccumulator::new())
            }
            AccumulatorType::Zk => {
                let zk_accumulator = ZkAccumulator::new(
                    &config.zk_service_url,
                    &config.zk_circuit_id,
                );
                AccumulatorVariant::Zk(ZkAccumulatorAdapter::new(zk_accumulator))
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
            AccumulatorVariant::Zk(inner) => inner.id(),
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
            AccumulatorVariant::Zk(inner) => inner.commit(records, result_tx).await,
        }
    }
}
