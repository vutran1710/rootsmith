//! Storage module for RocksDB-backed persistence.
//!
//! This module provides storage implementations for different entity types:
//! - `RecordStorage` - for upstream records (prefix 0x01)
//! - `BatchStorage` - for batch collection by time range before commitment (prefix 0x02)
//! - `CommitmentStorage` - for finalized commitments (prefix 0x03-0x05)
//!
//! All storages share a single RocksDB instance and use prefix bytes to distinguish tables.

use std::sync::Arc;

use anyhow::Result;
use rocksdb::Options;
use rocksdb::DB;

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

/// Open a shared RocksDB instance for all storages.
pub fn open_db(path: &str) -> Result<Arc<DB>> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.set_max_open_files(-1);
    let db = DB::open(&opts, path)?;
    Ok(Arc::new(db))
}

/// Container for all storage types sharing a single DB instance.
pub struct StorageManager {
    pub records: RecordStorage,
    pub batches: BatchStorage,
    pub commitments: CommitmentStorage,
}

impl StorageManager {
    /// Open all storages with a shared RocksDB instance.
    pub fn open(path: &str) -> Result<Self> {
        let db = open_db(path)?;
        Ok(Self {
            records: RecordStorage::new(Arc::clone(&db)),
            batches: BatchStorage::new(Arc::clone(&db)),
            commitments: CommitmentStorage::new(db),
        })
    }

    /// Create storage manager from an existing DB instance.
    pub fn from_db(db: Arc<DB>) -> Self {
        Self {
            records: RecordStorage::new(Arc::clone(&db)),
            batches: BatchStorage::new(Arc::clone(&db)),
            commitments: CommitmentStorage::new(db),
        }
    }
}
