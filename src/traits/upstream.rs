use anyhow::Result;

/// Trait for upstream data sources (websocket, Kafka, SQS, MQTT, etc.).
///
/// Implementations are responsible for producing `IncomingRecord`s into
/// the app's ingestion pipeline.
pub trait UpstreamConnector: Send + Sync {
    /// Human-readable connector name for logging.
    fn name(&self) -> &'static str;

    /// Open/start the connector.
    ///
    /// Typical implementation:
    /// - spawn a thread / async task,
    /// - read from external source,
    /// - push `IncomingRecord`s into a channel owned by the app.
    fn open(&mut self) -> Result<()>;

    /// Close/stop the connector and release resources.
    fn close(&mut self) -> Result<()>;
}

/// Single leaf that gets fed into the accumulator.
#[derive(Debug, Clone)]
pub struct Leaf {
    pub key: crate::types::Key32,
    pub value: Vec<u8>,
}
