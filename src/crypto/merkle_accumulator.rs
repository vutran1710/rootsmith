use crate::traits::Accumulator;
use crate::types::{Key32, Proof, ProofNode, Value32};
use anyhow::Result;
use rs_merkle::{algorithms::Sha256, Hasher, MerkleTree as RsMerkleTree};
use std::collections::HashMap;
use std::sync::Mutex;

pub struct MerkleAccumulator {
    leaves: Mutex<Vec<[u8; 32]>>,
    tree: Mutex<Option<RsMerkleTree<Sha256>>>,
    key_to_index: Mutex<HashMap<Key32, usize>>, // key -> leaf index
}

impl MerkleAccumulator {
    pub fn new() -> Self {
        Self {
            leaves: Mutex::new(Vec::new()),
            tree: Mutex::new(None),
            key_to_index: Mutex::new(HashMap::new()),
        }
    }

    #[inline]
    fn leaf_hash(value: &Value32) -> [u8; 32] {
        // value là raw 32 bytes -> hash trực tiếp
        Sha256::hash(value)
    }

    /// Ensure self.tree is built from current leaves (lazy cache).
    fn ensure_tree(&self) -> Result<()> {
        // lock theo thứ tự cố định để tránh deadlock
        let leaves = self.leaves.lock().unwrap();
        let mut tree_guard = self.tree.lock().unwrap();

        if tree_guard.is_none() {
            if leaves.is_empty() {
                *tree_guard = None;
            } else {
                *tree_guard = Some(RsMerkleTree::<Sha256>::from_leaves(&leaves));
            }
        }
        Ok(())
    }

    /// Read-only access to built tree (assumes ensure_tree called).
    fn with_tree<R>(&self, f: impl FnOnce(&RsMerkleTree<Sha256>) -> R) -> Result<Option<R>> {
        let tree_guard = self.tree.lock().unwrap();
        Ok(tree_guard.as_ref().map(f))
    }
}

impl Default for MerkleAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Accumulator for MerkleAccumulator {
    fn id(&self) -> &'static str {
        "merkle"
    }

    fn put(&mut self, key: Key32, value: Value32) -> Result<()> {
        let leaf = Self::leaf_hash(&value);

        let mut leaves = self.leaves.lock().unwrap();
        let mut key_to_index = self.key_to_index.lock().unwrap();
        let mut tree = self.tree.lock().unwrap();

        let index = leaves.len();
        leaves.push(leaf);
        key_to_index.insert(key, index);

        // invalidate cached tree
        *tree = None;

        Ok(())
    }

    fn build_root(&self) -> Result<Vec<u8>> {
        self.ensure_tree()?;

        let root = self
            .with_tree(|t| t.root())?
            .flatten()
            .unwrap_or([0u8; 32]);

        Ok(root.to_vec())
    }

    fn flush(&mut self) -> Result<()> {
        let mut leaves = self.leaves.lock().unwrap();
        let mut key_to_index = self.key_to_index.lock().unwrap();
        let mut tree = self.tree.lock().unwrap();

        leaves.clear();
        key_to_index.clear();
        *tree = None;

        Ok(())
    }

    fn prove(&self, key: &Key32) -> Result<Option<Proof>> {
        // lấy index từ key
        let index = {
            let map = self.key_to_index.lock().unwrap();
            match map.get(key).copied() {
                Some(i) => i,
                None => return Ok(None),
            }
        };

        // ensure tree cache
        self.ensure_tree()?;

        // lấy proof từ rs_merkle
        let merkle_proof = match self.with_tree(|t| t.proof(&[index]))? {
            Some(p) => p,
            None => return Ok(None),
        };

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

    fn verify_proof(
        &self,
        root: &[u8; 32],
        value: &[u8; 32], // raw value
        proof: Option<&Proof>,
    ) -> Result<bool> {
        let Some(proof) = proof else {
            return Ok(false);
        };

        // leaf = H(value) (không cần key)
        let mut cur = Sha256::hash(value);

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

    fn verify_inclusion(&self, key: &Key32, value: &[u8]) -> Result<bool> {
        if value.len() != 32 {
            return Ok(false);
        }

        let idx = {
            let map = self.key_to_index.lock().unwrap();
            match map.get(key).copied() {
                Some(i) => i,
                None => return Ok(false),
            }
        };

        let mut v = [0u8; 32];
        v.copy_from_slice(value);

        let leaf = Sha256::hash(&v);

        let leaves = self.leaves.lock().unwrap();
        Ok(leaves.get(idx) == Some(&leaf))
    }

    fn verify_non_inclusion(&self, _key: &Key32) -> Result<bool> {
        // Merkle thường không support non-inclusion proof
        Ok(false)
    }
}