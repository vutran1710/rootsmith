// Library exports for testing and external use

pub mod app;
pub mod config;
pub mod crypto;
pub mod downstream;
pub mod feature_select;
pub mod storage;
pub mod telemetry;
pub mod traits;
pub mod types;
pub mod upstream;

// Re-export commonly used types and traits
pub use app::App;
pub use config::BaseConfig;
pub use storage::Storage;
pub use traits::{Accumulator, CommitmentRegistry, ProofRegistry, UpstreamConnector};
pub use types::{
    BatchCommitmentMeta, BatchOutput, Commitment, CommitmentFilterOptions, IncomingRecord,
    Key32, Namespace, StoredProof,
};
