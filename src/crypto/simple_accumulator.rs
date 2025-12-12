use anyhow::Result;
use crate::types::{Key32, Value32};
use crate::traits::Accumulator;

/// Simple XOR-based accumulator for demonstration.
pub struct SimpleAccumulator {
    root: Vec<u8>,
}

impl SimpleAccumulator {
    pub fn new() -> Self {
        Self {
            root: vec![0u8; 32],
        }
    }
}

impl Default for SimpleAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Accumulator for SimpleAccumulator {
    fn id(&self) -> &'static str {
        "simple-accumulator"
    }
    
    fn put(&mut self, key: Key32, value: Value32) -> Result<()> {
        // Simple XOR-based accumulation
        for (i, &byte) in key.iter().enumerate() {
            self.root[i] ^= byte;
        }
        for (i, &byte) in value.iter().enumerate() {
            self.root[i] ^= byte;
        }
        Ok(())
    }
    
    fn build_root(&self) -> Result<Vec<u8>> {
        Ok(self.root.clone())
    }
    
    fn verify_inclusion(&self, _key: &Key32, _value: &[u8]) -> Result<bool> {
        Ok(true)
    }
    
    fn verify_non_inclusion(&self, _key: &Key32) -> Result<bool> {
        Ok(true)
    }
    
    fn flush(&mut self) -> Result<()> {
        self.root = vec![0u8; 32];
        Ok(())
    }
}
