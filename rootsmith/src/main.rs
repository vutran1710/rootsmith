use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tracing::info;

use rootsmith::config::Config;
use rootsmith::rootsmith::RootSmith;
use rootsmith::telemetry;
use rootsmith::wasm_host::WasmBuilder;

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

    let source_path = PathBuf::from(&config.source_path);
    let output_dir = PathBuf::from("./examples/output");

    if !source_path.exists() {
        anyhow::bail!("Source file not found: {:?}", source_path);
    }

    info!("Building WASM plugin from: {:?}", source_path);
    let wasm_path = WasmBuilder::build(&source_path, &output_dir)?;
    info!("Built WASM plugin: {:?}", wasm_path);

    let rootsmith = RootSmith::initialize(config, wasm_path).await;
    tracing::info!("RootSmith initialized successfully");

    return rootsmith.run().await.map_err(|e| {
        tracing::error!("RootSmith encountered an error: {:?}", e);
        e
    });
}
