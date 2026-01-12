use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

use crate::traits::ProofDelivery;
use crate::types::StoredProof;

/// File-based proof delivery.
/// Writes proofs to the file system as JSON files.
pub struct FileDelivery {
    directory: PathBuf,
}

impl FileDelivery {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
}

#[async_trait]
impl ProofDelivery for FileDelivery {
    fn name(&self) -> &'static str {
        "file"
    }

    async fn deliver(&self, proof: &StoredProof) -> Result<()> {
        let filename = format!("proof_{}.json", hex::encode(&proof.key));
        let filepath = self.directory.join(filename);

        tracing::info!(
            "File delivery: would write proof for key {:?} to {:?}",
            hex::encode(&proof.key),
            filepath
        );

        // TODO: Implement actual file writing
        // Example:
        // std::fs::create_dir_all(&self.directory)?;
        // let json = serde_json::to_string_pretty(proof)?;
        // tokio::fs::write(&filepath, json).await?;

        Ok(())
    }

    async fn deliver_batch(&self, proofs: &[StoredProof]) -> Result<()> {
        tracing::info!(
            "File delivery: would write batch of {} proofs to {:?}",
            proofs.len(),
            self.directory
        );

        // TODO: Could write as a single batch file or individual files
        for proof in proofs {
            self.deliver(proof).await?;
        }
        Ok(())
    }

    async fn open(&mut self) -> Result<()> {
        tracing::info!(
            "File delivery: initialized for directory {:?}",
            self.directory
        );
        // Could create directory if it doesn't exist
        // std::fs::create_dir_all(&self.directory)?;
        Ok(())
    }
}
