use super::{file::FileArchive, mock::MockArchive, noop::NoopArchive, s3_glacier::S3GlacierArchive};
use crate::traits::{ArchiveData, ArchiveFilter, ArchiveStorage};
use anyhow::Result;
use async_trait::async_trait;

/// Enum representing all possible archive storage implementations.
pub enum ArchiveStorageVariant {
    Noop(NoopArchive),
    Mock(MockArchive),
    S3Glacier(S3GlacierArchive),
    File(FileArchive),
}

#[async_trait]
impl ArchiveStorage for ArchiveStorageVariant {
    fn name(&self) -> &'static str {
        match self {
            ArchiveStorageVariant::Noop(inner) => inner.name(),
            ArchiveStorageVariant::Mock(inner) => inner.name(),
            ArchiveStorageVariant::S3Glacier(inner) => inner.name(),
            ArchiveStorageVariant::File(inner) => inner.name(),
        }
    }

    async fn archive(&self, data: &ArchiveData) -> Result<String> {
        match self {
            ArchiveStorageVariant::Noop(inner) => inner.archive(data).await,
            ArchiveStorageVariant::Mock(inner) => inner.archive(data).await,
            ArchiveStorageVariant::S3Glacier(inner) => inner.archive(data).await,
            ArchiveStorageVariant::File(inner) => inner.archive(data).await,
        }
    }

    async fn archive_batch(&self, items: &[ArchiveData]) -> Result<Vec<String>> {
        match self {
            ArchiveStorageVariant::Noop(inner) => inner.archive_batch(items).await,
            ArchiveStorageVariant::Mock(inner) => inner.archive_batch(items).await,
            ArchiveStorageVariant::S3Glacier(inner) => inner.archive_batch(items).await,
            ArchiveStorageVariant::File(inner) => inner.archive_batch(items).await,
        }
    }

    async fn retrieve(&self, archive_id: &str) -> Result<Option<ArchiveData>> {
        match self {
            ArchiveStorageVariant::Noop(inner) => inner.retrieve(archive_id).await,
            ArchiveStorageVariant::Mock(inner) => inner.retrieve(archive_id).await,
            ArchiveStorageVariant::S3Glacier(inner) => inner.retrieve(archive_id).await,
            ArchiveStorageVariant::File(inner) => inner.retrieve(archive_id).await,
        }
    }

    async fn query(&self, filter: &ArchiveFilter) -> Result<Vec<String>> {
        match self {
            ArchiveStorageVariant::Noop(inner) => inner.query(filter).await,
            ArchiveStorageVariant::Mock(inner) => inner.query(filter).await,
            ArchiveStorageVariant::S3Glacier(inner) => inner.query(filter).await,
            ArchiveStorageVariant::File(inner) => inner.query(filter).await,
        }
    }

    async fn delete(&self, archive_id: &str) -> Result<bool> {
        match self {
            ArchiveStorageVariant::Noop(inner) => inner.delete(archive_id).await,
            ArchiveStorageVariant::Mock(inner) => inner.delete(archive_id).await,
            ArchiveStorageVariant::S3Glacier(inner) => inner.delete(archive_id).await,
            ArchiveStorageVariant::File(inner) => inner.delete(archive_id).await,
        }
    }

    async fn open(&mut self) -> Result<()> {
        match self {
            ArchiveStorageVariant::Noop(inner) => inner.open().await,
            ArchiveStorageVariant::Mock(inner) => inner.open().await,
            ArchiveStorageVariant::S3Glacier(inner) => inner.open().await,
            ArchiveStorageVariant::File(inner) => inner.open().await,
        }
    }

    async fn close(&mut self) -> Result<()> {
        match self {
            ArchiveStorageVariant::Noop(inner) => inner.close().await,
            ArchiveStorageVariant::Mock(inner) => inner.close().await,
            ArchiveStorageVariant::S3Glacier(inner) => inner.close().await,
            ArchiveStorageVariant::File(inner) => inner.close().await,
        }
    }
}

