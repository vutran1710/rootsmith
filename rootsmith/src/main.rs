use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tracing::info;

use rootsmith::config::Config;
use rootsmith::rootsmith::RootSmith;
use rootsmith::telemetry;

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

    let mut rootsmith = RootSmith::initialize(config).await?;
    tracing::info!("RootSmith initialized successfully");

    rootsmith.run().await.map_err(|e| {
        tracing::error!("RootSmith encountered an error: {:?}", e);
        e
    })
}
