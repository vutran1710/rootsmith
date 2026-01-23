pub mod client;
pub mod error;
pub mod types;
pub mod builder;

// Re-export main types
pub use client::ZkServiceClient;
pub use error::{ZkServiceError, Result};
pub use types::*;
pub use builder::JobRequestBuilder;