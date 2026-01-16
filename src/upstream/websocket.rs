use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use kanal::AsyncSender;
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::parser::proto::parse_proto_message;
use crate::traits::UpstreamConnector;
use crate::types::IncomingRecord;

enum MessageHandleResult {
    Continue,
    Break,
}

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

    /// Parse a binary protobuf message into an IncomingRecord
    fn parse_binary_message(data: &[u8]) -> Result<IncomingRecord> {
        parse_proto_message(data)
    }

    /// Handle a single WebSocket message and forward it to the channel
    async fn handle_message(
        msg: Message,
        tx: &AsyncSender<IncomingRecord>,
    ) -> MessageHandleResult {
        match msg {
            Message::Binary(data) => {
                match Self::parse_binary_message(&data) {
                    Ok(record) => {
                        if tx.send(record).await.is_err() {
                            tracing::warn!("Channel closed, stopping WebSocket receiver");
                            return MessageHandleResult::Break;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse WebSocket protobuf message: {}", e);
                    }
                }
            }
            Message::Close(_) => {
                tracing::info!("WebSocket received close frame");
                return MessageHandleResult::Break;
            }
            Message::Ping(_) | Message::Pong(_) => {
                // Handle ping/pong automatically by tokio-tungstenite
            }
            Message::Text(text) => {
                tracing::warn!("Received unsupported Text message, only Binary protobuf is supported. Message: {}", text);
            }
            _ => {
                tracing::debug!("Received unsupported WebSocket message type");
            }
        }
        MessageHandleResult::Continue
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

                match Self::handle_message(msg, &tx_clone).await {
                    MessageHandleResult::Continue => continue,
                    MessageHandleResult::Break => break,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::proto::proto;
    use prost::Message as ProstMessage;
    use tokio_tungstenite::tungstenite::Message;

    /// Helper function to create a valid protobuf IncomingRecord for testing
    fn create_valid_proto_record() -> proto::IncomingRecord {
        proto::IncomingRecord {
            namespace: vec![0u8; 32],
            key: vec![1u8; 32],
            value: vec![2u8; 32],
            timestamp: 1234567890,
        }
    }

    /// Helper to encode protobuf message to bytes
    fn encode_proto(record: proto::IncomingRecord) -> Vec<u8> {
        let mut buf = Vec::new();
        record.encode(&mut buf).unwrap();
        buf
    }

    #[test]
    fn test_parse_binary_message_valid() {
        let proto_record = create_valid_proto_record();
        let bytes = encode_proto(proto_record);
        let result = WebSocketSource::parse_binary_message(&bytes);
        assert!(result.is_ok());
        let record = result.unwrap();
        assert_eq!(record.timestamp, 1234567890);
        assert_eq!(record.namespace, [0u8; 32]);
        assert_eq!(record.key, [1u8; 32]);
        assert_eq!(record.value, [2u8; 32]);
    }

    #[test]
    fn test_parse_binary_message_invalid() {
        let invalid_bytes = b"not a valid protobuf";
        let result = WebSocketSource::parse_binary_message(invalid_bytes);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_message_binary() {
        let (tx, rx) = kanal::unbounded_async();
        let proto_record = create_valid_proto_record();
        let bytes = encode_proto(proto_record);
        let msg = Message::Binary(bytes);

        let result = WebSocketSource::handle_message(msg, &tx).await;

        assert!(matches!(result, MessageHandleResult::Continue));

        let record = rx.recv().await;
        assert!(record.is_ok());
        let record = record.unwrap();
        assert_eq!(record.timestamp, 1234567890);
        assert_eq!(record.namespace, [0u8; 32]);
        assert_eq!(record.key, [1u8; 32]);
        assert_eq!(record.value, [2u8; 32]);
    }

    #[tokio::test]
    async fn test_handle_message_text_unsupported() {
        let (tx, _rx) = kanal::unbounded_async();
        let msg = Message::Text("some text".to_string());

        let result = WebSocketSource::handle_message(msg, &tx).await;

        // Should continue (but logs warning)
        assert!(matches!(result, MessageHandleResult::Continue));
    }

    #[tokio::test]
    async fn test_handle_message_close() {
        let (tx, _rx) = kanal::unbounded_async();
        let msg = Message::Close(None);

        let result = WebSocketSource::handle_message(msg, &tx).await;

        // Should break on close message
        assert!(matches!(result, MessageHandleResult::Break));
    }

    #[tokio::test]
    async fn test_handle_message_ping() {
        let (tx, _rx) = kanal::unbounded_async();
        let msg = Message::Ping(vec![]);

        let result = WebSocketSource::handle_message(msg, &tx).await;

        // Should continue on ping
        assert!(matches!(result, MessageHandleResult::Continue));
    }

    #[tokio::test]
    async fn test_handle_message_pong() {
        let (tx, _rx) = kanal::unbounded_async();
        let msg = Message::Pong(vec![]);

        let result = WebSocketSource::handle_message(msg, &tx).await;

        // Should continue on pong
        assert!(matches!(result, MessageHandleResult::Continue));
    }

    #[tokio::test]
    async fn test_handle_message_invalid_binary() {
        let (tx, _rx) = kanal::unbounded_async();
        let msg = Message::Binary(b"invalid protobuf data".to_vec());

        let result = WebSocketSource::handle_message(msg, &tx).await;

        // Should continue even with invalid message (logs warning and continues)
        assert!(matches!(result, MessageHandleResult::Continue));
    }

    #[test]
    fn test_new_websocket_source() {
        let url = "ws://localhost:8080".to_string();
        let source = WebSocketSource::new(url.clone());

        assert_eq!(source.url, url);
        assert!(source.connection_handle.is_none());
    }

    #[test]
    fn test_name() {
        let source = WebSocketSource::new("ws://localhost:8080".to_string());
        assert_eq!(source.name(), "websocket");
    }
}
