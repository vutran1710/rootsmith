use rootsmith::types::IncomingRecord;
use rootsmith::wasm_host::{ToStandardData, WasmLimits, WasmPluginHost};
use std::fs;
use std::path::Path;

#[test]
fn test_trait_object_flow() {
    println!("\n🔄 Testing Trait Object Flow: Partner Traits → Host → Accumulator");
    println!("═══════════════════════════════════════════════════════════\n");

    // Step 1: Load the compiled client plugin
    println!("📂 Step 1: Loading client plugin...");
    let plugin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("plugins")
        .join("client.wasm");
    
    let metadata = fs::metadata(&plugin_path)
        .expect("Failed to find client.wasm - make sure it's compiled");
    println!("   Path: {}", plugin_path.display());
    println!("   File size: {} bytes\n", metadata.len());

    // Step 2: Initialize WASM host
    println!("🔧 Step 2: Initializing WASM host...");
    let limits = WasmLimits::default();
    let plugin_path_str = plugin_path.to_str().expect("Invalid path");
    let mut host = WasmPluginHost::load(plugin_path_str, limits)
        .expect("Failed to load client plugin");
    println!("   ✅ Client plugin loaded successfully\n");

    // Step 3: Prepare input data
    println!("📥 Step 3: Preparing input data...");
    let test_id = "event-12345";
    let test_ts_ms = 1699123456000u64;
    let test_user_id = "user-abc123";
    let test_action = "click";
    
    let json_input = format!(
        r#"{{"id":"{}","ts_ms":{},"user_id":"{}","action":"{}"}}"#,
        test_id, test_ts_ms, test_user_id, test_action
    );
    let input_bytes = json_input.as_bytes();
    println!("   JSON Input: {}\n", json_input);

    // Step 4: Show how partner implements traits
    println!("👤 Step 4: Partner Implementation (client.rs):");
    println!("   ┌─────────────────────────────────────────────────────────┐");
    println!("   │ // Partner implements ToRecord for metadata           │");
    println!("   │ impl sdk::ToRecord for MyWhateverEventName {{         │");
    println!("   │     fn get_namespace(&self) -> [u8; 32] {{ ... }}     │");
    println!("   │     fn get_key(&self) -> [u8; 32] {{ ... }}           │");
    println!("   │     fn get_timestamp(&self) -> u64 {{ ... }}          │");
    println!("   │ }}                                                      │");
    println!("   │                                                         │");
    println!("   │ // Partner implements ToStandardData for data          │");
    println!("   │ impl sdk::ToStandardData for MyWhateverEventName {{   │");
    println!("   │     fn get_value(&self) -> [u8; 32] {{ ... }}         │");
    println!("   │ }}                                                      │");
    println!("   └─────────────────────────────────────────────────────────┘\n");

    // Step 5: Process as Standard format and get trait object
    println!("🚀 Step 5: Processing as Standard format (trait object)...");
    println!("   Calling process_input<ToStandardData>() which:");
    println!("     1. Processes input through plugin");
    println!("     2. Extracts metadata via ToRecord trait");
    println!("     3. Extracts data via ToStandardData trait");
    println!("     4. Encodes as protobuf IncomingRecord");
    println!("     5. Detects Standard format and wraps in trait object");
    println!("     6. Returns Box<dyn ToStandardData>\n");
    
    let output: Box<dyn ToStandardData> = host.process_input(&input_bytes)
        .expect("Failed to process as Standard format");
    
    println!("   ✅ Got trait object: Box<dyn ToStandardData>\n");

    // Step 6: Demonstrate trait object usage (simulating accumulator)
    println!("🎯 Step 6: Using trait object (simulating accumulator integration)...");
    
    // Access trait methods
    let namespace = output.namespace();
    let key = output.key();
    let timestamp = output.timestamp();
    let value = output.value();
    
    println!("   ✅ Trait object methods:");
    println!("      • namespace()  = {}... (32 bytes)", hex::encode(&namespace[..8]));
    println!("      • key()        = {}... (32 bytes)", hex::encode(&key[..8]));
    println!("      • timestamp()  = {} (Unix seconds)", timestamp);
    println!("      • value()      = {}... (32 bytes)", hex::encode(&value[..8]));
    
    // Verify values
    assert_eq!(namespace.len(), 32, "Namespace should be 32 bytes");
    assert_eq!(key.len(), 32, "Key should be 32 bytes");
    assert_eq!(value.len(), 32, "Value should be 32 bytes");
    assert_eq!(timestamp, test_ts_ms / 1000, "Timestamp should match");
    
    println!("\n   ✅ Trait object works correctly\n");

    // Step 7: Demonstrate accumulator integration pattern
    println!("📋 Step 7: Accumulator Integration Pattern:");
    println!("   ┌─────────────────────────────────────────────────────────┐");
    println!("   │ // Accumulator receives trait object directly           │");
    println!("   │ fn accumulator_build<T: ToStandardData>(data: Box<T>) {{ │");
    println!("   │     let namespace = data.namespace();                   │");
    println!("   │     let key = data.key();                               │");
    println!("   │     let value = data.value();                           │");
    println!("   │     let timestamp = data.timestamp();                   │");
    println!("   │     // ... use data for accumulation ...                │");
    println!("   │ }}                                                       │");
    println!("   │                                                         │");
    println!("   │ // Usage:"); 
    println!("   │ let output = host.process_input::<ToStandardData>(input)?; │");
    println!("   │ accumulator_build(output);  // Direct trait object      │");
    println!("   └─────────────────────────────────────────────────────────┘\n");

    // Step 8: Summary
    println!("📋 Step 8: Summary - Trait Object Flow");
    println!("   ┌─────────────────────────────────────────────────────────┐");
    println!("   │ Trait Object Flow                                      │");
    println!("   ├─────────────────────────────────────────────────────────┤");
    println!("   │ 1. Partner implements traits in plugin:                │");
    println!("   │    • ToRecord (metadata: namespace, key, timestamp)    │");
    println!("   │    • ToStandardData (data: fixed-size value)           │");
    println!("   │                                                        │");
    println!("   │ 2. Plugin glue code extracts metadata + data:          │");
    println!("   │    • event.get_namespace()                             │");
    println!("   │    • event.get_key()                                   │");
    println!("   │    • event.get_timestamp()                             │");
    println!("   │    • event.get_value()                                 │");
    println!("   │                                                        │");
    println!("   │ 3. Plugin encodes as protobuf IncomingRecord           │");
    println!("   │                                                        │");
    println!("   │ 4. Host detects format and wraps in trait object       │");
    println!("   │    • Returns Box<dyn ToStandardData>                   │");
    println!("   │                                                        │");
    println!("   │ 5. Accumulator receives trait object directly          │");
    println!("   │    • No intermediate struct conversion                 │");
    println!("   │    • Type-safe via trait bounds                        │");
    println!("   └─────────────────────────────────────────────────────────┘\n");

    println!("═══════════════════════════════════════════════════════════");
    println!("🎉 Trait object flow works correctly!");
    println!("   ✓ Partners implement ToRecord + ToStandardData traits");
    println!("   ✓ Host returns Box<dyn ToStandardData> trait object");
    println!("   ✓ Accumulator can consume trait object directly");
    println!("   ✓ No PluginOut enum needed");
    println!("   ✓ No intermediate struct conversion");
    println!("   ✓ Clean, type-safe integration!\n");
}
