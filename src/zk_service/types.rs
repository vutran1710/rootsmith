use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

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
        handler: String, // 20-byte hex address with 0x prefix
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
