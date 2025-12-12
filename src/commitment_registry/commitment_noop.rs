use crate::traits::CommitmentRegistry;
use crate::types::{BatchCommitmentMeta, Commitment, CommitmentFilterOptions};
use anyhow::Result;
use async_trait::async_trait;

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

#[async_trait]
impl CommitmentRegistry for CommitmentNoop {
    fn name(&self) -> &'static str {
        "noop"
    }

    async fn commit(&self, meta: &BatchCommitmentMeta) -> Result<()> {
        tracing::info!(
            "Noop commitment registration: namespace={:?}, root={:?}, committed_at={}",
            meta.commitment.namespace,
            meta.commitment.root,
            meta.commitment.committed_at
        );
        Ok(())
    }

    async fn get_prev_commitment(
        &self,
        _filter: &CommitmentFilterOptions,
    ) -> Result<Option<Commitment>> {
        // Noop registry doesn't store anything
        Ok(None)
    }
}
