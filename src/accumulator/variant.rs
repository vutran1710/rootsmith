use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;

use super::merkle_accumulator::MerkleAccumulator;
use super::sparse_merkle_accumulator::SparseMerkleAccumulator;
use super::zk_accumulator::ZkAccumulator;
use super::zk_adapter::ZkAccumulatorAdapter;
use crate::config::AccumulatorType;
use crate::config::BaseConfig;
use crate::traits::Accumulator;
use crate::types::CommitmentResult;
use crate::types::Record;

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
            AccumulatorType::Merkle => AccumulatorVariant::Merkle(MerkleAccumulator::default()),
            AccumulatorType::SparseMerkle => {
                AccumulatorVariant::SparseMerkle(SparseMerkleAccumulator::new())
            }
            AccumulatorType::Zk => {
                let zk_accumulator =
                    ZkAccumulator::new(&config.zk_service_url, &config.zk_circuit_id);
                AccumulatorVariant::Zk(ZkAccumulatorAdapter::new(zk_accumulator))
            }
        }
    }
}

#[async_trait]
impl Accumulator for AccumulatorVariant {
    fn accumulator_type(&self) -> AccumulatorType {
        match self {
            AccumulatorVariant::Merkle(_) => AccumulatorType::Merkle,
            AccumulatorVariant::SparseMerkle(_) => AccumulatorType::SparseMerkle,
            AccumulatorVariant::Zk(_) => AccumulatorType::Zk,
        }
    }

    async fn commit(
        &self,
        records: &[Record],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.commit(records, result_tx).await,
            AccumulatorVariant::SparseMerkle(inner) => inner.commit(records, result_tx).await,
            AccumulatorVariant::Zk(inner) => inner.commit(records, result_tx).await,
        }
    }
}
