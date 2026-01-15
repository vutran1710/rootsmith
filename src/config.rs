use clap::Parser;
use clap::ValueEnum;
use serde::Deserialize;
use serde::Serialize;

/// Type of upstream connector to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum UpstreamType {
    /// HTTP server upstream.
    Http,
    /// WebSocket upstream.
    WebSocket,
    /// No-op upstream (does nothing).
    Noop,
    /// Mock upstream (for testing).
    Mock,
}

impl Default for UpstreamType {
    fn default() -> Self {
        UpstreamType::Mock
    }
}

/// Type of downstream handler to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum DownstreamType {
    /// S3 downstream.
    S3,
    /// Blackhole downstream (discards data).
    Blackhole,
    /// Mock downstream (for testing).
    Mock,
}

impl Default for DownstreamType {
    fn default() -> Self {
        DownstreamType::Mock
    }
}

/// Type of cryptographic accumulator backend to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum AccumulatorType {
    /// Merkle tree accumulator.
    Merkle,
    /// Sparse Merkle tree accumulator.
    SparseMerkle,
}

impl Default for AccumulatorType {
    fn default() -> Self {
        AccumulatorType::Merkle
    }
}

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

    /// Type of cryptographic accumulator backend to use.
    #[arg(long, value_enum, default_value_t = AccumulatorType::default())]
    pub accumulator_type: AccumulatorType,
}

impl Default for BaseConfig {
    fn default() -> Self {
        BaseConfig {
            storage_path: "./data".to_string(),
            batch_interval_secs: 86400, // 1 day
            auto_commit: true,
            accumulator_type: AccumulatorType::default(),
        }
    }
}
