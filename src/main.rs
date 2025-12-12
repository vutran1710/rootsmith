use anyhow::Result;
use clap::Parser;
use tracing::info;

mod config;
mod feature_select;
mod telemetry;
mod types;
mod storage;
mod crypto;
mod upstream;
mod commitment_registry;
mod proof_registry;
mod rootsmith;
mod traits;

use config::BaseConfig;
use rootsmith::RootSmith;

fn main() -> Result<()> {
    // Initialize telemetry
    telemetry::init();
    info!("Starting rootsmith");
    
    // Parse configuration from CLI arguments
    let config = BaseConfig::parse();
    info!("Configuration: storage_path={}, batch_interval_secs={}", 
          config.storage_path, config.batch_interval_secs);
    
    // Initialize and run the app
    let app = RootSmith::initialize(config)?;
    app.run()?;
    
    info!("Rootsmith shutdown complete");
    Ok(())
}
