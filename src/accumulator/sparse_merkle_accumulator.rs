use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use async_trait::async_trait;
use monotree::database::MemoryDB;
use monotree::hasher::Blake3 as MonotreeBlake3;
use monotree::hasher::Hasher;
use monotree::verify_proof as monotree_verify_proof;
use monotree::Hash;
use monotree::Monotree;

use crate::traits::accumulator::AccumulatorRecord;
use crate::traits::accumulator::CommitmentResult;
use crate::traits::Accumulator;
use crate::types::Key32;
use crate::types::Proof;
use crate::types::ProofNode;
use crate::types::Value32;

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

#[async_trait]
impl Accumulator for SparseMerkleAccumulator {
    fn id(&self) -> &'static str {
        "sparse-merkle"
    }

    fn commit_batch(&mut self, records: &[AccumulatorRecord]) -> Result<CommitmentResult> {
        // Clear any existing state
        self.flush()?;

        // Add all records to the accumulator
        for record in records {
            self.put(record.key, record.value)?;
        }

        // Build the root
        let root = self.build_root()?;

        // Generate proofs for all records
        let keys: Vec<Key32> = records.iter().map(|r| r.key).collect();
        let proofs_vec = self.prove_many(&keys)?;

        let mut proofs = std::collections::HashMap::new();
        for (key, proof) in proofs_vec {
            if let Some(p) = proof {
                proofs.insert(key, p);
            }
        }

        let committed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before UNIX_EPOCH")
            .as_secs();

        Ok(CommitmentResult {
            root,
            proofs: Some(proofs),
            committed_at,
        })
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

    // ===== Legacy Methods =====

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
