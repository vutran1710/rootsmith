use anyhow::Result;
use arrow::array::RecordBatch;
use async_trait::async_trait;

use crate::types::Namespace;

/// Types of data that can be archived.
#[derive(Debug, Clone)]
pub enum ArchiveData {
    Json(serde_json::Value),
    Binary(Vec<u8>),
    Arrow(RecordBatch),
}

#[async_trait]
pub trait ArchiveStorage: Send + Sync {
    /// Human-readable archive storage name for logging.
    fn name(&self) -> &'static str;

    /// Archive a single data item.
    async fn archive(&self, ns: Namespace, timestamp: u64, data: ArchiveData) -> Result<String>;

    /// Initialize the archive storage (e.g., create buckets, directories).
    async fn open(&self) -> Result<()>;

    /// Close/cleanup the archive storage.
    async fn close(&self) -> Result<()>;
}
