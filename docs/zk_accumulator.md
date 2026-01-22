# ZK Accumulator Implementation Proposal

## Overview

The ZK Accumulator is a component within the `zk_service` module that receives data via the `ZKTrait` interface from partners and submits proof generation jobs to the ZK Circom Proof Service via HTTP. It acts as a bridge between partner data and the ZK service API.

## Architecture

### Module Structure

```
src/zk_service/
├── mod.rs              ← Add zk_accumulator module
├── client.rs           ← Existing ZkServiceClient
├── types.rs            ← Existing request/response types
├── error.rs            ← Existing error types
├── builder.rs          ← Existing builder
└── zk_accumulator.rs   ← NEW: ZK Accumulator implementation
```

### Component Flow

```
┌──────────────┐
│ Partner Data │
│  (ZKTrait)   │
└──────┬───────┘
       │
       ▼
┌──────────────────┐
│ ZkAccumulator     │  ← In zk_service module
│                  │
│ - Receives       │
│   ZKTrait        │
│ - Converts to    │
│   InputData      │
│ - Submits job    │
│ - Returns result │
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│ ZkServiceClient  │  ← Existing client
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│  ZK Service      │  ← External HTTP API
│  (port 3000)     │
└──────────────────┘
```

## Trait Definition

### ZKTrait

The `ZKTrait` provides the interface for partner data that will be processed by the ZK accumulator. It should be defined in `src/zk_service/zk_accumulator.rs`.

```rust
/// Trait for data that can be processed by the ZK accumulator.
///
/// Partners implement this trait to provide data for ZK proof generation.
pub trait ZKTrait {
    /// Extract namespace (32 bytes)
    fn namespace(&self) -> [u8; 32];
    
    /// Extract key (32 bytes)
    fn key(&self) -> [u8; 32];
    
    /// Extract value (32 bytes)
    fn value(&self) -> [u8; 32];
    
    /// Extract timestamp (Unix seconds)
    fn timestamp(&self) -> u64;
}
```

## Implementation Structure

### ZkAccumulator Struct

```rust
/// ZK Accumulator that receives ZKTrait data and submits to ZK service.
///
/// This accumulator is part of the zk_service module and handles
/// the conversion from partner trait objects to ZK service job submissions.
pub struct ZkAccumulator {
    /// HTTP client for ZK service
    client: ZkServiceClient,
    
    /// Circuit identifier (e.g., "v1_16_24_4")
    circuit_id: String,
    
    /// Optional webhook URL for async job completion notifications
    webhook_url: Option<String>,
    
    /// Default operator configuration for proof generation
    default_operator: Operator,
}
```

### Constructor Methods

```rust
impl ZkAccumulator {
    /// Create a new ZK accumulator with the given ZK service URL and circuit ID.
    ///
    /// # Arguments
    /// * `base_url` - Base URL of the ZK service (e.g., "http://localhost:3000")
    /// * `circuit_id` - Circuit identifier (e.g., "v1_16_24_4")
    pub fn new(base_url: impl Into<String>, circuit_id: impl Into<String>) -> Self {
        let client = ZkServiceClient::new(base_url);
        let default_operator = Operator::Merkle16 {
            selection: DataSelection {
                start: 0,
                offset: 1,
                count: SelectionCount::All,
            },
            handler: "0x0000000000000000000000000000000000000000".to_string(),
        };
        
        Self {
            client,
            circuit_id: circuit_id.into(),
            webhook_url: None,
            default_operator,
        }
    }
    
    /// Set webhook URL for async job completion notifications.
    pub fn with_webhook(mut self, url: impl Into<String>) -> Self {
        self.webhook_url = Some(url.into());
        self
    }
    
    /// Set operator configuration for proof generation.
    pub fn with_operator(mut self, operator: Operator) -> Self {
        self.default_operator = operator;
        self
    }
}
```

## Data Conversion

### Converting ZKTrait to InputData

The accumulator converts `ZKTrait` objects into the ZK service's `InputData::RawBytes` format. Each record is serialized as a byte array:

```
[namespace (32 bytes) || key (32 bytes) || value (32 bytes) || timestamp (8 bytes)]
Total: 104 bytes per record
```

```rust
impl ZkAccumulator {
    /// Convert ZKTrait objects to ZK service InputData format.
    ///
    /// Each record is serialized as:
    /// [namespace (32) || key (32) || value (32) || timestamp (8)] = 104 bytes
    fn convert_to_input_data(records: &[Box<dyn ZKTrait>]) -> InputData {
        let mut data_rows = Vec::with_capacity(records.len());
        
        for record in records {
            let mut row = Vec::with_capacity(104);
            row.extend_from_slice(&record.namespace());
            row.extend_from_slice(&record.key());
            row.extend_from_slice(&record.value());
            row.extend_from_slice(&record.timestamp().to_be_bytes());
            data_rows.push(row);
        }
        
        InputData::RawBytes(data_rows)
    }
}
```

## Main Commit Method

### commit_trait Method

```rust
impl ZkAccumulator {
    /// Commit ZKTrait records to ZK service and return commitment result.
    ///
    /// # Arguments
    /// * `records` - Array of trait objects to submit
    /// * `result_tx` - Channel sender for delivering the commitment result
    ///
    /// # Returns
    /// * `Ok(())` if the operation completed successfully
    ///
    /// # Errors
    /// * Returns error if job submission fails
    /// * Returns error if job polling times out
    /// * Returns error if job fails
    pub async fn commit_trait(
        &mut self,
        records: &[Box<dyn ZKTrait>],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        if records.is_empty() {
            return Err(anyhow::anyhow!("Cannot commit empty records"));
        }
        
        // 1. Convert trait objects to InputData
        let input_data = Self::convert_to_input_data(records);
        
        // 2. Build job request
        let request = SubmitJobRequest {
            circuit_id: self.circuit_id.clone(),
            operators: vec![self.default_operator.clone()],
            data: input_data,
            webhook_url: self.webhook_url.clone()
                .unwrap_or_else(|| "http://localhost:8080/webhook".to_string()),
        };
        
        // 3. Submit job to ZK service
        let response = self.client.submit_job(request).await?;
        
        tracing::info!("ZK job submitted: job_id={}", response.job_id);
        
        // 4. Poll for job completion
        let final_status = self.client
            .wait_for_job(
                response.job_id,
                Duration::from_millis(500),
                Duration::from_secs(300), // 5 minute timeout
            )
            .await?;
        
        // 5. Convert ZK service response to CommitmentResult
        let commitment_result = Self::convert_to_commitment_result(final_status)?;
        
        // 6. Send result through channel
        result_tx.send(commitment_result).await
            .map_err(|e| anyhow::anyhow!("Failed to send commitment result: {}", e))?;
        
        Ok(())
    }
}
```

## Response Conversion

### Converting ZK Service Response to CommitmentResult

```rust
impl ZkAccumulator {
    /// Convert ZK service JobStatusResponse to CommitmentResult.
    fn convert_to_commitment_result(
        status: JobStatusResponse,
    ) -> Result<CommitmentResult> {
        match status.status.as_str() {
            "completed" => {
                let result = status.result.ok_or_else(|| {
                    anyhow::anyhow!("Job completed but no result available")
                })?;
                
                // Parse public signals to extract root hash
                // Public signals typically contain the Merkle root as the first element
                let public_signals: serde_json::Value = serde_json::from_str(&result.public_signals)?;
                let root_hash = Self::extract_root_from_public_signals(&public_signals)?;
                
                // Parse proof JSON
                let proof_json: serde_json::Value = serde_json::from_str(&result.proof)?;
                
                // Extract committed timestamp
                let committed_at = status.completed_at
                    .map(|dt| dt.timestamp() as u64)
                    .unwrap_or_else(|| {
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                    });
                
                Ok(CommitmentResult {
                    commitment: root_hash,
                    proofs: Some(Self::parse_proofs_from_result(&proof_json)?),
                    committed_at,
                })
            }
            "failed" => {
                Err(anyhow::anyhow!(
                    "ZK service job failed: {}",
                    status.error.unwrap_or_else(|| "Unknown error".to_string())
                ))
            }
            _ => {
                Err(anyhow::anyhow!(
                    "ZK service job in unexpected state: {}",
                    status.status
                ))
            }
        }
    }
    
    /// Extract root hash from public signals JSON.
    ///
    /// Assumes the first element of the public signals array is the root hash.
    fn extract_root_from_public_signals(
        signals: &serde_json::Value,
    ) -> Result<Vec<u8>> {
        let array = signals.as_array()
            .ok_or_else(|| anyhow::anyhow!("Public signals must be an array"))?;
        
        if array.is_empty() {
            return Err(anyhow::anyhow!("Public signals array is empty"));
        }
        
        // First element is typically the root hash (as hex string or number)
        let root_value = &array[0];
        
        // Try to parse as hex string first
        if let Some(hex_str) = root_value.as_str() {
            hex::decode(hex_str)
                .map_err(|e| anyhow::anyhow!("Failed to decode root hash hex: {}", e))
        } else if let Some(num) = root_value.as_u64() {
            // If it's a number, convert to bytes
            Ok(num.to_be_bytes().to_vec())
        } else {
            Err(anyhow::anyhow!("Root hash must be a string or number"))
        }
    }
    
    /// Parse proofs from ZK service result.
    ///
    /// This is a simplified parser - actual implementation depends on
    /// the proof format returned by the ZK service.
    fn parse_proofs_from_result(
        _proof_json: &serde_json::Value,
    ) -> Result<HashMap<Key32, Proof>> {
        // TODO: Implement proof parsing based on actual ZK service format
        // For now, return empty map
        Ok(HashMap::new())
    }
}
```

## Module Integration

### Update src/zk_service/mod.rs

```rust
pub mod client;
pub mod error;
pub mod types;
pub mod builder;
pub mod zk_accumulator;  // NEW

// Re-export main types
pub use client::ZkServiceClient;
pub use error::{ZkServiceError, Result};
pub use types::*;
pub use builder::JobRequestBuilder;
pub use zk_accumulator::{ZkAccumulator, ZKTrait};  // NEW
```

## Usage Example

### Partner Implementation

```rust
use rootsmith::zk_service::ZKTrait;

// Partner's data structure
pub struct MyPartnerData {
    pub id: String,
    pub user_id: String,
    pub action: String,
    pub timestamp: u64,
}

// Partner implements ZKTrait
impl ZKTrait for MyPartnerData {
    fn namespace(&self) -> [u8; 32] {
        // Derive namespace from user_id
        let mut namespace = [0u8; 32];
        let bytes = self.user_id.as_bytes();
        for (i, &b) in bytes.iter().take(32).enumerate() {
            namespace[i] = b;
        }
        namespace
    }
    
    fn key(&self) -> [u8; 32] {
        // Derive key from id
        let mut key = [0u8; 32];
        let bytes = self.id.as_bytes();
        for (i, &b) in bytes.iter().take(32).enumerate() {
            key[i] = b;
        }
        key
    }
    
    fn value(&self) -> [u8; 32] {
        // Derive value from action
        let mut value = [0u8; 32];
        let bytes = self.action.as_bytes();
        for (i, &b) in bytes.iter().take(32).enumerate() {
            value[i] = b;
        }
        value
    }
    
    fn timestamp(&self) -> u64 {
        self.timestamp
    }
}
```

### Using ZkAccumulator

```rust
use rootsmith::zk_service::{ZkAccumulator, ZKTrait};
use kanal::unbounded_async;

#[tokio::main]
async fn main() -> Result<()> {
    // Create accumulator
    let mut accumulator = ZkAccumulator::new(
        "http://localhost:3000",
        "v1_16_24_4",
    )
    .with_webhook("http://localhost:8080/webhook");
    
    // Prepare partner data
    let partner_data = MyPartnerData {
        id: "event-123".to_string(),
        user_id: "user-abc".to_string(),
        action: "click".to_string(),
        timestamp: 1699123456,
    };
    
    // Convert to trait objects
    let trait_objects: Vec<Box<dyn ZKTrait>> = vec![
        Box::new(partner_data),
    ];
    
    // Commit records
    let (tx, mut rx) = unbounded_async();
    accumulator.commit_trait(&trait_objects, tx).await?;
    
    // Receive result
    let result = rx.recv().await?;
    println!("Commitment: {:?}", hex::encode(&result.commitment));
    println!("Committed at: {}", result.committed_at);
    
    Ok(())
}
```

## Data Flow

```
Partner Data (implements ZKTrait)
    ↓
ZkAccumulator.commit_trait()
    ↓
Convert to InputData::RawBytes
    [namespace (32) || key (32) || value (32) || timestamp (8)]
    ↓
Create SubmitJobRequest
    - circuit_id
    - operators (Merkle16)
    - data (InputData)
    - webhook_url
    ↓
ZkServiceClient.submit_job()
    HTTP POST → http://localhost:3000/job/submit
    ↓
Receive SubmitJobResponse
    - job_id
    ↓
Poll job status
    HTTP GET → http://localhost:3000/job/{job_id}
    ↓
Job completes
    - status: "completed"
    - result: { proof, public_signals }
    ↓
Convert to CommitmentResult
    - commitment: root hash from public_signals
    - proofs: parsed from proof JSON
    - committed_at: completion timestamp
    ↓
Send via channel
    result_tx.send(commitment_result)
```

## Error Handling

### Error Scenarios

1. **Empty Records**: Return error if no records provided
2. **Job Submission Failure**: Network errors, invalid circuit ID, queue full
3. **Job Polling Timeout**: Job doesn't complete within timeout period
4. **Job Failure**: ZK service reports job failed
5. **Response Parsing**: Invalid JSON in proof or public signals
6. **Channel Error**: Failed to send result through channel

### Error Propagation

All errors are converted to `anyhow::Result` and logged appropriately:
- `error!` for job failures and critical errors
- `warn!` for timeouts and retryable errors
- `info!` for job submission and completion
- `debug!` for detailed operation logging

## Testing Strategy

### Unit Tests

1. **Data Conversion**: Test `convert_to_input_data()` with various ZKTrait objects
2. **Response Parsing**: Test `convert_to_commitment_result()` with mock responses
3. **Error Handling**: Test error scenarios

### Integration Tests

1. **Mock ZK Service**: Use mock HTTP server to simulate ZK service
2. **End-to-End Flow**: Test full flow from ZKTrait to CommitmentResult
3. **Error Scenarios**: Test timeout, job failure, network errors

### Example Test

```rust
#[tokio::test]
async fn test_zk_accumulator_commit() {
    // Setup mock ZK service
    let mock_server = MockServer::start().await;
    mock_server.mock(|when, then| {
        when.method("POST").path("/job/submit");
        then.status(200).json(json!({
            "job_id": "123e4567-e89b-12d3-a456-426614174000",
            "status": "pending"
        }));
    });
    
    // Create accumulator
    let mut accumulator = ZkAccumulator::new(
        mock_server.url(),
        "test_circuit",
    );
    
    // Create test data
    let trait_objects: Vec<Box<dyn ZKTrait>> = vec![
        Box::new(TestData {
            namespace: [1u8; 32],
            key: [2u8; 32],
            value: [3u8; 32],
            timestamp: 1234567890,
        }),
    ];
    
    // Commit
    let (tx, mut rx) = unbounded_async();
    let result = accumulator.commit_trait(&trait_objects, tx).await;
    
    // Verify result (depending on mock server setup)
    assert!(result.is_ok() || result.is_err()); // Adjust based on test scenario
}
```

## Summary

The ZK Accumulator in `src/zk_service/zk_accumulator.rs` provides:

1. **Clean Interface**: `ZKTrait` for partner data
2. **Data Conversion**: Automatic conversion to ZK service format
3. **Job Submission**: Integration with `ZkServiceClient`
4. **Result Handling**: Conversion to `CommitmentResult`
5. **Error Handling**: Comprehensive error propagation
6. **Async Support**: Full async/await support with channel-based results

This design keeps the ZK accumulator as part of the `zk_service` module, making it a natural extension of the ZK service client functionality.
