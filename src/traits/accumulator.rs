use anyhow::Result;

use crate::types::Key32;
use crate::types::Proof;
use crate::types::Value32;

/// Stateful cryptographic accumulator.
///
/// The implementation maintains internal state of inserted leaves
/// (within the current batch/window).
pub trait Accumulator: Send + Sync {
    /// Identifier for logging/telemetry (e.g. "merkle", "sparse-merkle").
    fn id(&self) -> &'static str;
    fn put(&mut self, key: Key32, value: Value32) -> Result<()>;
    fn build_root(&self) -> Result<Vec<u8>>; // fixed 32-byte ???
    fn verify_inclusion(&self, key: &Key32, value: &[u8]) -> Result<bool>;

    /// Verify that `key` is *not* included under the current root.
    fn verify_non_inclusion(&self, key: &Key32) -> Result<bool>;

    /// Flush/reset internal state, preparing for a new batch.
    fn flush(&mut self) -> Result<()>;
    fn prove(&self, key: &Key32) -> Result<Option<Proof>>;
    fn prove_many(&self, keys: &[Key32]) -> Result<Vec<(Key32, Option<Proof>)>> {
        let mut out = Vec::with_capacity(keys.len());
        for k in keys {
            out.push((*k, self.prove(k)?));
        }
        Ok(out)
    }
    fn verify_proof(
        &self,
        root: &[u8; 32],
        key: &Key32,
        value: &Value32,
        proof: Option<&Proof>,
    ) -> Result<bool>;
}
