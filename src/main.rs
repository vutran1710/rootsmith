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
mod downstream;
mod app;
mod traits;

use config::BaseConfig;
use app::DefaultApp;

fn main() -> Result<()> {
    // Initialize telemetry
    telemetry::init();
    info!("Starting rootsmith");
    
    // Parse configuration from CLI arguments
    let config = BaseConfig::parse();
    info!("Configuration: storage_path={}, batch_interval_secs={}", 
          config.storage_path, config.batch_interval_secs);
    
    // Initialize and run the app
    let app = DefaultApp::initialize(config)?;
    app.run()?;
    
    info!("Rootsmith shutdown complete");
    Ok(())
}
