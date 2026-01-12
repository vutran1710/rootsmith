use super::{
    http::HttpSource, mock::MockUpstream,
    noop::NoopUpstream, websocket::WebSocketSource,
};
use crate::config::UpstreamType;
use crate::traits::UpstreamConnector;
use crate::types::IncomingRecord;
use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;

/// Enum representing all possible upstream connector implementations.
pub enum UpstreamVariant {
    Http(HttpSource),
    WebSocket(WebSocketSource),
    Noop(NoopUpstream),
    Mock(MockUpstream),
}

impl UpstreamVariant {
    /// Create a new upstream connector instance based on the specified type.
    pub fn new(upstream_type: UpstreamType) -> Self {
        match upstream_type {
            UpstreamType::Http => {
                UpstreamVariant::Http(HttpSource::new("127.0.0.1:8080".to_string()))
            }
            UpstreamType::WebSocket => {
                UpstreamVariant::WebSocket(WebSocketSource::new("ws://localhost:8080".to_string()))
            }
            UpstreamType::Noop => UpstreamVariant::Noop(NoopUpstream),
            UpstreamType::Mock => UpstreamVariant::Mock(MockUpstream::default()),
        }
    }
}

#[async_trait]
impl UpstreamConnector for UpstreamVariant {
    fn name(&self) -> &'static str {
        match self {
            UpstreamVariant::Http(inner) => inner.name(),
            UpstreamVariant::WebSocket(inner) => inner.name(),
            UpstreamVariant::Noop(inner) => inner.name(),
            UpstreamVariant::Mock(inner) => inner.name(),
        }
    }

    async fn open(&mut self, tx: AsyncSender<IncomingRecord>) -> Result<()> {
        match self {
            UpstreamVariant::Http(inner) => inner.open(tx).await,
            UpstreamVariant::WebSocket(inner) => inner.open(tx).await,
            UpstreamVariant::Noop(inner) => inner.open(tx).await,
            UpstreamVariant::Mock(inner) => inner.open(tx).await,
        }
    }

    async fn close(&mut self) -> Result<()> {
        match self {
            UpstreamVariant::Http(inner) => inner.close().await,
            UpstreamVariant::WebSocket(inner) => inner.close().await,
            UpstreamVariant::Noop(inner) => inner.close().await,
            UpstreamVariant::Mock(inner) => inner.close().await,
        }
    }
}
