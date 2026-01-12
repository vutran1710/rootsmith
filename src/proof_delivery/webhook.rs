use anyhow::Result;
use async_trait::async_trait;

use crate::traits::ProofDelivery;
use crate::types::StoredProof;

/// Webhook-based proof delivery.
/// POSTs proofs to an HTTP endpoint for downstream consumers.
pub struct WebhookDelivery {
    url: String,
    auth_token: Option<String>,
}

impl WebhookDelivery {
    pub fn new(url: String) -> Self {
        Self {
            url,
            auth_token: None,
        }
    }

    pub fn with_auth(url: String, token: String) -> Self {
        Self {
            url,
            auth_token: Some(token),
        }
    }
}

#[async_trait]
impl ProofDelivery for WebhookDelivery {
    fn name(&self) -> &'static str {
        "webhook"
    }

    async fn deliver(&self, proof: &StoredProof) -> Result<()> {
        tracing::info!(
            "Webhook delivery: would POST proof for key {:?} to {}",
            hex::encode(&proof.key),
            self.url
        );
        // TODO: Implement actual HTTP POST with reqwest or similar
        // Example:
        // let client = reqwest::Client::new();
        // let mut request = client.post(&self.url).json(proof);
        // if let Some(token) = &self.auth_token {
        //     request = request.bearer_auth(token);
        // }
        // let response = request.send().await?;
        // response.error_for_status()?;
        Ok(())
    }

    async fn deliver_batch(&self, proofs: &[StoredProof]) -> Result<()> {
        tracing::info!(
            "Webhook delivery: would POST batch of {} proofs to {}",
            proofs.len(),
            self.url
        );
        // TODO: Implement batch HTTP POST
        // Could send all proofs in a single JSON array
        // let client = reqwest::Client::new();
        // client.post(&self.url).json(&proofs).send().await?;

        // For now, deliver individually
        for proof in proofs {
            self.deliver(proof).await?;
        }
        Ok(())
    }

    async fn open(&mut self) -> Result<()> {
        tracing::info!("Webhook delivery: initialized for URL {}", self.url);
        // Could validate URL, test connection, etc.
        Ok(())
    }
}
