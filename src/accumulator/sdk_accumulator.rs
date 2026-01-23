use std::collections::HashMap;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;
use rs_merkle::algorithms::Sha256;
use rs_merkle::Hasher;
use rs_merkle::MerkleTree as RsMerkleTree;

use crate::traits::Accumulator;
use crate::types::CommitmentResult;
use crate::types::Key32;
use crate::types::Proof;
use crate::types::ProofNode;
use crate::types::RawRecord;

/// SDK trait for accumulator integration.
///
/// This trait represents the interface that WASM plugins provide
/// and accumulators consume directly. It combines metadata and data
/// extraction methods.
pub trait SDKTrait {
    fn namespace(&self) -> [u8; 32];
    fn key(&self) -> [u8; 32];
    fn value(&self) -> [u8; 32];
    fn timestamp(&self) -> u64;
}

/// SDK accumulator that accepts trait objects directly from WASM plugins.
///
/// This accumulator follows the pattern from the test example where
/// trait objects are passed directly without intermediate conversions.
pub struct SdkAccumulator {
    leaves: Vec<[u8; 32]>,
    key_to_index: HashMap<Key32, usize>,
}

impl SdkAccumulator {
    pub fn new() -> Self {
        Self {
            leaves: Vec::new(),
            key_to_index: HashMap::new(),
        }
    }

    #[inline]
    fn leaf_hash(key: &Key32, value: &[u8]) -> [u8; 32] {
        let mut data = Vec::with_capacity(key.len() + value.len());
        data.extend_from_slice(key);
        data.extend_from_slice(value);
        Sha256::hash(&data)
    }

    fn flush(&mut self) -> Result<()> {
        self.leaves.clear();
        self.key_to_index.clear();
        Ok(())
    }

    fn build_root(&self) -> Result<Vec<u8>> {
        if self.leaves.is_empty() {
            return Err(anyhow::anyhow!("Cannot build root: no leaves"));
        }

        let tree = RsMerkleTree::<Sha256>::from_leaves(&self.leaves);
        let root = tree.root();
        Ok(root.map(|r| r.to_vec()).unwrap_or_default())
    }

    fn prove(&self, key: &Key32) -> Result<Option<Proof>> {
        let Some(&index) = self.key_to_index.get(key) else {
            return Ok(None);
        };

        if self.leaves.is_empty() || index >= self.leaves.len() {
            return Ok(None);
        }

        let tree = RsMerkleTree::<Sha256>::from_leaves(&self.leaves);
        let merkle_proof = tree.proof(&[index]);

        let mut idx = index;
        let nodes: Vec<ProofNode> = merkle_proof
            .proof_hashes()
            .iter()
            .map(|sib_hash| {
                let is_left = (idx % 2) == 1;
                idx /= 2;
                ProofNode {
                    is_left,
                    sibling: sib_hash.to_vec(),
                }
            })
            .collect();

        Ok(Some(Proof { nodes }))
    }

    pub fn build(&mut self, data: Box<dyn SDKTrait>) -> Result<()> {
        let namespace = data.namespace();
        let key = data.key();
        let value = data.value();
        let timestamp = data.timestamp();

        let leaf = Self::leaf_hash(&key, &value);
        let index = self.leaves.len();
        self.leaves.push(leaf);
        self.key_to_index.insert(key, index);

        Ok(())
    }

    pub async fn commit_trait(
        &mut self,
        records: &[Box<dyn SDKTrait>],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        self.flush()?;

        for trait_obj in records {
            let key = trait_obj.key();
            let value = trait_obj.value();
            let leaf = Self::leaf_hash(&key, &value);
            let index = self.leaves.len();
            self.leaves.push(leaf);
            self.key_to_index.insert(key, index);
        }

        let root = self.build_root()?;

        let mut proofs = HashMap::new();
        for trait_obj in records {
            let key = trait_obj.key();
            if let Some(proof) = self.prove(&key)? {
                proofs.insert(key, proof);
            }
        }

        let committed_at = if !records.is_empty() {
            records[0].timestamp()
        } else {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("System time before UNIX_EPOCH")
                .as_secs()
        };

        let result = CommitmentResult {
            commitment: root,
            proofs: Some(proofs),
            committed_at,
        };

        result_tx.send(result).await
            .map_err(|e| anyhow::anyhow!("Failed to send commitment result: {}", e))?;

        Ok(())
    }
}

impl Default for SdkAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Accumulator for SdkAccumulator {
    fn id(&self) -> &'static str {
        "sdk"
    }

    async fn commit(
        &mut self,
        records: &[RawRecord],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        self.flush()?;

        for record in records {
            let leaf = Self::leaf_hash(&record.key, &record.value);
            let index = self.leaves.len();
            self.leaves.push(leaf);
            self.key_to_index.insert(record.key, index);
        }

        let root = self.build_root()?;

        let keys: Vec<Key32> = records.iter().map(|r| r.key).collect();
        let mut proofs = HashMap::new();
        for key in keys {
            if let Some(proof) = self.prove(&key)? {
                proofs.insert(key, proof);
            }
        }

        let committed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before UNIX_EPOCH")
            .as_secs();

        let result = CommitmentResult {
            commitment: root,
            proofs: Some(proofs),
            committed_at,
        };

        result_tx.send(result).await
            .map_err(|e| anyhow::anyhow!("Failed to send commitment result: {}", e))?;

        Ok(())
    }
}

use crate::wasm_host::traits::{RecordMeta, ToStandardData};
use crate::wasm_host::wrapper::StandardWrapper;

/// Helper to convert Box<dyn ToStandardData> to Box<dyn SDKTrait>
/// by extracting data and wrapping in StandardWrapper which implements SDKTrait
pub fn to_sdk_trait(data: Box<dyn ToStandardData>) -> Box<dyn SDKTrait> {
    let namespace = data.namespace();
    let key = data.key();
    let value = data.value();
    let timestamp = data.timestamp();
    
    Box::new(StandardWrapper {
        namespace,
        key,
        value,
        timestamp,
    }) as Box<dyn SDKTrait>
}

impl<T: ToStandardData> SDKTrait for T {
    fn namespace(&self) -> [u8; 32] {
        RecordMeta::namespace(self)
    }

    fn key(&self) -> [u8; 32] {
        RecordMeta::key(self)
    }

    fn value(&self) -> [u8; 32] {
        ToStandardData::value(self)
    }

    fn timestamp(&self) -> u64 {
        RecordMeta::timestamp(self)
    }
}
