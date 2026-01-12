use anyhow::Result;
use async_trait::async_trait;

use crate::types::StoredProof;

/// Where proofs are stored (e.g. S3, GitHub, on-chain, file system).
#[async_trait]
pub trait ProofRegistry: Send + Sync {
    /// Registry name for logging and metrics.
    fn name(&self) -> &'static str;

    /// Save a single proof.
    async fn save_proof(&self, proof: &StoredProof) -> Result<()>;

    /// Save multiple proofs in a batch, if supported.
    async fn save_proofs(&self, proofs: &[StoredProof]) -> Result<()> {
        for p in proofs {
            self.save_proof(p).await?;
        }
        Ok(())
    }
}
