use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;

// Re-export types from wasm_host for backward compatibility
pub use rootsmith_wasm_host::{Key16, Namespace, Record, UpstreamData};

/// A commitment produced by the system for a batch of leaves
/// belonging to a single namespace and time window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commitment {
    pub namespaces: Vec<Namespace>,
    pub root: Vec<u8>,
    pub committed_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentResult {
    pub commitment: Commitment,
    pub item_count: u64,
    pub timestamp: u64,
    pub proofs: HashMap<Key16, Vec<u8>>,
    pub meta: serde_json::Value,
}
