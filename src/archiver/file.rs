use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use anyhow::Result;
use async_trait::async_trait;

use crate::traits::ArchiveData;
use crate::traits::ArchiveStorage;
use crate::types::Namespace;

/// File system-based archive storage.
/// Archives data to the local file system as compressed JSON files.
pub struct FileArchive {
    directory: PathBuf,
    is_writing: AtomicBool,
}

impl FileArchive {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            is_writing: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl ArchiveStorage for FileArchive {
    fn name(&self) -> &'static str {
        "file-archive"
    }

    async fn archive(&self, _ns: Namespace, _timestamp: u64, data: ArchiveData) -> Result<String> {
        tracing::info!("File archive: would archive data {:?}", data);
        todo!("Implement file archiving logic");
    }

    async fn open(&self) -> Result<()> {
        tracing::info!("File archive: initializing directory {:?}", self.directory);
        // Open the directory, create if it doesn't exist
        // try write a temp file to ensure write access
        todo!("Implement directory initialization logic");
    }

    async fn close(&self) -> Result<()> {
        tracing::info!("File archive: closing archive storage");
        // Wait for any ongoing writes to finish
        // Clean up any temporary files if necessary
        todo!("Implement archive closing logic");
    }
}
