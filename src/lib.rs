// Library exports for testing and external use

pub mod archive;
pub mod commitment_registry;
pub mod config;
pub mod crypto;
pub mod feature_select;
pub mod parser;
pub mod proof_delivery;
pub mod proof_registry;
pub mod rootsmith;
pub mod storage;
pub mod telemetry;
pub mod traits;
pub mod types;
pub mod upstream;

// Re-export commonly used types and traits
// Re-export variant enums for convenience
pub use archive::ArchiveStorageVariant;
pub use archive::MockArchive;
pub use commitment_registry::CommitmentRegistryVariant;
pub use commitment_registry::MockCommitmentRegistry;
pub use config::AccumulatorType;
pub use config::BaseConfig;
pub use config::CommitmentRegistryType;
pub use config::ProofRegistryType;
pub use config::UpstreamType;
pub use crypto::AccumulatorVariant;
pub use proof_delivery::MockDelivery;
pub use proof_delivery::ProofDeliveryVariant;
pub use proof_registry::MockProofRegistry;
pub use proof_registry::ProofRegistryVariant;
pub use rootsmith::CommittedRecord;
pub use rootsmith::EpochPhase;
pub use rootsmith::RootSmith;
pub use storage::Storage;
pub use traits::Accumulator;
pub use traits::ArchiveData;
pub use traits::ArchiveFilter;
pub use traits::ArchiveStorage;
pub use traits::CommitmentRegistry;
pub use traits::ProofDelivery;
pub use traits::ProofRegistry;
pub use traits::UpstreamConnector;
pub use types::BatchCommitmentMeta;
pub use types::BatchOutput;
pub use types::Commitment;
pub use types::CommitmentFilterOptions;
pub use types::IncomingRecord;
pub use types::Key32;
pub use types::Namespace;
pub use types::StoredProof;
pub use upstream::UpstreamVariant;
