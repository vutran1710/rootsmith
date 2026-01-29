pub mod file;
pub mod s3_glacier;

use anyhow::Result;
use arrow::array::RecordBatch;
use async_trait::async_trait;
use file::FileArchive;
use s3_glacier::S3GlacierArchive;
use serde::Deserialize;
use serde::Serialize;

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
    fn name(&self) -> &'static str {
        unimplemented!()
    }

    /// Archive a single data item.
    async fn archive(&self, ns: Namespace, timestamp: u64, data: ArchiveData) -> Result<String>;

    /// Initialize the archive storage (e.g., create buckets, directories).
    async fn open(&self) -> Result<()>;

    /// Close/cleanup the archive storage.
    async fn close(&self) -> Result<()>;
}

/// Enum representing all possible archive storage implementations.
pub enum ArchiveVariant {
    S3(S3GlacierArchive),
    File(FileArchive),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveConfig {
    S3 {
        bucket: String,
        region: String,
        prefix: Option<String>,
    },
    File {
        directory: String,
    },
}

impl ArchiveVariant {
    pub fn new(config: ArchiveConfig) -> Self {
        match config {
            ArchiveConfig::S3 {
                bucket,
                region,
                prefix,
            } => {
                let archive = if let Some(p) = prefix {
                    S3GlacierArchive::with_prefix(bucket, region, p)
                } else {
                    S3GlacierArchive::new(bucket, region)
                };
                ArchiveVariant::S3(archive)
            }
            ArchiveConfig::File { directory } => {
                let archive = FileArchive::new(std::path::PathBuf::from(directory));
                ArchiveVariant::File(archive)
            }
        }
    }
}

#[async_trait]
impl ArchiveStorage for ArchiveVariant {
    fn name(&self) -> &'static str {
        match self {
            ArchiveVariant::S3(_) => "s3-glacier-archive",
            ArchiveVariant::File(_) => "file-archive",
        }
    }

    async fn archive(
        &self,
        ns: crate::types::Namespace,
        timestamp: u64,
        data: ArchiveData,
    ) -> Result<String> {
        match self {
            ArchiveVariant::S3(inner) => inner.archive(ns, timestamp, data).await,
            ArchiveVariant::File(inner) => inner.archive(ns, timestamp, data).await,
        }
    }

    async fn open(&self) -> Result<()> {
        match self {
            ArchiveVariant::S3(inner) => inner.open().await,
            ArchiveVariant::File(inner) => inner.open().await,
        }
    }

    async fn close(&self) -> Result<()> {
        match self {
            ArchiveVariant::S3(inner) => inner.close().await,
            ArchiveVariant::File(inner) => inner.close().await,
        }
    }
}
