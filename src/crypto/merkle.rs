use anyhow::Result;

pub struct MerkleTree {
    leaves: Vec<Vec<u8>>,
}

impl MerkleTree {
    pub fn new() -> Self {
        Self {
            leaves: Vec::new(),
        }
    }

    pub fn add_leaf(&mut self, data: Vec<u8>) {
        self.leaves.push(data);
    }

    pub fn root(&self) -> Result<Vec<u8>> {
        if self.leaves.is_empty() {
            return Ok(vec![0u8; 32]);
        }

        let mut current_level = self.leaves.clone();

        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            for chunk in current_level.chunks(2) {
                let hash = if chunk.len() == 2 {
                    self.hash_pair(&chunk[0], &chunk[1])
                } else {
                    chunk[0].clone()
                };
                next_level.push(hash);
            }

            current_level = next_level;
        }

        Ok(current_level[0].clone())
    }

    fn hash_pair(&self, left: &[u8], right: &[u8]) -> Vec<u8> {
        let mut combined = left.to_vec();
        combined.extend_from_slice(right);
        let mut result = vec![0u8; 32];
        for (i, byte) in combined.iter().enumerate() {
            result[i % 32] ^= byte;
        }
        result
    }

    pub fn generate_proof(&self, index: usize) -> Result<Vec<Vec<u8>>> {
        if index >= self.leaves.len() {
            anyhow::bail!("Index out of bounds");
        }

        let mut proof = Vec::new();
        let mut current_level = self.leaves.clone();
        let mut current_index = index;

        while current_level.len() > 1 {
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            if sibling_index < current_level.len() {
                proof.push(current_level[sibling_index].clone());
            }

            let mut next_level = Vec::new();
            for chunk in current_level.chunks(2) {
                let hash = if chunk.len() == 2 {
                    self.hash_pair(&chunk[0], &chunk[1])
                } else {
                    chunk[0].clone()
                };
                next_level.push(hash);
            }

            current_level = next_level;
            current_index /= 2;
        }

        Ok(proof)
    }
}

impl Default for MerkleTree {
    fn default() -> Self {
        Self::new()
    }
}
