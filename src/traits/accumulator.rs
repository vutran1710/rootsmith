use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;

use crate::types::CommitmentResult;
use crate::types::Record;
use crate::AccumulatorType;

/// Stateful cryptographic accumulator with async batch processing.
///
/// The accumulator is a blackbox module that handles batch processing of records.
/// It produces commitment results that are delivered asynchronously via channels,
/// supporting scenarios where commitment may take hours (e.g., external services).
#[async_trait]
pub trait Accumulator: Send + Sync {
    /// Identifier for logging/telemetry (e.g. "merkle", "sparse-merkle").
    fn accumulator_type(&self) -> AccumulatorType;

    async fn commit(
        &self,
        records: &[Record],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()>;
}
