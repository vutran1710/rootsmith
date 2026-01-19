use rootsmith::types::IncomingRecord;
use rootsmith::wasm_host::{WasmLimits, WasmPluginHost};
use std::fs;
use std::path::Path;

#[test]
fn test_client_plugin_converts_to_incoming_record() {
    println!("\n🔄 Testing Client Plugin: Data → IncomingRecord Conversion");
    println!("═══════════════════════════════════════════════════════════\n");

    // Step 1: Load the compiled client plugin
    println!("📂 Step 1: Loading client plugin...");
    let plugin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("plugins")
        .join("client.wasm");
    
    println!("   Path: {}", plugin_path.display());
    
    let metadata = fs::metadata(&plugin_path)
        .expect("Failed to find client.wasm - make sure it's compiled");
    println!("   File size: {} bytes\n", metadata.len());

    // Step 2: Initialize WASM host
    println!("🔧 Step 2: Initializing WASM host...");
    let limits = WasmLimits::default();
    let plugin_path_str = plugin_path.to_str().expect("Invalid path");
    let mut host = WasmPluginHost::load(plugin_path_str, limits)
        .expect("Failed to load client plugin");
    println!("   ✅ Client plugin loaded successfully\n");

    // Step 3: Prepare input data (JSON format)
    println!("📥 Step 3: Preparing input data (JSON format)...");
    
    // Create test event data
    let test_id = "event-12345";
    let test_ts_ms = 1699123456000u64; // milliseconds
    let test_user_id = "user-abc123";
    let test_action = "click";
    
    // Create JSON input (like what a real user would send)
    let json_input = format!(
        r#"{{"id":"{}","ts_ms":{},"user_id":"{}","action":"{}"}}"#,
        test_id, test_ts_ms, test_user_id, test_action
    );
    
    println!("   JSON Input: {}", json_input);
    println!("   Event ID: {}", test_id);
    println!("   Timestamp: {} ms", test_ts_ms);
    println!("   User ID: {}", test_user_id);
    println!("   Action: {}\n", test_action);
    
    // Expected output (based on plugin's conversion logic)
    // Plugin converts: user_id -> namespace, id -> key, action -> value
    let input_bytes = json_input.as_bytes();

    // Step 4: Process through plugin (returns IncomingRecord directly)
    println!("🚀 Step 4: Processing data through client plugin...");
    println!("   Calling process_to_record() which:");
    println!("     1. Processes input through plugin");
    println!("     2. Receives protobuf-encoded IncomingRecord");
    println!("     3. Parses protobuf automatically");
    println!("     4. Returns IncomingRecord directly\n");
    
    let record: IncomingRecord = host.process_to_record(&input_bytes)
        .expect("Failed to process data through plugin");
    
    println!("   ✅ Plugin processed and converted to IncomingRecord\n");

    // Step 5: Verify the conversion
    println!("✅ Step 5: Verifying conversion...");
    
    // Verify that we got a valid IncomingRecord
    // (The exact byte patterns depend on plugin implementation details)
    assert_eq!(record.namespace.len(), 32, "Namespace should be 32 bytes");
    assert_eq!(record.key.len(), 32, "Key should be 32 bytes");
    assert_eq!(record.value.len(), 32, "Value should be 32 bytes");
    
    // Verify timestamp (converted from ms to seconds)
    assert_eq!(record.timestamp, test_ts_ms / 1000, "Timestamp should be converted from ms to seconds");
    
    // Verify that the record is not all zeros (conversion happened)
    let namespace_sum: u32 = record.namespace.iter().map(|&b| b as u32).sum();
    let key_sum: u32 = record.key.iter().map(|&b| b as u32).sum();
    let value_sum: u32 = record.value.iter().map(|&b| b as u32).sum();
    
    assert!(namespace_sum > 0, "Namespace should contain data");
    assert!(key_sum > 0, "Key should contain data");
    assert!(value_sum > 0, "Value should contain data");
    
    println!("   ✓ Namespace: {}... (32 bytes, sum: {})", hex::encode(&record.namespace[..8]), namespace_sum);
    println!("   ✓ Key:       {}... (32 bytes, sum: {})", hex::encode(&record.key[..8]), key_sum);
    println!("   ✓ Value:     {}... (32 bytes, sum: {})", hex::encode(&record.value[..8]), value_sum);
    println!("   ✓ Timestamp: {} (converted from {} ms)\n", record.timestamp, test_ts_ms);

    // Step 6: Display the final IncomingRecord
    println!("📋 Step 6: Final IncomingRecord structure:");
    println!("   ┌─────────────────────────────────────────────────────────┐");
    println!("   │ IncomingRecord                                          │");
    println!("   ├─────────────────────────────────────────────────────────┤");
    println!("   │ namespace:  {}...", hex::encode(&record.namespace[..16]));
    println!("   │ key:        {}...", hex::encode(&record.key[..16]));
    println!("   │ value:      {}...", hex::encode(&record.value[..16]));
    println!("   │ timestamp:  {} (Unix seconds)", record.timestamp);
    println!("   └─────────────────────────────────────────────────────────┘\n");

    println!("═══════════════════════════════════════════════════════════");
    println!("🎉 Client plugin successfully converts JSON events to IncomingRecord!");
    println!("   ✓ User writes simple plugin with SDK-like API");
    println!("   ✓ Plugin decodes JSON envelope → MyWhateverEventName");
    println!("   ✓ Plugin converts event → IncomingRecord");
    println!("   ✓ Host automatically parses protobuf via process_to_record()");
    println!("   ✓ Simple, clean API - just like the SDK example!\n");
}
