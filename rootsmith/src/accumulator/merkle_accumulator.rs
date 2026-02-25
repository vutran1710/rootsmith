use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;
use rs_merkle::algorithms::Sha256;
use rs_merkle::Hasher;
use rs_merkle::MerkleTree as RsMerkleTree;
use tokio::sync::Mutex;

use super::Accumulator;
use crate::types::Commitment;
use crate::types::CommitmentResult;
use crate::types::Key16;
use crate::types::Record;

#[derive(Default)]
pub struct MerkleAccumulator {
    leaves: Arc<Mutex<Vec<[u8; 32]>>>,
}

impl MerkleAccumulator {
    #[inline]
    fn leaf_hash(key: &Key16, value: &[u8]) -> [u8; 32] {
        // Hash both key and value: H( key || value )
        let mut data = Vec::with_capacity(key.len() + value.len());
        data.extend_from_slice(key);
        data.extend_from_slice(value);
        Sha256::hash(&data)
    }

    fn build_root(leaves: &[[u8; 32]]) -> Result<Vec<u8>> {
        let tree = RsMerkleTree::<Sha256>::from_leaves(leaves);
        let root = tree.root().unwrap_or_default();
        Ok(root.to_vec())
    }
}

#[async_trait]
impl Accumulator for MerkleAccumulator {
    async fn commit(
        &self,
        records: &[Record],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<Option<String>> {
        let mut leaves = self.leaves.lock().await;

        // Add all records to the accumulator
        for record in records {
            let leaf = Self::leaf_hash(&record.key, &record.value.as_bytes());
            leaves.push(leaf);
        }

        // Build the root
        let root = Self::build_root(&leaves)?;

        // Get current timestamp
        let committed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before UNIX_EPOCH")
            .as_secs();

        // Create and send result via channel
        let commitment = Commitment {
            root: root.clone(),
            committed_at,
            namespaces: records.iter().map(|r| r.namespace.clone()).collect(),
        };
        let result = CommitmentResult {
            commitment,
            item_count: records.len() as u64,
            timestamp: committed_at,
            proofs: HashMap::new(), // Proofs not implemented in this simple Merkle tree
            meta: serde_json::json!({}),
        };

        result_tx
            .send(result)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send commitment result: {}", e))?;

        // Local accumulator produces immediate result, no job_id needed
        Ok(None)
    }
}
