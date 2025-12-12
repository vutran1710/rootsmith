use anyhow::Result;
use crate::types::StoredProof;

/// Where proofs are stored (e.g. S3, GitHub, on-chain, file system).
pub trait ProofRegistry: Send + Sync {
    /// Registry name for logging and metrics.
    fn name(&self) -> &'static str;

    /// Save a single proof.
    fn save_proof(&self, proof: &StoredProof) -> Result<()>;

    /// Save multiple proofs in a batch, if supported.
    fn save_proofs(&self, proofs: &[StoredProof]) -> Result<()> {
        for p in proofs {
            self.save_proof(p)?;
        }
        Ok(())
    }
}
