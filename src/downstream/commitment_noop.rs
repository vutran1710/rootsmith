use anyhow::Result;
use crate::types::{BatchCommitmentMeta, Commitment, CommitmentFilterOptions};
use crate::traits::CommitmentRegistry;

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
    fn name(&self) -> &'static str {
        "noop"
    }

    fn commit(&self, meta: &BatchCommitmentMeta) -> Result<()> {
        tracing::info!(
            "Noop commitment registration: namespace={:?}, root={:?}, committed_at={}",
            meta.commitment.namespace,
            meta.commitment.root,
            meta.commitment.committed_at
        );
        Ok(())
    }

    fn get_prev_commitment(
        &self,
        _filter: &CommitmentFilterOptions,
    ) -> Result<Option<Commitment>> {
        // Noop registry doesn't store anything
        Ok(None)
    }
}
