use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;
use monotree::database::MemoryDB;
use monotree::Hash;
use monotree::Monotree;
use serde::Deserialize;
use serde::Serialize;

use crate::traits::Accumulator;
use crate::types::Commitment;
use crate::types::CommitmentResult;
use crate::types::Key32;
use crate::types::Record;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ProofNode {
    pub is_left: bool,
    pub sibling: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Proof {
    pub nodes: Vec<ProofNode>,
}

/// Sparse Merkle tree based accumulator using monotree library.
pub struct SparseMerkleAccumulator {
    tree: Mutex<Monotree<MemoryDB>>,
    root: Mutex<Hash>,
}

impl SparseMerkleAccumulator {
    pub fn new() -> Self {
        Self {
            tree: Mutex::new(Monotree::default()),
            root: Mutex::new(Hash::default()),
        }
    }

    /// leaf = H(key || value) (32 bytes)  -- dùng blake3 crate để hash bytes
    #[inline]
    fn leaf_hash(key: &Key32, value: &[u8]) -> Hash {
        let mut buf = Vec::with_capacity(key.len() + value.len());
        buf.extend_from_slice(key);
        buf.extend_from_slice(value);

        let out = blake3::hash(&buf); // 32 bytes
        let mut arr = [0u8; 32];
        arr.copy_from_slice(out.as_bytes());
        Hash::from(arr)
    }

    #[inline]
    fn key_bit_msb(key: &Key32, depth: usize) -> bool {
        let byte = key[depth / 8];
        let bit = 7 - (depth % 8);
        ((byte >> bit) & 1) == 1
    }
}

impl Default for SparseMerkleAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Accumulator for SparseMerkleAccumulator {
    fn accumulator_type(&self) -> crate::config::AccumulatorType {
        crate::config::AccumulatorType::SparseMerkle
    }

    async fn commit(
        &self,
        records: &[Record],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        let mut tree = Monotree::default();
        let mut root = Hash::default();
        let mut proofs = HashMap::new();

        for record in records {
            let key_hash = Hash::from(record.key);
            let leaf = Self::leaf_hash(&record.key, &record.value.as_bytes());

            let new_root = tree
                .insert(Some(&root), &key_hash, &leaf)
                .map_err(|e| anyhow::anyhow!("Failed to insert into tree: {:?}", e))?;

            root = new_root.expect("empty root");
        }

        // TODO: Generate proofs for each key

        // Get current timestamp
        let committed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before UNIX_EPOCH")
            .as_secs();

        // Create and send result via channel
        let commitment = Commitment {
            namespaces: records.iter().map(|r| r.namespace.clone()).collect(),
            root: root.to_vec(),
            committed_at,
        };
        let result = CommitmentResult {
            commitment,
            item_count: records.len() as u64,
            timestamp: committed_at,
            proofs: proofs
                .into_iter()
                .map(|(k, v)| (k, serde_json::to_vec(&v).unwrap_or_default()))
                .collect(),
            meta: serde_json::json!({}),
        };

        result_tx
            .send(result)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send commitment result: {}", e))?;

        Ok(())
    }
}
