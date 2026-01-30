# RootSmith Upstream Integration

## Flow

```
HTTP POST → Http Upstream → Channel → WASM Plugin → Record → Storage
```

## Implementation

### 1. HTTP Upstream (`src/upstream/http.rs`)

```rust
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use hyper::header::CONTENT_TYPE;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use kanal::AsyncSender;

use super::UpstreamConnector;
use crate::types::UpstreamData;

pub struct Http {
    pub port: u16,
    pub api_key: Option<String>,
    shutdown: Arc<AtomicBool>,
}

impl Http {
    pub fn new(port: u16, api_key: Option<String>) -> Self {
        Self {
            port,
            api_key,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl UpstreamConnector for Http {
    fn name(&self) -> &'static str {
        "http_upstream"
    }

    async fn open(&self, tx: AsyncSender<UpstreamData>) -> Result<()> {
        let addr: SocketAddr = ([0, 0, 0, 0], self.port).into();
        let shutdown = self.shutdown.clone();

        let make_svc = make_service_fn(move |_| {
            let tx = tx.clone();
            async move {
                Ok::<_, hyper::Error>(service_fn(move |req| {
                    handle_request(req, tx.clone())
                }))
            }
        });

        tracing::info!("HTTP upstream listening on {}", addr);

        tokio::spawn(async move {
            let server = Server::bind(&addr).serve(make_svc);
            let graceful = server.with_graceful_shutdown(async move {
                while !shutdown.load(Ordering::Relaxed) {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            });
            if let Err(e) = graceful.await {
                tracing::error!("Server error: {}", e);
            }
        });

        Ok(())
    }

    async fn close(&self) -> Result<()> {
        self.shutdown.store(true, Ordering::Relaxed);
        Ok(())
    }
}

async fn handle_request(
    req: Request<Body>,
    tx: AsyncSender<UpstreamData>,
) -> Result<Response<Body>, hyper::Error> {
    if req.method() != Method::POST {
        return Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Body::empty())
            .unwrap());
    }

    let content_type = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream");

    let body_bytes = hyper::body::to_bytes(req.into_body()).await?;

    let data = if content_type.starts_with("application/json") {
        match serde_json::from_slice(&body_bytes) {
            Ok(json) => UpstreamData::Json(json),
            Err(_) => UpstreamData::Bytes(body_bytes.to_vec()),
        }
    } else if content_type.starts_with("text/") {
        match String::from_utf8(body_bytes.to_vec()) {
            Ok(text) => UpstreamData::Text(text),
            Err(_) => UpstreamData::Bytes(body_bytes.to_vec()),
        }
    } else {
        UpstreamData::Bytes(body_bytes.to_vec())
    };

    match tx.send(data).await {
        Ok(_) => Ok(Response::builder()
            .status(StatusCode::ACCEPTED)
            .body(Body::from("OK"))
            .unwrap()),
        Err(_) => Ok(Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .body(Body::from("Channel closed"))
            .unwrap()),
    }
}
```

### 2. Processing Loop (`src/rootsmith/core.rs`)

```rust
impl RootSmith {
    pub async fn run(&self) -> anyhow::Result<()> {
        let (tx, rx) = kanal::unbounded_async::<UpstreamData>();

        self.upstream.open(tx).await?;
        tracing::info!("Upstream started");

        while let Ok(data) = rx.recv().await {
            tracing::info!("Received: {:?}", data);

            let mut wasm = self.wasm_host.lock().await;
            match wasm.process_to_record(data) {
                Ok(record) => {
                    tracing::info!(
                        "WASM output: ns={} key={}",
                        hex::encode(&record.namespace[..8]),
                        hex::encode(&record.key[..8])
                    );

                    let storage = self.storage.lock().await;
                    storage.put(&record)?;
                    tracing::info!("Stored in RocksDB");
                }
                Err(e) => tracing::error!("Plugin error: {}", e),
            }
        }

        self.upstream.close().await
    }
}
```

## Integration Test

`tests/upstream_integration_test.rs`:

```rust
use std::time::Duration;
use anyhow::Result;

#[tokio::test]
async fn test_upstream_flow() -> Result<()> {
    // Send HTTP request to running server
    let client = reqwest::Client::new();
    let resp = client
        .post("http://127.0.0.1:8080")
        .header("Content-Type", "application/json")
        .body(r#"{"user_id": "alice", "event_type": "login"}"#)
        .send()
        .await?;

    assert_eq!(resp.status(), 202);
    println!("POST response: {}", resp.status());

    Ok(())
}
```

## Run

Terminal 1 - Start server:
```bash
RUST_LOG=info cargo run -p rootsmith -- --config config.toml
```

Terminal 2 - Run test:
```bash
cargo test -p rootsmith --test upstream_integration_test -- --nocapture
```

## Expected Server Logs

```
INFO rootsmith: Starting rootsmith
INFO rootsmith: Storage opened at: ./data
INFO rootsmith: HTTP upstream listening on 0.0.0.0:8080
INFO rootsmith: Upstream started
INFO rootsmith: Received: Json({"event_type": "login", "user_id": "alice"})
INFO rootsmith: WASM output: ns=616c696365 key=6c6f67696e
INFO rootsmith: Stored in RocksDB
```
