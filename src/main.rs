use anyhow::Result;
use tracing::info;

mod accumulator;
mod archiver;
mod config;
mod downstream;
mod parser;
mod rootsmith;
mod storage;
mod telemetry;
mod types;
mod upstream;
mod utils;
mod wasm_host;

use accumulator::{AccumulatorConfig, AccumulatorVariant};
use archiver::{ArchiveConfig, ArchiveVariant};
use config::Config;
use downstream::{DownstreamConfig, DownstreamVariant};
use rootsmith::RootSmith;
use storage::Storage;
use upstream::{UpstreamConfig, UpstreamConnector, UpstreamVariant};
use wasm_host::{WasmLimits, WasmPluginHost};

fn main() -> Result<()> {
    telemetry::init();
    info!("Starting rootsmith");

    let config = Config {
        storage_path: "./data".to_string(),
        upstream: UpstreamConfig::Http {
            port: 8080,
            api_key: None,
        },
        accumulator: AccumulatorConfig::Merkle,
        archive: ArchiveConfig::File {
            directory: "./archive".to_string(),
        },
        downstream: DownstreamConfig::Blackhole,
    };

    let storage = Storage::open(&config.storage_path)?;
    info!("Storage opened at: {}", config.storage_path);

    let upstream = UpstreamVariant::new(config.upstream.clone());
    let downstream = DownstreamVariant::new(config.downstream.clone());
    let archive = ArchiveVariant::new(config.archive.clone());
    let accumulator = AccumulatorVariant::new(&config.accumulator);

    let rootsmith = RootSmith::new(upstream, downstream, archive, config, storage)
        .with_accumulator(accumulator);

    info!("RootSmith initialized: {}", rootsmith.upstream.name());

    Ok(())
}
