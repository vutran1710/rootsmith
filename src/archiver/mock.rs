use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use async_trait::async_trait;

use crate::traits::ArchiveData;
use crate::traits::ArchiveFilter;
use crate::traits::ArchiveStorage;

/// Mock archive storage for testing.
/// Stores archived data in memory with auto-generated IDs.
#[derive(Clone)]
pub struct MockArchive {
    pub archives: Arc<Mutex<HashMap<String, ArchiveData>>>,
    next_id: Arc<Mutex<u64>>,
}

impl MockArchive {
    pub fn new() -> Self {
        Self {
            archives: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    /// Get all archived data (for testing/verification).
    pub fn get_all(&self) -> HashMap<String, ArchiveData> {
        self.archives.lock().unwrap().clone()
    }

    /// Get archive count.
    pub fn count(&self) -> usize {
        self.archives.lock().unwrap().len()
    }

    /// Clear all archives.
    pub fn clear(&self) {
        self.archives.lock().unwrap().clear();
        *self.next_id.lock().unwrap() = 1;
    }
}

impl Default for MockArchive {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ArchiveStorage for MockArchive {
    fn name(&self) -> &'static str {
        "mock-archive"
    }

    async fn archive(&self, data: &ArchiveData) -> Result<String> {
        let id = {
            let mut next = self.next_id.lock().unwrap();
            let id = *next;
            *next += 1;
            format!("archive_{:08}", id)
        };

        self.archives
            .lock()
            .unwrap()
            .insert(id.clone(), data.clone());

        tracing::debug!("MockArchive: archived data with ID {}", id);
        Ok(id)
    }

    async fn archive_batch(&self, items: &[ArchiveData]) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        for item in items {
            let id = self.archive(item).await?;
            ids.push(id);
        }
        tracing::debug!("MockArchive: archived batch of {} items", items.len());
        Ok(ids)
    }

    async fn retrieve(&self, archive_id: &str) -> Result<Option<ArchiveData>> {
        let archives = self.archives.lock().unwrap();
        Ok(archives.get(archive_id).cloned())
    }

    async fn query(&self, filter: &ArchiveFilter) -> Result<Vec<String>> {
        let archives = self.archives.lock().unwrap();

        // Simple filtering - could be enhanced
        let matching_ids: Vec<String> = archives
            .iter()
            .filter(|(_, data)| {
                match (filter.namespace.as_ref(), data) {
                    // Filter by namespace if provided
                    (Some(ns), ArchiveData::Commitment(meta)) => meta.commitment.namespace == *ns,
                    (Some(ns), ArchiveData::Record(rec)) => rec.namespace == *ns,
                    (Some(_), ArchiveData::Proof(_)) => true, // Proofs don't have direct namespace
                    (Some(ns), ArchiveData::CommitmentBatch(batch)) => {
                        batch.iter().any(|m| m.commitment.namespace == *ns)
                    }
                    (Some(ns), ArchiveData::RecordBatch(batch)) => {
                        batch.iter().any(|r| r.namespace == *ns)
                    }
                    (Some(_), ArchiveData::ProofBatch(_)) => true,
                    (None, _) => true, // No namespace filter
                }
            })
            .map(|(id, _)| id.clone())
            .collect();

        Ok(matching_ids)
    }

    async fn delete(&self, archive_id: &str) -> Result<bool> {
        let mut archives = self.archives.lock().unwrap();
        Ok(archives.remove(archive_id).is_some())
    }
}
