use super::Leaf;
use crate::types::{Key32, Value32};
use anyhow::Result;

/// Stateful cryptographic accumulator.
///
/// The implementation maintains internal state of inserted leaves
/// (within the current batch/window).
pub trait Accumulator: Send + Sync {
    /// Identifier for logging/telemetry (e.g. "merkle", "sparse-merkle").
    fn id(&self) -> &'static str;

    /// Insert a single leaf into the current accumulator state.
    fn put(&mut self, key: Key32, value: Value32) -> Result<()>;

    /// Insert multiple leaves into the current accumulator state.
    fn put_many(&mut self, items: &[Leaf]) -> Result<()> {
        for leaf in items {
            self.put(leaf.key, leaf.value)?;
        }
        Ok(())
    }

    /// Build and return the current root/commitment over all leaves
    /// inserted since the last `flush`.
    fn build_root(&self) -> Result<Vec<u8>>;

    /// Verify that (key, value) is included under the current root.
    ///
    /// Implementations may:
    /// - reconstruct proofs from internal state, or
    /// - use cached proofs, or
    /// - perform a recomputation.
    fn verify_inclusion(&self, key: &Key32, value: &[u8]) -> Result<bool>;

    /// Verify that `key` is *not* included under the current root.
    fn verify_non_inclusion(&self, key: &Key32) -> Result<bool>;

    /// Flush/reset internal state, preparing for a new batch.
    fn flush(&mut self) -> Result<()>;
}
