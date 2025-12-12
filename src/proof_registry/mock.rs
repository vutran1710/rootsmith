use anyhow::Result;
use crate::types::StoredProof;
use crate::traits::ProofRegistry;

/// Mock proof registry for testing.
#[derive(Clone)]
pub struct MockProofRegistry {
    pub proofs: std::sync::Arc<std::sync::Mutex<Vec<StoredProof>>>,
}

impl MockProofRegistry {
    pub fn new() -> Self {
        Self {
            proofs: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

impl ProofRegistry for MockProofRegistry {
    fn name(&self) -> &'static str {
        "mock-proof-registry"
    }

    fn save_proof(&self, proof: &StoredProof) -> Result<()> {
        self.proofs.lock().unwrap().push(proof.clone());
        Ok(())
    }

    fn save_proofs(&self, proofs: &[StoredProof]) -> Result<()> {
        for proof in proofs {
            self.save_proof(proof)?;
        }
        Ok(())
    }
}
