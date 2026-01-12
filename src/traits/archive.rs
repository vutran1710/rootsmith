use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;

use crate::types::BatchCommitmentMeta;
use crate::types::IncomingRecord;
use crate::types::StoredProof;

/// Filter options for querying archived data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveFilter {
    /// Optional namespace filter.
    pub namespace: Option<[u8; 32]>,
    /// Optional time range (start, end) in unix seconds.
    pub time_range: Option<(u64, u64)>,
}

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

/// Trait for archiving data to cold storage (S3 Glacier, file system, tape, etc.).
///
/// Implementations are responsible for:
/// - Long-term storage of old commitments, proofs, and raw data
/// - Efficient retrieval when needed
/// - Data compression and cost optimization
#[async_trait]
pub trait ArchiveStorage: Send + Sync {
    /// Human-readable archive storage name for logging.
    fn name(&self) -> &'static str;

    /// Archive a single data item.
    async fn archive(&self, data: &ArchiveData) -> Result<String>;

    /// Archive multiple data items in a batch.
    ///
    /// Returns a list of archive IDs for the archived items.
    /// Default implementation archives items one by one.
    async fn archive_batch(&self, items: &[ArchiveData]) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        for item in items {
            let id = self.archive(item).await?;
            ids.push(id);
        }
        Ok(ids)
    }

    /// Retrieve archived data by ID.
    async fn retrieve(&self, archive_id: &str) -> Result<Option<ArchiveData>>;

    /// Query archived data by filter.
    ///
    /// Returns a list of archive IDs matching the filter.
    async fn query(&self, filter: &ArchiveFilter) -> Result<Vec<String>> {
        // Default: not supported
        let _ = filter;
        Ok(Vec::new())
    }

    /// Delete archived data by ID.
    ///
    /// Returns true if data was deleted, false if not found.
    async fn delete(&self, archive_id: &str) -> Result<bool> {
        // Default: not supported
        let _ = archive_id;
        Ok(false)
    }

    /// Initialize the archive storage (e.g., create buckets, directories).
    async fn open(&mut self) -> Result<()> {
        // Default: no-op
        Ok(())
    }

    /// Close/cleanup the archive storage.
    async fn close(&mut self) -> Result<()> {
        // Default: no-op
        Ok(())
    }
}
