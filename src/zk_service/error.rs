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
