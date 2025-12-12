use anyhow::Result;
use crossbeam_channel::Sender;
use crate::traits::UpstreamConnector;
use crate::types::IncomingRecord;
use crate::config::UpstreamType;
use super::{
    websocket::WebSocketSource, 
    kafka::KafkaSource, 
    sqs::SqsSource, 
    mqtt::MqttSource,
    noop::NoopUpstream,
    mock::MockUpstream,
};

/// Enum representing all possible upstream connector implementations.
pub enum UpstreamVariant {
    WebSocket(WebSocketSource),
    Kafka(KafkaSource),
    Sqs(SqsSource),
    Mqtt(MqttSource),
    Noop(NoopUpstream),
    Mock(MockUpstream),
}

impl UpstreamVariant {
    /// Create a new upstream connector instance based on the specified type.
    pub fn new(upstream_type: UpstreamType) -> Self {
        match upstream_type {
            UpstreamType::WebSocket => UpstreamVariant::WebSocket(WebSocketSource::new("ws://localhost:8080".to_string())),
            UpstreamType::Kafka => UpstreamVariant::Kafka(KafkaSource::new("localhost:9092".to_string(), "rootsmith".to_string())),
            UpstreamType::Sqs => UpstreamVariant::Sqs(SqsSource::new("https://sqs.us-east-1.amazonaws.com/queue".to_string(), "us-east-1".to_string())),
            UpstreamType::Mqtt => UpstreamVariant::Mqtt(MqttSource::new("mqtt://localhost:1883".to_string(), "rootsmith".to_string())),
            UpstreamType::Noop => UpstreamVariant::Noop(NoopUpstream),
            UpstreamType::Mock => UpstreamVariant::Mock(MockUpstream::default()),
        }
    }
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


