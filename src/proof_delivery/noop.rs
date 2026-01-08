use crate::traits::ProofDelivery;
use crate::types::StoredProof;
use anyhow::Result;
use async_trait::async_trait;

/// Noop proof delivery that doesn't deliver proofs anywhere.
/// Useful for testing or when delivery is not needed.
pub struct NoopDelivery;

#[async_trait]
impl ProofDelivery for NoopDelivery {
    fn name(&self) -> &'static str {
        "noop-delivery"
    }

    async fn deliver(&self, _proof: &StoredProof) -> Result<()> {
        // No-op: proof is silently discarded
        Ok(())
    }

    async fn deliver_batch(&self, _proofs: &[StoredProof]) -> Result<()> {
        // No-op: proofs are silently discarded
        Ok(())
    }
}

