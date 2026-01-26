use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;

pub type Namespace = [u8; 32];
pub type Key32 = [u8; 32];

/// Data received from upstream connectors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpstreamData {
    Bytes(Vec<u8>),
    Json(serde_json::Value),
    Text(String),
}

/// Data from connectors, parsed into records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub namespace: Namespace,
    pub key: Key32,
    pub value: UpstreamData,
    pub timestamp: u64,
}

/// A commitment produced by the system for a batch of leaves
/// belonging to a single namespace and time window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commitment {
    pub namespace: Namespace,
    pub root: Vec<u8>,
    pub committed_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentResult {
    pub commitment: Commitment,
    pub item_count: u64,
    pub timestamp: u64,
    pub proofs: HashMap<Key32, Vec<u8>>,
    pub meta: serde_json::Value,
}
