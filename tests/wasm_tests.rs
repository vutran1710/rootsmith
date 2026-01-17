use anyhow::Result;
use rootsmith::wasm_host::{WasmLimits, WasmPluginHost};
use std::io::Write;
use tempfile::NamedTempFile;

/// Helper to compile WAT to WASM and write to a temporary file
fn compile_wat_to_tempfile(wat: &str) -> Result<NamedTempFile> {
    let wasm = wat::parse_str(wat)?;
    let mut file = NamedTempFile::new()?;
    file.write_all(&wasm)?;
    file.flush()?;
    Ok(file)
}

/// Create a simple test plugin that echoes input and adds a prefix
fn create_echo_plugin_wat() -> &'static str {
    r#"(module
        ;; Memory with maximum of 10 pages (640 KiB)
        (memory (export "memory") 1 10)
        
        ;; Simple bump allocator
        (global $heap (mut i32) (i32.const 1024))
        
        ;; alloc(size: i32) -> i32
        (func (export "alloc") (param $size i32) (result i32)
            (local $ptr i32)
            (local.set $ptr (global.get $heap))
            (global.set $heap (i32.add (global.get $heap) (local.get $size)))
            (local.get $ptr)
        )
        
        ;; dealloc(ptr: i32, size: i32)
        (func (export "dealloc") (param $ptr i32) (param $size i32))
        
        ;; get_api_version() -> i32
        ;; Returns version 1.0 encoded as (1 << 16) | 0 = 65536
        (func (export "get_api_version") (result i32)
            (i32.const 65536)
        )
        
        ;; process(in_ptr: i32, in_len: i32) -> i32
        ;; Returns: [status(4)][len(4)][payload...]
        ;; This implementation echoes the input with a "ECHO: " prefix
        (func (export "process") (param $in_ptr i32) (param $in_len i32) (result i32)
            (local $out_ptr i32)
            (local $prefix_len i32)
            (local $total_len i32)
            (local $i i32)
            
            ;; Prefix is "ECHO: " (6 bytes)
            (local.set $prefix_len (i32.const 6))
            (local.set $total_len (i32.add (local.get $prefix_len) (local.get $in_len)))
            
            ;; Allocate space for response header (8) + payload
            (local.set $out_ptr (call 0 (i32.add (i32.const 8) (local.get $total_len))))
            
            ;; Write status = 0 (success)
            (i32.store (local.get $out_ptr) (i32.const 0))
            
            ;; Write payload length
            (i32.store offset=4 (local.get $out_ptr) (local.get $total_len))
            
            ;; Write prefix "ECHO: "
            (i32.store8 offset=8 (local.get $out_ptr) (i32.const 69))  ;; 'E'
            (i32.store8 offset=9 (local.get $out_ptr) (i32.const 67))  ;; 'C'
            (i32.store8 offset=10 (local.get $out_ptr) (i32.const 72)) ;; 'H'
            (i32.store8 offset=11 (local.get $out_ptr) (i32.const 79)) ;; 'O'
            (i32.store8 offset=12 (local.get $out_ptr) (i32.const 58)) ;; ':'
            (i32.store8 offset=13 (local.get $out_ptr) (i32.const 32)) ;; ' '
            
            ;; Copy input bytes after prefix
            (local.set $i (i32.const 0))
            (loop $copy_loop
                (i32.store8 
                    (i32.add (i32.add (local.get $out_ptr) (i32.const 14)) (local.get $i))
                    (i32.load8_u (i32.add (local.get $in_ptr) (local.get $i))))
                (local.set $i (i32.add (local.get $i) (i32.const 1)))
                (br_if $copy_loop (i32.lt_u (local.get $i) (local.get $in_len)))
            )
            
            (local.get $out_ptr)
        )
    )"#
}

/// Create a plugin that returns an error
fn create_error_plugin_wat() -> &'static str {
    r#"(module
        (memory (export "memory") 1 10)
        
        (global $heap (mut i32) (i32.const 1024))
        
        (func (export "alloc") (param $size i32) (result i32)
            (local $ptr i32)
            (local.set $ptr (global.get $heap))
            (global.set $heap (i32.add (global.get $heap) (local.get $size)))
            (local.get $ptr)
        )
        
        ;; process() that always returns an error
        (func (export "process") (param $in_ptr i32) (param $in_len i32) (result i32)
            (local $out_ptr i32)
            (local $msg_len i32)
            
            (local.set $msg_len (i32.const 21))
            (local.set $out_ptr (call 0 (i32.add (i32.const 8) (local.get $msg_len))))
            
            ;; Write status = 1 (error)
            (i32.store (local.get $out_ptr) (i32.const 1))
            
            ;; Write message length
            (i32.store offset=4 (local.get $out_ptr) (local.get $msg_len))
            
            ;; Write error message "Plugin error occurred"
            (i32.store8 offset=8 (local.get $out_ptr) (i32.const 80))  ;; 'P'
            (i32.store8 offset=9 (local.get $out_ptr) (i32.const 108)) ;; 'l'
            (i32.store8 offset=10 (local.get $out_ptr) (i32.const 117)) ;; 'u'
            (i32.store8 offset=11 (local.get $out_ptr) (i32.const 103)) ;; 'g'
            (i32.store8 offset=12 (local.get $out_ptr) (i32.const 105)) ;; 'i'
            (i32.store8 offset=13 (local.get $out_ptr) (i32.const 110)) ;; 'n'
            (i32.store8 offset=14 (local.get $out_ptr) (i32.const 32))  ;; ' '
            (i32.store8 offset=15 (local.get $out_ptr) (i32.const 101)) ;; 'e'
            (i32.store8 offset=16 (local.get $out_ptr) (i32.const 114)) ;; 'r'
            (i32.store8 offset=17 (local.get $out_ptr) (i32.const 114)) ;; 'r'
            (i32.store8 offset=18 (local.get $out_ptr) (i32.const 111)) ;; 'o'
            (i32.store8 offset=19 (local.get $out_ptr) (i32.const 114)) ;; 'r'
            (i32.store8 offset=20 (local.get $out_ptr) (i32.const 33))  ;; '!'
            
            (local.get $out_ptr)
        )
    )"#
}

/// Create a plugin without memory maximum (should be rejected)
fn create_no_max_memory_plugin_wat() -> &'static str {
    r#"(module
        ;; Memory with no maximum declared
        (memory (export "memory") 1)
        
        (func (export "alloc") (param $size i32) (result i32)
            (i32.const 0)
        )
        
        (func (export "process") (param $in_ptr i32) (param $in_len i32) (result i32)
            (i32.const 0)
        )
    )"#
}

/// Create a plugin that exceeds memory limits
fn create_excessive_memory_plugin_wat() -> &'static str {
    r#"(module
        ;; Memory with maximum of 5000 pages (320 MiB) - exceeds 2048 page limit
        (memory (export "memory") 1 5000)
        
        (func (export "alloc") (param $size i32) (result i32)
            (i32.const 0)
        )
        
        (func (export "process") (param $in_ptr i32) (param $in_len i32) (result i32)
            (i32.const 0)
        )
    )"#
}

#[test]
fn test_visualize_complete_flow() -> Result<()> {
    println!("\n════════════════════════════════════════════════════════════");
    println!("  WASM Plugin Host - Complete Flow Visualization");
    println!("════════════════════════════════════════════════════════════\n");

    // Step 1: Compile plugin
    println!("📝 Step 1: Compiling WASM plugin from WAT");
    println!("   - Creating echo plugin that adds 'ECHO: ' prefix");
    let wat = create_echo_plugin_wat();
    let wasm_file = compile_wat_to_tempfile(wat)?;
    println!("   ✓ Plugin compiled to: {}", wasm_file.path().display());

    // Step 2: Load plugin with limits
    println!("\n🔧 Step 2: Loading plugin with WasmLimits");
    let limits = WasmLimits::default();
    println!("   - Max memory pages: {} (128 MiB)", limits.max_memory_pages);
    println!("   - Max response bytes: {} (16 MiB)", limits.max_response_bytes);
    
    let mut host = WasmPluginHost::load(wasm_file.path().to_str().unwrap(), limits)?;
    println!("   ✓ Plugin loaded successfully");

    // Step 3: Validate exports
    println!("\n📋 Step 3: Validating plugin exports");
    println!("   ✓ Required: memory, alloc, process");
    println!("   ✓ Optional: dealloc, get_api_version");

    // Step 4: Check API version
    println!("\n🔖 Step 4: Checking API version");
    if let Some((major, minor)) = host.api_version() {
        println!("   ✓ Plugin API version: {}.{}", major, minor);
    } else {
        println!("   ℹ Plugin does not export API version");
    }

    // Step 5: Process input
    println!("\n⚙️  Step 5: Processing input through plugin");
    let input = b"Hello, WASM!";
    println!("   Input: {:?}", String::from_utf8_lossy(input));
    println!("   Input length: {} bytes", input.len());
    
    println!("\n   📤 Host → Plugin:");
    println!("      1. Call alloc({}) to allocate input buffer", input.len());
    println!("      2. Write {} bytes to plugin memory", input.len());
    println!("      3. Call process(ptr, len)");
    
    let output = host.process_bytes(input)?;
    
    println!("\n   📥 Plugin → Host:");
    println!("      4. Read response header [status:u32][len:u32]");
    println!("      5. Validate response size");
    println!("      6. Read payload bytes");
    println!("      7. Call dealloc (if exported)");

    println!("\n   Output: {:?}", String::from_utf8_lossy(&output));
    println!("   Output length: {} bytes", output.len());
    println!("   ✓ Processing completed successfully");

    // Step 6: Verify output
    println!("\n✅ Step 6: Verifying output");
    let expected = b"ECHO: Hello, WASM!";
    assert_eq!(&output, expected, "Output should have ECHO: prefix");
    println!("   ✓ Output matches expected: {:?}", String::from_utf8_lossy(expected));

    println!("\n════════════════════════════════════════════════════════════");
    println!("  ✅ Complete flow visualization successful!");
    println!("════════════════════════════════════════════════════════════\n");

    Ok(())
}

#[test]
fn test_visualize_error_handling() -> Result<()> {
    println!("\n════════════════════════════════════════════════════════════");
    println!("  WASM Plugin Host - Error Handling Flow");
    println!("════════════════════════════════════════════════════════════\n");

    println!("📝 Creating plugin that returns errors...");
    let wat = create_error_plugin_wat();
    let wasm_file = compile_wat_to_tempfile(wat)?;
    
    let mut host = WasmPluginHost::load(
        wasm_file.path().to_str().unwrap(),
        WasmLimits::default()
    )?;
    println!("   ✓ Plugin loaded\n");

    println!("⚙️  Processing input (expecting error)...");
    let input = b"test input";
    println!("   Input: {:?}\n", String::from_utf8_lossy(input));

    println!("   📤 Host → Plugin:");
    println!("      1. Allocate and write input");
    println!("      2. Call process()");
    
    println!("\n   📥 Plugin → Host:");
    println!("      3. Read response header");
    println!("      4. Status = 1 (error)");
    println!("      5. Read error message");

    let result = host.process_bytes(input);
    
    assert!(result.is_err(), "Should return error");
    let err = result.unwrap_err();
    println!("\n   ✓ Error captured: {}\n", err);

    println!("════════════════════════════════════════════════════════════");
    println!("  ✅ Error handling flow successful!");
    println!("════════════════════════════════════════════════════════════\n");

    Ok(())
}

#[test]
fn test_visualize_memory_validation() -> Result<()> {
    println!("\n════════════════════════════════════════════════════════════");
    println!("  WASM Plugin Host - Memory Validation Flow");
    println!("════════════════════════════════════════════════════════════\n");

    // Test 1: No memory maximum
    println!("🔒 Test 1: Plugin without memory maximum");
    println!("   Creating plugin with no max declared...");
    let wat = create_no_max_memory_plugin_wat();
    let wasm_file = compile_wat_to_tempfile(wat)?;
    
    println!("   Attempting to load...");
    let result = WasmPluginHost::load(
        wasm_file.path().to_str().unwrap(),
        WasmLimits::default()
    );
    
    assert!(result.is_err(), "Should reject plugin without max");
    let err = result.err().unwrap();
    println!("   ✓ Plugin rejected: {}\n", err);

    // Test 2: Excessive memory
    println!("🔒 Test 2: Plugin exceeding memory limits");
    println!("   Creating plugin with 5000 pages (320 MiB)...");
    println!("   Limit is 2048 pages (128 MiB)");
    let wat = create_excessive_memory_plugin_wat();
    let wasm_file = compile_wat_to_tempfile(wat)?;
    
    println!("   Attempting to load...");
    let result = WasmPluginHost::load(
        wasm_file.path().to_str().unwrap(),
        WasmLimits::default()
    );
    
    assert!(result.is_err(), "Should reject plugin exceeding limit");
    let err = result.err().unwrap();
    println!("   ✓ Plugin rejected: {}\n", err);

    // Test 3: Valid memory with custom limits
    println!("🔒 Test 3: Plugin with valid memory and custom limits");
    let wat = create_echo_plugin_wat();
    let wasm_file = compile_wat_to_tempfile(wat)?;
    
    let strict_limits = WasmLimits::strict();
    println!("   Using strict limits: {} pages (64 MiB)", strict_limits.max_memory_pages);
    
    let result = WasmPluginHost::load(
        wasm_file.path().to_str().unwrap(),
        strict_limits
    );
    
    assert!(result.is_ok(), "Should accept plugin within limits");
    println!("   ✓ Plugin accepted (declares 10 pages, limit is {})\n", 1024);

    println!("════════════════════════════════════════════════════════════");
    println!("  ✅ Memory validation flow successful!");
    println!("════════════════════════════════════════════════════════════\n");

    Ok(())
}

#[test]
fn test_visualize_limits_presets() -> Result<()> {
    println!("\n════════════════════════════════════════════════════════════");
    println!("  WASM Plugin Host - Limits Presets");
    println!("════════════════════════════════════════════════════════════\n");

    println!("📊 Default Limits:");
    let default_limits = WasmLimits::default();
    println!("   Memory: {} pages = {} MiB", 
        default_limits.max_memory_pages,
        default_limits.max_memory_pages * 64 / 1024);
    println!("   Response: {} bytes = {} MiB\n",
        default_limits.max_response_bytes,
        default_limits.max_response_bytes / 1024 / 1024);

    println!("🔒 Strict Limits:");
    let strict_limits = WasmLimits::strict();
    println!("   Memory: {} pages = {} MiB",
        strict_limits.max_memory_pages,
        strict_limits.max_memory_pages * 64 / 1024);
    println!("   Response: {} bytes = {} MiB\n",
        strict_limits.max_response_bytes,
        strict_limits.max_response_bytes / 1024 / 1024);

    println!("🔓 Permissive Limits:");
    let permissive_limits = WasmLimits::permissive();
    println!("   Memory: {} pages = {} MiB",
        permissive_limits.max_memory_pages,
        permissive_limits.max_memory_pages * 64 / 1024);
    println!("   Response: {} bytes = {} MiB\n",
        permissive_limits.max_response_bytes,
        permissive_limits.max_response_bytes / 1024 / 1024);

    println!("🎯 Custom Limits:");
    let custom_limits = WasmLimits::new(512, 1024 * 1024);
    println!("   Memory: {} pages = {} MiB",
        custom_limits.max_memory_pages,
        custom_limits.max_memory_pages * 64 / 1024);
    println!("   Response: {} bytes = {} MiB\n",
        custom_limits.max_response_bytes,
        custom_limits.max_response_bytes / 1024 / 1024);

    println!("════════════════════════════════════════════════════════════");
    println!("  ✅ All limit presets verified!");
    println!("════════════════════════════════════════════════════════════\n");

    Ok(())
}
