// Library exports for testing and external use

pub mod archive;
pub mod config;
pub mod crypto;
pub mod downstream;
pub mod feature_select;
pub mod proof_delivery;
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
pub use config::AccumulatorType;
pub use config::BaseConfig;
pub use config::DownstreamType;
pub use config::UpstreamType;
pub use crypto::AccumulatorVariant;
pub use downstream::DownstreamVariant;
pub use proof_delivery::MockDelivery;
pub use proof_delivery::ProofDeliveryVariant;
pub use rootsmith::CommittedRecord;
pub use rootsmith::EpochPhase;
pub use rootsmith::RootSmith;
pub use storage::Storage;
pub use traits::Accumulator;
pub use traits::ArchiveData;
pub use traits::ArchiveFilter;
pub use traits::ArchiveStorage;
pub use traits::Downstream;
pub use traits::ProofDelivery;
pub use traits::UpstreamConnector;
pub use types::BatchCommitmentMeta;
pub use types::BatchOutput;
pub use types::Commitment;
pub use types::CommitmentFilterOptions;
pub use types::CommitmentResult;
pub use types::IncomingRecord;
pub use types::Key32;
pub use types::Namespace;
pub use types::StoredProof;
pub use upstream::UpstreamVariant;
