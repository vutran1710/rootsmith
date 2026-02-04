//! Storage operation enums for pattern-matched dispatch.

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
    Records(Vec<Record>),
    Batch(BatchMetadata),
    Commitment(StoredCommitment),
}

/// Keys for retrieving items from the database.
#[derive(Debug, Clone)]
pub enum Retrievable {
    /// Get record by namespace, key, and timestamp
    Record {
        namespace: Namespace,
        key: Key16,
        timestamp: u64,
    },
    /// Get latest record by namespace and key
    RecordLatest {
        namespace: Namespace,
        key: Key16,
    },
    /// Get all versions of a record
    RecordAllVersions {
        namespace: Namespace,
        key: Key16,
    },
    /// Get all records in a namespace
    RecordsByNamespace(Namespace),
    /// Get records matching a filter
    RecordsByFilter(StorageQueryFilter),
    /// Get batch by ID
    Batch(BatchId),
    /// Get all batches
    BatchAll,
    /// Get batches by status
    BatchByStatus(BatchStatus),
    /// Get records for a batch
    BatchRecords(BatchId),
    /// Get commitment by ID
    Commitment(CommitmentId),
    /// Get all commitments
    CommitmentAll,
    /// Get commitments by namespace
    CommitmentByNamespace(Namespace),
    /// Get commitments by time range
    CommitmentByTimeRange { start: u64, end: u64 },
}

/// Retrieved items from the database.
#[derive(Debug, Clone)]
pub enum Retrieved {
    Record(Option<Record>),
    Records(Vec<Record>),
    Batch(Option<BatchMetadata>),
    Batches(Vec<BatchMetadata>),
    Commitment(Option<StoredCommitment>),
    Commitments(Vec<(CommitmentId, StoredCommitment)>),
}

/// Keys for deleting items from the database.
#[derive(Debug, Clone)]
pub enum Deletable {
    /// Delete records matching filter
    Records(StorageQueryFilter),
    /// Delete batch by ID
    Batch(BatchId),
    /// Delete commitment by ID
    Commitment(CommitmentId),
}

/// Update operations for batches.
#[derive(Debug, Clone)]
pub enum Updatable {
    /// Update batch status
    BatchStatus {
        batch_id: BatchId,
        status: BatchStatus,
        timestamp: u64,
    },
    /// Update batch record count
    BatchRecordCount {
        batch_id: BatchId,
        count: u64,
        timestamp: u64,
    },
    /// Mark batch as committed
    BatchCommitted {
        batch_id: BatchId,
        commitment_id: CommitmentId,
        timestamp: u64,
    },
}
