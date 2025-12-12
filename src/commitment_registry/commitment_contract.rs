use anyhow::Result;
use async_trait::async_trait;
use crate::types::{BatchCommitmentMeta, Commitment, CommitmentFilterOptions};
use crate::traits::CommitmentRegistry;

pub struct CommitmentContract {
    contract_address: String,
    registered: Vec<Commitment>,
}

impl CommitmentContract {
    pub fn new(contract_address: String) -> Self {
        Self {
            contract_address,
            registered: Vec::new(),
        }
    }
}

#[async_trait]
impl CommitmentRegistry for CommitmentContract {
    fn name(&self) -> &'static str {
        "contract"
    }

    async fn commit(&self, meta: &BatchCommitmentMeta) -> Result<()> {
        tracing::info!(
            "Registering commitment to contract {}: namespace={:?}, root={:?}, committed_at={}",
            self.contract_address,
            meta.commitment.namespace,
            meta.commitment.root,
            meta.commitment.committed_at
        );
        // In a real implementation, this would interact with a smart contract
        Ok(())
    }

    async fn get_prev_commitment(
        &self,
        filter: &CommitmentFilterOptions,
    ) -> Result<Option<Commitment>> {
        // Find the latest commitment for this namespace with committed_at <= filter.time
        let result = self.registered
            .iter()
            .filter(|c| c.namespace == filter.namespace && c.committed_at <= filter.time)
            .max_by_key(|c| c.committed_at)
            .cloned();
        Ok(result)
    }
}
