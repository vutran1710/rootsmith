use crate::traits::Accumulator;
use crate::types::{Key32, Proof, ProofNode, Value32};
use anyhow::Result;
use monotree::database::MemoryDB;
use monotree::hasher::Blake3 as MonotreeBlake3;
use monotree::hasher::Hasher;
use monotree::{verify_proof as monotree_verify_proof, Hash, Monotree};
use std::sync::Mutex;

/// Sparse Merkle tree based accumulator using monotree library.
pub struct SparseMerkleAccumulator {
    tree: Mutex<Monotree<MemoryDB>>,
    root: Mutex<Option<Hash>>,
}

impl SparseMerkleAccumulator {
    pub fn new() -> Self {
        Self {
            tree: Mutex::new(Monotree::default()),
            root: Mutex::new(None),
        }
    }

    /// leaf = H(key || value) (32 bytes)  -- dùng blake3 crate để hash bytes
    #[inline]
    fn leaf_hash(key: &Key32, value: &Value32) -> Hash {
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(key);
        buf[32..].copy_from_slice(value);

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

impl Accumulator for SparseMerkleAccumulator {
    fn id(&self) -> &'static str {
        "sparse-merkle"
    }

    fn put(&mut self, key: Key32, value: Value32) -> Result<()> {
        let key_hash = Hash::from(key);
        let leaf = Self::leaf_hash(&key, &value);

        let mut tree = self.tree.lock().unwrap();
        let mut root = self.root.lock().unwrap();

        let new_root = tree
            .insert(root.as_ref(), &key_hash, &leaf)
            .map_err(|e| anyhow::anyhow!("Failed to insert into tree: {:?}", e))?;

        *root = new_root;
        Ok(())
    }

    fn build_root(&self) -> Result<Vec<u8>> {
        let root = self.root.lock().unwrap();
        Ok(match &*root {
            Some(r) => r.as_ref().to_vec(),
            None => vec![0u8; 32],
        })
    }

    fn prove(&self, key: &Key32) -> Result<Option<Proof>> {
        let key_hash = Hash::from(*key);

        let mut tree = self.tree.lock().unwrap();
        let root = self.root.lock().unwrap();

        if root.is_none() {
            return Ok(None);
        }

        // ✅ yêu cầu: nếu get_merkle_proof lỗi => return Ok(None)
        let monotree_proof = match tree.get_merkle_proof(root.as_ref(), &key_hash) {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };

        let Some(monotree_proof) = monotree_proof else {
            return Ok(None);
        };

        let nodes: Vec<ProofNode> = monotree_proof
            .into_iter()
            .map(|(is_left, sibling)| ProofNode { is_left, sibling })
            .collect();

        Ok(Some(Proof { nodes }))
    }

    fn verify_proof(
        &self,
        root: &[u8; 32],
        key: &Key32,
        value: &Value32,
        proof: Option<&Proof>,
    ) -> Result<bool> {
        let Some(proof) = proof else {
            return Ok(false);
        };

        // root == 0 => coi như empty tree (tuỳ convention)
        let root_hash = if root.iter().all(|&b| b == 0) {
            None
        } else {
            Some(Hash::from(*root))
        };

        // leaf = H(key||value)
        let leaf_hash = Self::leaf_hash(key, value);
        for (depth, node) in proof.nodes.iter().enumerate() {
            if depth >= 256 {
                return Ok(false);
            }

            let bit = Self::key_bit_msb(key, depth);
            let expected_is_left = bit; // bit=1 => sibling LEFT => is_left=true

            if node.is_left != expected_is_left {
                return Ok(false);
            }
        }

        // Convert custom Proof -> monotree::Proof (Vec<(bool, Vec<u8>)>)
        let monotree_proof: Vec<(bool, Vec<u8>)> = proof
            .nodes
            .iter()
            .map(|n| (n.is_left, n.sibling.clone()))
            .collect();

        // Verify bằng monotree (hashing internal nodes)
        let hasher = MonotreeBlake3::new();
        let ok = monotree_verify_proof(
            &hasher,
            root_hash.as_ref(),
            &leaf_hash,
            Some(&monotree_proof),
        );

        Ok(ok)
    }

    fn verify_inclusion(&self, key: &Key32, value: &[u8]) -> Result<bool> {
        if value.len() != 32 {
            return Ok(false);
        }

        let key_hash = Hash::from(*key);

        let mut tree = self.tree.lock().unwrap();
        let root = self.root.lock().unwrap();

        let stored_leaf = tree
            .get(root.as_ref(), &key_hash)
            .map_err(|e| anyhow::anyhow!("Failed to get from tree: {:?}", e))?;

        let Some(stored_leaf) = stored_leaf else {
            return Ok(false);
        };

        let mut v = [0u8; 32];
        v.copy_from_slice(value);

        let expected_leaf = Self::leaf_hash(key, &v);
        Ok(stored_leaf == expected_leaf)
    }

    fn verify_non_inclusion(&self, key: &Key32) -> Result<bool> {
        let key_hash = Hash::from(*key);

        let mut tree = self.tree.lock().unwrap();
        let root = self.root.lock().unwrap();

        let stored_value = tree
            .get(root.as_ref(), &key_hash)
            .map_err(|e| anyhow::anyhow!("Failed to get from tree: {:?}", e))?;

        Ok(stored_value.is_none())
    }

    fn flush(&mut self) -> Result<()> {
        let mut tree = self.tree.lock().unwrap();
        let mut root = self.root.lock().unwrap();

        *tree = Monotree::default();
        *root = None;
        Ok(())
    }
}