use anyhow::Result;
use clap::Parser;
use tracing::info;

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

use config::BaseConfig;
use rootsmith::RootSmith;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize telemetry
    telemetry::init();
    info!("Starting rootsmith");

    // Parse configuration from CLI arguments
    let config = BaseConfig::parse();
    info!(
        "Configuration: storage_path={}, batch_interval_secs={}",
        config.storage_path, config.batch_interval_secs
    );

    // Initialize and run the app
    let _app = RootSmith::initialize(config).await?;
    // TODO: Implement RootSmith::run() method
    // app.run().await?;

    info!("Rootsmith initialized (run() not yet implemented)");
    Ok(())
}
