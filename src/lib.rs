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
pub mod wasm_host;

// Re-export commonly used types and traits
// Re-export variant enums for convenience
pub use accumulator::AccumulatorVariant;
pub use archiver::ArchiveVariant;
pub use config::Config;
pub use downstream::DownstreamVariant;
pub use rootsmith::CommittedRecord;
pub use rootsmith::EpochPhase;
pub use rootsmith::RootSmith;
pub use storage::Storage;
pub use types::Commitment;
pub use types::Key16;
pub use types::Namespace;
pub use types::Record;
pub use utils::HttpClient;
pub use utils::HttpClientError;
pub use utils::MultipartValue;
pub use wasm_host::WasmHostError;
pub use wasm_host::WasmLimits;
pub use wasm_host::WasmPluginHost;
