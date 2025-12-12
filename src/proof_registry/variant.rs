use anyhow::Result;
use crate::types::StoredProof;
use crate::traits::ProofRegistry;
use super::{
    proof_s3::ProofS3, 
    proof_github::ProofGithub,
    noop::NoopProofRegistry,
    mock::MockProofRegistry,
};

/// Enum representing all possible proof registry implementations.
pub enum ProofRegistryVariant {
    S3(ProofS3),
    Github(ProofGithub),
    Noop(NoopProofRegistry),
    Mock(MockProofRegistry),
}

impl ProofRegistry for ProofRegistryVariant {
    fn name(&self) -> &'static str {
        match self {
            ProofRegistryVariant::S3(inner) => inner.name(),
            ProofRegistryVariant::Github(inner) => inner.name(),
            ProofRegistryVariant::Noop(inner) => inner.name(),
            ProofRegistryVariant::Mock(inner) => inner.name(),
        }
    }

    fn save_proof(&self, proof: &StoredProof) -> Result<()> {
        match self {
            ProofRegistryVariant::S3(inner) => inner.save_proof(proof),
            ProofRegistryVariant::Github(inner) => inner.save_proof(proof),
            ProofRegistryVariant::Noop(inner) => inner.save_proof(proof),
            ProofRegistryVariant::Mock(inner) => inner.save_proof(proof),
        }
    }

    fn save_proofs(&self, proofs: &[StoredProof]) -> Result<()> {
        match self {
            ProofRegistryVariant::S3(inner) => inner.save_proofs(proofs),
            ProofRegistryVariant::Github(inner) => inner.save_proofs(proofs),
            ProofRegistryVariant::Noop(inner) => inner.save_proofs(proofs),
            ProofRegistryVariant::Mock(inner) => inner.save_proofs(proofs),
        }
    }
}


