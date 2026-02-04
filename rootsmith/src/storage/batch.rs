//! Batch storage for collecting records by time range before commitment.
//!
//! A batch represents all records within a specific time window for given namespaces.
//! Workers create batches periodically (e.g., weekly, monthly) and collect records
//! by querying the record storage with the batch's time range.

use std::sync::Arc;

use anyhow::Result;
use rocksdb::DB;
use serde::Deserialize;
use serde::Serialize;

use crate::types::Namespace;
use crate::types::Record;

use super::RecordStorage;
use super::StorageQueryFilter;

const BATCH_PREFIX: u8 = 0x02;

pub type BatchId = [u8; 16];
pub type CommitmentId = [u8; 32];

/// Batch status in the processing pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchStatus {
    /// Batch created, waiting to process
    Pending,
    /// Submitted to accumulator
    Processing,
    /// Commitment received
    Committed,
    /// Processing failed, can retry
    Failed,
}

/// Batch metadata stored in the database.
///
/// A batch defines a time range query to collect records from specific namespaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMetadata {
    pub batch_id: BatchId,
    pub namespaces: Vec<Namespace>,
    /// Start of collection window (inclusive), unix timestamp
    pub time_start: u64,
    /// End of collection window (inclusive), unix timestamp
    pub time_end: u64,
    pub status: BatchStatus,
    /// Cached record count (set after collection)
    pub record_count: u64,
    pub created_at: u64,
    pub updated_at: u64,
    /// Links to commitment after processing
    pub commitment_id: Option<CommitmentId>,
}

mod key_layout {
    /// Key: prefix (1) + batch_id (16) = 17 bytes
    pub const BATCH_KEY_SIZE: usize = 1 + 16;
}

use key_layout::*;

type BatchKey = [u8; BATCH_KEY_SIZE];

fn make_batch_key(batch_id: &BatchId) -> BatchKey {
    let mut key = [0u8; BATCH_KEY_SIZE];
    key[0] = BATCH_PREFIX;
    key[1..17].copy_from_slice(batch_id);
    key
}

/// Storage for batch metadata.
pub struct BatchStorage {
    db: Arc<DB>,
}

impl BatchStorage {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    /// Create a new batch for the given namespaces and time range.
    pub fn create(
        &self,
        batch_id: BatchId,
        namespaces: Vec<Namespace>,
        time_start: u64,
        time_end: u64,
        created_at: u64,
    ) -> Result<()> {
        let metadata = BatchMetadata {
            batch_id,
            namespaces,
            time_start,
            time_end,
            status: BatchStatus::Pending,
            record_count: 0,
            created_at,
            updated_at: created_at,
            commitment_id: None,
        };

        let key = make_batch_key(&batch_id);
        let value = postcard::to_allocvec(&metadata)?;
        self.db.put(&key, &value)?;
        Ok(())
    }

    /// Get batch metadata by ID.
    pub fn get(&self, batch_id: &BatchId) -> Result<Option<BatchMetadata>> {
        let key = make_batch_key(batch_id);
        if let Some(value) = self.db.get(&key)? {
            let metadata: BatchMetadata = postcard::from_bytes(&value)?;
            return Ok(Some(metadata));
        }
        Ok(None)
    }

    /// Update batch status.
    pub fn update_status(
        &self,
        batch_id: &BatchId,
        status: BatchStatus,
        timestamp: u64,
    ) -> Result<bool> {
        let key = make_batch_key(batch_id);
        if let Some(value) = self.db.get(&key)? {
            let mut metadata: BatchMetadata = postcard::from_bytes(&value)?;
            metadata.status = status;
            metadata.updated_at = timestamp;
            let new_value = postcard::to_allocvec(&metadata)?;
            self.db.put(&key, &new_value)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Update batch record count (after collection).
    pub fn update_record_count(
        &self,
        batch_id: &BatchId,
        record_count: u64,
        timestamp: u64,
    ) -> Result<bool> {
        let key = make_batch_key(batch_id);
        if let Some(value) = self.db.get(&key)? {
            let mut metadata: BatchMetadata = postcard::from_bytes(&value)?;
            metadata.record_count = record_count;
            metadata.updated_at = timestamp;
            let new_value = postcard::to_allocvec(&metadata)?;
            self.db.put(&key, &new_value)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Mark batch as committed with the commitment ID.
    pub fn mark_committed(
        &self,
        batch_id: &BatchId,
        commitment_id: &CommitmentId,
        timestamp: u64,
    ) -> Result<bool> {
        let key = make_batch_key(batch_id);
        if let Some(value) = self.db.get(&key)? {
            let mut metadata: BatchMetadata = postcard::from_bytes(&value)?;
            metadata.status = BatchStatus::Committed;
            metadata.commitment_id = Some(*commitment_id);
            metadata.updated_at = timestamp;
            let new_value = postcard::to_allocvec(&metadata)?;
            self.db.put(&key, &new_value)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Get all records for a batch by querying the record storage.
    ///
    /// This queries records from all namespaces in the batch within the time range.
    pub fn get_records(
        &self,
        batch_id: &BatchId,
        record_storage: &RecordStorage,
    ) -> Result<Vec<Record>> {
        let metadata = match self.get(batch_id)? {
            Some(m) => m,
            None => return Ok(Vec::new()),
        };

        let mut records = Vec::new();

        for namespace in &metadata.namespaces {
            let filter = StorageQueryFilter {
                namespace: *namespace,
                time_range: Some((metadata.time_start, metadata.time_end)),
                key: None,
            };
            let namespace_records = record_storage.query(&filter)?;
            records.extend(namespace_records);
        }

        Ok(records)
    }

    /// Query batches by status.
    pub fn query_by_status(&self, status: BatchStatus) -> Result<Vec<BatchMetadata>> {
        let prefix = [BATCH_PREFIX];
        let mut results = Vec::new();

        let iter = self.db.prefix_iterator(&prefix);
        for item in iter {
            let (k, v) = item?;
            if k.is_empty() || k[0] != BATCH_PREFIX {
                break;
            }
            if k.len() == BATCH_KEY_SIZE {
                let metadata: BatchMetadata = postcard::from_bytes(&v)?;
                if metadata.status == status {
                    results.push(metadata);
                }
            }
        }

        Ok(results)
    }

    /// Get all batches.
    pub fn list_all(&self) -> Result<Vec<BatchMetadata>> {
        let prefix = [BATCH_PREFIX];
        let mut results = Vec::new();

        let iter = self.db.prefix_iterator(&prefix);
        for item in iter {
            let (k, v) = item?;
            if k.is_empty() || k[0] != BATCH_PREFIX {
                break;
            }
            if k.len() == BATCH_KEY_SIZE {
                let metadata: BatchMetadata = postcard::from_bytes(&v)?;
                results.push(metadata);
            }
        }

        Ok(results)
    }

    /// Delete a batch.
    pub fn delete(&self, batch_id: &BatchId) -> Result<bool> {
        let key = make_batch_key(batch_id);
        if self.db.get(&key)?.is_some() {
            self.db.delete(&key)?;
            return Ok(true);
        }
        Ok(false)
    }
}

/// Generate a batch ID from namespaces and time range.
///
/// This creates a deterministic ID based on the batch parameters.
pub fn generate_batch_id(namespaces: &[Namespace], time_start: u64, time_end: u64) -> BatchId {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();

    // Hash all namespaces (sorted for determinism)
    let mut sorted_ns = namespaces.to_vec();
    sorted_ns.sort();
    for ns in &sorted_ns {
        ns.hash(&mut hasher);
    }

    // Hash time range
    time_start.hash(&mut hasher);
    time_end.hash(&mut hasher);

    let hash = hasher.finish();

    // Create 16-byte ID from hash
    let mut batch_id = [0u8; 16];
    batch_id[..8].copy_from_slice(&hash.to_le_bytes());
    batch_id[8..16].copy_from_slice(&hash.to_be_bytes());
    batch_id
}
