pub mod commitment_contract;
pub mod commitment_noop;
pub mod proof_s3;
pub mod proof_github;

use anyhow::Result;
use crate::types::{Commitment, Proof};

pub trait CommitmentRegistry {
    fn register(&mut self, commitment: &Commitment) -> Result<()>;
    fn verify(&self, commitment: &Commitment) -> Result<bool>;
}

pub trait ProofRegistry {
    fn store(&mut self, proof: &Proof) -> Result<String>;
    fn retrieve(&self, proof_id: &str) -> Result<Option<Proof>>;
}
