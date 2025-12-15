use super::{
    commitment_contract::CommitmentContract, commitment_noop::CommitmentNoop,
    mock::MockCommitmentRegistry,
};
use crate::config::CommitmentRegistryType;
use crate::traits::CommitmentRegistry;
use crate::types::{BatchCommitmentMeta, Commitment, CommitmentFilterOptions};
use anyhow::Result;
use async_trait::async_trait;

/// Enum representing all possible commitment registry implementations.
pub enum CommitmentRegistryVariant {
    Contract(CommitmentContract),
    Noop(CommitmentNoop),
    Mock(MockCommitmentRegistry),
}

impl CommitmentRegistryVariant {
    /// Create a new commitment registry instance based on the specified type.
    pub fn new(registry_type: CommitmentRegistryType) -> Self {
        match registry_type {
            CommitmentRegistryType::Contract => CommitmentRegistryVariant::Contract(
                CommitmentContract::new("0x0000000000000000000000000000000000000000".to_string()),
            ),
            CommitmentRegistryType::Noop => CommitmentRegistryVariant::Noop(CommitmentNoop::new()),
            CommitmentRegistryType::Mock => {
                CommitmentRegistryVariant::Mock(MockCommitmentRegistry::new())
            }
        }
    }
}

#[async_trait]
impl CommitmentRegistry for CommitmentRegistryVariant {
    fn name(&self) -> &'static str {
        match self {
            CommitmentRegistryVariant::Contract(inner) => inner.name(),
            CommitmentRegistryVariant::Noop(inner) => inner.name(),
            CommitmentRegistryVariant::Mock(inner) => inner.name(),
        }
    }

    async fn commit(&self, meta: &BatchCommitmentMeta) -> Result<()> {
        match self {
            CommitmentRegistryVariant::Contract(inner) => inner.commit(meta).await,
            CommitmentRegistryVariant::Noop(inner) => inner.commit(meta).await,
            CommitmentRegistryVariant::Mock(inner) => inner.commit(meta).await,
        }
    }

    async fn get_prev_commitment(
        &self,
        filter: &CommitmentFilterOptions,
    ) -> Result<Option<Commitment>> {
        match self {
            CommitmentRegistryVariant::Contract(inner) => inner.get_prev_commitment(filter).await,
            CommitmentRegistryVariant::Noop(inner) => inner.get_prev_commitment(filter).await,
            CommitmentRegistryVariant::Mock(inner) => inner.get_prev_commitment(filter).await,
        }
    }
}
