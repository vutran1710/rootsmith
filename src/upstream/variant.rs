use anyhow::Result;
use crossbeam_channel::Sender;
use crate::traits::UpstreamConnector;
use crate::types::IncomingRecord;
use super::{websocket::WebSocketSource, kafka::KafkaSource, sqs::SqsSource, mqtt::MqttSource};

/// Enum representing all possible upstream connector implementations.
pub enum UpstreamVariant {
    WebSocket(WebSocketSource),
    Kafka(KafkaSource),
    Sqs(SqsSource),
    Mqtt(MqttSource),
    Noop(NoopUpstream),
    Mock(MockUpstream),
}

impl UpstreamConnector for UpstreamVariant {
    fn name(&self) -> &'static str {
        match self {
            UpstreamVariant::WebSocket(inner) => inner.name(),
            UpstreamVariant::Kafka(inner) => inner.name(),
            UpstreamVariant::Sqs(inner) => inner.name(),
            UpstreamVariant::Mqtt(inner) => inner.name(),
            UpstreamVariant::Noop(inner) => inner.name(),
            UpstreamVariant::Mock(inner) => inner.name(),
        }
    }

    fn open(&mut self, tx: Sender<IncomingRecord>) -> Result<()> {
        match self {
            UpstreamVariant::WebSocket(inner) => inner.open(tx),
            UpstreamVariant::Kafka(inner) => inner.open(tx),
            UpstreamVariant::Sqs(inner) => inner.open(tx),
            UpstreamVariant::Mqtt(inner) => inner.open(tx),
            UpstreamVariant::Noop(inner) => inner.open(tx),
            UpstreamVariant::Mock(inner) => inner.open(tx),
        }
    }

    fn close(&mut self) -> Result<()> {
        match self {
            UpstreamVariant::WebSocket(inner) => inner.close(),
            UpstreamVariant::Kafka(inner) => inner.close(),
            UpstreamVariant::Sqs(inner) => inner.close(),
            UpstreamVariant::Mqtt(inner) => inner.close(),
            UpstreamVariant::Noop(inner) => inner.close(),
            UpstreamVariant::Mock(inner) => inner.close(),
        }
    }
}

/// Noop upstream connector for demonstration purposes.
pub struct NoopUpstream;

impl UpstreamConnector for NoopUpstream {
    fn name(&self) -> &'static str {
        "noop-upstream"
    }
    
    fn open(&mut self, _tx: Sender<IncomingRecord>) -> Result<()> {
        tracing::info!("NoopUpstream: open() called - no data to send");
        Ok(())
    }
    
    fn close(&mut self) -> Result<()> {
        tracing::info!("NoopUpstream: close() called");
        Ok(())
    }
}

/// Mock upstream connector for testing.
pub struct MockUpstream {
    pub records: Vec<IncomingRecord>,
    pub delay_ms: u64,
}

impl MockUpstream {
    pub fn new(records: Vec<IncomingRecord>, delay_ms: u64) -> Self {
        Self { records, delay_ms }
    }
}

impl UpstreamConnector for MockUpstream {
    fn name(&self) -> &'static str {
        "mock-upstream"
    }

    fn open(&mut self, tx: Sender<IncomingRecord>) -> Result<()> {
        let records = self.records.clone();
        let delay = self.delay_ms;

        std::thread::spawn(move || {
            for record in records {
                if delay > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                }
                if tx.send(record).is_err() {
                    break;
                }
            }
        });

        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

