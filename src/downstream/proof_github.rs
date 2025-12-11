use anyhow::Result;
use crate::types::Proof;
use super::ProofRegistry;

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
    fn store(&mut self, proof: &Proof) -> Result<String> {
        let proof_id = format!("proof_{}_{}", proof.block_number, uuid::Uuid::new_v4());
        tracing::info!(
            "Storing proof to GitHub: repo={}, branch={}, id={}",
            self.repo,
            self.branch,
            proof_id
        );
        Ok(proof_id)
    }

    fn retrieve(&self, proof_id: &str) -> Result<Option<Proof>> {
        tracing::info!(
            "Retrieving proof from GitHub: repo={}, id={}",
            self.repo,
            proof_id
        );
        Ok(None)
    }
}
