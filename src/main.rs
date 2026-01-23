use anyhow::{Context, Result};
use clap::Parser;
use tracing::info;
use tokio::signal;

mod accumulator;
mod archiver;
mod config;
mod downstream;
mod parser;
mod rootsmith;
mod storage;
mod telemetry;
mod traits;
mod types;
mod upstream;
mod wasm_host;
mod zk_service;

use config::BaseConfig;
use rootsmith::RootSmith;
use upstream::{HttpSource, UpstreamVariant};
use downstream::{BlackholeDownstream, DownstreamVariant};
use archiver::{ArchiveStorageVariant, NoopArchive};
use wasm_host::{WasmLimits, WasmPluginHost};
use accumulator::AccumulatorVariant;
use storage::Storage;

#[tokio::main]
async fn main() -> Result<()> {
    telemetry::init();
    info!("Starting rootsmith");

    let config = BaseConfig::parse();
    info!(
        "Configuration: storage_path={}, batch_interval_secs={}, zk_service_url={}, zk_circuit_id={}, upstream_bind={}",
        config.storage_path, config.batch_interval_secs, config.zk_service_url, config.zk_circuit_id, config.upstream_bind
    );

    let storage = Storage::open(&config.storage_path)
        .context("Failed to open storage")?;
    info!("Storage opened at: {}", config.storage_path);

    let upstream = UpstreamVariant::Http(HttpSource::new(config.upstream_bind.clone()));
    let downstream = DownstreamVariant::Blackhole(BlackholeDownstream::new());
    let archive_storage = ArchiveStorageVariant::Noop(NoopArchive);

    let mut rootsmith = RootSmith::new(
        upstream,
        downstream,
        archive_storage,
        config.clone(),
        storage,
    );

    if let Some(wasm_path) = &config.wasm_plugin_path {
        info!("Loading WASM plugin from: {}", wasm_path);
        let limits = WasmLimits::default();
        let wasm_host = WasmPluginHost::load(wasm_path, limits)
            .with_context(|| format!("Failed to load WASM plugin from {}", wasm_path))?;
        rootsmith = rootsmith.with_wasm_host(wasm_host);
        info!("WASM plugin loaded successfully");
    } else {
        info!("No WASM plugin path provided - skipping WASM host initialization");
        return Err(anyhow::anyhow!("WASM plugin path is required (--wasm-plugin-path)"));
    }

    info!("Initializing accumulator: type={:?}", config.accumulator_type);
    let accumulator = AccumulatorVariant::new(config.accumulator_type, &config);
    let accumulator_id = match config.accumulator_type {
        config::AccumulatorType::Merkle => "merkle",
        config::AccumulatorType::SparseMerkle => "sparse-merkle",
        config::AccumulatorType::Zk => "zk",
    };
    rootsmith = rootsmith.with_accumulator(accumulator);
    info!("Accumulator initialized: {}", accumulator_id);

    info!("RootSmith initialized successfully");
    info!("HTTP upstream listening on: {}", config.upstream_bind);
    info!("Ready to receive data on POST /ingest/raw");

    let mut rootsmith = rootsmith;

    tokio::select! {
        result = rootsmith.run() => {
            if let Err(e) = result {
                tracing::error!("Run loop error: {}", e);
                return Err(e);
            }
        }
        _ = signal::ctrl_c() => {
            info!("Received shutdown signal (Ctrl+C)");
            info!("Shutting down gracefully...");
        }
    }

    info!("RootSmith stopped");
    Ok(())
}
