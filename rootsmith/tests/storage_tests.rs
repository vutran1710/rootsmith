use std::collections::HashMap;

use anyhow::Result;
use tempfile::TempDir;

use rootsmith::storage::{
    commitment_id_from_root, generate_batch_id, BatchMetadata, BatchStatus, Filter, Storable,
    StorageManager, StoredCommitment,
};
use rootsmith::types::{Key16, Namespace, Record, UpstreamData};

// =============================================================================
// Test Helpers
// =============================================================================

fn create_test_storage() -> (StorageManager, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage =
        StorageManager::open(temp_dir.path().to_str().unwrap()).expect("Failed to open storage");
    (storage, temp_dir)
}

fn make_record(ns: u8, key: u8, value: &str, timestamp: u64) -> Record {
    let mut namespace = [0u8; 16];
    namespace[0] = ns;
    let mut key_bytes = [0u8; 16];
    key_bytes[0] = key;

    Record {
        namespace,
        key: key_bytes,
        value: UpstreamData::Text(value.to_string()),
        timestamp,
        metadata: None,
    }
}

fn make_namespace(ns: u8) -> Namespace {
    let mut namespace = [0u8; 16];
    namespace[0] = ns;
    namespace
}

fn make_key(key: u8) -> Key16 {
    let mut key_bytes = [0u8; 16];
    key_bytes[0] = key;
    key_bytes
}

fn make_batch(ns: u8, time_start: u64, time_end: u64, created_at: u64) -> BatchMetadata {
    let namespaces = vec![make_namespace(ns)];
    let batch_id = generate_batch_id(&namespaces, time_start, time_end);
    BatchMetadata {
        batch_id,
        namespaces,
        time_start,
        time_end,
        status: BatchStatus::Pending,
        record_count: 0,
        created_at,
        updated_at: created_at,
        commitment_id: None,
    }
}

fn make_commitment(id: u8, ns: u8, batch_id: [u8; 16], record_count: u64, committed_at: u64) -> StoredCommitment {
    let mut root = vec![0xAB; 32];
    root[0] = id; // Make root unique
    StoredCommitment {
        root,
        namespaces: vec![make_namespace(ns)],
        batch_id,
        time_start: 1000,
        time_end: 2000,
        record_count,
        committed_at,
        proofs: HashMap::new(),
    }
}

// =============================================================================
// Record Tests
// =============================================================================

#[test]
fn test_record_put_and_get() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let record = make_record(1, 1, "hello", 1000);
    storage.put(Storable::Record(record.clone()))?;

    let results = storage.get(Filter::record(record.namespace, record.key, 1000))?;
    assert_eq!(results.len(), 1);
    match &results[0] {
        Storable::Record(r) => {
            assert_eq!(r.namespace, record.namespace);
            assert_eq!(r.key, record.key);
            assert_eq!(r.timestamp, 1000);
        }
        _ => panic!("Expected Record"),
    }

    Ok(())
}

#[test]
fn test_record_get_by_key() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Record(make_record(1, 1, "v1", 1000)))?;
    storage.put(Storable::Record(make_record(1, 1, "v2", 2000)))?;
    storage.put(Storable::Record(make_record(1, 1, "v3", 1500)))?;

    let results = storage.get(Filter::record_by_key(make_namespace(1), make_key(1)))?;
    assert_eq!(results.len(), 3);

    // Caller picks latest by max timestamp
    let latest = results
        .iter()
        .filter_map(|s| match s {
            Storable::Record(r) => Some(r),
            _ => None,
        })
        .max_by_key(|r| r.timestamp)
        .unwrap();
    assert_eq!(latest.timestamp, 2000);

    Ok(())
}

#[test]
fn test_record_get_all_versions() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Record(make_record(1, 1, "v1", 1000)))?;
    storage.put(Storable::Record(make_record(1, 1, "v2", 2000)))?;
    storage.put(Storable::Record(make_record(1, 1, "v3", 3000)))?;

    let results = storage.get(Filter::record_by_key(make_namespace(1), make_key(1)))?;
    assert_eq!(results.len(), 3);

    Ok(())
}

#[test]
fn test_record_get_nonexistent() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let results = storage.get(Filter::record(make_namespace(99), make_key(99), 1000))?;
    assert!(results.is_empty());

    Ok(())
}

#[test]
fn test_records_put_multiple() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Record(make_record(1, 1, "r1", 1000)))?;
    storage.put(Storable::Record(make_record(1, 2, "r2", 1000)))?;
    storage.put(Storable::Record(make_record(2, 1, "r3", 1000)))?;

    let results = storage.get(Filter::records_by_namespace(make_namespace(1)))?;
    assert_eq!(results.len(), 2);

    Ok(())
}

#[test]
fn test_records_query_by_namespace() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Record(make_record(1, 1, "ns1-k1", 1000)))?;
    storage.put(Storable::Record(make_record(1, 2, "ns1-k2", 1000)))?;
    storage.put(Storable::Record(make_record(2, 1, "ns2-k1", 1000)))?;

    let results = storage.get(Filter::records_by_namespace(make_namespace(1)))?;
    assert_eq!(results.len(), 2);

    let results = storage.get(Filter::records_by_namespace(make_namespace(2)))?;
    assert_eq!(results.len(), 1);

    Ok(())
}

#[test]
fn test_records_query_by_filter() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Record(make_record(1, 1, "early", 1000)))?;
    storage.put(Storable::Record(make_record(1, 2, "middle", 2000)))?;
    storage.put(Storable::Record(make_record(1, 3, "late", 3000)))?;

    let results = storage.get(Filter::records_by_time_range(make_namespace(1), 1500, 2500))?;
    assert_eq!(results.len(), 1);
    match &results[0] {
        Storable::Record(r) => {
            assert_eq!(r.timestamp, 2000);
        }
        _ => panic!("Expected Record"),
    }

    Ok(())
}

#[test]
fn test_records_delete() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Record(make_record(1, 1, "r1", 1000)))?;
    storage.put(Storable::Record(make_record(1, 2, "r2", 1000)))?;
    storage.put(Storable::Record(make_record(2, 1, "r3", 1000)))?;

    let deleted = storage.delete(Filter::records_by_namespace(make_namespace(1)))?;
    assert!(deleted);

    let results = storage.get(Filter::records_by_namespace(make_namespace(1)))?;
    assert!(results.is_empty());

    let results = storage.get(Filter::records_by_namespace(make_namespace(2)))?;
    assert_eq!(results.len(), 1);

    Ok(())
}

// =============================================================================
// Batch Tests
// =============================================================================

#[test]
fn test_batch_put_and_get() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let batch = make_batch(1, 1000, 2000, 500);
    let batch_id = batch.batch_id;

    storage.put(Storable::Batch(batch))?;

    let results = storage.get(Filter::batch(batch_id))?;
    assert_eq!(results.len(), 1);
    match &results[0] {
        Storable::Batch(b) => {
            assert_eq!(b.batch_id, batch_id);
            assert_eq!(b.time_start, 1000);
            assert_eq!(b.time_end, 2000);
            assert_eq!(b.status, BatchStatus::Pending);
        }
        _ => panic!("Expected Batch"),
    }

    Ok(())
}

#[test]
fn test_batch_get_nonexistent() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let results = storage.get(Filter::batch([0xFFu8; 16]))?;
    assert!(results.is_empty());

    Ok(())
}

#[test]
fn test_batch_list_all() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Batch(make_batch(1, 1000, 2000, 500)))?;
    storage.put(Storable::Batch(make_batch(2, 2000, 3000, 500)))?;
    storage.put(Storable::Batch(make_batch(3, 3000, 4000, 500)))?;

    let results = storage.get(Filter::batch_all())?;
    assert_eq!(results.len(), 3);

    Ok(())
}

#[test]
fn test_batch_delete() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let batch = make_batch(1, 1000, 2000, 500);
    let batch_id = batch.batch_id;

    storage.put(Storable::Batch(batch))?;

    let deleted = storage.delete(Filter::batch(batch_id))?;
    assert!(deleted);

    let results = storage.get(Filter::batch(batch_id))?;
    assert!(results.is_empty());

    Ok(())
}

// =============================================================================
// Commitment Tests
// =============================================================================

#[test]
fn test_commitment_put_and_get() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let commitment = make_commitment(1, 1, [0x01u8; 16], 100, 3000);

    storage.put(Storable::Commitment(commitment.clone()))?;
    let commitment_id = commitment_id_from_root(&commitment.root);

    let results = storage.get(Filter::commitment(commitment_id))?;
    assert_eq!(results.len(), 1);
    match &results[0] {
        Storable::Commitment(c) => {
            assert_eq!(c.record_count, 100);
            assert_eq!(c.committed_at, 3000);
        }
        _ => panic!("Expected Commitment"),
    }

    Ok(())
}

#[test]
fn test_commitment_get_nonexistent() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let results = storage.get(Filter::commitment([0xFFu8; 32]))?;
    assert!(results.is_empty());

    Ok(())
}

#[test]
fn test_commitment_list_all() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Commitment(make_commitment(1, 1, [0x01u8; 16], 10, 1000)))?;
    storage.put(Storable::Commitment(make_commitment(2, 2, [0x02u8; 16], 20, 2000)))?;

    let results = storage.get(Filter::commitment_all())?;
    assert_eq!(results.len(), 2);

    Ok(())
}

#[test]
fn test_commitment_by_namespace() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Commitment(make_commitment(1, 1, [0x01u8; 16], 10, 1000)))?;
    storage.put(Storable::Commitment(make_commitment(2, 2, [0x02u8; 16], 20, 2000)))?;

    let results = storage.get(Filter::commitment_by_namespace(make_namespace(1)))?;
    assert_eq!(results.len(), 1);

    Ok(())
}

#[test]
fn test_commitment_by_time_range() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Commitment(make_commitment(1, 1, [0x01u8; 16], 10, 1000)))?;
    storage.put(Storable::Commitment(make_commitment(2, 1, [0x02u8; 16], 20, 2000)))?;
    storage.put(Storable::Commitment(make_commitment(3, 1, [0x03u8; 16], 30, 3000)))?;

    let results = storage.get(Filter::commitment_by_time_range(1500, 2500))?;
    assert_eq!(results.len(), 1);
    match &results[0] {
        Storable::Commitment(c) => {
            assert_eq!(c.committed_at, 2000);
        }
        _ => panic!("Expected Commitment"),
    }

    Ok(())
}

#[test]
fn test_commitment_delete() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let commitment = make_commitment(1, 1, [0x01u8; 16], 100, 3000);
    let commitment_id = commitment_id_from_root(&commitment.root);
    storage.put(Storable::Commitment(commitment))?;

    let deleted = storage.delete(Filter::commitment(commitment_id))?;
    assert!(deleted);

    let results = storage.get(Filter::commitment(commitment_id))?;
    assert!(results.is_empty());

    Ok(())
}

// =============================================================================
// Cross-Storage Tests
// =============================================================================

#[test]
fn test_storage_isolation() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    // Store data in all three storages
    storage.put(Storable::Record(make_record(1, 1, "record", 1000)))?;
    storage.put(Storable::Batch(make_batch(1, 1000, 2000, 500)))?;
    storage.put(Storable::Commitment(make_commitment(1, 1, [0x01u8; 16], 10, 3000)))?;

    // Verify each storage has its own data
    let results = storage.get(Filter::records_by_namespace(make_namespace(1)))?;
    assert_eq!(results.len(), 1);

    let results = storage.get(Filter::batch_all())?;
    assert_eq!(results.len(), 1);

    let results = storage.get(Filter::commitment_all())?;
    assert_eq!(results.len(), 1);

    // Delete records, others should remain
    storage.delete(Filter::records_by_namespace(make_namespace(1)))?;

    let results = storage.get(Filter::batch_all())?;
    assert_eq!(results.len(), 1);

    let results = storage.get(Filter::commitment_all())?;
    assert_eq!(results.len(), 1);

    Ok(())
}

#[test]
fn test_storage_persistence() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path = temp_dir.path().to_str().unwrap();

    // Write data
    {
        let storage = StorageManager::open(path)?;
        storage.put(Storable::Record(make_record(1, 1, "record", 1000)))?;
        storage.put(Storable::Batch(make_batch(1, 1000, 2000, 500)))?;
        storage.put(Storable::Commitment(make_commitment(1, 1, [0x01u8; 16], 10, 3000)))?;
    }

    // Reopen and verify
    {
        let storage = StorageManager::open(path)?;

        let results = storage.get(Filter::records_by_namespace(make_namespace(1)))?;
        assert_eq!(results.len(), 1);

        let results = storage.get(Filter::batch_all())?;
        assert_eq!(results.len(), 1);

        let results = storage.get(Filter::commitment_all())?;
        assert_eq!(results.len(), 1);
    }

    Ok(())
}

