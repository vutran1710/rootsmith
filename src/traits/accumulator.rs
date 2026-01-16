use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

use crate::types::Key32;
use crate::types::Proof;
use crate::types::RawRecord;
use crate::types::Value32;

/// Stateful cryptographic accumulator with support for both sync and async operations.
///
/// The accumulator is a blackbox module that can handle batch processing of records.
/// It produces commitment results that may be delivered synchronously or asynchronously
/// via channels (e.g., when sending to external services that take hours to respond).
#[async_trait]
pub trait Accumulator: Send + Sync {
    /// Identifier for logging/telemetry (e.g. "merkle", "sparse-merkle").
    fn id(&self) -> &'static str;

    /// Process a batch of records and produce a commitment result synchronously.
    ///
    /// This is the primary method for batch commitment. It takes an array of records
    /// and returns the commitment result immediately (blocking/synchronous).
    ///
    /// # Arguments
    /// * `records` - Array of raw records to accumulate
    ///
    /// # Returns
    /// * Root hash and optional structured proofs
    fn commit_batch(&mut self, records: &[RawRecord]) -> Result<(Vec<u8>, Option<HashMap<Key32, Proof>>)>;

    /// Process a batch of records and send the commitment result asynchronously via a channel.
    ///
    /// This method is for accumulators that need to interact with external services
    /// (e.g., blockchain, external prover) where the result may not be available immediately.
    /// The result will be sent through the provided channel when ready.
    ///
    /// # Arguments
    /// * `records` - Array of raw records to accumulate
    /// * `result_tx` - Channel sender for delivering the commitment result asynchronously
    ///
    /// # Returns
    /// * `Ok(())` if the async operation was started successfully
    async fn commit_batch_async(
        &mut self,
        records: &[RawRecord],
        result_tx: tokio::sync::mpsc::UnboundedSender<(Vec<u8>, Option<HashMap<Key32, Proof>>)>,
    ) -> Result<()> {
        // Default implementation: compute synchronously and send via channel
        let result = self.commit_batch(records)?;
        result_tx
            .send(result)
            .map_err(|_| anyhow::anyhow!("Failed to send commitment result"))?;
        Ok(())
    }

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

    // ===== Legacy Methods (for backward compatibility) =====
    // These methods are kept for existing code that uses the old interface.
    // New code should use commit_batch/commit_batch_async instead.

    /// Legacy: Insert a single key-value pair (backward compatibility).
    fn put(&mut self, key: Key32, value: Value32) -> Result<()>;

    /// Legacy: Build root from accumulated state (backward compatibility).
    fn build_root(&self) -> Result<Vec<u8>>;

    /// Legacy: Verify inclusion of a key-value pair (backward compatibility).
    fn verify_inclusion(&self, key: &Key32, value: &[u8]) -> Result<bool>;

    /// Legacy: Verify non-inclusion of a key (backward compatibility).
    fn verify_non_inclusion(&self, key: &Key32) -> Result<bool>;

    /// Legacy: Flush/reset internal state (backward compatibility).
    fn flush(&mut self) -> Result<()>;

    /// Legacy: Generate proof for a single key (backward compatibility).
    fn prove(&self, key: &Key32) -> Result<Option<Proof>>;

    /// Legacy: Generate proofs for multiple keys (backward compatibility).
    fn prove_many(&self, keys: &[Key32]) -> Result<Vec<(Key32, Option<Proof>)>> {
        let mut out = Vec::with_capacity(keys.len());
        for k in keys {
            out.push((*k, self.prove(k)?));
        }
        Ok(out)
    }
}
