# Short-lived Webhook Server

## Overview

A lightweight, programmatically controllable HTTP webhook server for receiving job completion notifications from the ZK service. Designed for testing and temporary use cases.

## Architecture

```
ZK Service (port 3000)
    │
    │ POST /webhook
    ▼
WebhookServer (port 8080)
    │
    │ Parse & Log
    ▼
Notification Handler
```

## Module Structure

**File:** `src/zk_service/webhook.rs`

### WebhookServer

- `bind_addr: SocketAddr` - Server bind address
- `shutdown_tx: Option<oneshot::Sender<()>>` - Graceful shutdown signal
- `server_handle: Option<JoinHandle<Result<()>>>` - Background server task
- `notification_rx: Option<AsyncReceiver<WebhookNotification>>` - Optional notification channel

### WebhookNotification

Matches ZK service `JobStatusResponse` format:

```rust
pub struct WebhookNotification {
    pub job_id: Uuid,
    pub status: String,
    pub error: Option<String>,
    pub result: Option<ProofResult>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
```

## API

### Methods

- `new(bind_addr: impl Into<String>) -> Self` - Create server instance
- `start() -> Result<()>` - Start server in background task
- `stop() -> Result<()>` - Gracefully shutdown server
- `with_notification_channel(rx: AsyncReceiver<WebhookNotification>) -> Self` - Optional channel for notifications
- `actual_addr() -> Option<SocketAddr>` - Get bound address after start

### HTTP Endpoints

- `POST /webhook` - Receive job completion notifications (JSON body)
- `GET /health` - Health check endpoint

## Usage

```rust
use rootsmith::zk_service::WebhookServer;

// Start webhook server
let mut webhook_server = WebhookServer::new("127.0.0.1:8080");
webhook_server.start().await?;

// Configure ZK accumulator with webhook URL
let zk_accumulator = ZkAccumulator::new("http://localhost:3000", "v1_16_24_4")
    .with_webhook("http://localhost:8080/webhook");

// Submit job - ZK service will POST to webhook when complete
// ...

// Stop server when done
webhook_server.stop().await?;
```

## With Notification Channel

```rust
use kanal::unbounded_async;

let (tx, rx) = unbounded_async();
let mut webhook_server = WebhookServer::new("127.0.0.1:8080")
    .with_notification_channel(rx);
webhook_server.start().await?;

// In another task: receive notifications
tokio::spawn(async move {
    while let Ok(notification) = rx.recv().await {
        println!("Job {} completed: {}", notification.job_id, notification.status);
    }
});
```

## Implementation Details

- Uses `hyper` for HTTP server (already in dependencies)
- Runs in background tokio task
- Graceful shutdown via oneshot channel
- Default bind: `127.0.0.1:8080`
- Logs all notifications with tracing
- Returns `200 OK` for valid webhook requests

## Integration

**Module Export:** `src/zk_service/mod.rs`
```rust
pub mod webhook;
pub use webhook::{WebhookServer, WebhookNotification};
```

## Testing

The webhook server can be tested by:
1. Starting the server
2. Submitting a job to ZK service with webhook URL
3. Waiting for webhook notification
4. Verifying payload matches expected format
5. Stopping the server
