use anyhow::Result;
use crate::types::Block;
use super::UpstreamSource;

pub struct MqttSource {
    broker: String,
    topic: String,
    connected: bool,
}

impl MqttSource {
    pub fn new(broker: String, topic: String) -> Self {
        Self {
            broker,
            topic,
            connected: false,
        }
    }
}

impl UpstreamSource for MqttSource {
    fn connect(&mut self) -> Result<()> {
        tracing::info!("Connecting to MQTT: {} topic: {}", self.broker, self.topic);
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
        tracing::info!("Disconnecting from MQTT");
        self.connected = false;
        Ok(())
    }
}
