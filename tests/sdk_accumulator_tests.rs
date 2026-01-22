use rootsmith::accumulator::{to_sdk_trait, SdkAccumulator, SDKTrait};
use rootsmith::types::CommitmentResult;
use rootsmith::wasm_host::{ToStandardData, WasmLimits, WasmPluginHost};
use kanal::unbounded_async;
use std::path::Path;

#[test]
fn test_sdk_accumulator_with_trait_objects() {
    println!("\n🔄 Testing SDK Accumulator with Trait Objects");
    println!("═══════════════════════════════════════════════════════════\n");

    // Step 1: Load WASM plugin
    println!("📂 Step 1: Loading WASM plugin...");
    let plugin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("plugins")
        .join("client.wasm");
    let plugin_path_str = plugin_path.to_str().expect("Invalid path");
    let mut host = WasmPluginHost::load(plugin_path_str, WasmLimits::default())
        .expect("Failed to load plugin");
    println!("   ✅ Plugin loaded\n");

    // Step 2: Process input through WASM to get trait objects
    println!("🚀 Step 2: Processing input through WASM...");
    let json_input = r#"{"id":"event-12345","ts_ms":1699123456000,"user_id":"user-abc123","action":"click"}"#;
    let input_bytes = json_input.as_bytes();
    
    let trait_obj: Box<dyn ToStandardData> = host.process_input::<dyn ToStandardData>(input_bytes)
        .expect("Failed to process input");
    println!("   ✅ Got trait object: Box<dyn ToStandardData>\n");

    // Step 3: Create SDK accumulator
    println!("🌳 Step 3: Creating SDK accumulator...");
    let mut accumulator = SdkAccumulator::new();
    println!("   ✅ SDK accumulator created\n");

    // Step 4: Use accumulator_build pattern (from test example)
    println!("📋 Step 4: Using accumulator_build pattern...");
    println!("   ┌─────────────────────────────────────────────────────────┐");
    println!("   │ fn accumulator_build<T: SDKTrait>(data: Box<T>) {{      │");
    println!("   │     let namespace = data.namespace();                   │");
    println!("   │     let key = data.key();                               │");
    println!("   │     let value = data.value();                           │");
    println!("   │     let timestamp = data.timestamp();                   │");
    println!("   │     accumulator.build(data);                            │");
    println!("   │ }}                                                       │");
    println!("   └─────────────────────────────────────────────────────────┘\n");
    
    fn accumulator_build(accumulator: &mut SdkAccumulator, data: Box<dyn SDKTrait>) {
        let namespace = data.namespace();
        let key = data.key();
        let value = data.value();
        let timestamp = data.timestamp();
        
        println!("   📊 Extracted from trait object:");
        println!("      namespace: {}...", hex::encode(&namespace[..8]));
        println!("      key: {}...", hex::encode(&key[..8]));
        println!("      value: {}...", hex::encode(&value[..8]));
        println!("      timestamp: {}", timestamp);
        
        accumulator.build(data).expect("Failed to build");
    }
    
    // Convert Box<dyn ToStandardData> to Box<dyn SDKTrait> using safe helper
    let trait_obj_as_sdk = to_sdk_trait(trait_obj);
    accumulator_build(&mut accumulator, trait_obj_as_sdk);
    println!("   ✅ Trait object added to accumulator\n");

    // Step 5: Commit trait objects
    println!("📦 Step 5: Committing trait objects...");
    let (commit_tx, commit_rx) = unbounded_async::<CommitmentResult>();
    
    // Create trait objects vector
    // Process two inputs to get two trait objects
    let json_input2 = r#"{"id":"event-67890","ts_ms":1699123457000,"user_id":"user-xyz789","action":"view"}"#;
    let trait_obj1: Box<dyn ToStandardData> = host.process_input::<dyn ToStandardData>(json_input.as_bytes())
        .expect("Failed to process input");
    let trait_obj2: Box<dyn ToStandardData> = host.process_input::<dyn ToStandardData>(json_input2.as_bytes())
        .expect("Failed to process input");
    
    // Convert Box<dyn ToStandardData> to Box<dyn SDKTrait> using safe helper
    let trait_objects: Vec<Box<dyn SDKTrait>> = vec![
        to_sdk_trait(trait_obj1),
        to_sdk_trait(trait_obj2),
    ];
    
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        accumulator.commit_trait(&trait_objects, commit_tx).await
            .expect("Failed to commit");
    });
    
    println!("   ✅ Committed {} trait objects\n", trait_objects.len());

    // Step 6: Receive commitment result
    println!("🎯 Step 6: Receiving commitment result...");
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        if let Ok(result) = commit_rx.recv().await {
            println!("   ✅ Commitment result received:");
            println!("      root: {}...", hex::encode(&result.commitment[..8]));
            println!("      proofs: {}", result.proofs.as_ref().map(|p| p.len()).unwrap_or(0));
            println!("      committed_at: {}", result.committed_at);
        }
    });

    println!("\n═══════════════════════════════════════════════════════════");
    println!("🎉 SDK Accumulator with trait objects works correctly!");
    println!("   ✓ WASM host returns Box<dyn ToStandardData>");
    println!("   ✓ Trait objects implement SDKTrait");
    println!("   ✓ Accumulator accepts trait objects directly");
    println!("   ✓ No intermediate struct conversion needed\n");
}
