use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;

use crate::types::UpstreamData;

/// Trait for upstream data sources (websocket, Kafka, SQS, MQTT, etc.).
///
/// Implementations are responsible for producing `UpstreamData` into
/// the app's ingestion pipeline.
#[async_trait]
pub trait UpstreamConnector: Send + Sync {
    /// Human-readable connector name for logging.
    fn name(&self) -> &'static str;

    /// Open/start the connector with a channel to send data.
    ///
    /// Typical implementation:
    /// - spawn a thread / async task,
    /// - read from external source,
    /// - push `UpstreamData` into the provided channel.
    async fn open(&self, tx: AsyncSender<UpstreamData>) -> Result<()>;

    /// Close/stop the connector and release resources.
    async fn close(&self) -> Result<()>;
}
