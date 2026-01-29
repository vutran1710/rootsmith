use std::path::Path;

use anyhow::Result;
use serde_json::json;

use rootsmith::types::UpstreamData;
use rootsmith::wasm_host::{WasmBuilder, WasmLimits, WasmPluginHost};

#[test]
fn test_wasm_build_and_process() -> Result<()> {
    println!("\n=== WASM Plugin Test ===\n");

    // CARGO_MANIFEST_DIR is rootsmith/, go up 1 level to workspace root
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

    // 1. Build
    let source = workspace_root.join("examples/plugin/src/lib.rs");
    let output = workspace_root.join("examples/output");

    let wasm_path = WasmBuilder::build(&source, &output)?;
    println!("Built: {} -> {}", source.display(), wasm_path.display());

    // 2. Load
    let mut host = WasmPluginHost::load(wasm_path.to_str().unwrap(), Some(WasmLimits::default()))?;
    if let Some((major, minor)) = host.api_version() {
        println!("Loaded: API v{}.{}", major, minor);
    }

    // 3. Process
    let input = json!({
        "user_id": "user-123",
        "event_type": "click",
        "timestamp": 1_700_000_000u64,
        "data": "hello"
    });

    let record = host.process_to_record(UpstreamData::Json(input.clone()))?;

    let ns = String::from_utf8_lossy(&record.namespace);
    let key = String::from_utf8_lossy(&record.key);
    let value = match &record.value {
        UpstreamData::Bytes(b) => String::from_utf8_lossy(b).to_string(),
        _ => String::new(),
    };

    println!("Input:  {}", input);
    println!("Output: ns={:?} key={:?} value={:?}",
        ns.trim_end_matches('\0'),
        key.trim_end_matches('\0'),
        value
    );

    println!("\n=== Done ===\n");
    Ok(())
}
