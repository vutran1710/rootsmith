use anyhow::Result;
use tracing::{info, error};

mod config;
mod feature_select;
mod telemetry;
mod types;
mod storage;
mod crypto;
mod upstream;
mod downstream;
mod app;

fn main() -> Result<()> {
    telemetry::init();
    info!("Starting rootsmith");
    let config = config::load()?;
    info!("Configuration loaded successfully");
    match app::run(config) {
        Ok(_) => {
            info!("Application completed successfully");
            Ok(())
        }
        Err(e) => {
            error!("Application error: {}", e);
            Err(e)
        }
    }
}
