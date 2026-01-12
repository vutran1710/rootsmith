use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use futures_util::SinkExt;
use futures_util::StreamExt;
use kanal::AsyncSender;
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::traits::UpstreamConnector;
use crate::types::IncomingRecord;

pub struct WebSocketSource {
    url: String,
    connection_handle: Option<
        Arc<
            Mutex<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
            >,
        >,
    >,
}

impl WebSocketSource {
    pub fn new(url: String) -> Self {
        Self {
            url,
            connection_handle: None,
        }
    }
}

#[async_trait]
impl UpstreamConnector for WebSocketSource {
    fn name(&self) -> &'static str {
        "websocket"
    }

    async fn open(&mut self, tx: AsyncSender<IncomingRecord>) -> Result<()> {
        tracing::info!("Opening WebSocket connection: {}", self.url);

        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&self.url)
            .await
            .context("Failed to connect to WebSocket")?;

        let ws_stream = Arc::new(Mutex::new(ws_stream));
        self.connection_handle = Some(Arc::clone(&ws_stream));

        // Spawn task to receive messages and forward them to the channel
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

                match msg {
                    Message::Text(text) => {
                        // Parse JSON message as IncomingRecord
                        match serde_json::from_str::<IncomingRecord>(&text) {
                            Ok(record) => {
                                if tx_clone.send(record).await.is_err() {
                                    tracing::warn!("Channel closed, stopping WebSocket receiver");
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to parse WebSocket message as IncomingRecord: {} - message: {}", e, text);
                                // Continue processing other messages
                            }
                        }
                    }
                    Message::Binary(data) => {
                        // Try to parse binary data as JSON
                        match serde_json::from_slice::<IncomingRecord>(&data) {
                            Ok(record) => {
                                if tx_clone.send(record).await.is_err() {
                                    tracing::warn!("Channel closed, stopping WebSocket receiver");
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to parse WebSocket binary message as IncomingRecord: {}", e);
                            }
                        }
                    }
                    Message::Close(_) => {
                        tracing::info!("WebSocket received close frame");
                        break;
                    }
                    Message::Ping(_) | Message::Pong(_) => {
                        // Handle ping/pong automatically by tokio-tungstenite
                    }
                    _ => {
                        tracing::debug!("Received unsupported WebSocket message type");
                    }
                }
            }
        });

        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        tracing::info!("Closing WebSocket connection");

        if let Some(stream) = &self.connection_handle {
            let mut ws = stream.lock().await;
            ws.close(None)
                .await
                .context("Failed to close WebSocket connection")?;
        }

        self.connection_handle = None;
        Ok(())
    }
}
