use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncReceiver;
use kanal::AsyncSender;
use tokio::sync::Mutex;

use super::Downstream;
use crate::types::CommitmentResult;

/// Channel downstream that publishes commitment results to a kanal channel.
pub struct ChannelDownstream {
    tx: AsyncSender<CommitmentResult>,
    pub rx: Arc<Mutex<AsyncReceiver<CommitmentResult>>>,
}

impl Default for ChannelDownstream {
    fn default() -> Self {
        let (tx, rx) = kanal::unbounded_async::<CommitmentResult>();
        Self {
            tx,
            rx: Arc::new(Mutex::new(rx)),
        }
    }
}

#[async_trait]
impl Downstream for ChannelDownstream {
    async fn handle(&self, result: &CommitmentResult) -> Result<()> {
        self.tx
            .send(result.clone())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send to channel: {}", e))?;
        Ok(())
    }
}
