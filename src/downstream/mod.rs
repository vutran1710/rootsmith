pub mod commitment_contract;
pub mod commitment_noop;
pub mod proof_s3;
pub mod proof_github;

use anyhow::Result;
use crate::types::{LegacyCommitment, Proof};

pub trait CommitmentRegistry {
    fn register(&mut self, commitment: &LegacyCommitment) -> Result<()>;
    fn verify(&self, commitment: &LegacyCommitment) -> Result<bool>;
}

pub trait ProofRegistry {
    fn store(&mut self, proof: &Proof) -> Result<String>;
    fn retrieve(&self, proof_id: &str) -> Result<Option<Proof>>;
}
