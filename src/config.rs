use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Base configuration for the app.
/// Concrete CLI parsing (clap) can be built on top of this.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseConfig {
    /// Path for persistent storage (e.g. RocksDB).
    pub storage_path: String,

    /// Duration of a logical batch window in seconds
    /// (e.g. 86400 for "one commitment per namespace per day").
    pub batch_interval_secs: u64,

    /// Whether to auto-flush and commit when a batch window closes.
    pub auto_commit: bool,

    /// Optional: maximum number of leaves per batch before forcing a commit.
    pub max_batch_leaves: Option<u64>,
}

impl Default for BaseConfig {
    fn default() -> Self {
        BaseConfig {
            storage_path: "./data".to_string(),
            batch_interval_secs: 86400, // 1 day
            auto_commit: true,
            max_batch_leaves: None,
        }
    }
}

// Legacy config - kept for backwards compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub upstream: UpstreamConfig,
    pub downstream: DownstreamConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamConfig {
    pub source_type: String,
    pub connection_string: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownstreamConfig {
    pub commitment_type: String,
    pub proof_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub path: String,
    pub max_size_mb: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            upstream: UpstreamConfig {
                source_type: "websocket".to_string(),
                connection_string: "ws://localhost:8080".to_string(),
            },
            downstream: DownstreamConfig {
                commitment_type: "noop".to_string(),
                proof_type: "s3".to_string(),
            },
            storage: StorageConfig {
                path: "./data".to_string(),
                max_size_mb: 1024,
            },
        }
    }
}

pub fn load() -> Result<Config> {
    Ok(Config::default())
}
