use std::collections::HashMap;

use anyhow;
use serde::Deserialize;
use serde::Serialize;

pub type Namespace = [u8; 32];
pub type Key32 = [u8; 32];
pub type Value32 = [u8; 32];

/// Raw record as found in the database.
/// This is the minimal record format with key and value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawRecord {
    pub key: Key32,
    pub value: Vec<u8>,
}

/// Fine record with complete metadata and clear format.
/// This is the enriched record format used across components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FineRecord {
    pub namespace: String,
    pub key: Key32,
    pub value: Vec<u8>,
    pub timestamp: u64,
    pub metadata: Option<serde_json::Value>,
}

// Conversion from IncomingRecord to RawRecord
impl From<IncomingRecord> for RawRecord {
    fn from(record: IncomingRecord) -> Self {
        RawRecord {
            key: record.key,
            value: record.value.to_vec(),
        }
    }
}

// Conversion from IncomingRecord to FineRecord
impl From<IncomingRecord> for FineRecord {
    fn from(record: IncomingRecord) -> Self {
        FineRecord {
            namespace: hex::encode(record.namespace),
            key: record.key,
            value: record.value.to_vec(),
            timestamp: record.timestamp,
            metadata: None,
        }
    }
}

// Conversion from FineRecord to RawRecord
impl From<FineRecord> for RawRecord {
    fn from(record: FineRecord) -> Self {
        RawRecord {
            key: record.key,
            value: record.value,
        }
    }
}

// Conversion from FineRecord to IncomingRecord
impl TryFrom<FineRecord> for IncomingRecord {
    type Error = anyhow::Error;

    fn try_from(record: FineRecord) -> Result<Self, Self::Error> {
        // Parse namespace from hex string
        let namespace_bytes = hex::decode(&record.namespace)
            .map_err(|e| anyhow::anyhow!("Invalid namespace hex: {}", e))?;
        
        if namespace_bytes.len() != 32 {
            return Err(anyhow::anyhow!("Namespace must be 32 bytes, got {}", namespace_bytes.len()));
        }
        
        let mut namespace = [0u8; 32];
        namespace.copy_from_slice(&namespace_bytes);
        
        // Ensure value is exactly 32 bytes
        if record.value.len() != 32 {
            return Err(anyhow::anyhow!("Value must be 32 bytes for IncomingRecord, got {}", record.value.len()));
        }
        
        let mut value = [0u8; 32];
        value.copy_from_slice(&record.value);
        
        Ok(IncomingRecord {
            namespace,
            key: record.key,
            value,
            timestamp: record.timestamp,
        })
    }
}

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

/// Merkle proof node representing a sibling hash and its direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofNode {
    /// True if sibling is on the left, false if on the right
    pub is_left: bool,
    /// Sibling hash value (variable length, typically 32 bytes)
    pub sibling: Vec<u8>,
}

/// Merkle proof consisting of a sequence of proof nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proof {
    pub nodes: Vec<ProofNode>,
}

/// Proof object stored in the proof registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredProof {
    pub root: Vec<u8>,
    pub proof: Vec<u8>,
    pub key: Key32,
    pub meta: serde_json::Value,
}

/// Unified result containing commitment data and optional proofs.
/// This is the output from RootSmith that gets sent to downstream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentResult {
    /// Commitment data as raw bytes
    pub commitment: Vec<u8>,
    /// Optional flat map of key to proof bytes
    pub proofs: Option<HashMap<Key32, Vec<u8>>>,
    /// Timestamp when committed
    pub committed_at: u64,
}
