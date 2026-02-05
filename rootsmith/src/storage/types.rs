use crate::types::Key16;
use crate::types::Namespace;
use crate::types::Record;

use super::BatchId;
use super::BatchMetadata;
use super::CommitmentId;
use super::StoredCommitment;

/// Items that can be stored in the database.
#[derive(Debug, Clone)]
pub enum Storable {
    Record(Record),
    Batch(BatchMetadata),
    Commitment(StoredCommitment),
}

/// Entity types for filtering.
#[derive(Debug, Clone)]
pub enum Entity {
    Record,
    Batch,
    Commitment,
}

/// Query for finding items in the database.
/// Used by both `get` and `delete` operations.
/// Fields map to RocksDB key components and range bounds.
#[derive(Debug, Clone)]
pub struct Filter {
    pub entity: Entity,
    pub namespace: Option<Namespace>,
    pub key: Option<Key16>,
    pub timestamp: Option<u64>,
    pub batch_id: Option<BatchId>,
    pub commitment_id: Option<CommitmentId>,
    pub time_start: Option<u64>,
    pub time_end: Option<u64>,
}

impl Filter {
    /// Create a base filter for an entity type.
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            namespace: None,
            key: None,
            timestamp: None,
            batch_id: None,
            commitment_id: None,
            time_start: None,
            time_end: None,
        }
    }

    /// Get record by namespace, key, and exact timestamp.
    pub fn record(namespace: Namespace, key: Key16, timestamp: u64) -> Self {
        Self {
            namespace: Some(namespace),
            key: Some(key),
            timestamp: Some(timestamp),
            ..Self::new(Entity::Record)
        }
    }

    /// Scan all versions of a record by namespace and key.
    pub fn record_by_key(namespace: Namespace, key: Key16) -> Self {
        Self {
            namespace: Some(namespace),
            key: Some(key),
            ..Self::new(Entity::Record)
        }
    }

    /// Scan all records in a namespace.
    pub fn records_by_namespace(namespace: Namespace) -> Self {
        Self {
            namespace: Some(namespace),
            ..Self::new(Entity::Record)
        }
    }

    /// Scan records by time range within a namespace.
    pub fn records_by_time_range(namespace: Namespace, start: u64, end: u64) -> Self {
        Self {
            namespace: Some(namespace),
            time_start: Some(start),
            time_end: Some(end),
            ..Self::new(Entity::Record)
        }
    }

    /// Get batch by ID.
    pub fn batch(batch_id: BatchId) -> Self {
        Self {
            batch_id: Some(batch_id),
            ..Self::new(Entity::Batch)
        }
    }

    /// Scan all batches.
    pub fn batch_all() -> Self {
        Self::new(Entity::Batch)
    }

    /// Get commitment by ID.
    pub fn commitment(commitment_id: CommitmentId) -> Self {
        Self {
            commitment_id: Some(commitment_id),
            ..Self::new(Entity::Commitment)
        }
    }

    /// Scan all commitments.
    pub fn commitment_all() -> Self {
        Self::new(Entity::Commitment)
    }

    /// Scan commitments by namespace index.
    pub fn commitment_by_namespace(namespace: Namespace) -> Self {
        Self {
            namespace: Some(namespace),
            ..Self::new(Entity::Commitment)
        }
    }

    /// Scan commitments by time range index.
    pub fn commitment_by_time_range(start: u64, end: u64) -> Self {
        Self {
            time_start: Some(start),
            time_end: Some(end),
            ..Self::new(Entity::Commitment)
        }
    }
}
