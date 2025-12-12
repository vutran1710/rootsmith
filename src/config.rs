use clap::Parser;
use serde::{Deserialize, Serialize};

/// Base configuration for the app.
/// Concrete CLI parsing (clap) can be built on top of this.
#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
#[command(name = "rootsmith")]
#[command(about = "Cryptographic accumulator app with RocksDB and pluggable backends")]
pub struct BaseConfig {
    /// Path for persistent storage (e.g. RocksDB).
    #[arg(long, default_value = "./data")]
    pub storage_path: String,

    /// Duration of a logical batch window in seconds
    /// (e.g. 86400 for "one commitment per namespace per day").
    #[arg(long, default_value_t = 86400)]
    pub batch_interval_secs: u64,

    /// Whether to auto-flush and commit when a batch window closes.
    #[arg(long, default_value_t = true)]
    pub auto_commit: bool,

    /// Optional: maximum number of leaves per batch before forcing a commit.
    #[arg(long)]
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
