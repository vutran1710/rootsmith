use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use async_trait::async_trait;
use base64::prelude::*;
use flate2::write::GzEncoder;
use flate2::Compression;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use super::ArchiveData;
use super::ArchiveStorage;
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

    fn ns_to_hex(ns: &Namespace) -> String {
        hex::encode(ns)
    }

    fn serialize_to_json_gz(data: &ArchiveData) -> Result<Vec<u8>> {
        let json_bytes = match data {
            ArchiveData::Json(json) => serde_json::to_vec(json)?,
            ArchiveData::Binary(bin) => {
                let wrapper = serde_json::json!({
                    "format": "binary",
                    "data": BASE64_STANDARD.encode(bin)
                });
                serde_json::to_vec(&wrapper)?
            }
            ArchiveData::Arrow(batch) => {
                let buf = Vec::new();
                let mut writer = arrow_json::ArrayWriter::new(buf);
                writer.write(batch)?;
                writer.finish()?;
                writer.into_inner()
            }
        };

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&json_bytes)?;
        Ok(encoder.finish()?)
    }
}

#[async_trait]
impl ArchiveStorage for FileArchive {
    async fn archive(&self, ns: Namespace, timestamp: u64, data: ArchiveData) -> Result<String> {
        self.is_writing.store(true, Ordering::SeqCst);

        let ns_hex = Self::ns_to_hex(&ns);
        let subdir = self.directory.join(&ns_hex);
        fs::create_dir_all(&subdir).await?;

        let filename = format!("{}.json.gz", timestamp);
        let path = subdir.join(&filename);

        let compressed = Self::serialize_to_json_gz(&data)?;

        let mut file = fs::File::create(&path).await?;
        file.write_all(&compressed).await?;
        file.sync_all().await?;

        self.is_writing.store(false, Ordering::SeqCst);

        let path_str = path.to_string_lossy().into_owned();
        tracing::info!(
            "File archive: archived ns={} ts={} -> {} ({} bytes)",
            ns_hex,
            timestamp,
            path_str,
            compressed.len()
        );
        Ok(path_str)
    }

    async fn open(&self) -> Result<()> {
        tracing::info!("File archive: initializing directory {:?}", self.directory);

        fs::create_dir_all(&self.directory).await?;

        // Verify write access by creating a marker file
        let marker = self.directory.join(".rootsmith-writable");
        fs::write(&marker, b"").await?;
        fs::remove_file(&marker).await.ok();

        tracing::info!("File archive: directory ready at {:?}", self.directory);
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        tracing::info!("File archive: closing archive storage");

        // Spin briefly if a write is in progress (best-effort)
        for _ in 0..100 {
            if !self.is_writing.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        tracing::info!("File archive: closed");
        Ok(())
    }
}
