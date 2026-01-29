# Monorepo Structure Proposal

## Current Structure

```
rootsmith/
├── Cargo.toml              # single package
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── wasm_host/          # embedded module
│   └── ...
├── rootsmith-plugin-sdk/   # separate workspace
└── examples/plugin/        # example plugin
```

## Proposed Structure

```
rootsmith/
├── Cargo.toml                      # workspace root
├── crates/
│   ├── rootsmith/                  # main binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── lib.rs
│   │       ├── config.rs
│   │       ├── storage.rs
│   │       ├── types.rs
│   │       ├── accumulator/
│   │       ├── archiver/
│   │       ├── downstream/
│   │       ├── upstream/
│   │       └── ...
│   │
│   ├── rootsmith-wasm-host/        # wasm host (extracted)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── builder.rs
│   │       ├── host.rs
│   │       ├── error.rs
│   │       ├── limits.rs
│   │       └── functions.rs
│   │
│   └── rootsmith-plugin-sdk/       # plugin SDK (moved)
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs
│
├── examples/
│   ├── plugin/                     # example plugin
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   └── output/                     # built .wasm files
│
└── tests/                          # integration tests
```

## Root Cargo.toml

```toml
[workspace]
resolver = "2"
members = [
    "crates/rootsmith",
    "crates/rootsmith-wasm-host",
    "crates/rootsmith-plugin-sdk",
    "examples/plugin",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"

[workspace.dependencies]
# Shared dependencies
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"

# WASM
wasmer = "4"
wasmer-compiler-cranelift = "4"
postcard = { version = "1.1.3", features = ["alloc"] }

# Internal crates
rootsmith-wasm-host = { path = "crates/rootsmith-wasm-host" }
rootsmith-plugin-sdk = { path = "crates/rootsmith-plugin-sdk" }
```

## Crate Dependencies

```
┌─────────────────────┐
│     rootsmith       │  (binary)
│                     │
│  depends on:        │
│  - rootsmith-wasm-host
└─────────┬───────────┘
          │
          ▼
┌─────────────────────┐
│ rootsmith-wasm-host │  (library)
│                     │
│  depends on:        │
│  - wasmer           │
│  - postcard         │
└─────────────────────┘

┌─────────────────────┐
│rootsmith-plugin-sdk │  (library, no_std)
│                     │
│  used by:           │
│  - examples/plugin  │
│  - user plugins     │
└─────────────────────┘
```

## Migration Steps

1. Create `crates/` directory
2. Move `rootsmith-plugin-sdk/` to `crates/rootsmith-plugin-sdk/`
3. Create `crates/rootsmith-wasm-host/` from `src/wasm_host/`
4. Move remaining `src/` to `crates/rootsmith/src/`
5. Update root `Cargo.toml` to workspace
6. Update all internal path dependencies
7. Update `examples/plugin/Cargo.toml` path
8. Keep `tests/` at workspace root
9. Run `cargo build` to verify

## Test Compatibility

`tests/wasm_host_tests.rs` must continue to work. It uses:

```rust
use rootsmith::types::UpstreamData;
use rootsmith::wasm_host::{WasmBuilder, WasmLimits, WasmPluginHost};

let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/plugin/src/lib.rs");
```

### Solution

**1. Re-export `rootsmith-wasm-host` from `rootsmith` crate:**

In `crates/rootsmith/src/lib.rs`:
```rust
pub mod types;

// Re-export wasm_host from the separate crate
pub use rootsmith_wasm_host as wasm_host;
```

In `crates/rootsmith/Cargo.toml`:
```toml
[dependencies]
rootsmith-wasm-host = { path = "../rootsmith-wasm-host" }
```

**2. Move tests to `crates/rootsmith/tests/`:**

```
rootsmith/
├── crates/
│   └── rootsmith/
│       ├── tests/
│       │   └── wasm_host_tests.rs   # moved here
```

**3. Update paths in test:**

`env!("CARGO_MANIFEST_DIR")` will be `crates/rootsmith/`, so update paths:

```rust
// Before
let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/plugin/src/lib.rs");

// After (go up 2 levels to workspace root)
let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
let source = workspace_root.join("examples/plugin/src/lib.rs");
```

Or define workspace root in build.rs:
```rust
// crates/rootsmith/build.rs
fn main() {
    let workspace = std::env::var("CARGO_WORKSPACE_DIR")
        .unwrap_or_else(|_| "../..".to_string());
    println!("cargo:rustc-env=WORKSPACE_ROOT={}", workspace);
}
```

Then in test:
```rust
let source = Path::new(env!("WORKSPACE_ROOT")).join("examples/plugin/src/lib.rs");
```

## Benefits

- Clear separation of concerns
- `rootsmith-wasm-host` can be published independently
- `rootsmith-plugin-sdk` can be published for plugin authors
- Faster incremental builds (only rebuild changed crates)
- Easier testing of individual components
