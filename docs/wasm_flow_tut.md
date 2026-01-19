# WASM Host Test Flow Tutorial

This tutorial explains how to compile a WASM plugin and run the WASM host test to visualize the complete flow.

## Prerequisites

- Rust toolchain installed
- `wasm32-unknown-unknown` target installed:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```

## Step 1: Compile the WASM Plugin

Navigate to the plugins directory and compile the plugin:

```bash
cd tests/plugins
rustc \
  --target wasm32-unknown-unknown \
  -O \
  --crate-type=cdylib \
  -C link-arg=--max-memory=134217728 \
  plugin.rs \
  -o plugin.wasm
```

### Compilation Flags Explained

- `--target wasm32-unknown-unknown`: Compiles for WebAssembly target
- `-O`: Optimizes the output for size and performance
- `--crate-type=cdylib`: Creates a dynamic library (required for WASM)
- `-C link-arg=--max-memory=134217728`: Sets maximum memory to 128 MiB (2048 pages × 64 KiB)
  - **Important**: This is required! The WASM host validates that plugins declare a memory maximum
- `plugin.rs`: Source file containing the plugin logic
- `-o plugin.wasm`: Output file name

### Expected Output

If successful, you should see:
- No errors (warnings are okay)
- A `plugin.wasm` file created (typically ~500-700 KB)

## Step 2: Run the WASM Host Test

Navigate back to the project root and run the test:

```bash
cd ../..  # Back to project root
cargo test test_load_and_run_compiled_plugin --test load_wasm_tests -- --exact --nocapture
```

### Test Flags Explained

- `test_load_and_run_compiled_plugin`: The specific test function name
- `--test load_wasm_tests`: Only run tests from `tests/load_wasm_tests.rs`
- `-- --exact`: Match the test name exactly
- `--nocapture`: **Critical!** Shows all `println!` output (the visualization)

## Complete Flow Visualization

When you run the test, you'll see a step-by-step visualization:

```
🎯 Testing Compiled WASM Plugin
═══════════════════════════════════════════════════════════

📂 Step 1: Locating compiled WASM plugin...
   Path: /path/to/tests/plugins/plugin.wasm
   File size: 573384 bytes

🔧 Step 2: Initializing WASM host and loading plugin...
   Max memory pages: 2048
   Max response bytes: 16777216 bytes
   ✅ Plugin loaded and instantiated successfully

🔍 Step 3: Checking API version...
   Plugin does not export get_api_version (optional)

🚀 Step 4: Processing test data...
   Input:  "Hello, WASM World!"
   Input bytes: 18 bytes
   Output: "!dlroW MSAW ,olleH"
   Output bytes: 18 bytes

✅ Step 5: Verifying output...
   ✓ Output matches expected reversed bytes

🔄 Step 6: Testing with different input...
   Input:  "12345"
   Output: "54321"
   ✓ Second test passed

🔄 Step 7: Testing with empty input...
   ✓ Empty input handled correctly

🔄 Step 8: Testing with larger data...
   Input: 256 bytes (0x00..0xFF)
   ✓ Large data processed correctly

═══════════════════════════════════════════════════════════
🎉 All tests passed! Plugin is working correctly.
```

## What Happens Under the Hood

### 1. Plugin Loading (`WasmPluginHost::load()`)
- Reads the WASM binary from disk
- Validates the plugin has required exports:
  - `memory`: Linear memory export
  - `alloc(size: usize) -> *mut u8`: Memory allocation function
  - `process(ptr: *const u8, len: usize) -> *mut u8`: Main processing function
- Checks memory limits (must declare maximum ≤ 2048 pages)
- Instantiates the WASM module in a sandboxed environment

### 2. Memory Validation
- Ensures plugin declares a memory maximum (required for security)
- Validates maximum doesn't exceed host limits (2048 pages = 128 MiB)
- Rejects plugins without memory limits

### 3. Data Processing (`host.process_bytes()`)
- Allocates memory in WASM linear memory for input data
- Writes input bytes to WASM memory
- Calls the plugin's `process()` function
- Reads the response from WASM memory
- Deallocates temporary memory
- Returns the processed output

### 4. Response Format
The plugin returns data in this format:
```
[status: u32 (4 bytes)][length: u32 (4 bytes)][payload: bytes...]
```

Where:
- `status`: `0` = OK, `1` = Error
- `length`: Length of payload in bytes
- `payload`: The actual processed data

## Plugin Behavior

The test plugin (`tests/plugins/plugin.rs`) implements a simple byte-reversal function:
- Input: `"Hello, WASM World!"`
- Output: `"!dlroW MSAW ,olleH"`

This demonstrates:
- ✅ Memory allocation/deallocation
- ✅ Data passing between host and plugin
- ✅ Plugin execution in sandboxed environment
- ✅ Resource limit enforcement

## Troubleshooting

### Error: "duplicate export name `memory`"
**Solution**: Remove any manual `pub static mut memory` declarations from the plugin. The Rust compiler automatically exports memory.

### Error: "plugin memory must declare a maximum"
**Solution**: Add `-C link-arg=--max-memory=134217728` to the compilation command.

### Error: "Failed to find plugin.wasm"
**Solution**: Make sure you compiled the plugin first and it's in `tests/plugins/plugin.wasm`.

### No output visible
**Solution**: Always use `--nocapture` flag! Without it, `println!` output is hidden.

## Quick Reference

**Compile plugin:**
```bash
cd tests/plugins
rustc --target wasm32-unknown-unknown -O --crate-type=cdylib \
  -C link-arg=--max-memory=134217728 plugin.rs -o plugin.wasm
```

**Run test:**
```bash
cargo test test_load_and_run_compiled_plugin --test load_wasm_tests -- --exact --nocapture
```

**Run all WASM tests:**
```bash
cargo test --test load_wasm_tests -- --nocapture
```

## Next Steps

- Modify `tests/plugins/plugin.rs` to implement your own processing logic
- Adjust memory limits in `WasmLimits::default()` for different use cases
- Explore the WASM host API in `src/wasm_host/host.rs`
- Check out other WASM tests in `tests/wasm_tests.rs` for more examples
