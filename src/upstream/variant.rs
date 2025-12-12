use super::{
    kafka::KafkaSource, mock::MockUpstream, mqtt::MqttSource, noop::NoopUpstream, sqs::SqsSource,
    websocket::WebSocketSource,
};
use crate::config::UpstreamType;
use crate::traits::UpstreamConnector;
use crate::types::IncomingRecord;
use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;

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
            UpstreamType::WebSocket => {
                UpstreamVariant::WebSocket(WebSocketSource::new("ws://localhost:8080".to_string()))
            }
            UpstreamType::Kafka => UpstreamVariant::Kafka(KafkaSource::new(
                "localhost:9092".to_string(),
                "rootsmith".to_string(),
            )),
            UpstreamType::Sqs => UpstreamVariant::Sqs(SqsSource::new(
                "https://sqs.us-east-1.amazonaws.com/queue".to_string(),
                "us-east-1".to_string(),
            )),
            UpstreamType::Mqtt => UpstreamVariant::Mqtt(MqttSource::new(
                "mqtt://localhost:1883".to_string(),
                "rootsmith".to_string(),
            )),
            UpstreamType::Noop => UpstreamVariant::Noop(NoopUpstream),
            UpstreamType::Mock => UpstreamVariant::Mock(MockUpstream::default()),
        }
    }
}

#[async_trait]
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

    async fn open(&mut self, tx: AsyncSender<IncomingRecord>) -> Result<()> {
        match self {
            UpstreamVariant::WebSocket(inner) => inner.open(tx).await,
            UpstreamVariant::Kafka(inner) => inner.open(tx).await,
            UpstreamVariant::Sqs(inner) => inner.open(tx).await,
            UpstreamVariant::Mqtt(inner) => inner.open(tx).await,
            UpstreamVariant::Noop(inner) => inner.open(tx).await,
            UpstreamVariant::Mock(inner) => inner.open(tx).await,
        }
    }

    async fn close(&mut self) -> Result<()> {
        match self {
            UpstreamVariant::WebSocket(inner) => inner.close().await,
            UpstreamVariant::Kafka(inner) => inner.close().await,
            UpstreamVariant::Sqs(inner) => inner.close().await,
            UpstreamVariant::Mqtt(inner) => inner.close().await,
            UpstreamVariant::Noop(inner) => inner.close().await,
            UpstreamVariant::Mock(inner) => inner.close().await,
        }
    }
}
