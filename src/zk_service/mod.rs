pub mod client;
pub mod error;
pub mod types;
pub mod builder;
pub mod zk_accumulator;
pub mod adapter;

// Re-export main types
pub use client::ZkServiceClient;
pub use error::{ZkServiceError, Result};
pub use types::*;
pub use builder::JobRequestBuilder;
pub use zk_accumulator::{ZkAccumulator, ZKTrait};