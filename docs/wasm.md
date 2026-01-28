# WASM Plugin System

## Flow

```
User Input (arbitrary JSON)
        ↓
   client.rs (user code)
   - DecodeFromEnvelope: parse JSON → struct
   - ToRecord: struct → namespace, key, timestamp
   - ToStandardData: struct → value
        ↓
   Host receives ParsedRecord
        ↓
   Accumulator commits the record
```

## Usage

```rust
use rootsmith::wasm_host::{get_or_build_plugin, WasmPluginHost, WasmLimits};

let wasm_path = get_or_build_plugin("client.rs")?;
let mut host = WasmPluginHost::load(wasm_path.to_str().unwrap(), Some(WasmLimits::default()))?;

let input = r#"{"id":"evt-123","user_id":"user-456"}"#;
let parsed = host.process(input.as_bytes())?;

println!("namespace: {}", parsed.namespace_str());
println!("key: {}", parsed.key_str());
println!("value: {}", parsed.value_str());

let record = parsed.into_record();
// commit to accumulator...
```

## Plugin Format

`plugins/input/client.rs`:

```rust
#[derive(DecodeFromEnvelope)]
#[decode(from = "json")]
pub struct MyEvent {
    pub id: sdk::String,
    pub ts_ms: u64,
    pub user_id: sdk::String,
    pub action: sdk::String,
}

impl sdk::ToRecord for MyEvent {
    fn get_namespace(&self) -> [u8; 32] { /* user defines mapping */ }
    fn get_key(&self) -> [u8; 32] { /* user defines mapping */ }
    fn get_timestamp(&self) -> u64 { /* user defines mapping */ }
}

impl sdk::ToStandardData for MyEvent {
    fn get_value(&self) -> [u8; 32] { /* user defines mapping */ }
}

plugin! {
    name: "my-plugin",
    api_version: 1,
    type Input = MyEvent,
    record_kind: "my_event"
}
```

## Test

```bash
cargo test --test wasm_auto_build_test -- --nocapture
```

## Files

| File | Purpose |
|------|---------|
| `plugins/input/client.rs` | Plugin source |
| `plugins/output/client.wasm` | Compiled plugin |
| `src/wasm_host/builder.rs` | Build logic |
| `src/wasm_host/host.rs` | Plugin host |
| `src/wasm_host/parsed_record.rs` | Output struct |
| `src/wasm_host/infra.rs` | SDK infrastructure |
