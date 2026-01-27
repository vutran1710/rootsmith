pub mod external_service;
pub mod merkle_accumulator;
pub mod sparse_merkle_accumulator;

use anyhow::Result;
use async_trait::async_trait;
use external_service::ExternalServiceAccumulator;
use external_service::ExternalServiceConfig;
use kanal::AsyncSender;
use merkle_accumulator::MerkleAccumulator;
use serde::Deserialize;
use serde::Serialize;
use sparse_merkle_accumulator::SparseMerkleAccumulator;

use crate::config::AccumulatorType;
use crate::traits::Accumulator;
use crate::types::CommitmentResult;
use crate::types::Record;

/// Enum representing all possible accumulator implementations.
pub enum AccumulatorVariant {
    Merkle(MerkleAccumulator),
    SparseMerkle(SparseMerkleAccumulator),
    External(ExternalServiceAccumulator),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AccumulatorConfig {
    pub external: Option<ExternalServiceConfig>,
}

impl AccumulatorVariant {
    /// Create a new accumulator instance based on the specified type.
    pub fn new(accumulator_type: AccumulatorType, config: &AccumulatorConfig) -> Self {
        match accumulator_type {
            AccumulatorType::Merkle => AccumulatorVariant::Merkle(MerkleAccumulator::default()),
            AccumulatorType::SparseMerkle => {
                AccumulatorVariant::SparseMerkle(SparseMerkleAccumulator::new())
            }
            AccumulatorType::External => {
                let cfg = config
                    .external
                    .as_ref()
                    .expect("External accumulator config must be provided");
                AccumulatorVariant::External(ExternalServiceAccumulator::new(cfg.clone()))
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
            AccumulatorVariant::External(_) => AccumulatorType::External,
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
            AccumulatorVariant::External(inner) => inner.commit(records, result_tx).await,
        }
    }
}
