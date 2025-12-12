use anyhow::Result;
use crate::types::StoredProof;
use crate::traits::ProofRegistry;

/// Noop proof registry for demonstration purposes.
pub struct NoopProofRegistry;

impl ProofRegistry for NoopProofRegistry {
    fn name(&self) -> &'static str {
        "noop-proof"
    }
    
    fn save_proof(&self, _proof: &StoredProof) -> Result<()> {
        Ok(())
    }
    
    fn save_proofs(&self, _proofs: &[StoredProof]) -> Result<()> {
        Ok(())
    }
}
