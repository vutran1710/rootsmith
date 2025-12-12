use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type Namespace = [u8; 32];
pub type Key32 = [u8; 32];
pub type Value32 = [u8; 32];

/// Incoming data unit from upstream connectors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingRecord {
    pub namespace: Namespace,
    pub key: Key32,
    pub value: Value32,
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

/// Metadata about a batch commitment, suitable for registries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCommitmentMeta {
    pub commitment: Commitment,
    pub leaf_count: u64,
}

/// Unified output for a batch: meta + proofs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOutput {
    pub meta: BatchCommitmentMeta,
    pub proofs: HashMap<Key32, Vec<u8>>,
}

/// Filter options used when querying previous commitments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentFilterOptions {
    pub namespace: Namespace,
    pub time: u64,
}

/// Proof object stored in the proof registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredProof {
    pub root: Vec<u8>,
    pub proof: Vec<u8>,
    pub key: Key32,
    pub meta: serde_json::Value,
}
