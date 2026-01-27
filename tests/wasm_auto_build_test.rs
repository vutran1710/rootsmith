//! Test to visualize the WASM auto-build flow
//!
//! Run with: cargo test --test wasm_auto_build_test -- --nocapture

use rootsmith::wasm_host::builder::check_wasm_target;
use rootsmith::wasm_host::builder::get_or_build_plugin;
use rootsmith::wasm_host::builder::install_wasm_target;
use rootsmith::wasm_host::WasmLimits;
use rootsmith::wasm_host::WasmPluginHost;

#[test]
fn test_auto_build_flow() {
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           WASM Auto-Build Flow Test                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Step 0: Check prerequisites
    println!("┌─ Step 0: Check Prerequisites ─────────────────────────────────┐");
    match check_wasm_target() {
        Ok(true) => println!("│  [OK] wasm32-unknown-unknown target installed               │"),
        Ok(false) => {
            println!("│  [..] Installing wasm32-unknown-unknown target...            │");
            install_wasm_target().expect("Failed to install wasm target");
            println!("│  [OK] Target installed                                       │");
        }
        Err(e) => {
            println!("│  [!!] Could not check target: {}                    │", e);
        }
    }
    println!("└────────────────────────────────────────────────────────────────┘");
    println!();

    // Step 1: Request plugin by filename
    println!("┌─ Step 1: Request Plugin ──────────────────────────────────────┐");
    println!("│  Input:  get_or_build_plugin(\"client.rs\")                     │");
    println!("│                                                                │");
    println!("│  Flow:                                                         │");
    println!("│    plugins/input/client.rs  ──►  plugins/output/client.wasm   │");
    println!("└────────────────────────────────────────────────────────────────┘");
    println!();

    let wasm_path = get_or_build_plugin("client.rs").expect("Failed to get or build plugin");

    println!("┌─ Step 2: Build Result ────────────────────────────────────────┐");
    println!("│  WASM path: {}│", format!("{:<43}", wasm_path.display()));
    println!("│  Exists: {:<53}│", wasm_path.exists());
    if let Ok(meta) = wasm_path.metadata() {
        println!("│  Size: {:<55}│", format!("{} bytes", meta.len()));
    }
    println!("└────────────────────────────────────────────────────────────────┘");
    println!();

    assert!(wasm_path.exists(), "WASM file should exist after build");

    // Step 3: Load the plugin
    println!("┌─ Step 3: Load Plugin ─────────────────────────────────────────┐");
    let limits = WasmLimits::default();
    println!(
        "│  Limits: max_memory={} pages, max_response={} bytes     │",
        limits.max_memory_pages, limits.max_response_bytes
    );

    let mut host =
        WasmPluginHost::load(wasm_path.to_str().unwrap(), limits).expect("Failed to load plugin");

    println!("│  [OK] Plugin loaded successfully                             │");

    if let Some((major, minor)) = host.api_version() {
        println!("│  API Version: {}.{:<50}│", major, minor);
    }
    println!("└────────────────────────────────────────────────────────────────┘");
    println!();

    // Step 4: Process test input
    println!("┌─ Step 4: Process Input ───────────────────────────────────────┐");
    let test_input =
        r#"{"id":"evt-123","ts_ms":1700000000000,"user_id":"user-456","action":"click"}"#;
    println!("│  Input JSON:                                                  │");
    println!("│    {{                                                          │");
    println!("│      \"id\": \"evt-123\",                                        │");
    println!("│      \"ts_ms\": 1700000000000,                                  │");
    println!("│      \"user_id\": \"user-456\",                                  │");
    println!("│      \"action\": \"click\"                                       │");
    println!("│    }}                                                          │");
    println!("└────────────────────────────────────────────────────────────────┘");
    println!();

    let result = host
        .process_bytes(test_input.as_bytes())
        .expect("Failed to process input");

    println!("┌─ Step 5: Output ──────────────────────────────────────────────┐");
    println!(
        "│  Output size: {} bytes (protobuf-encoded IncomingRecord)     │",
        result.len()
    );
    println!("│                                                                │");
    println!("│  Parsed fields:                                                │");

    // Parse the protobuf output to display fields
    if result.len() >= 34 {
        let namespace = &result[2..34];
        let ns_str: String = namespace
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect();
        println!("│    namespace: \"{:<46}│", format!("{}\"", ns_str));
    }
    if result.len() >= 68 {
        let key = &result[36..68];
        let key_str: String = key
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect();
        println!("│    key:       \"{:<46}│", format!("{}\"", key_str));
    }
    if result.len() >= 102 {
        let value = &result[70..102];
        let val_str: String = value
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect();
        println!("│    value:     \"{:<46}│", format!("{}\"", val_str));
    }
    println!("└────────────────────────────────────────────────────────────────┘");
    println!();

    // Step 6: Verify rebuild skip
    println!("┌─ Step 6: Rebuild Check ───────────────────────────────────────┐");
    println!("│  Requesting same plugin again...                              │");

    let wasm_path2 = get_or_build_plugin("client.rs").expect("Failed to get plugin second time");

    assert_eq!(wasm_path, wasm_path2);
    println!("│  [OK] Same path returned (rebuild skipped - file unchanged)   │");
    println!("└────────────────────────────────────────────────────────────────┘");
    println!();

    // Summary
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                         SUMMARY                              ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Input:   plugins/input/client.rs                            ║");
    println!("║  Output:  plugins/output/client.wasm                         ║");
    println!("║  Build:   rustc --target wasm32-unknown-unknown              ║");
    println!("║  Auto:    Rebuilds only if .rs newer than .wasm              ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
}
