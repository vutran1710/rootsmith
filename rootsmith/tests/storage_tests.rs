use std::collections::HashMap;

use anyhow::Result;
use tempfile::TempDir;

use rootsmith::storage::{
    generate_batch_id, BatchMetadata, BatchStatus, Deletable, Retrievable, Retrieved, Storable,
    StorageManager, StorageQueryFilter, StoredCommitment, Updatable,
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

    match storage.get(Retrievable::Record {
        namespace: record.namespace,
        key: record.key,
        timestamp: 1000,
    })? {
        Retrieved::Record(Some(r)) => {
            assert_eq!(r.namespace, record.namespace);
            assert_eq!(r.key, record.key);
            assert_eq!(r.timestamp, 1000);
        }
        _ => panic!("Expected Record"),
    }

    Ok(())
}

#[test]
fn test_record_get_latest() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Record(make_record(1, 1, "v1", 1000)))?;
    storage.put(Storable::Record(make_record(1, 1, "v2", 2000)))?;
    storage.put(Storable::Record(make_record(1, 1, "v3", 1500)))?;

    match storage.get(Retrievable::RecordLatest {
        namespace: make_namespace(1),
        key: make_key(1),
    })? {
        Retrieved::Record(Some(r)) => {
            assert_eq!(r.timestamp, 2000);
        }
        _ => panic!("Expected Record"),
    }

    Ok(())
}

#[test]
fn test_record_get_all_versions() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Record(make_record(1, 1, "v1", 1000)))?;
    storage.put(Storable::Record(make_record(1, 1, "v2", 2000)))?;
    storage.put(Storable::Record(make_record(1, 1, "v3", 3000)))?;

    match storage.get(Retrievable::RecordAllVersions {
        namespace: make_namespace(1),
        key: make_key(1),
    })? {
        Retrieved::Records(records) => {
            assert_eq!(records.len(), 3);
        }
        _ => panic!("Expected Records"),
    }

    Ok(())
}

#[test]
fn test_record_get_nonexistent() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    match storage.get(Retrievable::Record {
        namespace: make_namespace(99),
        key: make_key(99),
        timestamp: 1000,
    })? {
        Retrieved::Record(None) => {}
        _ => panic!("Expected None"),
    }

    Ok(())
}

#[test]
fn test_records_put_batch() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let records = vec![
        make_record(1, 1, "r1", 1000),
        make_record(1, 2, "r2", 1000),
        make_record(2, 1, "r3", 1000),
    ];

    storage.put(Storable::Records(records))?;

    match storage.get(Retrievable::RecordsByNamespace(make_namespace(1)))? {
        Retrieved::Records(records) => assert_eq!(records.len(), 2),
        _ => panic!("Expected Records"),
    }

    Ok(())
}

#[test]
fn test_records_query_by_namespace() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Record(make_record(1, 1, "ns1-k1", 1000)))?;
    storage.put(Storable::Record(make_record(1, 2, "ns1-k2", 1000)))?;
    storage.put(Storable::Record(make_record(2, 1, "ns2-k1", 1000)))?;

    match storage.get(Retrievable::RecordsByNamespace(make_namespace(1)))? {
        Retrieved::Records(records) => assert_eq!(records.len(), 2),
        _ => panic!("Expected Records"),
    }

    match storage.get(Retrievable::RecordsByNamespace(make_namespace(2)))? {
        Retrieved::Records(records) => assert_eq!(records.len(), 1),
        _ => panic!("Expected Records"),
    }

    Ok(())
}

#[test]
fn test_records_query_by_filter() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Record(make_record(1, 1, "early", 1000)))?;
    storage.put(Storable::Record(make_record(1, 2, "middle", 2000)))?;
    storage.put(Storable::Record(make_record(1, 3, "late", 3000)))?;

    let filter = StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: Some((1500, 2500)),
        key: None,
    };

    match storage.get(Retrievable::RecordsByFilter(filter))? {
        Retrieved::Records(records) => {
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].timestamp, 2000);
        }
        _ => panic!("Expected Records"),
    }

    Ok(())
}

#[test]
fn test_records_delete() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Record(make_record(1, 1, "r1", 1000)))?;
    storage.put(Storable::Record(make_record(1, 2, "r2", 1000)))?;
    storage.put(Storable::Record(make_record(2, 1, "r3", 1000)))?;

    let filter = StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: None,
        key: None,
    };

    let deleted = storage.delete(Deletable::Records(filter))?;
    assert!(deleted);

    match storage.get(Retrievable::RecordsByNamespace(make_namespace(1)))? {
        Retrieved::Records(records) => assert!(records.is_empty()),
        _ => panic!("Expected Records"),
    }

    match storage.get(Retrievable::RecordsByNamespace(make_namespace(2)))? {
        Retrieved::Records(records) => assert_eq!(records.len(), 1),
        _ => panic!("Expected Records"),
    }

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

    match storage.get(Retrievable::Batch(batch_id))? {
        Retrieved::Batch(Some(b)) => {
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

    match storage.get(Retrievable::Batch([0xFFu8; 16]))? {
        Retrieved::Batch(None) => {}
        _ => panic!("Expected None"),
    }

    Ok(())
}

#[test]
fn test_batch_update_status() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let batch = make_batch(1, 1000, 2000, 500);
    let batch_id = batch.batch_id;

    storage.put(Storable::Batch(batch))?;

    storage.update(Updatable::BatchStatus {
        batch_id,
        status: BatchStatus::Processing,
        timestamp: 600,
    })?;

    match storage.get(Retrievable::Batch(batch_id))? {
        Retrieved::Batch(Some(b)) => {
            assert_eq!(b.status, BatchStatus::Processing);
            assert_eq!(b.updated_at, 600);
        }
        _ => panic!("Expected Batch"),
    }

    Ok(())
}

#[test]
fn test_batch_update_record_count() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let batch = make_batch(1, 1000, 2000, 500);
    let batch_id = batch.batch_id;

    storage.put(Storable::Batch(batch))?;

    storage.update(Updatable::BatchRecordCount {
        batch_id,
        count: 42,
        timestamp: 600,
    })?;

    match storage.get(Retrievable::Batch(batch_id))? {
        Retrieved::Batch(Some(b)) => {
            assert_eq!(b.record_count, 42);
        }
        _ => panic!("Expected Batch"),
    }

    Ok(())
}

#[test]
fn test_batch_mark_committed() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let batch = make_batch(1, 1000, 2000, 500);
    let batch_id = batch.batch_id;
    let commitment_id = [0xABu8; 32];

    storage.put(Storable::Batch(batch))?;

    storage.update(Updatable::BatchCommitted {
        batch_id,
        commitment_id,
        timestamp: 600,
    })?;

    match storage.get(Retrievable::Batch(batch_id))? {
        Retrieved::Batch(Some(b)) => {
            assert_eq!(b.status, BatchStatus::Committed);
            assert_eq!(b.commitment_id, Some(commitment_id));
        }
        _ => panic!("Expected Batch"),
    }

    Ok(())
}

#[test]
fn test_batch_query_by_status() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let batch1 = make_batch(1, 1000, 2000, 500);
    let batch2 = make_batch(2, 2000, 3000, 500);
    let batch1_id = batch1.batch_id;
    let batch2_id = batch2.batch_id;

    storage.put(Storable::Batch(batch1))?;
    storage.put(Storable::Batch(batch2))?;

    storage.update(Updatable::BatchStatus {
        batch_id: batch2_id,
        status: BatchStatus::Processing,
        timestamp: 600,
    })?;

    match storage.get(Retrievable::BatchByStatus(BatchStatus::Pending))? {
        Retrieved::Batches(batches) => {
            assert_eq!(batches.len(), 1);
            assert_eq!(batches[0].batch_id, batch1_id);
        }
        _ => panic!("Expected Batches"),
    }

    match storage.get(Retrievable::BatchByStatus(BatchStatus::Processing))? {
        Retrieved::Batches(batches) => {
            assert_eq!(batches.len(), 1);
            assert_eq!(batches[0].batch_id, batch2_id);
        }
        _ => panic!("Expected Batches"),
    }

    Ok(())
}

#[test]
fn test_batch_list_all() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Batch(make_batch(1, 1000, 2000, 500)))?;
    storage.put(Storable::Batch(make_batch(2, 2000, 3000, 500)))?;
    storage.put(Storable::Batch(make_batch(3, 3000, 4000, 500)))?;

    match storage.get(Retrievable::BatchAll)? {
        Retrieved::Batches(batches) => assert_eq!(batches.len(), 3),
        _ => panic!("Expected Batches"),
    }

    Ok(())
}

#[test]
fn test_batch_get_records() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    // Store records
    storage.put(Storable::Record(make_record(1, 1, "r1", 1500)))?;
    storage.put(Storable::Record(make_record(1, 2, "r2", 1800)))?;
    storage.put(Storable::Record(make_record(1, 3, "r3", 2500)))?; // Outside time range

    // Create batch
    let batch = make_batch(1, 1000, 2000, 500);
    let batch_id = batch.batch_id;
    storage.put(Storable::Batch(batch))?;

    match storage.get(Retrievable::BatchRecords(batch_id))? {
        Retrieved::Records(records) => assert_eq!(records.len(), 2),
        _ => panic!("Expected Records"),
    }

    Ok(())
}

#[test]
fn test_batch_delete() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let batch = make_batch(1, 1000, 2000, 500);
    let batch_id = batch.batch_id;

    storage.put(Storable::Batch(batch))?;

    let deleted = storage.delete(Deletable::Batch(batch_id))?;
    assert!(deleted);

    match storage.get(Retrievable::Batch(batch_id))? {
        Retrieved::Batch(None) => {}
        _ => panic!("Expected None"),
    }

    Ok(())
}

// =============================================================================
// Commitment Tests
// =============================================================================

#[test]
fn test_commitment_put_and_get() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let commitment = make_commitment(1, 1, [0x01u8; 16], 100, 3000);

    let result = storage.put(Storable::Commitment(commitment.clone()))?;
    assert!(result.is_some());
    let commitment_id = result.unwrap();

    match storage.get(Retrievable::Commitment(commitment_id))? {
        Retrieved::Commitment(Some(c)) => {
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

    match storage.get(Retrievable::Commitment([0xFFu8; 32]))? {
        Retrieved::Commitment(None) => {}
        _ => panic!("Expected None"),
    }

    Ok(())
}

#[test]
fn test_commitment_list_all() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Commitment(make_commitment(1, 1, [0x01u8; 16], 10, 1000)))?;
    storage.put(Storable::Commitment(make_commitment(2, 2, [0x02u8; 16], 20, 2000)))?;

    match storage.get(Retrievable::CommitmentAll)? {
        Retrieved::Commitments(commitments) => assert_eq!(commitments.len(), 2),
        _ => panic!("Expected Commitments"),
    }

    Ok(())
}

#[test]
fn test_commitment_by_namespace() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Commitment(make_commitment(1, 1, [0x01u8; 16], 10, 1000)))?;
    storage.put(Storable::Commitment(make_commitment(2, 2, [0x02u8; 16], 20, 2000)))?;

    match storage.get(Retrievable::CommitmentByNamespace(make_namespace(1)))? {
        Retrieved::Commitments(commitments) => assert_eq!(commitments.len(), 1),
        _ => panic!("Expected Commitments"),
    }

    Ok(())
}

#[test]
fn test_commitment_by_time_range() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(Storable::Commitment(make_commitment(1, 1, [0x01u8; 16], 10, 1000)))?;
    storage.put(Storable::Commitment(make_commitment(2, 1, [0x02u8; 16], 20, 2000)))?;
    storage.put(Storable::Commitment(make_commitment(3, 1, [0x03u8; 16], 30, 3000)))?;

    match storage.get(Retrievable::CommitmentByTimeRange {
        start: 1500,
        end: 2500,
    })? {
        Retrieved::Commitments(commitments) => {
            assert_eq!(commitments.len(), 1);
            assert_eq!(commitments[0].1.committed_at, 2000);
        }
        _ => panic!("Expected Commitments"),
    }

    Ok(())
}

#[test]
fn test_commitment_delete() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let commitment = make_commitment(1, 1, [0x01u8; 16], 100, 3000);
    let commitment_id = storage.put(Storable::Commitment(commitment))?.unwrap();

    let deleted = storage.delete(Deletable::Commitment(commitment_id))?;
    assert!(deleted);

    match storage.get(Retrievable::Commitment(commitment_id))? {
        Retrieved::Commitment(None) => {}
        _ => panic!("Expected None"),
    }

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
    match storage.get(Retrievable::RecordsByNamespace(make_namespace(1)))? {
        Retrieved::Records(records) => assert_eq!(records.len(), 1),
        _ => panic!("Expected Records"),
    }

    match storage.get(Retrievable::BatchAll)? {
        Retrieved::Batches(batches) => assert_eq!(batches.len(), 1),
        _ => panic!("Expected Batches"),
    }

    match storage.get(Retrievable::CommitmentAll)? {
        Retrieved::Commitments(commitments) => assert_eq!(commitments.len(), 1),
        _ => panic!("Expected Commitments"),
    }

    // Delete records, others should remain
    storage.delete(Deletable::Records(StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: None,
        key: None,
    }))?;

    match storage.get(Retrievable::BatchAll)? {
        Retrieved::Batches(batches) => assert_eq!(batches.len(), 1),
        _ => panic!("Expected Batches"),
    }

    match storage.get(Retrievable::CommitmentAll)? {
        Retrieved::Commitments(commitments) => assert_eq!(commitments.len(), 1),
        _ => panic!("Expected Commitments"),
    }

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

        match storage.get(Retrievable::RecordsByNamespace(make_namespace(1)))? {
            Retrieved::Records(records) => assert_eq!(records.len(), 1),
            _ => panic!("Expected Records"),
        }

        match storage.get(Retrievable::BatchAll)? {
            Retrieved::Batches(batches) => assert_eq!(batches.len(), 1),
            _ => panic!("Expected Batches"),
        }

        match storage.get(Retrievable::CommitmentAll)? {
            Retrieved::Commitments(commitments) => assert_eq!(commitments.len(), 1),
            _ => panic!("Expected Commitments"),
        }
    }

    Ok(())
}

#[test]
fn test_full_workflow() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    // 1. Store records
    storage.put(Storable::Records(vec![
        make_record(1, 1, "data1", 1500),
        make_record(1, 2, "data2", 1600),
        make_record(1, 3, "data3", 1700),
    ]))?;

    // 2. Create batch
    let batch = make_batch(1, 1000, 2000, 500);
    let batch_id = batch.batch_id;
    storage.put(Storable::Batch(batch))?;

    // 3. Get batch records
    let record_count = match storage.get(Retrievable::BatchRecords(batch_id))? {
        Retrieved::Records(records) => records.len() as u64,
        _ => 0,
    };
    assert_eq!(record_count, 3);

    // 4. Update batch status
    storage.update(Updatable::BatchStatus {
        batch_id,
        status: BatchStatus::Processing,
        timestamp: 600,
    })?;

    // 5. Update record count
    storage.update(Updatable::BatchRecordCount {
        batch_id,
        count: record_count,
        timestamp: 700,
    })?;

    // 6. Store commitment
    let commitment = StoredCommitment {
        root: vec![0xDE, 0xAD, 0xBE, 0xEF],
        namespaces: vec![make_namespace(1)],
        batch_id,
        time_start: 1000,
        time_end: 2000,
        record_count,
        committed_at: 800,
        proofs: HashMap::new(),
    };
    let commitment_id = storage.put(Storable::Commitment(commitment))?.unwrap();

    // 7. Mark batch as committed
    storage.update(Updatable::BatchCommitted {
        batch_id,
        commitment_id,
        timestamp: 800,
    })?;

    // 8. Verify final state
    match storage.get(Retrievable::Batch(batch_id))? {
        Retrieved::Batch(Some(b)) => {
            assert_eq!(b.status, BatchStatus::Committed);
            assert_eq!(b.record_count, 3);
            assert_eq!(b.commitment_id, Some(commitment_id));
        }
        _ => panic!("Expected Batch"),
    }

    match storage.get(Retrievable::Commitment(commitment_id))? {
        Retrieved::Commitment(Some(c)) => {
            assert_eq!(c.record_count, 3);
            assert_eq!(c.batch_id, batch_id);
        }
        _ => panic!("Expected Commitment"),
    }

    Ok(())
}
