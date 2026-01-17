# Wasmer WASM Host Implementation Guide (Plugin API v1)

This document is a **drop‑in implementation guide** for building a Wasmer‑based WASM host inside the ingestor. It is written so you can hand it to an AI agent or engineer and ask them to implement it as a standalone crate.

---

## 1. Design Goal (Non‑Negotiable)

The host must:

- Execute **user‑supplied WASM plugins** compiled from a **single Rust file**
- Enforce **strict sandboxing**
  - No WASI
  - No filesystem / network
- Enforce **hard resource limits**
  - **128 MiB max linear memory**
  - **bounded execution time** (instruction metering)
- Interact with plugins via a **stable byte‑oriented ABI**
- Support **API version pinning** via `plugin!{ api_version: N }`

The host must not rely on user‑provided Cargo projects, dependencies, or runtime configuration.

---

## 2. High‑Level Architecture

```
Connector (HTTP / WS / Bus)
        |
        v
   Envelope (protobuf bytes)
        |
        v
WasmPluginHost::process_bytes
        |
        v
+-----------------------------+
|   WASM PLUGIN (sandboxed)   |
|                             |
|  alloc -> write input       |
|  process(ptr,len)           |
|  -> decode Envelope         |
|  -> map Input -> Record     |
|  -> encode RecordBatch      |
|  -> return bytes            |
+-----------------------------+
        |
        v
   Host decodes output
```

---

## 3. ABI Contract (Host ↔ Plugin)

### Required Exports

The plugin **must export**:

- `memory: WebAssembly.Memory`
- `alloc(size: i32) -> i32`
- `process(ptr: i32, len: i32) -> i32`

### Optional Exports (Recommended)

- `dealloc(ptr: i32, size: i32)`
- `get_api_version() -> i32`  
  Encoded as `(MAJOR << 16) | MINOR`
- `get_input_format() -> i32`
- `get_output_format() -> i32`

### `process()` Return Layout

The pointer returned by `process()` must point to:

```
[u32 status][u32 len][u8 payload...]
```

- `status = 0` → success, payload is output bytes
- `status = 1` → error, payload is UTF‑8 error string

---

## 4. Hard Limits

### Memory

- Maximum allowed memory: **128 MiB**
- WASM page size: **64 KiB**
- Max pages: `128 MiB / 64 KiB = 2048 pages`

**Rules**:
- Plugin memory **must declare a maximum**
- Maximum must be `<= 2048` pages

### Execution Time

- Enforced using **instruction metering** (Wasmer middleware)
- Each `process()` call is limited to a fixed instruction budget
- Exceeding budget results in a trap → mapped to a host error

### Response Size

- Host enforces `max_response_bytes` (recommended: **16 MiB**)

---

## 5. Crate Layout

Create a crate inside the ingestor workspace:

```
crates/ingestor_wasm_host/
  Cargo.toml
  src/
    lib.rs
    host.rs
    limits.rs
    error.rs
    memory.rs
  tests/
    real_plugin.rs
```

---

## 6. Dependencies (`Cargo.toml`)

```toml
[dependencies]
anyhow = "1"
thiserror = "2"

wasmer = "4"
wasmer-compiler-cranelift = "4"
wasmer-middlewares = "4"

[dev-dependencies]
tempfile = "3"
```

---

## 7. Public API

```rust
pub struct WasmLimits {
    pub max_memory_pages: u32,     // 2048
    pub max_response_bytes: usize, // e.g. 16 MiB
    pub fuel_per_call: u64,        // instruction budget
}

pub struct WasmPluginHost {
    // store, instance, memory, exports
}

impl WasmPluginHost {
    pub fn load(path: &str, limits: WasmLimits) -> anyhow::Result<Self>;
    pub fn process_bytes(&mut self, input: &[u8]) -> anyhow::Result<Vec<u8>>;
}
```

---

## 8. Execution Metering (Duration Limit)

Use Wasmer middleware to enforce instruction limits.

```rust
use wasmer::{Engine, Store};
use wasmer_compiler_cranelift::Cranelift;
use wasmer_middlewares::Metering;

pub fn make_metered_store(fuel: u64) -> Store {
    let mut compiler = Cranelift::default();
    compiler.push_middleware(Metering::new(fuel, |_| 1u64));
    let engine: Engine = compiler.into();
    Store::new(engine)
}
```

- Each operator costs `1` instruction
- Exceeding fuel triggers a trap

---

## 9. Loading the Plugin

### `WasmPluginHost::load`

Steps:

1. Create metered `Store`
2. Load `Module` from file
3. Instantiate with **empty imports** (`imports! {}`)
4. Resolve exports
   - `memory`
   - `alloc`
   - `process`
   - optional `dealloc`
   - optional `get_api_version`
5. Validate memory maximum
6. Validate API version (if export exists)

### Memory Validation

- Read memory type
- Reject if:
  - no maximum declared
  - maximum > `limits.max_memory_pages`

---

## 10. Memory Read / Write Helpers

All memory access must be bounds‑checked.

Required helpers:

- `write_memory(ptr: i32, data: &[u8])`
- `read_memory(ptr: i32, len: usize) -> Vec<u8>`

Rules:
- Convert pointers to `u64`
- Ensure `[ptr, ptr+len)` is within `memory.data_size()`
- Error on out‑of‑bounds access

---

## 11. `process_bytes()` Flow

```text
1. alloc(input.len)
2. write input bytes into memory
3. call process(ptr, len)
4. read response header (8 bytes)
5. parse status + payload length
6. enforce max_response_bytes
7. read payload
8. optional dealloc
9. return Ok or Err
```

### Trap Handling

- If call traps due to metering → return `BudgetExceeded`
- Other traps → return `PluginTrap`

---

## 12. Error Model

Recommended error categories:

- `MissingExport`
- `MemoryLimitExceeded`
- `ResponseTooLarge`
- `BudgetExceeded`
- `PluginError(String)`
- `PluginTrap(String)`

Errors should be converted into `anyhow::Error` at the boundary.

---

## 13. API Version Pinning

`plugin! { api_version: N }` must generate:

```rust
#[no_mangle]
pub extern "C" fn get_api_version() -> i32 {
    (N << 16)
}
```

Host behavior:
- If export exists, validate major version
- Reject incompatible versions

---

## 14. Integration Test (End‑to‑End)

The host must include a real integration test that:

1. Writes a temporary `plugin.rs`
2. Compiles it to WASM (controlled build)
3. Loads it using `WasmPluginHost`
4. Calls `process_bytes`
5. Asserts output or error

This verifies:
- ABI correctness
- memory limits
- execution metering

---

## 15. Security Model Summary

This host guarantees:

- No filesystem or network access
- Bounded memory usage
- Bounded execution time

It does **not** guarantee:

- semantic correctness
- resistance to logic bombs within budget

This is acceptable for v1.

---

## 16. Definition of Done

The implementation is complete when:

- Plugins without memory max are rejected
- Plugins exceeding 128 MiB are rejected
- Budget exhaustion is detected and reported
- Valid plugins execute correctly
- Integration test passes on a clean machine with `wasm32-unknown-unknown`

---

## 17. Next Evolution (Out of Scope)

- Multiple record outputs
- Streaming results
- Per‑plugin persistent state
- Dynamic wire format negotiation

---

**This document defines the authoritative v1 WASM host contract.**

