use std::path::Path;

use anyhow::Result;
use serde_json::json;

use rootsmith::types::{Record, UpstreamData};
use rootsmith::wasm_host::{WasmLimits, WasmPluginHost};

/// E2E test: load the example WASM plugin, send a JSON event through the host,
/// decode it to a `Record`, and print the result.
///
/// This test assumes you have already built the plugin:
///
/// ```bash
/// cargo build --manifest-path examples/plugin/Cargo.toml --target wasm32-unknown-unknown --release \
///   && mkdir -p examples/output \
///   && cp examples/plugin/target/wasm32-unknown-unknown/release/example_parser_plugin.wasm examples/output/lib.wasm
/// ```
#[test]
fn test_wasm_plugin_parses_record_and_prints() -> Result<()> {
    // Locate the built plugin
    let plugin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("output")
        .join("lib.wasm");

    if !plugin_path.exists() {
        panic!(
            "WASM plugin not found at {:?}. \
             Please build it with:\n\
             \n  cargo build --manifest-path examples/plugin/Cargo.toml --target wasm32-unknown-unknown --release \\\n  \\\n  && mkdir -p examples/output \\\n  && cp examples/plugin/target/wasm32-unknown-unknown/release/example_parser_plugin.wasm examples/output/lib.wasm\n",
            plugin_path
        );
    }

    // Load the plugin via the host
    let mut host = WasmPluginHost::load(
        plugin_path
            .to_str()
            .expect("Failed to convert plugin path to string"),
        Some(WasmLimits::default()),
    )?;

    // Build a JSON event that matches `examples/plugin/src/lib.rs::UserEvent`
    let event = json!({
        "user_id": "user-123",
        "event_type": "click",
        "timestamp": 1_700_000_000u64,
        "data": "hello from wasm test"
    });

    // Wrap the JSON into host UpstreamData and let the host handle the envelope
    let upstream = UpstreamData::Json(event);

    // Process through the plugin and decode to the host `Record`
    let record = host
        .process_to_record(upstream)
        .expect("Failed to process record via WASM plugin");

    // Print the decoded record so you can inspect the output
    println!("Decoded Record from WASM plugin:\n{:#?}", record);

    Ok(())
}