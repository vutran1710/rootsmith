use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};

/// Type of upstream connector to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum UpstreamType {
    /// HTTP server upstream.
    Http,
    /// WebSocket upstream.
    WebSocket,
    /// Kafka upstream.
    Kafka,
    /// SQS upstream.
    Sqs,
    /// MQTT upstream.
    Mqtt,
    /// No-op upstream (does nothing).
    Noop,
    /// Channel-based upstream (for testing and benchmarking).
    PubChannel,
}

impl Default for UpstreamType {
    fn default() -> Self {
        UpstreamType::PubChannel
    }
}

/// Type of commitment registry to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum CommitmentRegistryType {
    /// Smart contract commitment registry.
    Contract,
    /// No-op commitment registry (does nothing).
    Noop,
    /// Mock commitment registry (for testing).
    Mock,
}

impl Default for CommitmentRegistryType {
    fn default() -> Self {
        CommitmentRegistryType::Mock
    }
}

/// Type of proof registry to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum ProofRegistryType {
    /// S3 proof registry.
    S3,
    /// GitHub proof registry.
    Github,
    /// No-op proof registry (does nothing).
    Noop,
    /// Mock proof registry (for testing).
    Mock,
}

impl Default for ProofRegistryType {
    fn default() -> Self {
        ProofRegistryType::Mock
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
