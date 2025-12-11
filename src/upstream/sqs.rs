use anyhow::Result;
use crate::types::Block;
use super::UpstreamSource;

pub struct SqsSource {
    queue_url: String,
    region: String,
    connected: bool,
}

impl SqsSource {
    pub fn new(queue_url: String, region: String) -> Self {
        Self {
            queue_url,
            region,
            connected: false,
        }
    }
}

impl UpstreamSource for SqsSource {
    fn connect(&mut self) -> Result<()> {
        tracing::info!("Connecting to SQS: {} region: {}", self.queue_url, self.region);
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
        tracing::info!("Disconnecting from SQS");
        self.connected = false;
        Ok(())
    }
}
