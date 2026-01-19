# WASM Plugin Build & Test Tutorial

## Building Client Plugin

Build `tests/plugins/client.rs` to `client.wasm` using Cargo:

```bash
cd tests/plugins
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/wasm_plugin_client.wasm client.wasm
```

**What happens:**
- Uses `tests/plugins/Cargo.toml` which includes the `wasm_plugin_sdk_derive` proc-macro crate
- Supports `#[derive(DecodeFromEnvelope)]` with `#[decode(from = "json")]` syntax
- Compiles to WASM with 128 MiB memory limit (configured in `.cargo/config.toml`)
- Output: `tests/plugins/client.wasm`

**Note:** The memory limit is configured in `.cargo/config.toml` at the project root.

## Running the Test

```bash
cargo test test_client_plugin_converts_to_incoming_record \
  --test load_wasm_incoming_record_tests -- --exact --nocapture
```

**What the test does:**
1. Loads `client.wasm` plugin
2. Processes JSON input: `{"id":"...","ts_ms":...,"user_id":"...","action":"..."}`
3. Plugin converts to `IncomingRecord` (protobuf)
4. Host parses and verifies the result

## Quick Reference

**Build:**
```bash
cd tests/plugins
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/wasm_plugin_client.wasm client.wasm
```

**Test:**
```bash
cargo test test_client_plugin_converts_to_incoming_record --test load_wasm_incoming_record_tests -- --exact --nocapture
```

**Files:**
- `tests/plugins/client.rs` - Your plugin code (user writes this)
- `tests/plugins/src/lib.rs` - Plugin entry point
- `tests/plugins/Cargo.toml` - Cargo configuration
- `wasm_plugin_sdk_derive/` - Proc-macro crate for `#[derive(DecodeFromEnvelope)]`
- `src/wasm_host/infra.rs` - Infrastructure (SDK, allocator, protobuf, JSON parsing)

## Writing Your Plugin

Simply define your struct with `#[derive(DecodeFromEnvelope)]` and `#[decode(from = "json")]`:

```rust
#[derive(DecodeFromEnvelope)]
#[decode(from = "json")]
pub struct MyEvent {
    pub id: sdk::String,
    pub ts_ms: u64,
    pub user_id: sdk::String,
    pub action: sdk::String,
}
```

The derive macro automatically generates the JSON parsing implementation!
