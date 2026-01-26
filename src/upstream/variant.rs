use anyhow::Result;
use async_trait::async_trait;
use clap::ValueEnum;
use kanal::AsyncSender;
use serde::Deserialize;
use serde::Serialize;

#[cfg(test)]
use super::channel::Channel;
use super::http::Http;
use super::websocket::WebSocketSource;
use crate::traits::UpstreamConnector;
use crate::types::UpstreamData;

/// Type of upstream connector to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum UpstreamType {
    Http,
    WebSocket,
    #[cfg(test)]
    Channel,
}

/// Enum representing all possible upstream connector implementations.
pub enum UpstreamVariant {
    Http(Http),
    WebSocket(WebSocketSource),
    #[cfg(test)]
    Channel(Channel),
}

pub enum UpstreamConfig {
    HttpConfig { port: u16, api_key: Option<String> },
    WebSocketConfig { port: u16, api_key: Option<String> },
}

impl UpstreamVariant {
    /// Create a new upstream connector instance based on the specified type.
    pub fn new(upstream_type: UpstreamType, config: UpstreamConfig) -> Self {
        match upstream_type {
            UpstreamType::Http => {
                let (port, api_key) = match config {
                    UpstreamConfig::HttpConfig { port, api_key } => (port, api_key),
                    _ => panic!("Invalid config for HTTP upstream"),
                };
                UpstreamVariant::Http(Http { port, api_key })
            }
            UpstreamType::WebSocket => {
                let (port, api_key) = match config {
                    UpstreamConfig::WebSocketConfig { port, api_key } => (port, api_key),
                    _ => panic!("Invalid config for WebSocket upstream"),
                };
                UpstreamVariant::WebSocket(WebSocketSource::new(port, api_key))
            }
            #[cfg(test)]
            UpstreamType::Channel => UpstreamVariant::Channel(Channel::default()),
        }
    }
}

#[async_trait]
impl UpstreamConnector for UpstreamVariant {
    fn name(&self) -> &'static str {
        match self {
            UpstreamVariant::Http(inner) => inner.name(),
            UpstreamVariant::WebSocket(inner) => inner.name(),
            #[cfg(test)]
            UpstreamVariant::Channel(_) => "[kanal-based-channel-upstream for testing]",
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
