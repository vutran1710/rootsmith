use crate::accumulator::AccumulatorConfig;
use crate::archiver::ArchiveConfig;
use crate::downstream::DownstreamConfig;
use crate::upstream::UpstreamConfig;

// TODO: implement this as clap config + toml deserializable struct
pub struct Config {
    pub storage_path: String,

    pub upstream: UpstreamConfig,

    pub accumulator: AccumulatorConfig,

    pub archive: ArchiveConfig,

    pub downstream: DownstreamConfig,
    // TODO: rest goes here
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
