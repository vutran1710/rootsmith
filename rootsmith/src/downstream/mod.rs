pub mod blackhole;
pub mod channel;
pub mod s3;

use anyhow::Result;
use async_trait::async_trait;
use blackhole::BlackholeDownstream;
#[cfg(test)]
use channel::ChannelDownstream;
use s3::S3Downstream;
use serde::Deserialize;
use serde::Serialize;

use crate::types::CommitmentResult;

#[async_trait]
pub trait Downstream: Send + Sync {
    /// Downstream name for logging and metrics.
    fn name(&self) -> &'static str {
        unimplemented!()
    }

    /// Handle a commitment result containing multiple namespace commitments and optional proofs.
    async fn handle(&self, result: &CommitmentResult) -> Result<()>;
}

/// Enum representing all possible downstream implementations.
pub enum DownstreamVariant {
    S3(S3Downstream),
    Blackhole(BlackholeDownstream),
    #[cfg(test)]
    Channel(ChannelDownstream),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownstreamConfig {
    S3 {
        bucket: String,
        region: String,
        api_key: Option<String>,
    },
    Blackhole,
    #[cfg(test)]
    Channel,
}

impl DownstreamVariant {
    /// Create a new downstream instance based on the specified type.
    pub fn new(config: DownstreamConfig) -> Self {
        match config {
            DownstreamConfig::S3 {
                bucket,
                region,
                api_key,
            } => DownstreamVariant::S3(S3Downstream::new(bucket, region, api_key)),
            DownstreamConfig::Blackhole => DownstreamVariant::Blackhole(BlackholeDownstream::new()),
            #[cfg(test)]
            DownstreamConfig::Channel => DownstreamVariant::Channel(ChannelDownstream::default()),
        }
    }
}

#[async_trait]
impl Downstream for DownstreamVariant {
    fn name(&self) -> &'static str {
        match self {
            DownstreamVariant::S3(_) => "s3-downstream",
            DownstreamVariant::Blackhole(_) => "blackhole-downstream",
            #[cfg(test)]
            DownstreamVariant::Channel(_) => "channel-downstream",
        }
    }

    async fn handle(&self, result: &CommitmentResult) -> Result<()> {
        match self {
            DownstreamVariant::S3(inner) => inner.handle(result).await,
            DownstreamVariant::Blackhole(inner) => inner.handle(result).await,
            #[cfg(test)]
            DownstreamVariant::Channel(inner) => inner.handle(result).await,
        }
    }
}
