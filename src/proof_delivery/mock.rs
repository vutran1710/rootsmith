use crate::traits::ProofDelivery;
use crate::types::StoredProof;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

/// Mock proof delivery for testing.
/// Stores delivered proofs in memory for verification.
#[derive(Clone)]
pub struct MockDelivery {
    pub delivered: Arc<Mutex<Vec<StoredProof>>>,
}

impl MockDelivery {
    pub fn new() -> Self {
        Self {
            delivered: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get all delivered proofs (for testing/verification).
    pub fn get_delivered(&self) -> Vec<StoredProof> {
        self.delivered.lock().unwrap().clone()
    }

    /// Clear delivered proofs.
    pub fn clear(&self) {
        self.delivered.lock().unwrap().clear();
    }
}

impl Default for MockDelivery {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProofDelivery for MockDelivery {
    fn name(&self) -> &'static str {
        "mock-delivery"
    }

    async fn deliver(&self, proof: &StoredProof) -> Result<()> {
        self.delivered.lock().unwrap().push(proof.clone());
        tracing::debug!(
            "MockDelivery: delivered proof for key {:?}",
            hex::encode(&proof.key)
        );
        Ok(())
    }

    async fn deliver_batch(&self, proofs: &[StoredProof]) -> Result<()> {
        let mut delivered = self.delivered.lock().unwrap();
        for proof in proofs {
            delivered.push(proof.clone());
        }
        tracing::debug!("MockDelivery: delivered batch of {} proofs", proofs.len());
        Ok(())
    }
}

