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
mod types;
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

// Storage operation types
pub use types::Entity;
pub use types::Filter;
pub use types::Storable;

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
    pub fn put(&self, item: Storable) -> Result<()> {
        match item {
            Storable::Record(record) => {
                self.records.put(&record)?;
                Ok(())
            }
            Storable::Batch(batch) => {
                self.batches.create(
                    batch.batch_id,
                    batch.namespaces,
                    batch.time_start,
                    batch.time_end,
                    batch.created_at,
                )?;
                Ok(())
            }
            Storable::Commitment(commitment) => {
                self.commitments.store(
                    commitment.root,
                    commitment.namespaces,
                    commitment.batch_id,
                    commitment.time_start,
                    commitment.time_end,
                    commitment.record_count,
                    commitment.committed_at,
                    commitment.proofs,
                )?;
                Ok(())
            }
        }
    }

    /// Get items from the appropriate storage.
    pub fn get(&self, filter: Filter) -> Result<Vec<Storable>> {
        match filter.entity {
            Entity::Record => {
                if let Some(batch_id) = filter.batch_id {
                    let records = self.batches.get_records(&batch_id, &self.records)?;
                    Ok(records.into_iter().map(Storable::Record).collect())
                } else if let Some(query) = filter.query {
                    let records = self.records.query(&query)?;
                    Ok(records.into_iter().map(Storable::Record).collect())
                } else if let (Some(namespace), Some(key)) = (&filter.namespace, &filter.key) {
                    if let Some(timestamp) = filter.timestamp {
                        let record = self.records.get_version(namespace, key, timestamp)?;
                        Ok(record.into_iter().map(Storable::Record).collect())
                    } else if filter.all_versions {
                        let records = self.records.get_all_versions(namespace, key)?;
                        Ok(records.into_iter().map(Storable::Record).collect())
                    } else {
                        let record = self.records.get_latest(namespace, key)?;
                        Ok(record.into_iter().map(Storable::Record).collect())
                    }
                } else if let Some(namespace) = &filter.namespace {
                    let records = self.records.query_namespace(namespace)?;
                    Ok(records.into_iter().map(Storable::Record).collect())
                } else {
                    Err(anyhow::anyhow!("Invalid record filter"))
                }
            }
            Entity::Batch => {
                if let Some(batch_id) = filter.batch_id {
                    let batch = self.batches.get(&batch_id)?;
                    Ok(batch.into_iter().map(Storable::Batch).collect())
                } else if let Some(status) = filter.status {
                    let batches = self.batches.query_by_status(status)?;
                    Ok(batches.into_iter().map(Storable::Batch).collect())
                } else {
                    let batches = self.batches.list_all()?;
                    Ok(batches.into_iter().map(Storable::Batch).collect())
                }
            }
            Entity::Commitment => {
                if let Some(commitment_id) = filter.commitment_id {
                    let commitment = self.commitments.get(&commitment_id)?;
                    Ok(commitment.into_iter().map(Storable::Commitment).collect())
                } else if let Some(namespace) = &filter.namespace {
                    let commitments = self.commitments.get_by_namespace(namespace)?;
                    Ok(commitments.into_iter().map(|(_, c)| Storable::Commitment(c)).collect())
                } else if let (Some(start), Some(end)) = (filter.time_start, filter.time_end) {
                    let commitments = self.commitments.query_by_time_range(start, end)?;
                    Ok(commitments.into_iter().map(|(_, c)| Storable::Commitment(c)).collect())
                } else {
                    let commitments = self.commitments.list_all()?;
                    Ok(commitments.into_iter().map(|(_, c)| Storable::Commitment(c)).collect())
                }
            }
        }
    }

    /// Delete items matching the filter.
    /// Records require a query filter, batches require a batch_id, commitments require a commitment_id.
    pub fn delete(&self, filter: Filter) -> Result<bool> {
        match filter.entity {
            Entity::Record => {
                if let Some(query) = filter.query {
                    let count = self.records.delete(&query)?;
                    Ok(count > 0)
                } else {
                    Err(anyhow::anyhow!("Record delete requires a query filter"))
                }
            }
            Entity::Batch => {
                if let Some(batch_id) = filter.batch_id {
                    self.batches.delete(&batch_id)
                } else {
                    Err(anyhow::anyhow!("Batch delete requires a batch_id"))
                }
            }
            Entity::Commitment => {
                if let Some(commitment_id) = filter.commitment_id {
                    self.commitments.delete(&commitment_id)
                } else {
                    Err(anyhow::anyhow!("Commitment delete requires a commitment_id"))
                }
            }
        }
    }
}
