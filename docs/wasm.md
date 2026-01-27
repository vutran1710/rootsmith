# WASM Plugin Auto-Build System

## Overview

Pass a `.rs` filename, get a ready-to-use `.wasm` path. Auto-builds if needed using a temporary Cargo project.

## Directory Structure

```
rootsmith/
├── plugins/
│   ├── input/           # Source .rs files (same format as tests/plugins/client.rs)
│   │   └── client.rs
│   └── output/          # Compiled .wasm files (auto-generated)
│       └── client.wasm
```

## Usage

```rust
use rootsmith::wasm_host::{get_or_build_plugin, WasmPluginHost, WasmLimits};

// Just pass the filename - builds automatically if needed
let wasm_path = get_or_build_plugin("client.rs")?;
let mut host = WasmPluginHost::load(wasm_path.to_str().unwrap(), WasmLimits::default())?;

// Process input
let output = host.process_bytes(input_json.as_bytes())?;
```

## Plugin Format

`plugins/input/client.rs` uses the same format as `tests/plugins/client.rs`:

```rust
// Define your event struct with derive macro
#[derive(DecodeFromEnvelope)]
#[decode(from = "json")]
pub struct MyWhateverEventName {
    pub id: sdk::String,
    pub ts_ms: u64,
    pub user_id: sdk::String,
    pub action: sdk::String,
}

// Implement ToRecord for metadata extraction
impl sdk::ToRecord for MyWhateverEventName {
    fn get_namespace(&self) -> [u8; 32] { /* ... */ }
    fn get_key(&self) -> [u8; 32] { /* ... */ }
    fn get_timestamp(&self) -> u64 { /* ... */ }
}

// Implement ToStandardData for value extraction
impl sdk::ToStandardData for MyWhateverEventName {
    fn get_value(&self) -> [u8; 32] { /* ... */ }
}

plugin! {
    name: "my-plugin",
    api_version: 1,
    type Input = MyWhateverEventName,
    record_kind: "my_event"
}
```

## Build Process

```
get_or_build_plugin("client.rs")
           │
           ▼
┌─────────────────────────────────────────┐
│  Check: plugins/output/client.wasm      │
│  exists and is newer than client.rs?    │
└─────────────────────────────────────────┘
           │
     ┌─────┴─────┐
     │ NO        │ YES
     ▼           ▼
┌───────────────────────────────┐  ┌─────────────┐
│  Create temp Cargo project:   │  │ Skip build  │
│  - Cargo.toml                 │  └─────────────┘
│  - .cargo/config.toml         │        │
│  - src/lib.rs (includes       │        │
│    infra.rs + client.rs)      │        │
│  - Build with cargo           │        │
│  - Copy .wasm to output/      │        │
└───────────────────────────────┘        │
           │                             │
           └─────────────┬───────────────┘
                         ▼
        ┌─────────────────────────────────────────┐
        │  Return: plugins/output/client.wasm     │
        └─────────────────────────────────────────┘
```

## Test

```bash
cargo test --test wasm_auto_build_test -- --nocapture
```

Output:
```
╔══════════════════════════════════════════════════════════════╗
║           WASM Auto-Build Flow Test                          ║
╚══════════════════════════════════════════════════════════════╝

┌─ Step 1: Request Plugin ──────────────────────────────────────┐
│  Input:  get_or_build_plugin("client.rs")                     │
│  Flow:   plugins/input/client.rs  ──►  plugins/output/client.wasm
└────────────────────────────────────────────────────────────────┘

┌─ Step 2: Build Result ────────────────────────────────────────┐
│  WASM path: plugins/output/client.wasm                        │
│  Size: ~10 KB (optimized with LTO)                            │
└────────────────────────────────────────────────────────────────┘

┌─ Step 3: Load & Process ──────────────────────────────────────┐
│  Input:  {"id":"evt-123","ts_ms":1700000000000,...}          │
│  Output: 108 bytes (protobuf IncomingRecord)                  │
└────────────────────────────────────────────────────────────────┘
```

## Files

| File | Purpose |
|------|---------|
| `plugins/input/client.rs` | Plugin source (same format as tests/plugins/client.rs) |
| `plugins/output/client.wasm` | Compiled plugin (auto-generated, ~10 KB) |
| `src/wasm_host/builder.rs` | Build logic using temp Cargo project |
| `src/wasm_host/infra.rs` | SDK infrastructure included in plugins |
| `wasm_plugin_sdk_derive/` | Proc-macro for `#[derive(DecodeFromEnvelope)]` |
| `tests/wasm_auto_build_test.rs` | Integration test |
