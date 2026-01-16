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
use crate::types::Value32;

pub struct MerkleAccumulator {
    leaves: Vec<[u8; 32]>,
    key_to_index: HashMap<Key32, usize>, // Map keys to leaf indices
}

impl MerkleAccumulator {
    pub fn new() -> Self {
        Self {
            leaves: Vec::new(),
            key_to_index: HashMap::new(),
        }
    }

    #[inline]
    fn leaf_hash(key: &Key32, value: &[u8]) -> [u8; 32] {
        // Hash both key and value: H( key || value )
        let mut data = Vec::with_capacity(key.len() + value.len());
        data.extend_from_slice(key);
        data.extend_from_slice(value);
        Sha256::hash(&data)
    }
}

impl Default for MerkleAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Accumulator for MerkleAccumulator {
    fn id(&self) -> &'static str {
        "merkle"
    }

    async fn commit(
        &mut self,
        records: &[RawRecord],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        // Clear any existing state
        self.flush()?;

        // Add all records to the accumulator
        for record in records {
            let leaf = Self::leaf_hash(&record.key, &record.value);
            let index = self.leaves.len();
            self.leaves.push(leaf);
            self.key_to_index.insert(record.key, index);
        }

        // Build the root
        let root = self.build_root()?;

        // Generate proofs for all records
        let keys: Vec<Key32> = records.iter().map(|r| r.key).collect();
        let proofs_vec = self.prove_many(&keys)?;

        let mut proofs = HashMap::new();
        for (key, proof) in proofs_vec {
            if let Some(p) = proof {
                proofs.insert(key, p);
            }
        }

        // Get current timestamp
        let committed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before UNIX_EPOCH")
            .as_secs();

        // Create and send result via channel
        let result = CommitmentResult {
            commitment: root,
            proofs: Some(proofs),
            committed_at,
        };

        result_tx
            .send(result)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send commitment result: {}", e))?;

        Ok(())
    }

    fn verify_proof(
        &self,
        root: &[u8; 32],
        key: &Key32,
        value: &[u8],
        proof: Option<&Proof>,
    ) -> Result<bool> {
        let Some(proof) = proof else {
            return Ok(false);
        };

        // leaf = H(key || value)
        let mut cur = Self::leaf_hash(key, value);

        for node in &proof.nodes {
            if node.sibling.len() != 32 {
                return Ok(false);
            }

            let mut sib = [0u8; 32];
            sib.copy_from_slice(&node.sibling);

            let mut buf = [0u8; 64];
            if node.is_left {
                // sibling || cur
                buf[..32].copy_from_slice(&sib);
                buf[32..].copy_from_slice(&cur);
            } else {
                // cur || sibling
                buf[..32].copy_from_slice(&cur);
                buf[32..].copy_from_slice(&sib);
            }

            cur = Sha256::hash(&buf);
        }

        Ok(&cur == root)
    }

    // ===== Legacy Methods =====

    fn put(&mut self, key: Key32, value: Value32) -> Result<()> {
        let leaf = Self::leaf_hash(&key, &value);
        let index = self.leaves.len();
        self.leaves.push(leaf);
        self.key_to_index.insert(key, index);
        Ok(())
    }

    fn build_root(&self) -> Result<Vec<u8>> {
        if self.leaves.is_empty() {
            return Ok(vec![0u8; 32]);
        }
        // Build tree from leaves
        let tree = RsMerkleTree::<Sha256>::from_leaves(&self.leaves);
        match tree.root() {
            Some(root) => Ok(root.to_vec()),
            None => Ok(vec![0u8; 32]),
        }
    }

    fn flush(&mut self) -> Result<()> {
        self.leaves.clear();
        self.key_to_index.clear();
        Ok(())
    }

    fn prove(&self, key: &Key32) -> Result<Option<Proof>> {
        // Get index from key
        let index = match self.key_to_index.get(key).copied() {
            Some(i) => i,
            None => return Ok(None),
        };

        if index >= self.leaves.len() {
            return Ok(None);
        }

        let tree = RsMerkleTree::<Sha256>::from_leaves(&self.leaves);

        // Get proof from rs_merkle
        let merkle_proof = tree.proof(&[index]);

        // rs_merkle trả proof_hashes bottom-up
        // ta convert sang ProofNode với is_left (sibling nằm bên trái hay không)
        let mut idx = index;
        let nodes: Vec<ProofNode> = merkle_proof
            .proof_hashes()
            .iter()
            .map(|sib_hash| {
                // nếu idx là right-child (idx odd) => sibling nằm LEFT
                let is_left = (idx % 2) == 1;
                idx /= 2;

                ProofNode {
                    is_left,
                    sibling: sib_hash.to_vec(), // 32 bytes
                }
            })
            .collect();

        Ok(Some(Proof { nodes }))
    }

    fn prove_many(&self, keys: &[Key32]) -> Result<Vec<(Key32, Option<Proof>)>> {
        if self.leaves.is_empty() {
            return Ok(keys.iter().map(|k| (*k, None)).collect());
        }

        let tree = RsMerkleTree::<Sha256>::from_leaves(&self.leaves);

        let mut out = Vec::with_capacity(keys.len());
        for k in keys {
            let Some(&index) = self.key_to_index.get(k) else {
                out.push((*k, None));
                continue;
            };
            if index >= self.leaves.len() {
                out.push((*k, None));
                continue;
            }

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

            out.push((*k, Some(Proof { nodes })));
        }
        Ok(out)
    }

    fn verify_inclusion(&self, key: &Key32, value: &[u8]) -> Result<bool> {
        if value.len() != 32 {
            return Ok(false);
        }

        let idx = match self.key_to_index.get(key).copied() {
            Some(i) => i,
            None => return Ok(false),
        };

        let mut v = [0u8; 32];
        v.copy_from_slice(value);

        // Reconstruct leaf hash using the same scheme as in put(): H(key || value)
        let leaf = Self::leaf_hash(key, &v);

        Ok(self.leaves.get(idx) == Some(&leaf))
    }

    fn verify_non_inclusion(&self, _key: &Key32) -> Result<bool> {
        Ok(false)
    }
}
