use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use futures_util::StreamExt;
use kanal::AsyncSender;
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;

use crate::traits::UpstreamConnector;
use crate::types::UpstreamData;

type Connection = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

pub struct WebSocketSource {
    port: u16,
    api_key: Option<String>,
    should_close: Arc<AtomicBool>,
}

impl WebSocketSource {
    pub fn new(port: u16, api_key: Option<String>) -> Self {
        Self {
            port,
            api_key,
            should_close: Arc::new(AtomicBool::new(false)),
        }
    }

    async fn handle_message(msg: Message, tx: &AsyncSender<UpstreamData>) -> Result<()> {
        match msg {
            Message::Binary(data) => tx
                .send(UpstreamData::Bytes(data))
                .await
                .map_err(|_| anyhow::anyhow!("Failed to send binary data to upstream channel")),
            Message::Text(text) => {
                let json = serde_json::from_str(&text);

                if let Ok(json_value) = json {
                    tx.send(UpstreamData::Json(json_value)).await?;
                    return Ok(());
                }

                tx.send(UpstreamData::Text(text)).await?;
                Ok(())
            }

            Message::Ping(_) | Message::Pong(_) => Ok(()),

            Message::Close(_) => {
                tracing::info!("WebSocket connection closed by peer");
                anyhow::bail!("WebSocket connection closed by peer");
            }

            Message::Frame(_) => {
                tracing::error!("Received unsupported WebSocket frame message");
                anyhow::bail!("Unsupported WebSocket frame message");
            }
        }
    }
}

#[async_trait]
impl UpstreamConnector for WebSocketSource {
    fn name(&self) -> &'static str {
        "websocket"
    }

    async fn open(&self, tx: AsyncSender<UpstreamData>) -> Result<()> {
        let url = format!("ws://localhost:{}", self.port);
        tracing::info!("Opening WebSocket connection: {}", url);

        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&url)
            .await
            .context("Failed to connect to WebSocket")?;

        let ws_stream = Arc::new(Mutex::new(ws_stream));
        let should_close = self.should_close.clone();

        let tx_clone = tx.clone();
        let stream_clone = Arc::clone(&ws_stream);
        tokio::spawn(async move {
            loop {
                let msg = {
                    let mut ws = stream_clone.lock().await;
                    match ws.next().await {
                        Some(Ok(msg)) => msg,
                        Some(Err(e)) => {
                            tracing::error!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            tracing::info!("WebSocket connection closed");
                            break;
                        }
                    }
                };

                Self::handle_message(msg, &tx_clone).await?;

                if should_close.load(std::sync::atomic::Ordering::Relaxed) {
                    tracing::info!("WebSocket connection is closing as requested");
                    return Ok(());
                }
            }
            Ok::<(), anyhow::Error>(())
        });

        Ok(())
    }

    async fn close(&self) -> Result<()> {
        tracing::info!("Closing WebSocket connection");
        self.should_close
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
}
