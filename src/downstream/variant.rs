use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;

use super::blackhole::BlackholeDownstream;
use super::channel::ChannelDownstream;
use super::s3::S3Downstream;
use crate::config::DownstreamType;
use crate::traits::Downstream;
use crate::types::CommitmentResult;

/// Enum representing all possible downstream implementations.
pub enum DownstreamVariant {
    S3(S3Downstream),
    Blackhole(BlackholeDownstream),
    Channel(ChannelDownstream),
}

impl DownstreamVariant {
    /// Create a new downstream instance based on the specified type.
    pub fn new(downstream_type: DownstreamType) -> Self {
        match downstream_type {
            DownstreamType::S3 => {
                DownstreamVariant::S3(S3Downstream::new("rootsmith-results".to_string(), "us-east-1".to_string()))
            }
            DownstreamType::Blackhole => DownstreamVariant::Blackhole(BlackholeDownstream::new()),
        }
    }

    /// Create a channel downstream with a custom sender.
    pub fn new_channel(sender: AsyncSender<CommitmentResult>) -> Self {
        DownstreamVariant::Channel(ChannelDownstream::new(sender))
    }
}

#[async_trait]
impl Downstream for DownstreamVariant {
    fn name(&self) -> &'static str {
        match self {
            DownstreamVariant::S3(inner) => inner.name(),
            DownstreamVariant::Blackhole(inner) => inner.name(),
            DownstreamVariant::Channel(inner) => inner.name(),
        }
    }

    async fn handle(&self, result: &CommitmentResult) -> Result<()> {
        match self {
            DownstreamVariant::S3(inner) => inner.handle(result).await,
            DownstreamVariant::Blackhole(inner) => inner.handle(result).await,
            DownstreamVariant::Channel(inner) => inner.handle(result).await,
        }
    }
}
