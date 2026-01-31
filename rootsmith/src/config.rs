use crate::accumulator::AccumulatorConfig;
use crate::archiver::ArchiveConfig;
use crate::downstream::DownstreamConfig;
use crate::upstream::UpstreamConfig;

// TODO: implement this as clap config + toml deserializable struct
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Config {
    pub storage_path: String,

    pub http_port: u16,

    pub upstream: UpstreamConfig,

    pub source_path: String,

    pub accumulator: AccumulatorConfig,

    pub archive: ArchiveConfig,

    pub downstream: DownstreamConfig,
    // TODO: rest goes here
}

impl Default for Config {
    fn default() -> Self {
        Self {
            storage_path: "./data".to_string(),
            http_port: 9000,
            source_path: "./examples/plugin/src/lib.rs".to_string(),
            upstream: UpstreamConfig::Http {
                port: 8080,
                api_key: None,
            },
            accumulator: AccumulatorConfig::Merkle,
            archive: ArchiveConfig::File {
                directory: "./archive".to_string(),
            },
            downstream: DownstreamConfig::Blackhole,
        }
    }
}

// Expected TOML config (not exactly but close enough):
// ```toml
// storage_path = "/path/to/storage"
//
// [upstream.s3]
// bucket = "my-bucket"
// region = "us-west-2"
// access_key = "your-access-key"
// secret_key = "your-secret-key"
//
// [accumulator.external]
// endpoint = "https://external-accumulator.example.com"
// api_key = "your-api-key"
//
// [archive.local]
// path = "/path/to/archive"
// max_size_mb = 10240
//
// [downstream.http]
// base_url = "https://downstream-service.example.com"
// timeout_seconds = 30
// ```
// NOTE The first variant found will be used.
