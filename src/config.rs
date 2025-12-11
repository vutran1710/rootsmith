use anyhow::Result;
use serde::{Deserialize, Serialize};

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
