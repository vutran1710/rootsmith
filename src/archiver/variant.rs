use anyhow::Result;
use async_trait::async_trait;

use super::file::FileArchive;
use super::s3_glacier::S3GlacierArchive;
use crate::traits::ArchiveData;
use crate::traits::ArchiveStorage;

/// Enum representing all possible archive storage implementations.
pub enum ArchiveStorageVariant {
    S3Glacier(S3GlacierArchive),
    File(FileArchive),
}

#[async_trait]
impl ArchiveStorage for ArchiveStorageVariant {
    fn name(&self) -> &'static str {
        match self {
            ArchiveStorageVariant::S3Glacier(inner) => inner.name(),
            ArchiveStorageVariant::File(inner) => inner.name(),
        }
    }

    async fn archive(&self, data: &ArchiveData) -> Result<String> {
        match self {
            ArchiveStorageVariant::S3Glacier(inner) => inner.archive(data).await,
            ArchiveStorageVariant::File(inner) => inner.archive(data).await,
        }
    }

    async fn open(&self) -> Result<()> {
        match self {
            ArchiveStorageVariant::S3Glacier(inner) => inner.open().await,
            ArchiveStorageVariant::File(inner) => inner.open().await,
        }
    }

    async fn close(&self) -> Result<()> {
        match self {
            ArchiveStorageVariant::S3Glacier(inner) => inner.close().await,
            ArchiveStorageVariant::File(inner) => inner.close().await,
        }
    }
}
