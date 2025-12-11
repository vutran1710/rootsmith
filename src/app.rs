use anyhow::Result;
use tracing::info;
use crate::config::Config;
use crate::storage::Storage;

pub fn run(config: Config) -> Result<()> {
    info!("Initializing application with config: {:?}", config);
    let _storage = Storage::new(&config.storage.path)?;
    info!("Storage initialized at: {}", config.storage.path);
    info!("Application running...");
    Ok(())
}
