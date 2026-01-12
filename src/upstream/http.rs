use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use hyper::service::make_service_fn;
use hyper::service::service_fn;
use hyper::Body;
use hyper::Method;
use hyper::Request;
use hyper::Response;
use hyper::Server;
use hyper::StatusCode;
use kanal::AsyncSender;
use tokio::sync::Mutex;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::traits::UpstreamConnector;
use crate::types::IncomingRecord;

/// HTTP upstream connector that runs an HTTP server to receive IncomingRecords.
///
/// # Protocol
/// - POST /ingest - Accept single IncomingRecord as JSON
/// - POST /ingest/batch - Accept array of IncomingRecords as JSON
/// - GET /health - Health check endpoint
///
/// # Example POST /ingest payload:
/// ```json
/// {
///   "namespace": "0101010101010101010101010101010101010101010101010101010101010101",
///   "key": "0202020202020202020202020202020202020202020202020202020202020202",
///   "value": "0303030303030303030303030303030303030303030303030303030303030303",
///   "timestamp": 1234567890
/// }
/// ```
///
/// # Example POST /ingest/batch payload:
/// ```json
/// [
///   {
///     "namespace": "0101010101010101010101010101010101010101010101010101010101010101",
///     "key": "0202020202020202020202020202020202020202020202020202020202020202",
///     "value": "0303030303030303030303030303030303030303030303030303030303030303",
///     "timestamp": 1234567890
///   },
///   {
///     "namespace": "0101010101010101010101010101010101010101010101010101010101010101",
///     "key": "0404040404040404040404040404040404040404040404040404040404040404",
///     "value": "0505050505050505050505050505050505050505050505050505050505050505",
///     "timestamp": 1234567891
///   }
/// ]
/// ```
pub struct HttpSource {
    /// Address to bind the HTTP server to (e.g., "127.0.0.1:8080")
    bind_addr: String,
    /// Parsed socket address
    socket_addr: SocketAddr,
    /// Actual bound address (set after server starts)
    actual_addr: Arc<Mutex<Option<SocketAddr>>>,
    /// Channel sender for forwarding records
    tx: Arc<Mutex<Option<AsyncSender<IncomingRecord>>>>,
    /// Server shutdown signal
    shutdown_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl HttpSource {
    /// Create a new HTTP source with the specified bind address.
    pub fn new(bind_addr: String) -> Self {
        let socket_addr = bind_addr
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:8080".parse().unwrap());

        Self {
            bind_addr,
            socket_addr,
            actual_addr: Arc::new(Mutex::new(None)),
            tx: Arc::new(Mutex::new(None)),
            shutdown_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Get the actual bound address (available after server starts).
    pub async fn actual_addr(&self) -> Option<SocketAddr> {
        *self.actual_addr.lock().await
    }

    /// Handle incoming HTTP requests.
    async fn handle_request(
        req: Request<Body>,
        tx: Arc<Mutex<Option<AsyncSender<IncomingRecord>>>>,
    ) -> Result<Response<Body>, Infallible> {
        let method = req.method();
        let path = req.uri().path();

        debug!("HTTP request: {} {}", method, path);

        match (method, path) {
            // Health check endpoint
            (&Method::GET, "/health") => Ok(Response::builder()
                .status(StatusCode::OK)
                .body(Body::from(r#"{"status":"ok"}"#))
                .unwrap()),

            // Single record ingestion
            (&Method::POST, "/ingest") => Self::handle_ingest_single(req, tx).await,

            // Batch record ingestion
            (&Method::POST, "/ingest/batch") => Self::handle_ingest_batch(req, tx).await,

            // 404 for all other routes
            _ => Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(r#"{"error":"not_found"}"#))
                .unwrap()),
        }
    }

    /// Handle single record ingestion.
    async fn handle_ingest_single(
        req: Request<Body>,
        tx: Arc<Mutex<Option<AsyncSender<IncomingRecord>>>>,
    ) -> Result<Response<Body>, Infallible> {
        // Read request body
        let whole_body = match hyper::body::to_bytes(req.into_body()).await {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to read request body: {}", e);
                return Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(format!(
                        r#"{{"error":"invalid_body","message":"{}"}}"#,
                        e
                    )))
                    .unwrap());
            }
        };

        // Parse JSON
        let record: IncomingRecord = match serde_json::from_slice(&whole_body) {
            Ok(rec) => rec,
            Err(e) => {
                error!("Failed to parse IncomingRecord: {}", e);
                return Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(format!(
                        r#"{{"error":"invalid_json","message":"{}"}}"#,
                        e
                    )))
                    .unwrap());
            }
        };

        // Send to channel
        let tx_guard = tx.lock().await;
        if let Some(sender) = tx_guard.as_ref() {
            match sender.send(record).await {
                Ok(_) => {
                    debug!("Successfully ingested single record");
                    Ok(Response::builder()
                        .status(StatusCode::OK)
                        .body(Body::from(r#"{"status":"ok","ingested":1}"#))
                        .unwrap())
                }
                Err(e) => {
                    error!("Failed to send record to channel: {}", e);
                    Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from(r#"{"error":"channel_error"}"#))
                        .unwrap())
                }
            }
        } else {
            error!("Channel sender not initialized");
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(r#"{"error":"not_initialized"}"#))
                .unwrap())
        }
    }

    /// Handle batch record ingestion.
    async fn handle_ingest_batch(
        req: Request<Body>,
        tx: Arc<Mutex<Option<AsyncSender<IncomingRecord>>>>,
    ) -> Result<Response<Body>, Infallible> {
        // Read request body
        let whole_body = match hyper::body::to_bytes(req.into_body()).await {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to read request body: {}", e);
                return Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(format!(
                        r#"{{"error":"invalid_body","message":"{}"}}"#,
                        e
                    )))
                    .unwrap());
            }
        };

        // Parse JSON array
        let records: Vec<IncomingRecord> = match serde_json::from_slice(&whole_body) {
            Ok(recs) => recs,
            Err(e) => {
                error!("Failed to parse IncomingRecord array: {}", e);
                return Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(format!(
                        r#"{{"error":"invalid_json","message":"{}"}}"#,
                        e
                    )))
                    .unwrap());
            }
        };

        let record_count = records.len();

        // Send all records to channel
        let tx_guard = tx.lock().await;
        if let Some(sender) = tx_guard.as_ref() {
            let mut success_count = 0;
            for record in records {
                match sender.send(record).await {
                    Ok(_) => success_count += 1,
                    Err(e) => {
                        error!("Failed to send record to channel: {}", e);
                        break;
                    }
                }
            }

            debug!(
                "Successfully ingested {}/{} records",
                success_count, record_count
            );

            if success_count == record_count {
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::from(format!(
                        r#"{{"status":"ok","ingested":{}}}"#,
                        success_count
                    )))
                    .unwrap())
            } else {
                Ok(Response::builder()
                    .status(StatusCode::PARTIAL_CONTENT)
                    .body(Body::from(format!(
                        r#"{{"status":"partial","ingested":{},"total":{}}}"#,
                        success_count, record_count
                    )))
                    .unwrap())
            }
        } else {
            error!("Channel sender not initialized");
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(r#"{"error":"not_initialized"}"#))
                .unwrap())
        }
    }
}

#[async_trait]
impl UpstreamConnector for HttpSource {
    fn name(&self) -> &'static str {
        "http-source"
    }

    async fn open(&mut self, tx: AsyncSender<IncomingRecord>) -> Result<()> {
        info!("Starting HTTP server on {}", self.bind_addr);

        // Store the channel sender
        {
            let mut tx_guard = self.tx.lock().await;
            *tx_guard = Some(tx);
        }

        let tx_arc = Arc::clone(&self.tx);

        // Create service
        let make_svc = make_service_fn(move |_conn| {
            let tx_clone = Arc::clone(&tx_arc);
            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    Self::handle_request(req, Arc::clone(&tx_clone))
                }))
            }
        });

        // Create server
        let server = Server::bind(&self.socket_addr).serve(make_svc);
        let addr = server.local_addr();

        // Store the actual bound address
        {
            let mut actual_addr_guard = self.actual_addr.lock().await;
            *actual_addr_guard = Some(addr);
        }

        info!("HTTP server listening on http://{}", addr);

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        {
            let mut shutdown_guard = self.shutdown_tx.lock().await;
            *shutdown_guard = Some(shutdown_tx);
        }

        // Spawn server task with graceful shutdown
        tokio::spawn(async move {
            let graceful = server.with_graceful_shutdown(async {
                shutdown_rx.await.ok();
                info!("HTTP server shutdown signal received");
            });

            if let Err(e) = graceful.await {
                error!("HTTP server error: {}", e);
            } else {
                info!("HTTP server stopped gracefully");
            }
        });

        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        info!("Closing HTTP server");

        // Send shutdown signal
        let mut shutdown_guard = self.shutdown_tx.lock().await;
        if let Some(shutdown_tx) = shutdown_guard.take() {
            if shutdown_tx.send(()).is_err() {
                warn!("Failed to send shutdown signal (receiver already dropped)");
            }
        }

        // Clear the channel sender
        {
            let mut tx_guard = self.tx.lock().await;
            *tx_guard = None;
        }

        info!("HTTP server closed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use kanal::unbounded_async;

    use super::*;

    #[tokio::test]
    async fn test_http_source_new() {
        let source = HttpSource::new("127.0.0.1:9999".to_string());
        assert_eq!(source.bind_addr, "127.0.0.1:9999");
        assert_eq!(source.name(), "http-source");
    }

    #[tokio::test]
    async fn test_http_source_open_close() {
        let mut source = HttpSource::new("127.0.0.1:0".to_string()); // Port 0 = random port
        let (tx, _rx) = unbounded_async();

        // Open should succeed
        assert!(source.open(tx).await.is_ok());

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Close should succeed
        assert!(source.close().await.is_ok());
    }

    #[tokio::test]
    async fn test_http_health_endpoint() {
        let mut source = HttpSource::new("127.0.0.1:0".to_string());
        let (tx, _rx) = unbounded_async();

        source.open(tx).await.unwrap();

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let addr = source
            .actual_addr()
            .await
            .expect("Server should have bound address");

        // Test health endpoint
        let client = hyper::Client::new();
        let uri = format!("http://{}/health", addr);
        let response = client.get(uri.parse().unwrap()).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("ok"));

        source.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_http_ingest_single_record() {
        let mut source = HttpSource::new("127.0.0.1:0".to_string());
        let (tx, rx) = unbounded_async();

        source.open(tx).await.unwrap();

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let addr = source
            .actual_addr()
            .await
            .expect("Server should have bound address");

        // Create test record
        let record = IncomingRecord {
            namespace: [1u8; 32],
            key: [2u8; 32],
            value: [3u8; 32],
            timestamp: 1234567890,
        };

        // Send POST request
        let client = hyper::Client::new();
        let uri = format!("http://{}/ingest", addr);
        let json_body = serde_json::to_string(&record).unwrap();

        let req = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(json_body))
            .unwrap();

        let response = client.request(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify record was received
        let received = rx.recv().await.unwrap();
        assert_eq!(received.namespace, record.namespace);
        assert_eq!(received.key, record.key);
        assert_eq!(received.value, record.value);
        assert_eq!(received.timestamp, record.timestamp);

        source.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_http_ingest_batch_records() {
        let mut source = HttpSource::new("127.0.0.1:0".to_string());
        let (tx, rx) = unbounded_async();

        source.open(tx).await.unwrap();

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let addr = source
            .actual_addr()
            .await
            .expect("Server should have bound address");

        // Create test records
        let records = vec![
            IncomingRecord {
                namespace: [1u8; 32],
                key: [2u8; 32],
                value: [3u8; 32],
                timestamp: 1234567890,
            },
            IncomingRecord {
                namespace: [1u8; 32],
                key: [4u8; 32],
                value: [5u8; 32],
                timestamp: 1234567891,
            },
        ];

        // Send POST request
        let client = hyper::Client::new();
        let uri = format!("http://{}/ingest/batch", addr);
        let json_body = serde_json::to_string(&records).unwrap();

        let req = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(json_body))
            .unwrap();

        let response = client.request(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify records were received
        let received1 = rx.recv().await.unwrap();
        let received2 = rx.recv().await.unwrap();

        assert_eq!(received1.key, [2u8; 32]);
        assert_eq!(received2.key, [4u8; 32]);

        source.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_http_invalid_json() {
        let mut source = HttpSource::new("127.0.0.1:0".to_string());
        let (tx, _rx) = unbounded_async();

        source.open(tx).await.unwrap();

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let addr = source
            .actual_addr()
            .await
            .expect("Server should have bound address");

        // Send invalid JSON
        let client = hyper::Client::new();
        let uri = format!("http://{}/ingest", addr);

        let req = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from("invalid json"))
            .unwrap();

        let response = client.request(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("error"));

        source.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_http_not_found() {
        let mut source = HttpSource::new("127.0.0.1:0".to_string());
        let (tx, _rx) = unbounded_async();

        source.open(tx).await.unwrap();

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let addr = source
            .actual_addr()
            .await
            .expect("Server should have bound address");

        // Test non-existent endpoint
        let client = hyper::Client::new();
        let uri = format!("http://{}/nonexistent", addr);
        let response = client.get(uri.parse().unwrap()).await.unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        source.close().await.unwrap();
    }
}
