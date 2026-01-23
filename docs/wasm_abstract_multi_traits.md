# WASM Abstract Multi-Traits Approach

## Core Idea

Separate **metadata** (namespace, key, timestamp) from **payload** (value/data) using two traits:
- `ToRecord`: Provides metadata fields (namespace, key, timestamp)
- `ToData`: Provides the payload (value/data bytes)

This allows partners to:
- Reuse the same metadata extraction logic across different output formats
- Choose different payload formats (fixed-size, variable-length, JSON, raw) independently
- Keep code DRY and flexible

## Trait Definitions

### `ToRecord` Trait (Metadata)

```rust
// src/wasm_host/infra.rs
pub mod sdk {
    pub trait ToRecord {
        /// Extract namespace (32 bytes)
        fn get_namespace(&self) -> [u8; 32];
        
        /// Extract key (32 bytes)
        fn get_key(&self) -> [u8; 32];
        
        /// Extract timestamp (Unix seconds)
        fn get_timestamp(&self) -> u64;
    }
}
```

**Purpose**: Extract common metadata fields that are needed for all output formats.

### `ToData` Trait (Payload)

```rust
pub mod sdk {
    /// Payload format variants
    pub enum DataFormat {
        /// Fixed-size value (32 bytes) - for Standard format
        Fixed([u8; 32]),
        /// Variable-length value - for Extended format
        Variable(Vec<u8>),
        /// JSON string payload - for Custom format
        Json(&'static str),
        /// Raw bytes - for Raw format
        Raw(&'static [u8]),
    }
    
    pub trait ToData {
        /// Extract the payload data in the desired format
        fn get_data(&self) -> DataFormat;
    }
}
```

**Purpose**: Extract the payload/data portion, allowing partners to choose the format.

## Partner Implementation Example

```rust
// tests/plugins/client.rs

#[derive(DecodeFromEnvelope)]
#[decode(from = "json")]
pub struct MyWhateverEventName {
    pub id: sdk::String,
    pub ts_ms: u64,
    pub user_id: sdk::String,
    pub action: sdk::String,
}

impl ToRecord for MyWhateverEventName {
    fn get_namespace(&self) -> [u8; 32] {
        // Convert user_id to namespace
        let mut namespace = [0u8; 32];
        let user_id_str = self.user_id.as_str();
        let user_id_bytes = user_id_str.as_bytes();
        for (i, &b) in user_id_bytes.iter().take(32).enumerate() {
            namespace[i] = b;
        }
        for i in user_id_bytes.len().min(32)..32 {
            namespace[i] = (i * 7) as u8;
        }
        namespace
    }
    
    fn get_key(&self) -> [u8; 32] {
        // Convert id to key
        let mut key = [0u8; 32];
        let id_str = self.id.as_str();
        let id_bytes = id_str.as_bytes();
        for (i, &b) in id_bytes.iter().take(32).enumerate() {
            key[i] = b;
        }
        for i in id_bytes.len().min(32)..32 {
            key[i] = (i * 11) as u8;
        }
        key
    }
    
    fn get_timestamp(&self) -> u64 {
        self.ts_ms / 1000 // Convert ms to seconds
    }
}

impl ToData for MyWhateverEventName {
    fn get_data(&self) -> DataFormat {
        // Option 1: Fixed-size (Standard format)
        let mut value = [0u8; 32];
        let action_str = self.action.as_str();
        let action_bytes = action_str.as_bytes();
        for (i, &b) in action_bytes.iter().take(32).enumerate() {
            value[i] = b;
        }
        for i in action_bytes.len().min(32)..32 {
            value[i] = (i * 13) as u8;
        }
        DataFormat::Fixed(value)
        
        // Option 2: Variable-length (Extended format)
        // DataFormat::Variable(self.action.as_str().as_bytes().to_vec())
        
        // Option 3: JSON (Custom format)
        // let json = format!(r#"{{"action":"{}"}}"#, self.action.as_str());
        // DataFormat::Json(store_str_static(&json))
        
        // Option 4: Raw bytes
        // DataFormat::Raw(store_bytes_static(self.action.as_str().as_bytes()))
    }
}
```

## Plugin Glue Logic

The plugin's `process()` function combines metadata + payload:

```rust
// tests/plugins/src/lib.rs

#[no_mangle]
pub extern "C" fn process(ptr: *const u8, len: usize) -> *mut u8 {
    if ptr.is_null() {
        return encode_response(1, b"null input pointer");
    }

    let input = unsafe { slice::from_raw_parts(ptr, len) };
    
    // Decode from JSON envelope
    let event = match MyWhateverEventName::decode_from_json(input) {
        Some(e) => e,
        None => return encode_response(1, b"failed to decode JSON"),
    };
    
    // Extract metadata
    let namespace = event.get_namespace();
    let key = event.get_key();
    let timestamp = event.get_timestamp();
    
    // Extract payload
    let data = event.get_data();
    
    // Encode based on data format
    let (out_ptr, out_len) = match data {
        DataFormat::Fixed(value) => {
            // Standard format: protobuf IncomingRecord
            let record = IncomingRecord {
                namespace,
                key,
                value,
                timestamp,
            };
            encode_protobuf(&record)
        }
        DataFormat::Variable(bytes) => {
            // Extended format: encode as variable-length protobuf or raw bytes
            // For MVP: encode as raw bytes (host will classify)
            encode_raw_bytes(&bytes)
        }
        DataFormat::Json(json_str) => {
            // Custom format: encode JSON bytes
            encode_json_bytes(json_str.as_bytes())
        }
        DataFormat::Raw(bytes) => {
            // Raw format: encode raw bytes
            encode_raw_bytes(bytes)
        }
    };
    
    let out_slice = unsafe { slice::from_raw_parts(out_ptr, out_len) };
    encode_response(0, out_slice)
}
```

## Alternative: Separate Traits for Each Format

Instead of `DataFormat` enum, use separate traits:

```rust
pub mod sdk {
    pub trait ToRecord {
        fn get_namespace(&self) -> [u8; 32];
        fn get_key(&self) -> [u8; 32];
        fn get_timestamp(&self) -> u64;
    }
    
    /// For Standard format (fixed-size value)
    pub trait ToStandardData {
        fn get_value(&self) -> [u8; 32];
    }
    
    /// For Extended format (variable-length value)
    pub trait ToExtendedData {
        fn get_value(&self) -> &[u8];
        fn get_metadata_json(&self) -> Option<&str>;
    }
    
    /// For Custom format (JSON payload)
    pub trait ToCustomJsonData {
        fn get_payload_json(&self) -> &str;
    }
    
    /// For Raw format (raw bytes)
    pub trait ToRawData {
        fn get_raw_bytes(&self) -> &[u8];
    }
}
```

**Partner implements**:
- `ToRecord` (always)
- One of: `ToStandardData`, `ToExtendedData`, `ToCustomJsonData`, or `ToRawData`

## Revised Approach: Trait Objects Instead of Structs/Enums

**Key insight**: Since we have good traits defined, we don't need to convert plugin output to structs or enums anymore. Instead, the host returns **trait objects** (`dyn Trait`) that are passed directly to the accumulator.

### Host-Side API


```rust
// src/wasm_host/host.rs
impl WasmPluginHost {
    /// Process input and return trait object based on plugin output format
    pub fn process_input<T>(&mut self, input: &[u8]) -> Result<Box<dyn T>>
    where
        T: ?Sized + 'static,
    {
        let payload = self.process_bytes(input)?;
        // Decode/classify and return appropriate trait object
        // Implementation depends on detecting format from bytes
    }
}
```
Use only one method process_input to handle data
### Accumulator Integration

```rust
// Example usage in accumulator

// Direct trait object passing - no intermediate structs/enums
let output: Box<dyn sdk::ToCustomJsonData> = host.process_as_custom_json(input)?;
accumulator.build(output);

// Or with generic trait bound
fn process_plugin_output<T: sdk::ToCustomJsonData>(output: Box<dyn T>) {
    accumulator.build(output);
}
```

### Benefits

1. **No intermediate conversion**: Plugin output → trait object → accumulator
2. **No struct/enum overhead**: Direct trait usage throughout
3. **Flexible**: Accumulator can accept any trait that meets its requirements
4. **Type-safe**: Compile-time guarantees via trait bounds
5. **Clean separation**: Plugin outputs trait, accumulator consumes trait

### Implementation Strategy

The host needs to:
1. Detect plugin output format (from bytes: protobuf vs JSON vs raw)
2. Parse bytes into appropriate data structure
3. Wrap in a trait object that implements the desired trait
4. Return `Box<dyn Trait>` to the accumulator

**Example trait wrapper**:

```rust
// Host-side wrapper that implements plugin traits
struct PluginOutputWrapper {
    namespace: [u8; 32],
    key: [u8; 32],
    timestamp: u64,
    payload_json: String,  // or other payload types
}

impl sdk::ToCustomJsonData for PluginOutputWrapper {
    fn get_payload_json(&self) -> &str {
        &self.payload_json
    }
}

// Host creates this wrapper from plugin bytes and returns as trait object
```

### Summary

- **Plugin side**: Partners implement traits (`ToCustomJsonData`, `ToStandardData`, etc.)
- **Host side**: Returns `Box<dyn Trait>` objects, not structs/enums
- **Accumulator side**: Accepts trait objects directly, no conversion needed
- **No intermediate structs**: Everything works through trait objects

This approach eliminates the need for `PluginOut` enum and intermediate struct conversions!