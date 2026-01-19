# WASM Plugin Build & Test Tutorial

## Building Client Plugin

Build `tests/plugins/client.rs` to `client.wasm`:

```bash
cd src/wasm_host
rustc --target wasm32-unknown-unknown -O --crate-type=cdylib \
  -C link-arg=--max-memory=134217728 \
  client_build.rs -o ../../tests/plugins/client.wasm
```

**What happens:**
- `client_build.rs` includes infrastructure (`infra.rs`) and your plugin code (`client.rs`)
- Compiles to WASM with 128 MiB memory limit
- Output: `tests/plugins/client.wasm`

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
cd src/wasm_host && rustc --target wasm32-unknown-unknown -O --crate-type=cdylib -C link-arg=--max-memory=134217728 client_build.rs -o ../../tests/plugins/client.wasm
```

**Test:**
```bash
cargo test test_client_plugin_converts_to_incoming_record --test load_wasm_incoming_record_tests -- --exact --nocapture
```

**Files:**
- `tests/plugins/client.rs` - Your plugin code (user writes this)
- `src/wasm_host/client_build.rs` - Build wrapper (includes infrastructure)
- `src/wasm_host/infra.rs` - Infrastructure (SDK, allocator, protobuf)
