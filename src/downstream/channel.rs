use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;

use crate::traits::Downstream;
use crate::types::CommitmentResult;

/// Channel downstream that publishes commitment results to a kanal channel.
pub struct ChannelDownstream {
    sender: AsyncSender<CommitmentResult>,
}

impl ChannelDownstream {
    pub fn new(sender: AsyncSender<CommitmentResult>) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl Downstream for ChannelDownstream {
    fn name(&self) -> &'static str {
        "channel"
    }

    async fn handle(&self, result: &CommitmentResult) -> Result<()> {
        self.sender
            .send(result.clone())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send to channel: {}", e))?;
        Ok(())
    }
}
