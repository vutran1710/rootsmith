//! WASM plugin host for rootsmith.
//!
//! This crate provides functionality to load and execute WASM plugins
//! with sandboxing and resource limits.

pub mod builder;
pub mod error;
pub mod functions;
pub mod host;
pub mod limits;

pub use builder::WasmBuilder;
pub use error::WasmHostError;
pub use functions::read_memory_safe;
pub use functions::set_plugin_memory;
pub use host::WasmPluginHost;
pub use limits::WasmLimits;

// Re-export types from types crate
pub use types::{Key16, Namespace, Record, UpstreamData};
