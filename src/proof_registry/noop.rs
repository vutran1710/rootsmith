use anyhow::Result;
use async_trait::async_trait;
use crate::types::StoredProof;
use crate::traits::ProofRegistry;

/// Noop proof registry for demonstration purposes.
pub struct NoopProofRegistry;

#[async_trait]
impl ProofRegistry for NoopProofRegistry {
    fn name(&self) -> &'static str {
        "noop-proof"
    }
    
    async fn save_proof(&self, _proof: &StoredProof) -> Result<()> {
        Ok(())
    }
    
    async fn save_proofs(&self, _proofs: &[StoredProof]) -> Result<()> {
        Ok(())
    }
}
