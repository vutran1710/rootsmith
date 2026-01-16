use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

use crate::types::Key32;
use crate::types::Proof;
use crate::types::RawRecord;
use crate::types::Value32;

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
        result_tx: tokio::sync::mpsc::UnboundedSender<(Vec<u8>, Option<HashMap<Key32, Proof>>)>,
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

    // ===== Legacy Methods (for backward compatibility) =====
    // These methods are kept for existing code that uses the old interface.

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
