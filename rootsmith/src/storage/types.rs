use crate::types::Key16;
use crate::types::Namespace;
use crate::types::Record;

use super::BatchId;
use super::BatchMetadata;
use super::BatchStatus;
use super::CommitmentId;
use super::StorageQueryFilter;
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
#[derive(Debug, Clone)]
pub struct Filter {
    pub entity: Entity,
    pub namespace: Option<Namespace>,
    pub key: Option<Key16>,
    pub timestamp: Option<u64>,
    pub batch_id: Option<BatchId>,
    pub commitment_id: Option<CommitmentId>,
    pub status: Option<BatchStatus>,
    pub time_start: Option<u64>,
    pub time_end: Option<u64>,
    pub query: Option<StorageQueryFilter>,
    pub all_versions: bool,
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
            status: None,
            time_start: None,
            time_end: None,
            query: None,
            all_versions: false,
        }
    }

    /// Find record by namespace, key, and timestamp.
    pub fn record(namespace: Namespace, key: Key16, timestamp: u64) -> Self {
        Self {
            namespace: Some(namespace),
            key: Some(key),
            timestamp: Some(timestamp),
            ..Self::new(Entity::Record)
        }
    }

    /// Find latest record by namespace and key.
    pub fn record_latest(namespace: Namespace, key: Key16) -> Self {
        Self {
            namespace: Some(namespace),
            key: Some(key),
            ..Self::new(Entity::Record)
        }
    }

    /// Find all versions of a record.
    pub fn record_all_versions(namespace: Namespace, key: Key16) -> Self {
        Self {
            namespace: Some(namespace),
            key: Some(key),
            all_versions: true,
            ..Self::new(Entity::Record)
        }
    }

    /// Find all records in a namespace.
    pub fn records_by_namespace(namespace: Namespace) -> Self {
        Self {
            namespace: Some(namespace),
            ..Self::new(Entity::Record)
        }
    }

    /// Find records matching a query filter.
    pub fn records_by_query(query: StorageQueryFilter) -> Self {
        Self {
            query: Some(query),
            ..Self::new(Entity::Record)
        }
    }

    /// Find records belonging to a batch.
    pub fn batch_records(batch_id: BatchId) -> Self {
        Self {
            batch_id: Some(batch_id),
            ..Self::new(Entity::Record)
        }
    }

    /// Find batch by ID.
    pub fn batch(batch_id: BatchId) -> Self {
        Self {
            batch_id: Some(batch_id),
            ..Self::new(Entity::Batch)
        }
    }

    /// Find all batches.
    pub fn batch_all() -> Self {
        Self::new(Entity::Batch)
    }

    /// Find batches by status.
    pub fn batch_by_status(status: BatchStatus) -> Self {
        Self {
            status: Some(status),
            ..Self::new(Entity::Batch)
        }
    }

    /// Find commitment by ID.
    pub fn commitment(commitment_id: CommitmentId) -> Self {
        Self {
            commitment_id: Some(commitment_id),
            ..Self::new(Entity::Commitment)
        }
    }

    /// Find all commitments.
    pub fn commitment_all() -> Self {
        Self::new(Entity::Commitment)
    }

    /// Find commitments by namespace.
    pub fn commitment_by_namespace(namespace: Namespace) -> Self {
        Self {
            namespace: Some(namespace),
            ..Self::new(Entity::Commitment)
        }
    }

    /// Find commitments by time range.
    pub fn commitment_by_time_range(start: u64, end: u64) -> Self {
        Self {
            time_start: Some(start),
            time_end: Some(end),
            ..Self::new(Entity::Commitment)
        }
    }
}
