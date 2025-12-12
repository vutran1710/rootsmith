use anyhow::Result;
use crate::types::{BatchCommitmentMeta, Commitment, CommitmentFilterOptions};
use crate::traits::CommitmentRegistry;
use crate::config::CommitmentRegistryType;
use super::{
    commitment_contract::CommitmentContract, 
    commitment_noop::CommitmentNoop,
    mock::MockCommitmentRegistry,
};

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
            CommitmentRegistryType::Contract => CommitmentRegistryVariant::Contract(CommitmentContract::new("0x0000000000000000000000000000000000000000".to_string())),
            CommitmentRegistryType::Noop => CommitmentRegistryVariant::Noop(CommitmentNoop::new()),
            CommitmentRegistryType::Mock => CommitmentRegistryVariant::Mock(MockCommitmentRegistry::new()),
        }
    }
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


