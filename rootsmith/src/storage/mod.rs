//! Storage module for RocksDB-backed persistence.
//!
//! This module provides storage implementations for different entity types:
//! - `RecordStorage` - for upstream records
//! - `BatchStorage` - for batch collection by time range before commitment
//! - `CommitmentStorage` - for finalized commitments

mod batch;
mod commitment;
mod record;

// Batch types and storage
pub use batch::generate_batch_id;
pub use batch::BatchId;
pub use batch::BatchMetadata;
pub use batch::BatchStatus;
pub use batch::BatchStorage;
pub use batch::CommitmentId;

// Commitment types and storage
pub use commitment::commitment_id_from_root;
pub use commitment::CommitmentStorage;
pub use commitment::StoredCommitment;

// Record types and storage
pub use record::RecordStorage;
pub use record::StorageQueryFilter;

// Backward compatibility alias
pub use RecordStorage as Storage;
