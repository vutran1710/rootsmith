use crate::traits::Accumulator;
use crate::types::{Key32, Value32};
use anyhow::Result;
use monotree::database::MemoryDB;
use monotree::{Hash, Monotree};
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
        // Convert key and value to Hash type
        let key_hash = Hash::from(key);
        let value_hash = Hash::from(value);

        // Insert into sparse merkle tree and update root
        let mut tree = self.tree.lock().unwrap();
        let mut root = self.root.lock().unwrap();

        let new_root = tree
            .insert(root.as_ref(), &key_hash, &value_hash)
            .map_err(|e| anyhow::anyhow!("Failed to insert into tree: {:?}", e))?;

        *root = new_root;

        Ok(())
    }

    fn build_root(&self) -> Result<Vec<u8>> {
        let root = self.root.lock().unwrap();
        match &*root {
            Some(root_hash) => Ok(root_hash.as_ref().to_vec()),
            None => Ok(vec![0u8; 32]),
        }
    }

    fn verify_inclusion(&self, key: &Key32, value: &[u8]) -> Result<bool> {
        let key_hash = Hash::from(*key);

        // Get the value from the tree
        let mut tree = self.tree.lock().unwrap();
        let root = self.root.lock().unwrap();

        let stored_value = tree
            .get(root.as_ref(), &key_hash)
            .map_err(|e| anyhow::anyhow!("Failed to get from tree: {:?}", e))?;

        match stored_value {
            Some(stored_hash) => {
                // Compare stored value with provided value
                if value.len() == 32 {
                    let value_array: [u8; 32] = value
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("Invalid value length"))?;
                    let value_hash = Hash::from(value_array);
                    Ok(stored_hash == value_hash)
                } else {
                    Ok(false)
                }
            }
            None => Ok(false),
        }
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
        // Create a new tree and reset root
        let mut tree = self.tree.lock().unwrap();
        let mut root = self.root.lock().unwrap();

        *tree = Monotree::default();
        *root = None;
        Ok(())
    }
}
