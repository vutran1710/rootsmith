use anyhow::Result;
use crate::types::{Key32, Value32};
use crate::traits::Accumulator;

/// Mock accumulator for testing.
pub struct MockAccumulator {
    pub leaves: std::collections::HashMap<Key32, Value32>,
}

impl MockAccumulator {
    pub fn new() -> Self {
        Self {
            leaves: std::collections::HashMap::new(),
        }
    }
}

impl Accumulator for MockAccumulator {
    fn id(&self) -> &'static str {
        "mock-accumulator"
    }

    fn put(&mut self, key: Key32, value: Value32) -> Result<()> {
        self.leaves.insert(key, value);
        Ok(())
    }

    fn build_root(&self) -> Result<Vec<u8>> {
        // Simple hash: XOR all keys and values together
        let mut root = vec![0u8; 32];
        
        for (key, value) in &self.leaves {
            for (i, &byte) in key.iter().enumerate() {
                root[i] ^= byte;
            }
            for (i, &byte) in value.iter().enumerate() {
                root[i] ^= byte;
            }
        }

        Ok(root)
    }

    fn verify_inclusion(&self, key: &Key32, value: &[u8]) -> Result<bool> {
        Ok(self.leaves.get(key).map(|v| v.as_slice() == value).unwrap_or(false))
    }

    fn verify_non_inclusion(&self, key: &Key32) -> Result<bool> {
        Ok(!self.leaves.contains_key(key))
    }

    fn flush(&mut self) -> Result<()> {
        self.leaves.clear();
        Ok(())
    }
}
