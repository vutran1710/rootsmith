use anyhow::Result;
use crate::types::StoredProof;
use crate::traits::ProofRegistry;

pub struct ProofGithub {
    repo: String,
    branch: String,
}

impl ProofGithub {
    pub fn new(repo: String, branch: String) -> Self {
        Self { repo, branch }
    }
}

impl ProofRegistry for ProofGithub {
    fn name(&self) -> &'static str {
        "github"
    }

    fn save_proof(&self, proof: &StoredProof) -> Result<()> {
        let proof_id = format!("proof_{:?}_{}", proof.key, uuid::Uuid::new_v4());
        tracing::info!(
            "Storing proof to GitHub: repo={}, branch={}, id={}",
            self.repo,
            self.branch,
            proof_id
        );
        // In a real implementation, this would commit to GitHub
        Ok(())
    }
}
