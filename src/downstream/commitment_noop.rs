use anyhow::Result;
use crate::types::LegacyCommitment;
use super::CommitmentRegistry;

pub struct CommitmentNoop;

impl CommitmentNoop {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CommitmentNoop {
    fn default() -> Self {
        Self::new()
    }
}

impl CommitmentRegistry for CommitmentNoop {
    fn register(&mut self, commitment: &LegacyCommitment) -> Result<()> {
        tracing::info!(
            "Noop commitment registration: epoch={}, root={}",
            commitment.epoch,
            commitment.merkle_root
        );
        Ok(())
    }

    fn verify(&self, _commitment: &LegacyCommitment) -> Result<bool> {
        Ok(true)
    }
}
