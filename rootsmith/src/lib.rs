// Library exports for testing and external use

pub mod accumulator;
pub mod archiver;
pub mod config;
pub mod downstream;
pub mod rootsmith;
pub mod storage;
pub mod telemetry;
pub mod types;
pub mod upstream;
pub mod utils;

// Re-export rootsmith-wasm-host as wasm_host
pub use rootsmith_wasm_host as wasm_host;

// Re-export commonly used types and traits
pub use accumulator::AccumulatorVariant;
pub use archiver::ArchiveVariant;
pub use config::Config;
pub use downstream::DownstreamVariant;
pub use rootsmith::CommittedRecord;
pub use rootsmith::EpochPhase;
pub use rootsmith::RootSmith;
pub use storage::Storage;
pub use types::Commitment;
pub use utils::HttpClient;
pub use utils::HttpClientError;
pub use utils::MultipartValue;

// Re-export wasm_host types for convenience
pub use wasm_host::Key16;
pub use wasm_host::Namespace;
pub use wasm_host::Record;
pub use wasm_host::UpstreamData;
pub use wasm_host::WasmBuilder;
pub use wasm_host::WasmHostError;
pub use wasm_host::WasmLimits;
pub use wasm_host::WasmPluginHost;
