// Library exports for testing and external use

pub mod commitment_registry;
pub mod config;
pub mod crypto;
pub mod feature_select;
pub mod proof_registry;
pub mod rootsmith;
pub mod storage;
pub mod telemetry;
pub mod traits;
pub mod types;
pub mod upstream;

// Re-export commonly used types and traits
pub use config::{
    AccumulatorType, BaseConfig, CommitmentRegistryType, ProofRegistryType, UpstreamType,
};
pub use rootsmith::RootSmith;
pub use storage::Storage;
pub use traits::{Accumulator, CommitmentRegistry, ProofRegistry, UpstreamConnector};
pub use types::{
    BatchCommitmentMeta, BatchOutput, Commitment, CommitmentFilterOptions, IncomingRecord, Key32,
    Namespace, StoredProof,
};

// Re-export variant enums for convenience
pub use commitment_registry::{CommitmentRegistryVariant, MockCommitmentRegistry};
pub use crypto::AccumulatorVariant;
pub use proof_registry::{MockProofRegistry, ProofRegistryVariant};
pub use upstream::UpstreamVariant;
