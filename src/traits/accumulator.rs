use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;

use crate::types::CommitmentResult;
use crate::types::Key32;
use crate::types::Proof;
use crate::types::RawRecord;

/// Stateful cryptographic accumulator with async batch processing.
///
/// The accumulator is a blackbox module that handles batch processing of records.
/// It produces commitment results that are delivered asynchronously via channels,
/// supporting scenarios where commitment may take hours (e.g., external services).
#[async_trait]
pub trait Accumulator: Send + Sync {
    /// Identifier for logging/telemetry (e.g. "merkle", "sparse-merkle").
    fn id(&self) -> &'static str;

    /// Process a batch of records and send the commitment result asynchronously via a channel.
    ///
    /// This is the primary method for batch commitment. It processes records and sends
    /// the result (root hash and proofs) through the provided channel when ready.
    ///
    /// # Arguments
    /// * `records` - Array of raw records to accumulate
    /// * `result_tx` - Channel sender for delivering the commitment result
    ///
    /// # Returns
    /// * `Ok(())` if the operation completed successfully
    async fn commit(
        &mut self,
        records: &[RawRecord],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()>;

    /// Verify a proof for a specific key-value pair against a known root.
    ///
    /// # Arguments
    /// * `root` - The commitment root to verify against
    /// * `key` - The key being proved
    /// * `value` - The value being proved
    /// * `proof` - The proof to verify
    ///
    /// # Returns
    /// * `true` if the proof is valid, `false` otherwise
    fn verify_proof(
        &self,
        root: &[u8; 32],
        key: &Key32,
        value: &[u8],
        proof: Option<&Proof>,
    ) -> Result<bool>;
}
