use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

use crate::traits::ArchiveData;
use crate::traits::ArchiveStorage;

/// File system-based archive storage.
/// Archives data to the local file system as compressed JSON files.
pub struct FileArchive {
    directory: PathBuf,
}

impl FileArchive {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
}

#[async_trait]
impl ArchiveStorage for FileArchive {
    fn name(&self) -> &'static str {
        "file-archive"
    }

    async fn archive(&self, data: &ArchiveData) -> Result<String> {
        tracing::info!("File archive: archiving data {:?}", data);

        todo!("Implement file archiving logic");
    }

    async fn open(&self) -> Result<()> {
        tracing::info!("File archive: initializing directory {:?}", self.directory);

        todo!("Implement directory creation");
    }

    async fn close(&self) -> Result<()> {
        tracing::info!("File archive: closing archive storage");

        todo!("Implement any necessary cleanup");
    }
}
