use crate::traits::UpstreamConnector;
use crate::types::IncomingRecord;
use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;

pub struct KafkaSource {
    brokers: String,
    topic: String,
}

impl KafkaSource {
    pub fn new(brokers: String, topic: String) -> Self {
        Self { brokers, topic }
    }
}

#[async_trait]
impl UpstreamConnector for KafkaSource {
    fn name(&self) -> &'static str {
        "kafka"
    }

    async fn open(&mut self, _tx: AsyncSender<IncomingRecord>) -> Result<()> {
        tracing::info!(
            "Opening Kafka connection: {} topic: {}",
            self.brokers,
            self.topic
        );
        // TODO: Implement actual Kafka connection
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        tracing::info!("Closing Kafka connection");
        // TODO: Implement actual Kafka disconnection
        Ok(())
    }
}
