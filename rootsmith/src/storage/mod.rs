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

// Storage prefixes for RocksDB key separation
pub(crate) const RECORD_PREFIX: u8 = 0x01;
pub(crate) const BATCH_PREFIX: u8 = 0x02;
pub(crate) const COMMITMENT_PREFIX: u8 = 0x03;
pub(crate) const COMMITMENT_NS_INDEX: u8 = 0x04;
pub(crate) const COMMITMENT_TIME_INDEX: u8 = 0x05;

mod batch;
mod commitment;
mod enums;
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

// Storage operation enums
pub use enums::Deletable;
pub use enums::Retrievable;
pub use enums::Retrieved;
pub use enums::Storable;
pub use enums::Updatable;

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
    records: RecordStorage,
    batches: BatchStorage,
    commitments: CommitmentStorage,
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

    /// Put an item into the appropriate storage using pattern matching.
    pub fn put(&self, item: Storable) -> Result<Option<CommitmentId>> {
        match item {
            Storable::Record(record) => {
                self.records.put(&record)?;
                Ok(None)
            }
            Storable::Records(records) => {
                self.records.put_batch(&records)?;
                Ok(None)
            }
            Storable::Batch(batch) => {
                self.batches.create(
                    batch.batch_id,
                    batch.namespaces,
                    batch.time_start,
                    batch.time_end,
                    batch.created_at,
                )?;
                Ok(None)
            }
            Storable::Commitment(commitment) => {
                let id = self.commitments.store(
                    commitment.root,
                    commitment.namespaces,
                    commitment.batch_id,
                    commitment.time_start,
                    commitment.time_end,
                    commitment.record_count,
                    commitment.committed_at,
                    commitment.proofs,
                )?;
                Ok(Some(id))
            }
        }
    }

    /// Get an item from the appropriate storage using pattern matching.
    pub fn get(&self, key: Retrievable) -> Result<Retrieved> {
        match key {
            Retrievable::Record {
                namespace,
                key,
                timestamp,
            } => {
                let record = self.records.get_version(&namespace, &key, timestamp)?;
                Ok(Retrieved::Record(record))
            }
            Retrievable::RecordLatest { namespace, key } => {
                let record = self.records.get_latest(&namespace, &key)?;
                Ok(Retrieved::Record(record))
            }
            Retrievable::RecordAllVersions { namespace, key } => {
                let records = self.records.get_all_versions(&namespace, &key)?;
                Ok(Retrieved::Records(records))
            }
            Retrievable::RecordsByNamespace(namespace) => {
                let records = self.records.query_namespace(&namespace)?;
                Ok(Retrieved::Records(records))
            }
            Retrievable::RecordsByFilter(filter) => {
                let records = self.records.query(&filter)?;
                Ok(Retrieved::Records(records))
            }
            Retrievable::Batch(batch_id) => {
                let batch = self.batches.get(&batch_id)?;
                Ok(Retrieved::Batch(batch))
            }
            Retrievable::BatchAll => {
                let batches = self.batches.list_all()?;
                Ok(Retrieved::Batches(batches))
            }
            Retrievable::BatchByStatus(status) => {
                let batches = self.batches.query_by_status(status)?;
                Ok(Retrieved::Batches(batches))
            }
            Retrievable::BatchRecords(batch_id) => {
                let records = self.batches.get_records(&batch_id, &self.records)?;
                Ok(Retrieved::Records(records))
            }
            Retrievable::Commitment(commitment_id) => {
                let commitment = self.commitments.get(&commitment_id)?;
                Ok(Retrieved::Commitment(commitment))
            }
            Retrievable::CommitmentAll => {
                let commitments = self.commitments.list_all()?;
                Ok(Retrieved::Commitments(commitments))
            }
            Retrievable::CommitmentByNamespace(namespace) => {
                let commitments = self.commitments.get_by_namespace(&namespace)?;
                Ok(Retrieved::Commitments(commitments))
            }
            Retrievable::CommitmentByTimeRange { start, end } => {
                let commitments = self.commitments.query_by_time_range(start, end)?;
                Ok(Retrieved::Commitments(commitments))
            }
        }
    }

    /// Delete an item from the appropriate storage using pattern matching.
    pub fn delete(&self, key: Deletable) -> Result<bool> {
        match key {
            Deletable::Records(filter) => {
                let count = self.records.delete(&filter)?;
                Ok(count > 0)
            }
            Deletable::Batch(batch_id) => self.batches.delete(&batch_id),
            Deletable::Commitment(commitment_id) => self.commitments.delete(&commitment_id),
        }
    }

    /// Update an item using pattern matching.
    pub fn update(&self, op: Updatable) -> Result<bool> {
        match op {
            Updatable::BatchStatus {
                batch_id,
                status,
                timestamp,
            } => self.batches.update_status(&batch_id, status, timestamp),
            Updatable::BatchRecordCount {
                batch_id,
                count,
                timestamp,
            } => self.batches.update_record_count(&batch_id, count, timestamp),
            Updatable::BatchCommitted {
                batch_id,
                commitment_id,
                timestamp,
            } => self
                .batches
                .mark_committed(&batch_id, &commitment_id, timestamp),
        }
    }
}
