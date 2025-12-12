use anyhow::Result;
use crate::types::StoredProof;
use crate::traits::ProofRegistry;
use crate::config::ProofRegistryType;
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

impl ProofRegistryVariant {
    /// Create a new proof registry instance based on the specified type.
    pub fn new(registry_type: ProofRegistryType) -> Self {
        match registry_type {
            ProofRegistryType::S3 => ProofRegistryVariant::S3(ProofS3::new("rootsmith-proofs".to_string(), "us-east-1".to_string())),
            ProofRegistryType::Github => ProofRegistryVariant::Github(ProofGithub::new("owner/repo".to_string(), "main".to_string())),
            ProofRegistryType::Noop => ProofRegistryVariant::Noop(NoopProofRegistry),
            ProofRegistryType::Mock => ProofRegistryVariant::Mock(MockProofRegistry::new()),
        }
    }
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


