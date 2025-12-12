use anyhow::Result;
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
mod app_v2;
mod traits;

fn main() -> Result<()> {
    telemetry::init();
    info!("Starting rootsmith");
    
    // TODO: Implement concrete upstream, commitment registry, proof registry
    // and instantiate App with them.
    
    info!("Rootsmith initialized - awaiting concrete implementation");
    Ok(())
}
