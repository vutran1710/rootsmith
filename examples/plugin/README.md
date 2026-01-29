# Example Parser Plugin

This is an example plugin for RootSmith that parses JSON events containing user data.

## Building

To compile this plugin to WebAssembly:

```bash
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release
```

The compiled `.wasm` file will be at:
```
target/wasm32-unknown-unknown/release/example_parser_plugin.wasm
```

## Plugin Functionality

This plugin expects JSON input with the following structure:

```json
{
  "user_id": "user123",
  "event_type": "login",
  "timestamp": 1738046590,
  "data": "optional metadata"
}
```

It creates records with:
- Namespace: `user_events_{user_id}` (truncated/padded to 16 bytes)
- Key: `{event_type}_{timestamp}` (truncated/padded to 16 bytes)
- Value: The data field as bytes
- Timestamp: The event timestamp

## Running with RootSmith

Update your `config.toml` to point to the compiled wasm file:

```toml
plugin_path = "examples/plugin/target/wasm32-unknown-unknown/release/example_parser_plugin.wasm"
```

Then run RootSmith:

```bash
cargo run -- -c examples/config.toml
```
