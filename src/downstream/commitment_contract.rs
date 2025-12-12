use anyhow::Result;
use crate::types::LegacyCommitment;
use super::CommitmentRegistry;

pub struct CommitmentContract {
    contract_address: String,
    registered: Vec<LegacyCommitment>,
}

impl CommitmentContract {
    pub fn new(contract_address: String) -> Self {
        Self {
            contract_address,
            registered: Vec::new(),
        }
    }
}

impl CommitmentRegistry for CommitmentContract {
    fn register(&mut self, commitment: &LegacyCommitment) -> Result<()> {
        tracing::info!(
            "Registering commitment to contract {}: epoch={}, root={}",
            self.contract_address,
            commitment.epoch,
            commitment.merkle_root
        );
        self.registered.push(commitment.clone());
        Ok(())
    }

    fn verify(&self, commitment: &LegacyCommitment) -> Result<bool> {
        let found = self.registered.iter().any(|c| {
            c.epoch == commitment.epoch && c.merkle_root == commitment.merkle_root
        });
        Ok(found)
    }
}
