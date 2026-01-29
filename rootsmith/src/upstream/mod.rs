pub mod channel;
pub mod http;
pub mod websocket;

use anyhow::Result;
use async_trait::async_trait;
#[cfg(test)]
use channel::Channel;
use http::Http;
use kanal::AsyncSender;
use serde::Deserialize;
use serde::Serialize;
use websocket::WebSocketSource;

use crate::types::UpstreamData;

/// Trait for upstream data sources (websocket, Kafka, SQS, MQTT, etc.).
///
/// Implementations are responsible for producing `UpstreamData` into
/// the app's ingestion pipeline.
#[async_trait]
pub trait UpstreamConnector: Send + Sync {
    /// Human-readable connector name for logging.
    fn name(&self) -> &'static str {
        unimplemented!("")
    }

    /// Typical implementation:
    /// - spawn a thread / async task,
    /// - read from external source,
    /// - push `UpstreamData` into the provided channel.
    async fn open(&self, tx: AsyncSender<UpstreamData>) -> Result<()>;

    /// Close/stop the connector and release resources.
    async fn close(&self) -> Result<()>;
}

/// Enum representing all possible upstream connector implementations.
pub enum UpstreamVariant {
    Http(Http),
    WebSocket(WebSocketSource),
    #[cfg(test)]
    Channel(Channel),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpstreamConfig {
    Http {
        port: u16,
        api_key: Option<String>,
    },
    WebSocket {
        port: u16,
        api_key: Option<String>,
    },
    #[cfg(test)]
    Channel,
}

impl UpstreamVariant {
    pub fn new(config: UpstreamConfig) -> Self {
        match config {
            UpstreamConfig::Http { port, api_key } => UpstreamVariant::Http(Http { port, api_key }),
            UpstreamConfig::WebSocket { port, api_key } => {
                UpstreamVariant::WebSocket(WebSocketSource::new(port, api_key))
            }
            #[cfg(test)]
            UpstreamConfig::Channel => UpstreamVariant::Channel(Channel::default()),
        }
    }
}

#[async_trait]
impl UpstreamConnector for UpstreamVariant {
    fn name(&self) -> &'static str {
        match self {
            UpstreamVariant::Http(_) => "http_upstream",
            UpstreamVariant::WebSocket(_) => "websocket_upstream",
            #[cfg(test)]
            UpstreamVariant::Channel(_) => "channel_upstream",
        }
    }

    async fn open(&self, tx: AsyncSender<UpstreamData>) -> Result<()> {
        match self {
            UpstreamVariant::Http(inner) => inner.open(tx).await,
            UpstreamVariant::WebSocket(inner) => inner.open(tx).await,
            #[cfg(test)]
            UpstreamVariant::Channel(inner) => inner.bind_forward_loop(tx).await,
        }
    }

    async fn close(&self) -> Result<()> {
        match self {
            UpstreamVariant::Http(inner) => inner.close().await,
            UpstreamVariant::WebSocket(inner) => inner.close().await,
            #[cfg(test)]
            UpstreamVariant::Channel(inner) => inner.close().await,
        }
    }
}
