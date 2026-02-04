//! Commitment storage for finalized Merkle roots.
//!
//! Commitments represent the finalized state of a batch of records.
//! Each commitment contains the Merkle root and metadata about the
//! time range and namespaces it covers.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use rocksdb::WriteBatch;
use rocksdb::DB;
use serde::Deserialize;
use serde::Serialize;

use crate::types::Key16;
use crate::types::Namespace;

use super::batch::BatchId;
use super::batch::CommitmentId;

const COMMITMENT_PREFIX: u8 = 0x03;
const COMMITMENT_NS_INDEX: u8 = 0x04;
const COMMITMENT_TIME_INDEX: u8 = 0x05;

mod key_layout {
    /// Key: prefix (1) + commitment_id (32) = 33 bytes
    pub const COMMITMENT_KEY_SIZE: usize = 1 + 32;
    /// Key: prefix (1) + namespace (16) + commitment_id (32) = 49 bytes
    pub const NS_INDEX_KEY_SIZE: usize = 1 + 16 + 32;
    /// Key: prefix (1) + timestamp (8) + commitment_id (32) = 41 bytes
    pub const TIME_INDEX_KEY_SIZE: usize = 1 + 8 + 32;
}

use key_layout::*;

type CommitmentKey = [u8; COMMITMENT_KEY_SIZE];
type NsIndexKey = [u8; NS_INDEX_KEY_SIZE];
type TimeIndexKey = [u8; TIME_INDEX_KEY_SIZE];

/// Stored commitment data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCommitment {
    /// Merkle root (typically 32 bytes)
    pub root: Vec<u8>,
    /// Namespaces included in this commitment
    pub namespaces: Vec<Namespace>,
    /// Source batch ID
    pub batch_id: BatchId,
    /// Time range start (from batch)
    pub time_start: u64,
    /// Time range end (from batch)
    pub time_end: u64,
    /// Number of records in commitment
    pub record_count: u64,
    /// When commitment was created
    pub committed_at: u64,
    /// Inclusion proofs for specific keys (optional)
    pub proofs: HashMap<Key16, Vec<u8>>,
}

fn make_commitment_key(commitment_id: &CommitmentId) -> CommitmentKey {
    let mut key = [0u8; COMMITMENT_KEY_SIZE];
    key[0] = COMMITMENT_PREFIX;
    key[1..33].copy_from_slice(commitment_id);
    key
}

fn make_ns_index_key(namespace: &Namespace, commitment_id: &CommitmentId) -> NsIndexKey {
    let mut key = [0u8; NS_INDEX_KEY_SIZE];
    key[0] = COMMITMENT_NS_INDEX;
    key[1..17].copy_from_slice(namespace);
    key[17..49].copy_from_slice(commitment_id);
    key
}

fn make_ns_index_prefix(namespace: &Namespace) -> [u8; 17] {
    let mut prefix = [0u8; 17];
    prefix[0] = COMMITMENT_NS_INDEX;
    prefix[1..17].copy_from_slice(namespace);
    prefix
}

fn make_time_index_key(timestamp: u64, commitment_id: &CommitmentId) -> TimeIndexKey {
    let mut key = [0u8; TIME_INDEX_KEY_SIZE];
    key[0] = COMMITMENT_TIME_INDEX;
    key[1..9].copy_from_slice(&timestamp.to_be_bytes());
    key[9..41].copy_from_slice(commitment_id);
    key
}

fn make_time_index_prefix() -> [u8; 1] {
    [COMMITMENT_TIME_INDEX]
}

/// Generate commitment ID from Merkle root.
pub fn commitment_id_from_root(root: &[u8]) -> CommitmentId {
    let mut id = [0u8; 32];
    let len = root.len().min(32);
    id[..len].copy_from_slice(&root[..len]);
    id
}

/// Storage for commitments and their indices.
pub struct CommitmentStorage {
    db: Arc<DB>,
}

impl CommitmentStorage {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    /// Store a commitment with namespace and time indices.
    ///
    /// Returns the commitment ID (derived from root).
    pub fn store(
        &self,
        root: Vec<u8>,
        namespaces: Vec<Namespace>,
        batch_id: BatchId,
        time_start: u64,
        time_end: u64,
        record_count: u64,
        committed_at: u64,
        proofs: HashMap<Key16, Vec<u8>>,
    ) -> Result<CommitmentId> {
        let commitment_id = commitment_id_from_root(&root);

        let commitment = StoredCommitment {
            root,
            namespaces: namespaces.clone(),
            batch_id,
            time_start,
            time_end,
            record_count,
            committed_at,
            proofs,
        };

        let mut write_batch = WriteBatch::default();

        // Write commitment data
        let key = make_commitment_key(&commitment_id);
        let value = postcard::to_allocvec(&commitment)?;
        write_batch.put(&key, &value);

        // Write namespace indices (dedup namespaces)
        let unique_namespaces: Vec<_> = {
            let mut ns = namespaces;
            ns.sort();
            ns.dedup();
            ns
        };
        for namespace in &unique_namespaces {
            let index_key = make_ns_index_key(namespace, &commitment_id);
            write_batch.put(&index_key, &[]);
        }

        // Write time index (using committed_at for ordering)
        let time_key = make_time_index_key(committed_at, &commitment_id);
        write_batch.put(&time_key, &[]);

        self.db.write(write_batch)?;
        Ok(commitment_id)
    }

    /// Get a commitment by ID.
    pub fn get(&self, commitment_id: &CommitmentId) -> Result<Option<StoredCommitment>> {
        let key = make_commitment_key(commitment_id);
        if let Some(value) = self.db.get(&key)? {
            let commitment: StoredCommitment = postcard::from_bytes(&value)?;
            return Ok(Some(commitment));
        }
        Ok(None)
    }

    /// Check if a commitment exists.
    pub fn exists(&self, commitment_id: &CommitmentId) -> Result<bool> {
        let key = make_commitment_key(commitment_id);
        Ok(self.db.get(&key)?.is_some())
    }

    /// Get all commitments for a namespace.
    pub fn get_by_namespace(
        &self,
        namespace: &Namespace,
    ) -> Result<Vec<(CommitmentId, StoredCommitment)>> {
        let prefix = make_ns_index_prefix(namespace);
        let mut results = Vec::new();

        let iter = self.db.prefix_iterator(&prefix);
        for item in iter {
            let (k, _) = item?;
            if !k.starts_with(&prefix) {
                break;
            }
            if k.len() == NS_INDEX_KEY_SIZE {
                let mut commitment_id = [0u8; 32];
                commitment_id.copy_from_slice(&k[17..49]);
                if let Some(commitment) = self.get(&commitment_id)? {
                    results.push((commitment_id, commitment));
                }
            }
        }

        Ok(results)
    }

    /// Query commitments by committed_at time range.
    pub fn query_by_time_range(
        &self,
        start: u64,
        end: u64,
    ) -> Result<Vec<(CommitmentId, StoredCommitment)>> {
        let prefix = make_time_index_prefix();
        let mut results = Vec::new();

        let iter = self.db.prefix_iterator(&prefix);
        for item in iter {
            let (k, _) = item?;
            if k.is_empty() || k[0] != COMMITMENT_TIME_INDEX {
                break;
            }
            if k.len() == TIME_INDEX_KEY_SIZE {
                // Extract timestamp from key
                let mut ts_bytes = [0u8; 8];
                ts_bytes.copy_from_slice(&k[1..9]);
                let timestamp = u64::from_be_bytes(ts_bytes);

                if timestamp >= start && timestamp <= end {
                    let mut commitment_id = [0u8; 32];
                    commitment_id.copy_from_slice(&k[9..41]);
                    if let Some(commitment) = self.get(&commitment_id)? {
                        results.push((commitment_id, commitment));
                    }
                } else if timestamp > end {
                    // Keys are ordered by timestamp, can stop early
                    break;
                }
            }
        }

        Ok(results)
    }

    /// Get proof for a specific key from a commitment.
    pub fn get_proof(
        &self,
        commitment_id: &CommitmentId,
        key: &Key16,
    ) -> Result<Option<Vec<u8>>> {
        if let Some(commitment) = self.get(commitment_id)? {
            return Ok(commitment.proofs.get(key).cloned());
        }
        Ok(None)
    }

    /// Delete a commitment and its indices.
    pub fn delete(&self, commitment_id: &CommitmentId) -> Result<bool> {
        let key = make_commitment_key(commitment_id);

        if let Some(value) = self.db.get(&key)? {
            let commitment: StoredCommitment = postcard::from_bytes(&value)?;

            let mut write_batch = WriteBatch::default();

            // Delete commitment
            write_batch.delete(&key);

            // Delete namespace indices
            for namespace in &commitment.namespaces {
                let index_key = make_ns_index_key(namespace, commitment_id);
                write_batch.delete(&index_key);
            }

            // Delete time index
            let time_key = make_time_index_key(commitment.committed_at, commitment_id);
            write_batch.delete(&time_key);

            self.db.write(write_batch)?;
            return Ok(true);
        }

        Ok(false)
    }

    /// List all commitments.
    pub fn list_all(&self) -> Result<Vec<(CommitmentId, StoredCommitment)>> {
        let prefix = [COMMITMENT_PREFIX];
        let mut results = Vec::new();

        let iter = self.db.prefix_iterator(&prefix);
        for item in iter {
            let (k, v) = item?;
            if k.is_empty() || k[0] != COMMITMENT_PREFIX {
                break;
            }
            if k.len() == COMMITMENT_KEY_SIZE {
                let mut commitment_id = [0u8; 32];
                commitment_id.copy_from_slice(&k[1..33]);
                let commitment: StoredCommitment = postcard::from_bytes(&v)?;
                results.push((commitment_id, commitment));
            }
        }

        Ok(results)
    }
}
