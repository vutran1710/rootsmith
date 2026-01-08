use anyhow::Result;
use clap::Parser;
use tracing::info;

mod archive;
mod commitment_registry;
mod config;
mod crypto;
mod feature_select;
mod proof_delivery;
mod proof_registry;
mod rootsmith;
mod storage;
mod telemetry;
mod traits;
mod types;
mod upstream;

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
    let app = RootSmith::initialize(config).await?;
    app.run().await?;

    info!("Rootsmith shutdown complete");
    Ok(())
}
