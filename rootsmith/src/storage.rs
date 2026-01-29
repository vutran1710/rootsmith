use std::sync::Arc;

use anyhow::Result;
use rocksdb::Options;
use rocksdb::WriteBatch;
use rocksdb::DB;
use serde::Deserialize;
use serde::Serialize;

use crate::types::Key16;
use crate::types::Namespace;
use crate::types::Record;
use crate::types::UpstreamData;

mod key_layout {
    pub const NAMESPACE_OFFSET: usize = 0;
    pub const NAMESPACE_SIZE: usize = 16;
    pub const KEY_OFFSET: usize = NAMESPACE_OFFSET + NAMESPACE_SIZE;
    pub const KEY_SIZE: usize = 16;
    pub const TIMESTAMP_OFFSET: usize = KEY_OFFSET + KEY_SIZE;
    pub const TIMESTAMP_SIZE: usize = 8;
    pub const TOTAL_SIZE: usize = TIMESTAMP_OFFSET + TIMESTAMP_SIZE;
    pub const NAMESPACE_KEY_PREFIX_SIZE: usize = NAMESPACE_SIZE + KEY_SIZE;
}

use key_layout::*;

type StorageKey = [u8; TOTAL_SIZE];

#[derive(Debug, Clone)]
pub struct StorageQueryFilter {
    pub namespace: Namespace,
    pub time_range: Option<(u64, u64)>,
    pub key: Option<Key16>,
}

#[derive(Serialize, Deserialize)]
struct PackedValue {
    data: Vec<u8>,
    format: u8,
    metadata: Option<Vec<u8>>,
}

struct StoredRecord {
    key: StorageKey,
    value: Vec<u8>,
}

impl From<&Record> for StoredRecord {
    fn from(record: &Record) -> Self {
        let key: StorageKey = {
            let mut storage_key = [0u8; TOTAL_SIZE];
            storage_key[NAMESPACE_OFFSET..KEY_OFFSET].copy_from_slice(&record.namespace);
            storage_key[KEY_OFFSET..TIMESTAMP_OFFSET].copy_from_slice(&record.key);
            storage_key[TIMESTAMP_OFFSET..TOTAL_SIZE]
                .copy_from_slice(&record.timestamp.to_be_bytes());
            storage_key
        };

        let value = {
            let data = record.value.as_bytes();
            let format = match &record.value {
                UpstreamData::Bytes(_) => 0,
                UpstreamData::Text(_) => 1,
                UpstreamData::Json(_) => 2,
            };
            let metadata = record
                .metadata
                .as_ref()
                .map(|m| serde_json::to_vec(m).unwrap_or_default());
            let packed = PackedValue {
                data,
                metadata,
                format,
            };
            postcard::to_allocvec(&packed).unwrap_or_default()
        };

        Self { key, value }
    }
}

impl From<StoredRecord> for Record {
    fn from(stored: StoredRecord) -> Self {
        let (namespace, key, timestamp) = {
            let mut namespace_bytes = Namespace::default();
            namespace_bytes.copy_from_slice(&stored.key[NAMESPACE_OFFSET..KEY_OFFSET]);
            let mut key_bytes = Key16::default();
            key_bytes.copy_from_slice(&stored.key[KEY_OFFSET..TIMESTAMP_OFFSET]);
            let mut ts_bytes = [0u8; TIMESTAMP_SIZE];
            ts_bytes.copy_from_slice(&stored.key[TIMESTAMP_OFFSET..TOTAL_SIZE]);
            let timestamp = u64::from_be_bytes(ts_bytes);
            (namespace_bytes, key_bytes, timestamp)
        };

        let (value, metadata) = {
            let PackedValue {
                data: raw_data,
                format,
                metadata,
            } = postcard::from_bytes(&stored.value).unwrap_or(PackedValue {
                data: vec![],
                metadata: None,
                format: 0,
            });
            let value = match format {
                0 => UpstreamData::Bytes(raw_data),
                1 => {
                    let text = String::from_utf8_lossy(&raw_data).to_string();
                    UpstreamData::Text(text)
                }
                2 => {
                    let json = serde_json::from_slice(&raw_data).unwrap_or(serde_json::Value::Null);
                    UpstreamData::Json(json)
                }
                _ => UpstreamData::Bytes(raw_data),
            };
            let metadata = metadata.and_then(|m| serde_json::from_slice(&m).ok());
            (value, metadata)
        };

        Self {
            namespace,
            key,
            value,
            timestamp,
            metadata,
        }
    }
}

pub struct Storage {
    db: Arc<DB>,
}

impl Storage {
    pub fn open(path: &str) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_max_open_files(-1);
        let db = DB::open(&opts, path)?;
        Ok(Self { db: Arc::new(db) })
    }

    pub fn put(&self, record: &Record) -> Result<()> {
        let stored: StoredRecord = record.into();
        self.db.put(&stored.key, &stored.value)?;
        Ok(())
    }

    pub fn put_batch(&self, records: &[Record]) -> Result<()> {
        let mut batch = WriteBatch::default();
        for record in records {
            let stored: StoredRecord = record.into();
            batch.put(&stored.key, &stored.value);
        }
        self.db.write(batch)?;
        Ok(())
    }

    pub fn get_version(
        &self,
        namespace: &Namespace,
        key: &Key16,
        timestamp: u64,
    ) -> Result<Option<Record>> {
        let mut storage_key = [0u8; TOTAL_SIZE];
        storage_key[NAMESPACE_OFFSET..KEY_OFFSET].copy_from_slice(namespace);
        storage_key[KEY_OFFSET..TIMESTAMP_OFFSET].copy_from_slice(key);
        storage_key[TIMESTAMP_OFFSET..TOTAL_SIZE].copy_from_slice(&timestamp.to_be_bytes());

        if let Some(value) = self.db.get(&storage_key)? {
            let record: Record = StoredRecord {
                key: storage_key,
                value: value.to_vec(),
            }
            .into();
            return Ok(Some(record));
        }
        Ok(None)
    }

    pub fn get_latest(&self, namespace: &Namespace, key: &Key16) -> Result<Option<Record>> {
        let versions = self.get_all_versions(namespace, key)?;
        Ok(versions.into_iter().max_by_key(|r| r.timestamp))
    }

    pub fn get_all_versions(&self, namespace: &Namespace, key: &Key16) -> Result<Vec<Record>> {
        let mut prefix = [0u8; NAMESPACE_KEY_PREFIX_SIZE];
        prefix[NAMESPACE_OFFSET..KEY_OFFSET].copy_from_slice(namespace);
        prefix[KEY_OFFSET..NAMESPACE_KEY_PREFIX_SIZE].copy_from_slice(key);

        let mut results = Vec::new();
        let iter = self.db.prefix_iterator(&prefix);

        for item in iter {
            let (k, value) = item?;
            if !k.starts_with(&prefix) {
                break;
            }
            let mut storage_key = [0u8; TOTAL_SIZE];
            storage_key.copy_from_slice(&k);
            let record: Record = StoredRecord {
                key: storage_key,
                value: value.to_vec(),
            }
            .into();
            results.push(record);
        }

        Ok(results)
    }

    pub fn query_namespace(&self, namespace: &Namespace) -> Result<Vec<Record>> {
        let mut prefix = [0u8; NAMESPACE_SIZE];
        prefix.copy_from_slice(namespace);

        let mut results = Vec::new();
        let iter = self.db.prefix_iterator(&prefix);

        for item in iter {
            let (k, value) = item?;
            if !k.starts_with(&prefix) {
                break;
            }
            let mut storage_key = [0u8; TOTAL_SIZE];
            storage_key.copy_from_slice(&k);
            let record: Record = StoredRecord {
                key: storage_key,
                value: value.to_vec(),
            }
            .into();
            results.push(record);
        }

        Ok(results)
    }

    pub fn query(&self, filter: &StorageQueryFilter) -> Result<Vec<Record>> {
        let prefix = self.build_prefix(filter)?;
        let mut results = Vec::new();

        let iter = self.db.prefix_iterator(&prefix);
        for item in iter {
            let (k, value) = item?;
            if !k.starts_with(&prefix) {
                break;
            }

            let mut storage_key = [0u8; TOTAL_SIZE];
            storage_key.copy_from_slice(&k);
            let record: Record = StoredRecord {
                key: storage_key,
                value: value.to_vec(),
            }
            .into();

            if self.matches_filter(&record, filter) {
                results.push(record);
            }
        }

        Ok(results)
    }

    pub fn delete(&self, filter: &StorageQueryFilter) -> Result<u64> {
        let prefix = self.build_prefix(filter)?;
        let mut count = 0;
        let mut batch = WriteBatch::default();
        let iter = self.db.prefix_iterator(&prefix);

        for item in iter {
            let (k, _value) = item?;
            if !k.starts_with(&prefix) {
                break;
            }

            let mut storage_key = [0u8; TOTAL_SIZE];
            storage_key.copy_from_slice(&k);
            let record: Record = StoredRecord {
                key: storage_key,
                value: _value.to_vec(),
            }
            .into();

            if self.matches_filter(&record, filter) {
                batch.delete(&storage_key);
                count += 1;
            }
        }

        if count > 0 {
            self.db.write(batch)?;
        }

        Ok(count)
    }

    fn build_prefix(&self, filter: &StorageQueryFilter) -> Result<Vec<u8>> {
        if let Some(key) = &filter.key {
            let mut prefix = [0u8; NAMESPACE_KEY_PREFIX_SIZE];
            prefix[NAMESPACE_OFFSET..KEY_OFFSET].copy_from_slice(&filter.namespace);
            prefix[KEY_OFFSET..NAMESPACE_KEY_PREFIX_SIZE].copy_from_slice(key);
            Ok(prefix.to_vec())
        } else {
            let mut prefix = [0u8; NAMESPACE_SIZE];
            prefix.copy_from_slice(&filter.namespace);
            Ok(prefix.to_vec())
        }
    }

    fn matches_filter(&self, record: &Record, filter: &StorageQueryFilter) -> bool {
        if record.namespace != filter.namespace {
            return false;
        }

        if let Some(ref key) = filter.key {
            if record.key != *key {
                return false;
            }
        }

        if let Some((start, end)) = filter.time_range {
            if record.timestamp < start || record.timestamp > end {
                return false;
            }
        }

        true
    }
}
