use anyhow::Result;
use tracing::info;

mod accumulator;
mod archiver;
mod config;
mod downstream;
mod rootsmith;
mod storage;
mod telemetry;
mod types;
mod upstream;
mod utils;
mod wasm_host;

use config::Config;
use rootsmith::RootSmith;

#[tokio::main]
async fn main() -> Result<()> {
    telemetry::init();
    info!("Starting rootsmith");

    let config = Config::default();
    tracing::info!("Loaded configuration: {:?}", config);

    let rootsmith = RootSmith::initialize(config.clone()).await;
    tracing::info!("RootSmith initialized successfully");

    rootsmith.run().await.map_err(|e| {
        tracing::error!("RootSmith encountered an error: {:?}", e);
        e
    })
}
