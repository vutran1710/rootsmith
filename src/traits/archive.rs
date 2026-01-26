use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;

use crate::types::BatchCommitmentMeta;
use crate::types::IncomingRecord;
use crate::types::StoredProof;

/// Types of data that can be archived.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArchiveData {
    /// Commitment metadata.
    Commitment(BatchCommitmentMeta),
    /// Proof.
    Proof(StoredProof),
    /// Raw incoming record.
    Record(IncomingRecord),
    /// Batch of commitments.
    CommitmentBatch(Vec<BatchCommitmentMeta>),
    /// Batch of proofs.
    ProofBatch(Vec<StoredProof>),
    /// Batch of records.
    RecordBatch(Vec<IncomingRecord>),
}

#[async_trait]
pub trait ArchiveStorage: Send + Sync {
    /// Human-readable archive storage name for logging.
    fn name(&self) -> &'static str;

    /// Archive a single data item.
    async fn archive(&self, data: &ArchiveData) -> Result<String>;

    /// Initialize the archive storage (e.g., create buckets, directories).
    async fn open(&self) -> Result<()>;

    /// Close/cleanup the archive storage.
    async fn close(&self) -> Result<()>;
}
