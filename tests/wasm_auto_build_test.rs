use rootsmith::accumulator::merkle_accumulator::MerkleAccumulator;
use rootsmith::accumulator::Accumulator;
use rootsmith::wasm_host::builder::get_or_build_plugin;
use rootsmith::wasm_host::ParsedRecord;
use rootsmith::wasm_host::PluginOutput;
use rootsmith::wasm_host::WasmLimits;
use rootsmith::wasm_host::WasmPluginHost;

fn bytes_to_str(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take_while(|&&b| b != 0 && b.is_ascii_graphic())
        .map(|&b| b as char)
        .collect()
}

#[tokio::test]
async fn test_wasm_plugin_to_accumulator() {
    println!("\n=== WASM Plugin → Accumulator ===\n");

    let wasm_path = get_or_build_plugin("client.rs").expect("Failed to build");
    println!("Plugin: {}", wasm_path.display());

    let mut host = WasmPluginHost::load(
        wasm_path.to_str().unwrap(),
        Some(WasmLimits::default()),
    )
    .expect("Failed to load");

    if let Some((major, minor)) = host.api_version() {
        println!("Version: {}.{}", major, minor);
    }

    // User input (arbitrary JSON)
    let input = r#"{"id":"evt-123","ts_ms":1700000000000,"user_id":"user-456","action":"click"}"#;
    println!("\nInput: {}", input);

    // Process returns trait - same methods user implemented in client.rs
    let output: Box<dyn PluginOutput> = host.process(input.as_bytes()).expect("Failed to process");

    // Use the trait methods (mirrors user's ToRecord/ToStandardData)
    println!("\nOutput (via PluginOutput trait):");
    println!("  get_namespace(): {}", bytes_to_str(&output.get_namespace()));
    println!("  get_key():       {}", bytes_to_str(&output.get_key()));
    println!("  get_value():     {}", bytes_to_str(&output.get_value()));
    println!("  get_timestamp(): {}", output.get_timestamp());

    // Convert to Record for accumulator (need ParsedRecord for into_record)
    let parsed = ParsedRecord {
        namespace: output.get_namespace(),
        key: output.get_key(),
        value: output.get_value(),
        timestamp: output.get_timestamp(),
    };
    let record = parsed.into_record();

    let accumulator = MerkleAccumulator::default();
    let (tx, rx) = kanal::bounded_async(1);

    accumulator
        .commit(&[record], tx)
        .await
        .expect("Failed to commit");

    let result = rx.recv().await.expect("Failed to receive");

    println!("\nAccumulator:");
    println!("  root:  {}", hex::encode(&result.commitment.root));
    println!("  items: {}", result.item_count);

    println!("\n=== Done ===\n");
}
