use rootsmith::wasm_host::{WasmLimits, WasmPluginHost};
use std::fs;
use std::path::Path;

#[test]
fn test_load_and_run_compiled_plugin() {
    println!("\n🎯 Testing Compiled WASM Plugin");
    println!("═══════════════════════════════════════════════════════════\n");

    // Step 1: Get path to the compiled WASM binary
    println!("📂 Step 1: Locating compiled WASM plugin...");
    let plugin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("plugins")
        .join("plugin.wasm");
    
    println!("   Path: {}", plugin_path.display());
    
    // Check file exists and get size
    let metadata = fs::metadata(&plugin_path)
        .expect("Failed to find plugin.wasm - make sure it's compiled");
    println!("   File size: {} bytes\n", metadata.len());

    // Step 2: Initialize WASM host with default limits
    println!("🔧 Step 2: Initializing WASM host and loading plugin...");
    let limits = WasmLimits::default();
    println!("   Max memory pages: {}", limits.max_memory_pages);
    println!("   Max response bytes: {} bytes\n", limits.max_response_bytes);

    let plugin_path_str = plugin_path.to_str().expect("Invalid path");
    let mut host = WasmPluginHost::load(plugin_path_str, limits)
        .expect("Failed to load plugin");
    println!("   ✅ Plugin loaded and instantiated successfully\n");

    // Step 3: Check API version
    println!("🔍 Step 3: Checking API version...");
    match host.api_version() {
        Some((major, minor)) => {
            println!("   Plugin API version: {}.{}\n", major, minor);
        }
        None => {
            println!("   Plugin does not export get_api_version (optional)\n");
        }
    }

    // Step 4: Process test data (the plugin reverses bytes)
    println!("🚀 Step 4: Processing test data...");
    let test_input = b"Hello, WASM World!";
    println!("   Input:  {:?}", String::from_utf8_lossy(test_input));
    println!("   Input bytes: {} bytes", test_input.len());

    let result = host.process_bytes(test_input)
        .expect("Failed to process data");
    
    println!("   Output: {:?}", String::from_utf8_lossy(&result));
    println!("   Output bytes: {} bytes\n", result.len());

    // Step 5: Verify the reversal
    println!("✅ Step 5: Verifying output...");
    let mut expected = test_input.to_vec();
    expected.reverse();
    
    assert_eq!(result, expected, "Output should be reversed input");
    println!("   ✓ Output matches expected reversed bytes\n");

    // Step 6: Test with different input
    println!("🔄 Step 6: Testing with different input...");
    let test_input2 = b"12345";
    println!("   Input:  {:?}", String::from_utf8_lossy(test_input2));
    
    let result2 = host.process_bytes(test_input2)
        .expect("Failed to process data");
    
    println!("   Output: {:?}", String::from_utf8_lossy(&result2));
    
    let mut expected2 = test_input2.to_vec();
    expected2.reverse();
    assert_eq!(result2, expected2);
    println!("   ✓ Second test passed\n");

    // Step 7: Test with empty input
    println!("🔄 Step 7: Testing with empty input...");
    let empty_input = b"";
    let empty_result = host.process_bytes(empty_input)
        .expect("Failed to process empty data");
    
    assert_eq!(empty_result.len(), 0, "Empty input should produce empty output");
    println!("   ✓ Empty input handled correctly\n");

    // Step 8: Test with larger data
    println!("🔄 Step 8: Testing with larger data...");
    let large_input: Vec<u8> = (0..=255).collect();
    println!("   Input: 256 bytes (0x00..0xFF)");
    
    let large_result = host.process_bytes(&large_input)
        .expect("Failed to process large data");
    
    let mut expected_large = large_input.clone();
    expected_large.reverse();
    assert_eq!(large_result, expected_large);
    println!("   ✓ Large data processed correctly\n");

    println!("═══════════════════════════════════════════════════════════");
    println!("🎉 All tests passed! Plugin is working correctly.\n");
}