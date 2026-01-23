# Protobuf Support for WebSocket Upstream

## Overview

RootSmith now supports **Protocol Buffers (protobuf)** for parsing binary messages received via WebSocket connections. This provides efficient, schema-based serialization for high-throughput data ingestion.

## Architecture

### Files Created

1. **`proto/incoming_record.proto`** - Protobuf schema definition
2. **`build.rs`** - Build script to compile `.proto` files
3. **`src/parser/mod.rs`** - Parser module declaration
4. **`src/parser/proto.rs`** - Protobuf parsing logic and validation

### Configuration

**`.cargo/config.toml`** - Enables git CLI for dependency fetching:
```toml
[net]
git-fetch-with-cli = true
```

**`Cargo.toml`** dependencies:
```toml
prost = "0.13"
prost-types = "0.13"

[build-dependencies]
prost-build = "0.13"
```

## Protobuf Message Schema

```protobuf
message IncomingRecord {
  bytes namespace = 1;  // 32-byte namespace identifier
  bytes key = 2;        // 32-byte key
  bytes value = 3;      // 32-byte value
  uint64 timestamp = 4; // Unix timestamp in seconds
}
```

## Usage

### WebSocket Binary Messages

The `WebSocketSource` automatically parses binary protobuf messages:

```rust
use rootsmith::upstream::WebSocketSource;
use rootsmith::traits::UpstreamConnector;

let mut ws = WebSocketSource::new("ws://localhost:8080".to_string());
ws.open(tx).await?;  // Automatically parses protobuf binary messages
```

### Manual Parsing

You can also parse protobuf messages manually:

```rust
use rootsmith::parser::proto::parse_proto_message;

let bytes = /* binary protobuf data */;
let record = parse_proto_message(&bytes)?;
```

## Validation

The parser validates:
- ✅ Valid protobuf encoding
- ✅ Namespace is exactly 32 bytes
- ✅ Key is exactly 32 bytes
- ✅ Value is exactly 32 bytes

Invalid messages are logged and skipped (not fatal).

## Testing

All 10 websocket tests pass:
```bash
cargo test --lib upstream::websocket
```

Tests cover:
- Valid protobuf message parsing
- Invalid protobuf handling
- Binary message handling
- Text message rejection
- Close/Ping/Pong frame handling

## Git Dependencies

The project uses SSH git dependencies for ZK proof generation:

```toml
multiple-circuit-prover = { git = "ssh://git@github.com/OneMatrixL1/multiple-rust-package.git", branch = "main" }
multiple-circuit-verifier = { git = "ssh://git@github.com/OneMatrixL1/multiple-rust-package.git", branch = "main" }
```

## Next Steps

Ready for integration options:
1. **HTTP API** - Already implemented with `/ingest` endpoint
2. **WebSocket + Protobuf** - Now complete ✅
3. **ZK Proof Generation** - Ready to integrate

Would you like to proceed with ZK proof integration?
