use anyhow::Result;
use async_trait::async_trait;

use super::file::FileDelivery;
use super::kafka::KafkaDelivery;
use super::mock::MockDelivery;
use super::noop::NoopDelivery;
use super::webhook::WebhookDelivery;
use crate::traits::ProofDelivery;
use crate::types::StoredProof;

/// Enum representing all possible proof delivery implementations.
pub enum ProofDeliveryVariant {
    Noop(NoopDelivery),
    Mock(MockDelivery),
    Kafka(KafkaDelivery),
    Webhook(WebhookDelivery),
    File(FileDelivery),
}

#[async_trait]
impl ProofDelivery for ProofDeliveryVariant {
    fn name(&self) -> &'static str {
        match self {
            ProofDeliveryVariant::Noop(inner) => inner.name(),
            ProofDeliveryVariant::Mock(inner) => inner.name(),
            ProofDeliveryVariant::Kafka(inner) => inner.name(),
            ProofDeliveryVariant::Webhook(inner) => inner.name(),
            ProofDeliveryVariant::File(inner) => inner.name(),
        }
    }

    async fn deliver(&self, proof: &StoredProof) -> Result<()> {
        match self {
            ProofDeliveryVariant::Noop(inner) => inner.deliver(proof).await,
            ProofDeliveryVariant::Mock(inner) => inner.deliver(proof).await,
            ProofDeliveryVariant::Kafka(inner) => inner.deliver(proof).await,
            ProofDeliveryVariant::Webhook(inner) => inner.deliver(proof).await,
            ProofDeliveryVariant::File(inner) => inner.deliver(proof).await,
        }
    }

    async fn deliver_batch(&self, proofs: &[StoredProof]) -> Result<()> {
        match self {
            ProofDeliveryVariant::Noop(inner) => inner.deliver_batch(proofs).await,
            ProofDeliveryVariant::Mock(inner) => inner.deliver_batch(proofs).await,
            ProofDeliveryVariant::Kafka(inner) => inner.deliver_batch(proofs).await,
            ProofDeliveryVariant::Webhook(inner) => inner.deliver_batch(proofs).await,
            ProofDeliveryVariant::File(inner) => inner.deliver_batch(proofs).await,
        }
    }

    async fn open(&mut self) -> Result<()> {
        match self {
            ProofDeliveryVariant::Noop(inner) => inner.open().await,
            ProofDeliveryVariant::Mock(inner) => inner.open().await,
            ProofDeliveryVariant::Kafka(inner) => inner.open().await,
            ProofDeliveryVariant::Webhook(inner) => inner.open().await,
            ProofDeliveryVariant::File(inner) => inner.open().await,
        }
    }

    async fn close(&mut self) -> Result<()> {
        match self {
            ProofDeliveryVariant::Noop(inner) => inner.close().await,
            ProofDeliveryVariant::Mock(inner) => inner.close().await,
            ProofDeliveryVariant::Kafka(inner) => inner.close().await,
            ProofDeliveryVariant::Webhook(inner) => inner.close().await,
            ProofDeliveryVariant::File(inner) => inner.close().await,
        }
    }
}
