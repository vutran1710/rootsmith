use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;
use crate::types::IncomingRecord;

/// Trait for upstream data sources (websocket, Kafka, SQS, MQTT, etc.).
///
/// Implementations are responsible for producing `IncomingRecord`s into
/// the app's ingestion pipeline.
#[async_trait]
pub trait UpstreamConnector: Send + Sync {
    /// Human-readable connector name for logging.
    fn name(&self) -> &'static str;

    /// Open/start the connector with a channel to send records.
    ///
    /// Typical implementation:
    /// - spawn a thread / async task,
    /// - read from external source,
    /// - push `IncomingRecord`s into the provided channel.
    async fn open(&mut self, tx: AsyncSender<IncomingRecord>) -> Result<()>;

    /// Close/stop the connector and release resources.
    async fn close(&mut self) -> Result<()>;
}

/// Single leaf that gets fed into the accumulator.
#[derive(Debug, Clone, Copy)]
pub struct Leaf {
    pub key: crate::types::Key32,
    pub value: crate::types::Value32,
}
