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
        .unwrap_or("application/octet-stream")
        .to_string();

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
