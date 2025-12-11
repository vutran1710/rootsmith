use anyhow::Result;
use std::collections::HashMap;

pub struct SparseMerkleTree {
    nodes: HashMap<Vec<u8>, Vec<u8>>,
    depth: usize,
}

impl SparseMerkleTree {
    pub fn new(depth: usize) -> Self {
        Self {
            nodes: HashMap::new(),
            depth,
        }
    }

    pub fn update(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        self.nodes.insert(key, value);
        Ok(())
    }

    pub fn get(&self, key: &[u8]) -> Option<&Vec<u8>> {
        self.nodes.get(key)
    }

    pub fn root(&self) -> Result<Vec<u8>> {
        let mut result = vec![0u8; 32];
        for (key, value) in &self.nodes {
            for (i, byte) in key.iter().chain(value.iter()).enumerate() {
                result[i % 32] ^= byte;
            }
        }
        Ok(result)
    }

    pub fn generate_proof(&self, key: &[u8]) -> Result<Vec<Vec<u8>>> {
        let mut proof = Vec::new();
        if let Some(value) = self.nodes.get(key) {
            proof.push(value.clone());
        }
        Ok(proof)
    }
}

impl Default for SparseMerkleTree {
    fn default() -> Self {
        Self::new(256)
    }
}
