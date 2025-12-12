use anyhow::Result;
use rocksdb::{Options, DB, Direction, IteratorMode};
use std::sync::Arc;

use crate::types::{IncomingRecord, Namespace, Key32, Block};

const NS_LEN: usize = 32;
const TS_LEN: usize = 8;
const KEY_LEN: usize = 32;
const DB_KEY_LEN: usize = NS_LEN + TS_LEN + KEY_LEN;

/// Filter for scan.
#[derive(Debug, Clone)]
pub struct StorageScanFilter {
    pub namespace: Namespace,
    /// If None: all records for namespace.
    /// If Some(t): records with timestamp <= t.
    pub timestamp: Option<u64>,
}

/// Filter for delete.
#[derive(Debug, Clone)]
pub struct StorageDeleteFilter {
    pub namespace: Namespace,
    /// Delete records with timestamp <= this value.
    pub timestamp: u64,
}

/// Concrete RocksDB storage.
pub struct Storage {
    db: Arc<DB>,
}

impl Storage {
    pub fn open(path: &str) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path)?;
        Ok(Self { db: Arc::new(db) })
    }

    /// Encode composite key: namespace || timestamp_be || key
    fn encode_key(namespace: &Namespace, timestamp: u64, key: &Key32) -> [u8; DB_KEY_LEN] {
        let mut buf = [0u8; DB_KEY_LEN];
        buf[0..NS_LEN].copy_from_slice(namespace);
        buf[NS_LEN..NS_LEN + TS_LEN].copy_from_slice(&timestamp.to_be_bytes());
        buf[NS_LEN + TS_LEN..DB_KEY_LEN].copy_from_slice(key);
        buf
    }

    fn decode_key(raw: &[u8]) -> Option<(Namespace, u64, Key32)> {
        if raw.len() != DB_KEY_LEN {
            return None;
        }
        let mut ns = [0u8; NS_LEN];
        ns.copy_from_slice(&raw[0..NS_LEN]);

        let mut ts_bytes = [0u8; TS_LEN];
        ts_bytes.copy_from_slice(&raw[NS_LEN..NS_LEN + TS_LEN]);
        let ts = u64::from_be_bytes(ts_bytes);

        let mut key = [0u8; KEY_LEN];
        key.copy_from_slice(&raw[NS_LEN + TS_LEN..DB_KEY_LEN]);

        Some((ns, ts, key))
    }

    /// Put a record into storage.
    pub fn put(&self, record: &IncomingRecord) -> Result<()> {
        let k = Self::encode_key(&record.namespace, record.timestamp, &record.key);
        self.db.put(k, &record.value)?;
        Ok(())
    }

    /// Get latest record for (namespace, key) with optional timestamp bound.
    ///
    /// If timestamp is None: latest record for that key.
    /// If Some(t): latest record with ts <= t.
    pub fn get(
        &self,
        namespace: &Namespace,
        key: &Key32,
        timestamp: Option<u64>,
    ) -> Result<Option<IncomingRecord>> {
        // Strategy: iterate backwards over time for this namespace+key.
        // Simplest approach: forward scan + track best match.
        let filter = StorageScanFilter {
            namespace: *namespace,
            timestamp,
        };
        let all = self.scan(&filter)?;
        let mut best: Option<IncomingRecord> = None;
        for rec in all.into_iter().filter(|r| r.key == *key) {
            if let Some(cur) = &best {
                if rec.timestamp > cur.timestamp {
                    best = Some(rec);
                }
            } else {
                best = Some(rec);
            }
        }
        Ok(best)
    }

    /// Scan by namespace and optional timestamp upper bound.
    pub fn scan(&self, filter: &StorageScanFilter) -> Result<Vec<IncomingRecord>> {
        let ns = filter.namespace;
        let mut out = Vec::new();

        // Start from the minimal possible key for this namespace.
        let start_key = Self::encode_key(&ns, 0, &[0u8; KEY_LEN]);
        let iter = self
            .db
            .iterator(IteratorMode::From(&start_key, Direction::Forward));

        for item in iter {
            let (raw_key, value) = item?;
            if let Some((ns_dec, ts, key)) = Self::decode_key(&raw_key) {
                if ns_dec != ns {
                    // Namespace changed; stop.
                    break;
                }

                if let Some(limit) = filter.timestamp {
                    if ts > limit {
                        break;
                    }
                }

                out.push(IncomingRecord {
                    namespace: ns_dec,
                    key,
                    value: value.to_vec(),
                    timestamp: ts,
                });
            }
        }

        Ok(out)
    }

    /// Delete records for namespace with timestamp <= filter.timestamp.
    ///
    /// Returns a best-effort count of deleted keys (may be approximate).
    pub fn delete(&self, filter: &StorageDeleteFilter) -> Result<u64> {
        let ns = filter.namespace;

        let start_key = Self::encode_key(&ns, 0, &[0u8; KEY_LEN]);
        let end_key = Self::encode_key(&ns, filter.timestamp, &[0xFFu8; KEY_LEN]);

        // Iterate and delete individual keys
        let iter = self
            .db
            .iterator(IteratorMode::From(&start_key, Direction::Forward));

        let mut count = 0u64;
        let mut keys_to_delete = Vec::new();
        
        for item in iter {
            let (raw_key, _) = item?;
            if raw_key.as_ref() >= end_key.as_ref() {
                break;
            }
            keys_to_delete.push(raw_key.to_vec());
        }

        for key in keys_to_delete {
            self.db.delete(key)?;
            count += 1;
        }

        Ok(count)
    }

    // Legacy methods - kept for backwards compatibility
    pub fn new(path: &str) -> Result<Self> {
        Self::open(path)
    }

    pub fn store_block(&self, block: &Block) -> Result<()> {
        let key = format!("block:{}", block.number);
        let value = serde_json::to_vec(block)?;
        self.db.put(key.as_bytes(), value)?;
        Ok(())
    }

    pub fn get_block(&self, number: u64) -> Result<Option<Block>> {
        let key = format!("block:{}", number);
        match self.db.get(key.as_bytes())? {
            Some(data) => {
                let block: Block = serde_json::from_slice(&data)?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    pub fn scan_blocks(&self) -> Result<Vec<u64>> {
        let mut block_numbers = Vec::new();
        let iter = self.db.iterator(IteratorMode::Start);
        
        for item in iter {
            let (key, _value) = item?;
            let key_str = String::from_utf8_lossy(&key);
            
            if let Some(num_str) = key_str.strip_prefix("block:") {
                if let Ok(num) = num_str.parse::<u64>() {
                    block_numbers.push(num);
                }
            }
        }
        
        block_numbers.sort();
        Ok(block_numbers)
    }
}
