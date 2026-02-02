//! Storage module for RocksDB-backed persistence.
//!
//! This module provides storage implementations for different entity types:
//! - `RecordStorage` - for upstream records

mod record;

pub use record::RecordStorage;
pub use record::StorageQueryFilter;

// Backward compatibility alias
pub use RecordStorage as Storage;
