use anyhow::Result;
use async_trait::async_trait;

use crate::types::StoredProof;

/// Trait for proof delivery mechanisms (Kafka, webhooks, message queues, etc.).
///
/// Implementations are responsible for delivering `StoredProof`s to downstream
/// consumers after they have been generated and persisted to the proof registry.
#[async_trait]
pub trait ProofDelivery: Send + Sync {
    /// Human-readable delivery mechanism name for logging.
    fn name(&self) -> &'static str;

    /// Deliver a single proof to downstream consumers.
    ///
    /// This could push to a message queue, send to a webhook, write to a file, etc.
    async fn deliver(&self, proof: &StoredProof) -> Result<()>;

    /// Deliver multiple proofs in a batch, if supported.
    ///
    /// Default implementation delivers proofs one by one, but implementations
    /// can override this for more efficient batch delivery.
    async fn deliver_batch(&self, proofs: &[StoredProof]) -> Result<()> {
        for proof in proofs {
            self.deliver(proof).await?;
        }
        Ok(())
    }

    /// Open/initialize the delivery mechanism (e.g., establish connections).
    ///
    /// Called once before any delivery operations.
    async fn open(&mut self) -> Result<()> {
        // Default: no-op
        Ok(())
    }

    /// Close/cleanup the delivery mechanism (e.g., close connections).
    ///
    /// Called when shutting down.
    async fn close(&mut self) -> Result<()> {
        // Default: no-op
        Ok(())
    }
}
