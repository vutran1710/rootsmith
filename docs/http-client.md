# HTTP Client Module Documentation

This document provides specifications for implementing the `zk-service` HTTP client module that interacts with the ZK Circom Proof Service API.

## Overview

The `zk-service` module is a Rust HTTP client library for interacting with the ZK Circom Proof Service. It provides a type-safe, async interface for submitting proof generation jobs, checking job status, and managing circuits.

**Note**: This module should be placed at `src/zk_service/` within your existing project structure.

## Module Structure

The `zk-service` module should be placed at `src/zk-service/` in your project:

```
your-project/
├── Cargo.toml
├── src/
│   ├── lib.rs (or main.rs)
│   └── zk-service/          # HTTP client module
│       ├── mod.rs          # Module declaration and re-exports
│       ├── client.rs       # Main client implementation
│       ├── types.rs        # Request/response types
│       ├── error.rs        # Error types
│       └── builder.rs      # Request builders (optional)
```

## Dependencies

Add these dependencies to your project's `Cargo.toml`:

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json", "multipart"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.0", features = ["serde", "v4"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1.0"
async-trait = "0.1"  # Optional, for trait-based design
```

## Module Setup

In your `src/lib.rs` (or `src/main.rs`), declare the module:

```rust
// src/lib.rs
pub mod zk_service;  // or pub mod zk_service { ... }
```

Or if using a directory structure, create `src/zk_service/mod.rs`:

```rust
// src/zk_service/mod.rs
pub mod client;
pub mod error;
pub mod types;

// Re-export main types
pub use client::ZkServiceClient;
pub use error::{ZkServiceError, Result};
pub use types::*;

// Optional: re-export builder
#[cfg(feature = "builder")]
pub mod builder;
#[cfg(feature = "builder")]
pub use builder::JobRequestBuilder;
```

## Type Definitions

### Request Types

```rust
// src/zk_service/types.rs

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Selection count for data selection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionCount {
    /// Exact count
    Exact(usize),
    /// Select all available
    All,
}

/// Data selection parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSelection {
    /// Starting index in the data array
    pub start: usize,
    /// Step offset between selected elements
    pub offset: usize,
    /// Number of elements to select
    pub count: SelectionCount,
}

/// Operator types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Operator {
    /// Merkle16 operator
    Merkle16 {
        selection: DataSelection,
        handler: String,  // 20-byte hex address with 0x prefix
    },
}

/// Input data for job submission
#[derive(Debug, Clone)]
pub enum InputData {
    /// Raw bytes: array of byte arrays
    RawBytes(Vec<Vec<u8>>),
    /// Table data: columns with named fields
    Table {
        columns: std::collections::HashMap<String, Vec<serde_json::Value>>,
        column_order: Vec<String>,
    },
}

/// Job submission request
#[derive(Debug, Clone)]
pub struct SubmitJobRequest {
    pub circuit_id: String,
    pub operators: Vec<Operator>,
    pub data: InputData,
    pub webhook_url: String,
}
```

### Response Types

```rust
// src/types.rs (continued)

/// Job submission response
#[derive(Debug, Clone, Deserialize)]
pub struct SubmitJobResponse {
    pub job_id: Uuid,
    pub status: String,
}

/// Job status response
#[derive(Debug, Clone, Deserialize)]
pub struct JobStatusResponse {
    pub job_id: Uuid,
    pub status: String,
    pub error: Option<String>,
    pub result: Option<ProofResult>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Proof generation result
#[derive(Debug, Clone, Deserialize)]
pub struct ProofResult {
    /// The proof JSON string (Groth16 proof)
    pub proof: String,
    /// Public signals JSON string
    pub public_signals: String,
}

/// Circuit information
#[derive(Debug, Clone, Deserialize)]
pub struct CircuitInfo {
    pub id: String,
    pub params: CircuitParams,
}

/// Circuit parameters
#[derive(Debug, Clone, Deserialize)]
pub struct CircuitParams {
    pub max_data: usize,
    pub max_ops: usize,
}

/// Circuits list response
#[derive(Debug, Clone, Deserialize)]
pub struct CircuitsResponse {
    pub circuits: Vec<CircuitInfo>,
}

/// Health check response
#[derive(Debug, Clone, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub circuits_loaded: usize,
    pub workers: WorkerStatus,
}

/// Worker pool status
#[derive(Debug, Clone, Deserialize)]
pub struct WorkerStatus {
    pub total: usize,
    pub available: usize,
    pub queue_size: usize,
    pub max_queue_size: usize,
}

/// Error response from API
#[derive(Debug, Clone, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}
```

## Error Types

```rust
// src/zk_service/error.rs

use thiserror::Error;

/// Client error types
#[derive(Debug, Error)]
pub enum ZkServiceError {
    /// HTTP request error
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    
    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    /// API returned an error response
    #[error("API error: {0}")]
    Api(String),
    
    /// Invalid input data
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    /// Job not found
    #[error("Job not found: {0}")]
    JobNotFound(String),
    
    /// Circuit not found
    #[error("Circuit not found: {0}")]
    CircuitNotFound(String),
    
    /// Queue is full
    #[error("Queue is full")]
    QueueFull,
}

pub type Result<T> = std::result::Result<T, ZkServiceError>;
```

## Client Implementation

### Basic Client Structure

```rust
// src/zk_service/client.rs

use crate::error::{Result, ZkServiceError};
use crate::types::*;
use reqwest::Client;
use std::time::Duration;
use uuid::Uuid;

/// ZK Service HTTP client
#[derive(Debug, Clone)]
pub struct ZkServiceClient {
    client: Client,
    base_url: String,
}

impl ZkServiceClient {
    /// Create a new client with the given base URL
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
        }
    }
    
    /// Create a new client with custom timeout
    pub fn with_timeout(base_url: impl Into<String>, timeout: Duration) -> Result<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .build()?;
        
        Ok(Self {
            client,
            base_url: base_url.into(),
        })
    }
    
    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}
```

### Submit Job (JSON Format)

```rust
// src/client.rs (continued)

impl ZkServiceClient {
    /// Submit a job using JSON format
    pub async fn submit_job(&self, request: SubmitJobRequest) -> Result<SubmitJobResponse> {
        // Convert InputData to JSON value
        let data = match request.data {
            InputData::RawBytes(bytes) => {
                serde_json::json!(bytes)
            }
            InputData::Table { columns, column_order } => {
                serde_json::json!({
                    "columns": columns,
                    "column_order": column_order
                })
            }
        };
        
        // Determine data_type
        let data_type = match request.data {
            InputData::RawBytes(_) => "raw_bytes",
            InputData::Table { .. } => "table",
        };
        
        // Build request body
        let body = serde_json::json!({
            "circuit_id": request.circuit_id,
            "operators": request.operators,
            "data_type": data_type,
            "data": data,
            "webhook_url": request.webhook_url,
        });
        
        let url = format!("{}/job/submit", self.base_url);
        
        let response = self.client
            .post(&url)
            .json(&body)
            .send()
            .await?;
        
        // Handle errors
        let status = response.status();
        if !status.is_success() {
            return self.handle_error_response(response).await;
        }
        
        let result: SubmitJobResponse = response.json().await?;
        Ok(result)
    }
}
```

### Submit Job (Multipart Format)

```rust
// src/client.rs (continued)

impl ZkServiceClient {
    /// Submit a job using multipart format
    pub async fn submit_job_multipart(&self, request: SubmitJobRequest) -> Result<SubmitJobResponse> {
        use reqwest::multipart;
        
        // Serialize operators to JSON string
        let operators_json = serde_json::to_string(&request.operators)?;
        
        // Build multipart form
        let mut form = multipart::Form::new()
            .text("circuit_id", request.circuit_id)
            .text("webhook_url", request.webhook_url)
            .text("operators", operators_json);
        
        // Add data fields based on type
        match request.data {
            InputData::RawBytes(rows) => {
                for (index, row) in rows.into_iter().enumerate() {
                    form = form.part(
                        format!("data[{}]", index),
                        multipart::Part::bytes(row),
                    );
                }
            }
            InputData::Table { columns, column_order } => {
                // Build row index map
                let max_rows = columns.values()
                    .map(|v| v.len())
                    .max()
                    .unwrap_or(0);
                
                for row_idx in 0..max_rows {
                    for col_name in &column_order {
                        if let Some(col_values) = columns.get(col_name) {
                            if let Some(value) = col_values.get(row_idx) {
                                // Convert value to string
                                let value_str = match value {
                                    serde_json::Value::Number(n) => n.to_string(),
                                    serde_json::Value::String(s) => s.clone(),
                                    _ => value.to_string(),
                                };
                                form = form.text(
                                    format!("data[{}][{}]", row_idx, col_name),
                                    value_str,
                                );
                            }
                        }
                    }
                }
            }
        }
        
        let url = format!("{}/job/submit", self.base_url);
        
        let response = self.client
            .post(&url)
            .multipart(form)
            .send()
            .await?;
        
        let status = response.status();
        if !status.is_success() {
            return self.handle_error_response(response).await;
        }
        
        let result: SubmitJobResponse = response.json().await?;
        Ok(result)
    }
}
```

### Other Endpoints

```rust
// src/client.rs (continued)

impl ZkServiceClient {
    /// Get job status
    pub async fn get_job_status(&self, job_id: Uuid) -> Result<JobStatusResponse> {
        let url = format!("{}/job/{}", self.base_url, job_id);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
        
        let status = response.status();
        if status == 404 {
            return Err(ZkServiceError::JobNotFound(job_id.to_string()));
        }
        
        if !status.is_success() {
            return self.handle_error_response(response).await;
        }
        
        let result: JobStatusResponse = response.json().await?;
        Ok(result)
    }
    
    /// List available circuits
    pub async fn list_circuits(&self) -> Result<CircuitsResponse> {
        let url = format!("{}/circuits", self.base_url);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return self.handle_error_response(response).await;
        }
        
        let result: CircuitsResponse = response.json().await?;
        Ok(result)
    }
    
    /// Health check
    pub async fn health(&self) -> Result<HealthResponse> {
        let url = format!("{}/health", self.base_url);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return self.handle_error_response(response).await;
        }
        
        let result: HealthResponse = response.json().await?;
        Ok(result)
    }
    
    /// Wait for job completion (polling)
    pub async fn wait_for_job(
        &self,
        job_id: Uuid,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<JobStatusResponse> {
        let start = std::time::Instant::now();
        
        loop {
            let status = self.get_job_status(job_id).await?;
            
            match status.status.as_str() {
                "completed" | "failed" => {
                    return Ok(status);
                }
                _ => {
                    if start.elapsed() > timeout {
                        return Err(ZkServiceError::InvalidInput(
                            format!("Job {} did not complete within timeout", job_id)
                        ));
                    }
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }
    
    /// Helper to handle error responses
    async fn handle_error_response(&self, response: reqwest::Response) -> Result<()> {
        let status = response.status();
        let error_text = response.text().await?;
        
        // Try to parse as ErrorResponse
        if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&error_text) {
            let error_msg = error_response.error;
            
            return match status.as_u16() {
                400 => Err(ZkServiceError::InvalidInput(error_msg)),
                404 if error_msg.contains("Circuit") => {
                    Err(ZkServiceError::CircuitNotFound(error_msg))
                }
                404 if error_msg.contains("Job") => {
                    Err(ZkServiceError::JobNotFound(error_msg))
                }
                503 if error_msg.contains("Queue") => {
                    Err(ZkServiceError::QueueFull)
                }
                _ => Err(ZkServiceError::Api(error_msg)),
            };
        }
        
        Err(ZkServiceError::Api(format!("HTTP {}: {}", status, error_text)))
    }
}
```

## Builder Pattern (Optional)

For a more ergonomic API, you can add builder methods:

```rust
// src/zk_service/builder.rs

use crate::types::*;

/// Builder for job submission requests
pub struct JobRequestBuilder {
    circuit_id: Option<String>,
    operators: Vec<Operator>,
    data: Option<InputData>,
    webhook_url: Option<String>,
}

impl JobRequestBuilder {
    pub fn new() -> Self {
        Self {
            circuit_id: None,
            operators: Vec::new(),
            data: None,
            webhook_url: None,
        }
    }
    
    pub fn circuit_id(mut self, circuit_id: impl Into<String>) -> Self {
        self.circuit_id = Some(circuit_id.into());
        self
    }
    
    pub fn operator(mut self, operator: Operator) -> Self {
        self.operators.push(operator);
        self
    }
    
    pub fn raw_bytes(mut self, data: Vec<Vec<u8>>) -> Self {
        self.data = Some(InputData::RawBytes(data));
        self
    }
    
    pub fn table(
        mut self,
        columns: std::collections::HashMap<String, Vec<serde_json::Value>>,
        column_order: Vec<String>,
    ) -> Self {
        self.data = Some(InputData::Table { columns, column_order });
        self
    }
    
    pub fn webhook_url(mut self, url: impl Into<String>) -> Self {
        self.webhook_url = Some(url.into());
        self
    }
    
    pub fn build(self) -> Result<SubmitJobRequest, String> {
        Ok(SubmitJobRequest {
            circuit_id: self.circuit_id.ok_or("circuit_id is required")?,
            operators: self.operators,
            data: self.data.ok_or("data is required")?,
            webhook_url: self.webhook_url.ok_or("webhook_url is required")?,
        })
    }
}

impl Default for JobRequestBuilder {
    fn default() -> Self {
        Self::new()
    }
}
```

## Usage Examples

### Basic Usage

```rust
// If module is declared as `pub mod zk_service;`
use your_crate::zk_service::{ZkServiceClient, SubmitJobRequest, Operator, DataSelection, SelectionCount, InputData};
// Or if re-exported at crate root:
// use your_crate::zk_service::*;

use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let client = ZkServiceClient::new("http://localhost:3333");
    
    // Submit job with raw bytes
    let request = SubmitJobRequest {
        circuit_id: "v1_16_24_4".to_string(),
        operators: vec![Operator::Merkle16 {
            selection: DataSelection {
                start: 0,
                offset: 1,
                count: SelectionCount::All,
            },
            handler: "0x0000000000000000000000000000000000000000".to_string(),
        }],
        data: InputData::RawBytes(vec![
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
        ]),
        webhook_url: "http://localhost:8080/webhook".to_string(),
    };
    
    let response = client.submit_job(request).await?;
    println!("Job ID: {}", response.job_id);
    
    // Check status
    let status = client.get_job_status(response.job_id).await?;
    println!("Status: {}", status.status);
    
    Ok(())
}
```

### Using Builder Pattern

```rust
use your_crate::zk_service::{ZkServiceClient, JobRequestBuilder, Operator, DataSelection, SelectionCount};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ZkServiceClient::new("http://localhost:3333");
    
    // Build request with table data
    let mut columns = HashMap::new();
    columns.insert("price".to_string(), vec![json!(100), json!(200), json!(300)]);
    columns.insert("amount".to_string(), vec![json!(1), json!(2), json!(3)]);
    
    let request = JobRequestBuilder::new()
        .circuit_id("v1_16_24_4")
        .operator(Operator::Merkle16 {
            selection: DataSelection {
                start: 0,
                offset: 1,
                count: SelectionCount::All,
            },
            handler: "0x0000000000000000000000000000000000000000".to_string(),
        })
        .table(columns, vec!["price".to_string(), "amount".to_string()])
        .webhook_url("http://localhost:8080/webhook")
        .build()?;
    
    let response = client.submit_job(request).await?;
    println!("Job submitted: {}", response.job_id);
    
    // Wait for completion
    let final_status = client
        .wait_for_job(
            response.job_id,
            std::time::Duration::from_millis(500),
            std::time::Duration::from_secs(60),
        )
        .await?;
    
    if let Some(result) = final_status.result {
        println!("Proof: {}", result.proof);
    }
    
    Ok(())
}
```

### Multipart Submission

```rust
use your_crate::zk_service::{ZkServiceClient, SubmitJobRequest, Operator, DataSelection, SelectionCount, InputData};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ZkServiceClient::new("http://localhost:3333");
    
    let request = SubmitJobRequest {
        circuit_id: "v1_16_24_4".to_string(),
        operators: vec![Operator::Merkle16 {
            selection: DataSelection {
                start: 0,
                offset: 1,
                count: SelectionCount::All,
            },
            handler: "0x0000000000000000000000000000000000000000".to_string(),
        }],
        data: InputData::RawBytes(vec![
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
        ]),
        webhook_url: "http://localhost:8080/webhook".to_string(),
    };
    
    // Use multipart format
    let response = client.submit_job_multipart(request).await?;
    println!("Job ID: {}", response.job_id);
    
    Ok(())
}
```

### List Circuits and Health Check

```rust
use your_crate::zk_service::ZkServiceClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ZkServiceClient::new("http://localhost:3333");
    
    // Health check
    let health = client.health().await?;
    println!("Service status: {}", health.status);
    println!("Circuits loaded: {}", health.circuits_loaded);
    println!("Workers: {}/{}", health.workers.available, health.workers.total);
    
    // List circuits
    let circuits = client.list_circuits().await?;
    for circuit in circuits.circuits {
        println!("Circuit: {} (max_data: {}, max_ops: {})",
            circuit.id,
            circuit.params.max_data,
            circuit.params.max_ops
        );
    }
    
    Ok(())
}
```

## Error Handling

```rust
use your_crate::zk_service::{ZkServiceClient, ZkServiceError};

#[tokio::main]
async fn main() {
    let client = ZkServiceClient::new("http://localhost:3333");
    
    match client.health().await {
        Ok(health) => println!("Service is healthy"),
        Err(ZkServiceError::Http(e)) => {
            eprintln!("Network error: {}", e);
        }
        Err(ZkServiceError::Api(msg)) => {
            eprintln!("API error: {}", msg);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}
```

## Module Declaration

Create `src/zk_service/mod.rs` to declare and re-export all submodules:

```rust
// src/zk_service/mod.rs

pub mod client;
pub mod error;
pub mod types;

// Re-export main types
pub use client::ZkServiceClient;
pub use error::{ZkServiceError, Result};
pub use types::*;

// Optional: re-export builder
#[cfg(feature = "builder")]
pub mod builder;
#[cfg(feature = "builder")]
pub use builder::JobRequestBuilder;
```

Then in your main `src/lib.rs`:

```rust
// src/lib.rs
pub mod zk_service;
```

Or if you prefer a different module name:

```rust
// src/lib.rs
pub mod zk_service as zk;  // Use as `zk::ZkServiceClient`
```

## Testing

Add test dependencies:

```toml
[dev-dependencies]
tokio = { version = "1.0", features = ["full"] }
tokio-test = "0.4"
```

Example test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_submit_job() {
        let client = ZkServiceClient::new("http://localhost:3333");
        
        let request = SubmitJobRequest {
            circuit_id: "v1_16_24_4".to_string(),
            operators: vec![Operator::Merkle16 {
                selection: DataSelection {
                    start: 0,
                    offset: 1,
                    count: SelectionCount::All,
                },
                handler: "0x0000000000000000000000000000000000000000".to_string(),
            }],
            data: InputData::RawBytes(vec![
                vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
            ]),
            webhook_url: "http://localhost:8080/webhook".to_string(),
        };
        
        let result = client.submit_job(request).await;
        assert!(result.is_ok());
    }
}
```

## Features

Consider adding these optional features:

```toml
[features]
default = []
builder = []  # Enable builder pattern
retry = ["reqwest/retry"]  # Enable automatic retries
```

## Additional Considerations

1. **Retry Logic**: Add automatic retry for transient errors
2. **Rate Limiting**: Implement rate limiting if needed
3. **Connection Pooling**: Reuse HTTP connections
4. **Logging**: Add optional logging support
5. **Metrics**: Add optional metrics collection
6. **Async Traits**: Use `async-trait` if you want trait-based design
7. **Streaming**: Support streaming for large data uploads

## Integration into Existing Project

### Step 1: Add Dependencies

Add the required dependencies to your existing `Cargo.toml`:

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json", "multipart"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.0", features = ["serde", "v4"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1.0"

[dev-dependencies]
tokio = { version = "1.0", features = ["full"] }
tokio-test = "0.4"
```

### Step 2: Create Module Directory

Create the directory structure:

```bash
mkdir -p src/zk_service
```

### Step 3: Create Module Files

Create the following files:
- `src/zk_service/mod.rs` - Module declaration
- `src/zk_service/client.rs` - Client implementation
- `src/zk_service/types.rs` - Type definitions
- `src/zk_service/error.rs` - Error types
- `src/zk_service/builder.rs` - Builder pattern (optional)

### Step 4: Declare Module

In your `src/lib.rs` (or `src/main.rs`):

```rust
pub mod zk_service;
```

### Step 5: Use the Module

```rust
use your_crate::zk_service::ZkServiceClient;

let client = ZkServiceClient::new("http://localhost:3333");
```

This documentation provides a complete specification for implementing the `zk-service` HTTP client crate. Follow the structure and examples to create a production-ready client library.
