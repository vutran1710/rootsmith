# Rootsmith Plugin SDK

Minimal effort parser development for Rootsmith WASM plugins.

## Quick Start

Only implement the `decode()` function:

```rust
use rootsmith_plugin_sdk::*;

#[derive(Deserialize)]
struct UserEvent {
    user_id: String,
    event_type: String,
    timestamp: u64,
    data: Option<String>,
}

pub fn decode(input: UpstreamData) -> Result<Option<Record>> {
    info!("Processing input");

    match input {
        UpstreamData::Json(bytes) => {
            info!("Decoding JSON: {} bytes", bytes.len());
            let event: UserEvent = from_json(bytes)?;

            let metadata = json! { event_type: event.event_type, user_id: event.user_id };

            info!("Created record for user: {}", event.user_id);

            Ok(Some(Record {
                namespace: to_fixed_bytes(&format!("user_events_{}", event.user_id), 16),
                key: to_fixed_bytes(&format!("{}_{}", event.event_type, event.timestamp), 16),
                value: event.data.unwrap_or_default().into_bytes(),
                timestamp: event.timestamp,
                metadata: Some(metadata),
            }))
        }
        _ => {
            warn!("Unsupported input type");
            Ok(None)
        }
    }
}

rootsmith_plugin!(decode);
```

## Logging

Use logging macros to debug your plugin:

```rust
info!("Processing {}", user_id);
warn!("Skipping invalid record");
error!("Failed to parse: {}", e);
```

Logs appear in host logs with `[PLUGIN]` prefix.

## `json!` Macro

Create JSON bytes easily for metadata:

```rust
// Simple object
let metadata = json! { name: "test", count: 42 };

// With trailing comma
let metadata = json! { name: "test", count: 42, };
```

## Building

```bash
rootsmith -c config.toml
```

The CLI automatically compiles `.rs` plugin files to WASM.

## Features

- `no_std` compatible
- Uses `serde_json_core` for JSON parsing
- `json!` macro for easy JSON creation
- `info!`, `warn!`, `error!` logging macros
- Automatic postcard encoding for Records
- Zero-boilerplate plugin development
