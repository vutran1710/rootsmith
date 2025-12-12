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

// Re-export commonly used types for internal use
use config::BaseConfig;
use traits::{Accumulator, CommitmentRegistry, ProofRegistry, UpstreamConnector, Leaf};
use app::App;
use app_v2::AppV2;

fn main() -> Result<()> {
    telemetry::init();
    info!("Starting rootsmith");
    
    // TODO: Implement concrete upstream, commitment registry, proof registry
    // and instantiate App with them.
    
    info!("Rootsmith initialized - awaiting concrete implementation");
    Ok(())
}
