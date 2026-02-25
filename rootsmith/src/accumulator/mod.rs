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

use crate::types::CommitmentResult;
use crate::types::Record;

/// The accumulator is a blackbox module that handles batch processing of records.
/// It produces commitment results that are delivered asynchronously via channels,
/// supporting scenarios where commitment may take hours (e.g., external services).
#[async_trait]
pub trait Accumulator: Send + Sync {
    /// Identifier for logging/telemetry (e.g. "merkle", "sparse-merkle").
    fn name(&self) -> &'static str {
        unimplemented!()
    }

    /// Commit records to the accumulator.
    ///
    /// Returns `Ok(Some(job_id))` for external services that return a job ID for tracking.
    /// Returns `Ok(None)` for local accumulators that produce immediate results.
    ///
    /// For external services, the actual commitment result arrives via webhook callback.
    async fn commit(
        &self,
        records: &[Record],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<Option<String>>;
}

/// Enum representing all possible accumulator implementations.
pub enum AccumulatorVariant {
    Merkle(MerkleAccumulator),
    SparseMerkle(SparseMerkleAccumulator),
    External(ExternalServiceAccumulator),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AccumulatorConfig {
    Merkle,
    SparseMerkle,
    External(ExternalServiceConfig),
}

impl AccumulatorVariant {
    /// Create a new accumulator instance based on the specified type.
    pub fn new(config: &AccumulatorConfig) -> Self {
        match config {
            AccumulatorConfig::Merkle => AccumulatorVariant::Merkle(MerkleAccumulator::default()),
            AccumulatorConfig::SparseMerkle => {
                AccumulatorVariant::SparseMerkle(SparseMerkleAccumulator::new())
            }
            AccumulatorConfig::External(cfg) => {
                AccumulatorVariant::External(ExternalServiceAccumulator::new(cfg.clone()))
            }
        }
    }
}

#[async_trait]
impl Accumulator for AccumulatorVariant {
    fn name(&self) -> &'static str {
        match self {
            AccumulatorVariant::Merkle(_) => "merkle",
            AccumulatorVariant::SparseMerkle(_) => "sparse-merkle",
            AccumulatorVariant::External(_) => "external",
        }
    }

    async fn commit(
        &self,
        records: &[Record],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<Option<String>> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.commit(records, result_tx).await,
            AccumulatorVariant::SparseMerkle(inner) => inner.commit(records, result_tx).await,
            AccumulatorVariant::External(inner) => inner.commit(records, result_tx).await,
        }
    }
}
