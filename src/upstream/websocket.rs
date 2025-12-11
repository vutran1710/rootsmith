use anyhow::Result;
use crate::types::Block;
use super::UpstreamSource;

pub struct WebSocketSource {
    url: String,
    connected: bool,
}

impl WebSocketSource {
    pub fn new(url: String) -> Self {
        Self {
            url,
            connected: false,
        }
    }
}

impl UpstreamSource for WebSocketSource {
    fn connect(&mut self) -> Result<()> {
        tracing::info!("Connecting to WebSocket: {}", self.url);
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
        tracing::info!("Disconnecting from WebSocket");
        self.connected = false;
        Ok(())
    }
}
