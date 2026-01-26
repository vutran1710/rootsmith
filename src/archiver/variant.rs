use anyhow::Result;
use async_trait::async_trait;

use super::file::FileArchive;
use super::s3_glacier::S3GlacierArchive;
use crate::traits::ArchiveData;
use crate::traits::ArchiveStorage;

/// Enum representing all possible archive storage implementations.
pub enum ArchiveVariant {
    S3Glacier(S3GlacierArchive),
    File(FileArchive),
}

#[async_trait]
impl ArchiveStorage for ArchiveVariant {
    fn name(&self) -> &'static str {
        match self {
            ArchiveVariant::S3Glacier(inner) => inner.name(),
            ArchiveVariant::File(inner) => inner.name(),
        }
    }

    async fn archive(
        &self,
        ns: crate::types::Namespace,
        timestamp: u64,
        data: ArchiveData,
    ) -> Result<String> {
        match self {
            ArchiveVariant::S3Glacier(inner) => inner.archive(ns, timestamp, data).await,
            ArchiveVariant::File(inner) => inner.archive(ns, timestamp, data).await,
        }
    }

    async fn open(&self) -> Result<()> {
        match self {
            ArchiveVariant::S3Glacier(inner) => inner.open().await,
            ArchiveVariant::File(inner) => inner.open().await,
        }
    }

    async fn close(&self) -> Result<()> {
        match self {
            ArchiveVariant::S3Glacier(inner) => inner.close().await,
            ArchiveVariant::File(inner) => inner.close().await,
        }
    }
}
