use crate::traits::{ArchiveData, ArchiveFilter, ArchiveStorage};
use anyhow::Result;
use async_trait::async_trait;

/// Noop archive storage that doesn't archive anything.
/// Useful for testing or when archiving is not needed.
pub struct NoopArchive;

#[async_trait]
impl ArchiveStorage for NoopArchive {
    fn name(&self) -> &'static str {
        "noop-archive"
    }

    async fn archive(&self, _data: &ArchiveData) -> Result<String> {
        // Return a dummy ID
        Ok("noop-id".to_string())
    }

    async fn retrieve(&self, _archive_id: &str) -> Result<Option<ArchiveData>> {
        // Always return None (nothing archived)
        Ok(None)
    }

    async fn query(&self, _filter: &ArchiveFilter) -> Result<Vec<String>> {
        // No archives
        Ok(Vec::new())
    }
}

