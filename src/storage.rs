use std::sync::Arc;

use anyhow::Result;
use rocksdb::Options;
use rocksdb::DB;

use crate::types::Key32;
use crate::types::Namespace;
use crate::types::Record;

/// Filter for scan.
#[derive(Debug, Clone)]
pub struct StorageQueryFilter {
    pub namespace: Namespace,
    pub time_range: Option<(u64, u64)>,
    pub key: Option<Key32>,
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

    /// Put a record into storage.
    pub fn put(&self, _record: &Record) -> Result<()> {
        todo!("Implement single record put method");
    }

    /// Scan by namespace and optional timestamp upper bound.
    pub fn query(&self, _filter: &StorageQueryFilter) -> Result<Vec<Record>> {
        todo!("Implement query method");
    }

    pub fn delete(&self, _filter: &StorageQueryFilter) -> Result<u64> {
        todo!("Implement delete method");
    }
}
