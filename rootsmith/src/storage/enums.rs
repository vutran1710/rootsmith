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
    Batch(BatchMetadata),
    Commitment(StoredCommitment),
}

/// Query for finding items in the database.
/// Used by both `get` and `delete` operations.
#[derive(Debug, Clone)]
pub enum Filter {
    /// Find record by namespace, key, and timestamp
    Record {
        namespace: Namespace,
        key: Key16,
        timestamp: u64,
    },
    /// Find latest record by namespace and key
    RecordLatest {
        namespace: Namespace,
        key: Key16,
    },
    /// Find all versions of a record
    RecordAllVersions {
        namespace: Namespace,
        key: Key16,
    },
    /// Find all records in a namespace
    RecordsByNamespace(Namespace),
    /// Find records matching a filter
    RecordsByFilter(StorageQueryFilter),
    /// Find batch by ID
    Batch(BatchId),
    /// Find all batches
    BatchAll,
    /// Find batches by status
    BatchByStatus(BatchStatus),
    /// Find records for a batch
    BatchRecords(BatchId),
    /// Find commitment by ID
    Commitment(CommitmentId),
    /// Find all commitments
    CommitmentAll,
    /// Find commitments by namespace
    CommitmentByNamespace(Namespace),
    /// Find commitments by time range
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
