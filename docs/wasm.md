# Building WASM Plugins

## Prerequisites

```bash
rustup target add wasm32-unknown-unknown
```

## Build

```bash
cargo build --manifest-path examples/plugin/Cargo.toml --target wasm32-unknown-unknown --release && mkdir -p examples/output && cp examples/plugin/target/wasm32-unknown-unknown/release/example_parser_plugin.wasm examples/output/lib.wasm
```

Output: `examples/output/lib.wasm`

## Plugin Structure

```rust
// examples/plugin/src/lib.rs
#![no_std]
extern crate alloc;

use rootsmith_plugin_sdk::*;

#[derive(Deserialize)]
struct UserEvent {
    user_id: String,
    event_type: String,
    timestamp: u64,
}

pub fn decode(input: UpstreamData) -> Result<Option<Record>> {
    match input {
        UpstreamData::Json(bytes) => {
            let event: UserEvent = from_json(bytes)?;
            Ok(Some(Record {
                namespace: to_fixed_bytes(&event.user_id, 16),
                key: to_fixed_bytes(&event.event_type, 16),
                value: vec![],
                timestamp: event.timestamp,
                metadata: None,
            }))
        }
        _ => Ok(None),
    }
}

rootsmith_plugin!(decode);
```

## SDK Exports

| Function                    | Description                        |
| --------------------------- | ---------------------------------- |
| `from_json(bytes)`          | Parse JSON bytes to struct         |
| `to_fixed_bytes(s, len)`    | Convert string to fixed-size bytes |
| `to_postcard_bytes(record)` | Serialize Record                   |

## WASM Exports

| Export            | Signature           | Description                        |
| ----------------- | ------------------- | ---------------------------------- |
| `alloc`           | `(i32) -> i32`      | Allocate memory                    |
| `process`         | `(i32, i32) -> i32` | Process input, return response ptr |
| `get_api_version` | `() -> i32`         | Return API version                 |
