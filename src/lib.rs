// Library exports for testing and external use

pub mod rootsmith;
pub mod config;
pub mod crypto;
pub mod commitment_registry;
pub mod proof_registry;
pub mod feature_select;
pub mod storage;
pub mod telemetry;
pub mod traits;
pub mod types;
pub mod upstream;

// Re-export commonly used types and traits
pub use rootsmith::RootSmith;
pub use config::{BaseConfig, AccumulatorType};
pub use storage::Storage;
pub use traits::{Accumulator, CommitmentRegistry, ProofRegistry, UpstreamConnector};
pub use types::{
    BatchCommitmentMeta, BatchOutput, Commitment, CommitmentFilterOptions, IncomingRecord,
    Key32, Namespace, StoredProof,
};

// Re-export variant enums for convenience
pub use upstream::UpstreamVariant;
pub use commitment_registry::{CommitmentRegistryVariant, MockCommitmentRegistry};
pub use proof_registry::{ProofRegistryVariant, MockProofRegistry};
pub use crypto::AccumulatorVariant;
