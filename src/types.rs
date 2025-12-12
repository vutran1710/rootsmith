use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Fixed-size types used across the system.
pub type Namespace = [u8; 32];
pub type Key32 = [u8; 32];

/// Incoming data unit from upstream connectors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingRecord {
    /// Logical data namespace (e.g. customer, dataset, provider).
    pub namespace: Namespace,
    /// Logical key within namespace.
    pub key: Key32,
    /// Arbitrary value, opaque to the accumulator core.
    pub value: Vec<u8>,
    /// UTC unix timestamp in seconds.
    pub timestamp: u64,
}

/// A commitment produced by the system for a batch of leaves
/// belonging to a single namespace and time window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commitment {
    /// Namespace for which this commitment is built.
    pub namespace: Namespace,
    /// Root / accumulator commitment bytes (e.g. Merkle root).
    pub root: Vec<u8>,
    /// UTC unix timestamp of when the commitment was created/finalized.
    pub committed_at: u64,
}

/// Metadata about a batch commitment, suitable for registries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCommitmentMeta {
    /// The underlying commitment.
    pub commitment: Commitment,
    /// Number of leaves included in this batch.
    pub leaf_count: u64,
}

/// Unified output for a batch: meta + proofs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOutput {
    /// Commitment metadata.
    pub meta: BatchCommitmentMeta,
    /// Per-key proof bytes (encoding is accumulator-specific).
    pub proofs: HashMap<Key32, Vec<u8>>,
}

/// Filter options used when querying previous commitments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentFilterOptions {
    pub namespace: Namespace,
    /// Reference time (UTC unix seconds).
    /// Semantics depend on implementation (e.g. latest <= time).
    pub time: u64,
}

/// Proof object stored in the proof registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredProof {
    /// Commitment root this proof is associated with.
    pub root: Vec<u8>,
    /// Serialized proof bytes (format is backend-specific).
    pub proof: Vec<u8>,
    /// Key this proof concerns.
    pub key: Key32,
    /// Optional metadata (namespace, timestamps, etc.).
    pub meta: serde_json::Value,
}
