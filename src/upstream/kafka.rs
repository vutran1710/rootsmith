use anyhow::Result;
use crate::types::Block;
use super::UpstreamSource;

pub struct KafkaSource {
    brokers: String,
    topic: String,
    connected: bool,
}

impl KafkaSource {
    pub fn new(brokers: String, topic: String) -> Self {
        Self {
            brokers,
            topic,
            connected: false,
        }
    }
}

impl UpstreamSource for KafkaSource {
    fn connect(&mut self) -> Result<()> {
        tracing::info!("Connecting to Kafka: {} topic: {}", self.brokers, self.topic);
        self.connected = true;
        Ok(())
    }

    fn receive_block(&mut self) -> Result<Option<Block>> {
        if !self.connected {
            anyhow::bail!("Not connected");
        }
        Ok(None)
    }

    fn disconnect(&mut self) -> Result<()> {
        tracing::info!("Disconnecting from Kafka");
        self.connected = false;
        Ok(())
    }
}
