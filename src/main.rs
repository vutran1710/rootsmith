use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
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

#[derive(Parser, Debug)]
#[command(name = "rootsmith")]
#[command(about = "Cryptographic accumulator app with pluggable parsers", long_about = None)]
struct Cli {
    #[arg(short, long, value_name = "FILE", default_value = "config.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    telemetry::init();
    info!("Starting rootsmith");

    let config_path = if cli.config.exists() {
        cli.config
    } else {
        PathBuf::from("config.toml")
    };

    let config: Config = if config_path.exists() {
        let config_str = std::fs::read_to_string(&config_path)?;
        toml::from_str(&config_str)?
    } else {
        info!("Config file not found, using defaults");
        Config::default()
    };

    info!("Loaded configuration: {:?}", config);

    let plugin_path = PathBuf::from(&config.plugin_path);

    if !plugin_path.exists() {
        anyhow::bail!("Plugin file not found: {:?}", plugin_path);
    }

    if plugin_path.extension().map_or(false, |ext| ext == "wasm") {
        info!("Loading WASM plugin: {:?}", plugin_path);
    } else {
        anyhow::bail!("Plugin must be a .wasm file, got: {:?}", plugin_path);
    }

    let rootsmith = RootSmith::initialize(config).await;
    tracing::info!("RootSmith initialized successfully");

    return rootsmith.run().await.map_err(|e| {
        tracing::error!("RootSmith encountered an error: {:?}", e);
        e
    });
}
