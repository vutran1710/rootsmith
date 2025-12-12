use anyhow::Result;
use crate::types::{BatchCommitmentMeta, Commitment, CommitmentFilterOptions};
use crate::traits::CommitmentRegistry;
use super::{commitment_contract::CommitmentContract, commitment_noop::CommitmentNoop};

/// Enum representing all possible commitment registry implementations.
pub enum CommitmentRegistryVariant {
    Contract(CommitmentContract),
    Noop(CommitmentNoop),
    Mock(MockCommitmentRegistry),
}

impl CommitmentRegistry for CommitmentRegistryVariant {
    fn name(&self) -> &'static str {
        match self {
            CommitmentRegistryVariant::Contract(inner) => inner.name(),
            CommitmentRegistryVariant::Noop(inner) => inner.name(),
            CommitmentRegistryVariant::Mock(inner) => inner.name(),
        }
    }

    fn commit(&self, meta: &BatchCommitmentMeta) -> Result<()> {
        match self {
            CommitmentRegistryVariant::Contract(inner) => inner.commit(meta),
            CommitmentRegistryVariant::Noop(inner) => inner.commit(meta),
            CommitmentRegistryVariant::Mock(inner) => inner.commit(meta),
        }
    }

    fn get_prev_commitment(
        &self,
        filter: &CommitmentFilterOptions,
    ) -> Result<Option<Commitment>> {
        match self {
            CommitmentRegistryVariant::Contract(inner) => inner.get_prev_commitment(filter),
            CommitmentRegistryVariant::Noop(inner) => inner.get_prev_commitment(filter),
            CommitmentRegistryVariant::Mock(inner) => inner.get_prev_commitment(filter),
        }
    }
}

/// Mock commitment registry for testing.
#[derive(Clone)]
pub struct MockCommitmentRegistry {
    pub commitments: std::sync::Arc<std::sync::Mutex<Vec<BatchCommitmentMeta>>>,
}

impl MockCommitmentRegistry {
    pub fn new() -> Self {
        Self {
            commitments: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn get_commitments(&self) -> Vec<BatchCommitmentMeta> {
        self.commitments.lock().unwrap().clone()
    }
}

impl CommitmentRegistry for MockCommitmentRegistry {
    fn name(&self) -> &'static str {
        "mock-commitment-registry"
    }

    fn commit(&self, meta: &BatchCommitmentMeta) -> Result<()> {
        self.commitments.lock().unwrap().push(meta.clone());
        println!(
            "MockCommitmentRegistry: committed {} leaves for namespace {:?}",
            meta.leaf_count,
            meta.commitment.namespace
        );
        Ok(())
    }

    fn get_prev_commitment(
        &self,
        filter: &CommitmentFilterOptions,
    ) -> Result<Option<Commitment>> {
        let commitments = self.commitments.lock().unwrap();
        
        let result = commitments
            .iter()
            .filter(|m| {
                m.commitment.namespace == filter.namespace
                    && m.commitment.committed_at <= filter.time
            })
            .max_by_key(|m| m.commitment.committed_at)
            .map(|m| m.commitment.clone());

        Ok(result)
    }
}

