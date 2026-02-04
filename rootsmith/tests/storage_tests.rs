use std::collections::HashMap;

use anyhow::Result;
use tempfile::TempDir;

use rootsmith::storage::{generate_batch_id, BatchStatus, StorageManager, StorageQueryFilter};
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

// =============================================================================
// RecordStorage: Basic CRUD Operations
// =============================================================================

#[test]
fn test_record_put_and_get_version() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let record = make_record(1, 1, "hello", 1000);
    storage.records.put(&record)?;

    let retrieved = storage.records.get_version(&record.namespace, &record.key, 1000)?;
    assert!(retrieved.is_some());

    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.namespace, record.namespace);
    assert_eq!(retrieved.key, record.key);
    assert_eq!(retrieved.timestamp, 1000);

    if let UpstreamData::Text(text) = &retrieved.value {
        assert_eq!(text, "hello");
    } else {
        panic!("Expected Text value");
    }

    Ok(())
}

#[test]
fn test_record_put_and_get_latest() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let ns = make_namespace(1);
    let key = make_key(1);

    // Insert multiple versions
    storage.records.put(&make_record(1, 1, "v1", 1000))?;
    storage.records.put(&make_record(1, 1, "v2", 2000))?;
    storage.records.put(&make_record(1, 1, "v3", 1500))?; // Out of order

    let latest = storage.records.get_latest(&ns, &key)?;
    assert!(latest.is_some());

    let latest = latest.unwrap();
    assert_eq!(latest.timestamp, 2000);

    if let UpstreamData::Text(text) = &latest.value {
        assert_eq!(text, "v2");
    } else {
        panic!("Expected Text value");
    }

    Ok(())
}

#[test]
fn test_record_get_all_versions() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let ns = make_namespace(1);
    let key = make_key(1);

    storage.records.put(&make_record(1, 1, "v1", 1000))?;
    storage.records.put(&make_record(1, 1, "v2", 2000))?;
    storage.records.put(&make_record(1, 1, "v3", 3000))?;

    let versions = storage.records.get_all_versions(&ns, &key)?;
    assert_eq!(versions.len(), 3);

    let timestamps: Vec<u64> = versions.iter().map(|r| r.timestamp).collect();
    assert!(timestamps.contains(&1000));
    assert!(timestamps.contains(&2000));
    assert!(timestamps.contains(&3000));

    Ok(())
}

#[test]
fn test_record_get_nonexistent() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let ns = make_namespace(99);
    let key = make_key(99);

    let result = storage.records.get_version(&ns, &key, 1000)?;
    assert!(result.is_none());

    let result = storage.records.get_latest(&ns, &key)?;
    assert!(result.is_none());

    let versions = storage.records.get_all_versions(&ns, &key)?;
    assert!(versions.is_empty());

    Ok(())
}

// =============================================================================
// RecordStorage: Batch Operations
// =============================================================================

#[test]
fn test_record_put_batch() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let records = vec![
        make_record(1, 1, "r1", 1000),
        make_record(1, 2, "r2", 1000),
        make_record(2, 1, "r3", 1000),
    ];

    storage.records.put_batch(&records)?;

    // Verify all records were stored
    assert!(storage
        .records
        .get_version(&make_namespace(1), &make_key(1), 1000)?
        .is_some());
    assert!(storage
        .records
        .get_version(&make_namespace(1), &make_key(2), 1000)?
        .is_some());
    assert!(storage
        .records
        .get_version(&make_namespace(2), &make_key(1), 1000)?
        .is_some());

    Ok(())
}

#[test]
fn test_record_batch_same_key_different_timestamps() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let records = vec![
        make_record(1, 1, "v1", 1000),
        make_record(1, 1, "v2", 2000),
        make_record(1, 1, "v3", 3000),
    ];

    storage.records.put_batch(&records)?;

    let versions = storage.records.get_all_versions(&make_namespace(1), &make_key(1))?;
    assert_eq!(versions.len(), 3);

    Ok(())
}

// =============================================================================
// RecordStorage: Query Operations
// =============================================================================

#[test]
fn test_record_query_namespace() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    // Insert records in different namespaces
    storage.records.put(&make_record(1, 1, "ns1-k1", 1000))?;
    storage.records.put(&make_record(1, 2, "ns1-k2", 1000))?;
    storage.records.put(&make_record(2, 1, "ns2-k1", 1000))?;

    let ns1_records = storage.records.query_namespace(&make_namespace(1))?;
    assert_eq!(ns1_records.len(), 2);

    let ns2_records = storage.records.query_namespace(&make_namespace(2))?;
    assert_eq!(ns2_records.len(), 1);

    Ok(())
}

#[test]
fn test_record_empty_namespace_query() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let records = storage.records.query_namespace(&make_namespace(99))?;
    assert!(records.is_empty());

    Ok(())
}

#[test]
fn test_record_query_with_time_range() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.records.put(&make_record(1, 1, "early", 1000))?;
    storage.records.put(&make_record(1, 2, "middle", 2000))?;
    storage.records.put(&make_record(1, 3, "late", 3000))?;

    let filter = StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: Some((1500, 2500)),
        key: None,
    };

    let results = storage.records.query(&filter)?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].timestamp, 2000);

    Ok(())
}

#[test]
fn test_record_query_with_key_filter() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.records.put(&make_record(1, 1, "k1-v1", 1000))?;
    storage.records.put(&make_record(1, 1, "k1-v2", 2000))?;
    storage.records.put(&make_record(1, 2, "k2-v1", 1000))?;

    let filter = StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: None,
        key: Some(make_key(1)),
    };

    let results = storage.records.query(&filter)?;
    assert_eq!(results.len(), 2);

    for r in &results {
        assert_eq!(r.key, make_key(1));
    }

    Ok(())
}

#[test]
fn test_record_query_combined_filters() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.records.put(&make_record(1, 1, "match", 2000))?;
    storage.records.put(&make_record(1, 1, "too-early", 1000))?;
    storage.records.put(&make_record(1, 2, "wrong-key", 2000))?;
    storage.records.put(&make_record(2, 1, "wrong-ns", 2000))?;

    let filter = StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: Some((1500, 2500)),
        key: Some(make_key(1)),
    };

    let results = storage.records.query(&filter)?;
    assert_eq!(results.len(), 1);

    if let UpstreamData::Text(text) = &results[0].value {
        assert_eq!(text, "match");
    } else {
        panic!("Expected Text value");
    }

    Ok(())
}

// =============================================================================
// RecordStorage: Delete Operations
// =============================================================================

#[test]
fn test_record_delete_by_namespace() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.records.put(&make_record(1, 1, "ns1-r1", 1000))?;
    storage.records.put(&make_record(1, 2, "ns1-r2", 1000))?;
    storage.records.put(&make_record(2, 1, "ns2-r1", 1000))?;

    let filter = StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: None,
        key: None,
    };

    let deleted = storage.records.delete(&filter)?;
    assert_eq!(deleted, 2);

    // Verify ns1 is empty
    let ns1_records = storage.records.query_namespace(&make_namespace(1))?;
    assert!(ns1_records.is_empty());

    // Verify ns2 still has records
    let ns2_records = storage.records.query_namespace(&make_namespace(2))?;
    assert_eq!(ns2_records.len(), 1);

    Ok(())
}

#[test]
fn test_record_delete_by_key() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.records.put(&make_record(1, 1, "k1-v1", 1000))?;
    storage.records.put(&make_record(1, 1, "k1-v2", 2000))?;
    storage.records.put(&make_record(1, 2, "k2-v1", 1000))?;

    let filter = StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: None,
        key: Some(make_key(1)),
    };

    let deleted = storage.records.delete(&filter)?;
    assert_eq!(deleted, 2);

    // Key 1 should be gone
    let k1_versions = storage.records.get_all_versions(&make_namespace(1), &make_key(1))?;
    assert!(k1_versions.is_empty());

    // Key 2 should remain
    let k2_versions = storage.records.get_all_versions(&make_namespace(1), &make_key(2))?;
    assert_eq!(k2_versions.len(), 1);

    Ok(())
}

#[test]
fn test_record_delete_with_time_range() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.records.put(&make_record(1, 1, "early", 1000))?;
    storage.records.put(&make_record(1, 1, "middle", 2000))?;
    storage.records.put(&make_record(1, 1, "late", 3000))?;

    let filter = StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: Some((1500, 2500)),
        key: None,
    };

    let deleted = storage.records.delete(&filter)?;
    assert_eq!(deleted, 1);

    let remaining = storage.records.get_all_versions(&make_namespace(1), &make_key(1))?;
    assert_eq!(remaining.len(), 2);

    let timestamps: Vec<u64> = remaining.iter().map(|r| r.timestamp).collect();
    assert!(timestamps.contains(&1000));
    assert!(timestamps.contains(&3000));
    assert!(!timestamps.contains(&2000));

    Ok(())
}

#[test]
fn test_record_delete_nonexistent() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let filter = StorageQueryFilter {
        namespace: make_namespace(99),
        time_range: None,
        key: None,
    };

    let deleted = storage.records.delete(&filter)?;
    assert_eq!(deleted, 0);

    Ok(())
}

// =============================================================================
// RecordStorage: Data Format Tests
// =============================================================================

#[test]
fn test_record_bytes_format_roundtrip() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let mut record = make_record(1, 1, "", 1000);
    record.value = UpstreamData::Bytes(vec![0x00, 0x01, 0x02, 0xFF]);

    storage.records.put(&record)?;

    let retrieved = storage.records.get_version(&record.namespace, &record.key, 1000)?;
    assert!(retrieved.is_some());

    if let UpstreamData::Bytes(data) = &retrieved.unwrap().value {
        assert_eq!(data, &vec![0x00, 0x01, 0x02, 0xFF]);
    } else {
        panic!("Expected Bytes value");
    }

    Ok(())
}

#[test]
fn test_record_json_format_roundtrip() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let json_value = serde_json::json!({
        "name": "test",
        "count": 42,
        "nested": { "foo": "bar" }
    });

    let mut record = make_record(1, 1, "", 1000);
    record.value = UpstreamData::Json(json_value.clone());

    storage.records.put(&record)?;

    let retrieved = storage.records.get_version(&record.namespace, &record.key, 1000)?;
    assert!(retrieved.is_some());

    if let UpstreamData::Json(data) = &retrieved.unwrap().value {
        assert_eq!(data["name"], "test");
        assert_eq!(data["count"], 42);
        assert_eq!(data["nested"]["foo"], "bar");
    } else {
        panic!("Expected Json value");
    }

    Ok(())
}

#[test]
fn test_record_text_format_roundtrip() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let record = make_record(1, 1, "Hello, World! Unicode: \u{1F600}", 1000);
    storage.records.put(&record)?;

    let retrieved = storage.records.get_version(&record.namespace, &record.key, 1000)?;
    assert!(retrieved.is_some());

    if let UpstreamData::Text(text) = &retrieved.unwrap().value {
        assert_eq!(text, "Hello, World! Unicode: \u{1F600}");
    } else {
        panic!("Expected Text value");
    }

    Ok(())
}

#[test]
fn test_record_metadata_roundtrip() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let mut record = make_record(1, 1, "with-metadata", 1000);
    record.metadata = Some(serde_json::json!({
        "source": "test",
        "version": 1
    }));

    storage.records.put(&record)?;

    let retrieved = storage.records.get_version(&record.namespace, &record.key, 1000)?;
    assert!(retrieved.is_some());

    let retrieved = retrieved.unwrap();
    assert!(retrieved.metadata.is_some());

    let meta = retrieved.metadata.unwrap();
    assert_eq!(meta["source"], "test");
    assert_eq!(meta["version"], 1);

    Ok(())
}

// =============================================================================
// RecordStorage: Edge Cases
// =============================================================================

#[test]
fn test_record_max_key_values() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let namespace = [0xFF; 16];
    let key = [0xFF; 16];
    let timestamp = u64::MAX;

    let record = Record {
        namespace,
        key,
        value: UpstreamData::Text("max values".to_string()),
        timestamp,
        metadata: None,
    };

    storage.records.put(&record)?;

    let retrieved = storage.records.get_version(&namespace, &key, timestamp)?;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().timestamp, u64::MAX);

    Ok(())
}

#[test]
fn test_record_zero_key_values() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let namespace = [0x00; 16];
    let key = [0x00; 16];
    let timestamp = 0u64;

    let record = Record {
        namespace,
        key,
        value: UpstreamData::Text("zero values".to_string()),
        timestamp,
        metadata: None,
    };

    storage.records.put(&record)?;

    let retrieved = storage.records.get_version(&namespace, &key, timestamp)?;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().timestamp, 0);

    Ok(())
}

#[test]
fn test_record_large_value() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    // 1MB of data
    let large_data = vec![0xAB; 1024 * 1024];
    let mut record = make_record(1, 1, "", 1000);
    record.value = UpstreamData::Bytes(large_data.clone());

    storage.records.put(&record)?;

    let retrieved = storage.records.get_version(&record.namespace, &record.key, 1000)?;
    assert!(retrieved.is_some());

    if let UpstreamData::Bytes(data) = &retrieved.unwrap().value {
        assert_eq!(data.len(), 1024 * 1024);
        assert!(data.iter().all(|&b| b == 0xAB));
    } else {
        panic!("Expected Bytes value");
    }

    Ok(())
}

#[test]
fn test_record_empty_value() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let mut record = make_record(1, 1, "", 1000);
    record.value = UpstreamData::Bytes(vec![]);

    storage.records.put(&record)?;

    let retrieved = storage.records.get_version(&record.namespace, &record.key, 1000)?;
    assert!(retrieved.is_some());

    if let UpstreamData::Bytes(data) = &retrieved.unwrap().value {
        assert!(data.is_empty());
    } else {
        panic!("Expected Bytes value");
    }

    Ok(())
}

// =============================================================================
// RecordStorage: Persistence Tests
// =============================================================================

#[test]
fn test_record_reopen_persistence() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path = temp_dir.path().to_str().unwrap();

    // Write data
    {
        let storage = StorageManager::open(path)?;
        storage.records.put(&make_record(1, 1, "persisted", 1000))?;
    }

    // Reopen and verify
    {
        let storage = StorageManager::open(path)?;
        let retrieved = storage.records.get_version(&make_namespace(1), &make_key(1), 1000)?;
        assert!(retrieved.is_some());

        if let UpstreamData::Text(text) = &retrieved.unwrap().value {
            assert_eq!(text, "persisted");
        } else {
            panic!("Expected Text value");
        }
    }

    Ok(())
}

// =============================================================================
// BatchStorage: Basic Operations
// =============================================================================

#[test]
fn test_batch_create_and_get() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let namespaces = vec![make_namespace(1), make_namespace(2)];
    let batch_id = generate_batch_id(&namespaces, 1000, 2000);

    storage.batches.create(batch_id, namespaces.clone(), 1000, 2000, 500)?;

    let batch = storage.batches.get(&batch_id)?;
    assert!(batch.is_some());

    let batch = batch.unwrap();
    assert_eq!(batch.batch_id, batch_id);
    assert_eq!(batch.namespaces.len(), 2);
    assert_eq!(batch.time_start, 1000);
    assert_eq!(batch.time_end, 2000);
    assert_eq!(batch.status, BatchStatus::Pending);
    assert_eq!(batch.record_count, 0);
    assert_eq!(batch.created_at, 500);

    Ok(())
}

#[test]
fn test_batch_get_nonexistent() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let batch_id = [0xFFu8; 16];
    let batch = storage.batches.get(&batch_id)?;
    assert!(batch.is_none());

    Ok(())
}

#[test]
fn test_batch_update_status() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let namespaces = vec![make_namespace(1)];
    let batch_id = generate_batch_id(&namespaces, 1000, 2000);

    storage.batches.create(batch_id, namespaces, 1000, 2000, 500)?;

    // Update to Processing
    let updated = storage.batches.update_status(&batch_id, BatchStatus::Processing, 600)?;
    assert!(updated);

    let batch = storage.batches.get(&batch_id)?.unwrap();
    assert_eq!(batch.status, BatchStatus::Processing);
    assert_eq!(batch.updated_at, 600);

    // Update to Failed
    let updated = storage.batches.update_status(&batch_id, BatchStatus::Failed, 700)?;
    assert!(updated);

    let batch = storage.batches.get(&batch_id)?.unwrap();
    assert_eq!(batch.status, BatchStatus::Failed);
    assert_eq!(batch.updated_at, 700);

    Ok(())
}

#[test]
fn test_batch_update_status_nonexistent() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let batch_id = [0xFFu8; 16];
    let updated = storage.batches.update_status(&batch_id, BatchStatus::Processing, 600)?;
    assert!(!updated);

    Ok(())
}

#[test]
fn test_batch_update_record_count() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let namespaces = vec![make_namespace(1)];
    let batch_id = generate_batch_id(&namespaces, 1000, 2000);

    storage.batches.create(batch_id, namespaces, 1000, 2000, 500)?;

    let updated = storage.batches.update_record_count(&batch_id, 42, 600)?;
    assert!(updated);

    let batch = storage.batches.get(&batch_id)?.unwrap();
    assert_eq!(batch.record_count, 42);
    assert_eq!(batch.updated_at, 600);

    Ok(())
}

#[test]
fn test_batch_mark_committed() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let namespaces = vec![make_namespace(1)];
    let batch_id = generate_batch_id(&namespaces, 1000, 2000);
    let commitment_id = [0xABu8; 32];

    storage.batches.create(batch_id, namespaces, 1000, 2000, 500)?;

    let updated = storage.batches.mark_committed(&batch_id, &commitment_id, 600)?;
    assert!(updated);

    let batch = storage.batches.get(&batch_id)?.unwrap();
    assert_eq!(batch.status, BatchStatus::Committed);
    assert_eq!(batch.commitment_id, Some(commitment_id));
    assert_eq!(batch.updated_at, 600);

    Ok(())
}

#[test]
fn test_batch_query_by_status() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    // Create batches with different statuses
    let ns1 = vec![make_namespace(1)];
    let ns2 = vec![make_namespace(2)];
    let ns3 = vec![make_namespace(3)];

    let batch1 = generate_batch_id(&ns1, 1000, 2000);
    let batch2 = generate_batch_id(&ns2, 2000, 3000);
    let batch3 = generate_batch_id(&ns3, 3000, 4000);

    storage.batches.create(batch1, ns1, 1000, 2000, 500)?;
    storage.batches.create(batch2, ns2, 2000, 3000, 500)?;
    storage.batches.create(batch3, ns3, 3000, 4000, 500)?;

    // Update statuses
    storage.batches.update_status(&batch2, BatchStatus::Processing, 600)?;
    storage.batches.update_status(&batch3, BatchStatus::Processing, 600)?;

    // Query pending
    let pending = storage.batches.query_by_status(BatchStatus::Pending)?;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].batch_id, batch1);

    // Query processing
    let processing = storage.batches.query_by_status(BatchStatus::Processing)?;
    assert_eq!(processing.len(), 2);

    Ok(())
}

#[test]
fn test_batch_list_all() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let ns1 = vec![make_namespace(1)];
    let ns2 = vec![make_namespace(2)];

    let batch1 = generate_batch_id(&ns1, 1000, 2000);
    let batch2 = generate_batch_id(&ns2, 2000, 3000);

    storage.batches.create(batch1, ns1, 1000, 2000, 500)?;
    storage.batches.create(batch2, ns2, 2000, 3000, 600)?;

    let all = storage.batches.list_all()?;
    assert_eq!(all.len(), 2);

    Ok(())
}

#[test]
fn test_batch_delete() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let namespaces = vec![make_namespace(1)];
    let batch_id = generate_batch_id(&namespaces, 1000, 2000);

    storage.batches.create(batch_id, namespaces, 1000, 2000, 500)?;

    let deleted = storage.batches.delete(&batch_id)?;
    assert!(deleted);

    let batch = storage.batches.get(&batch_id)?;
    assert!(batch.is_none());

    // Delete again should return false
    let deleted = storage.batches.delete(&batch_id)?;
    assert!(!deleted);

    Ok(())
}

#[test]
fn test_batch_get_records() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    // Insert some records
    storage.records.put(&make_record(1, 1, "r1", 1500))?;
    storage.records.put(&make_record(1, 2, "r2", 1800))?;
    storage.records.put(&make_record(1, 3, "r3", 2500))?; // Outside time range
    storage.records.put(&make_record(2, 1, "r4", 1600))?;

    // Create batch for namespace 1, time range 1000-2000
    let namespaces = vec![make_namespace(1)];
    let batch_id = generate_batch_id(&namespaces, 1000, 2000);
    storage.batches.create(batch_id, namespaces, 1000, 2000, 500)?;

    // Get records for batch
    let records = storage.batches.get_records(&batch_id, &storage.records)?;
    assert_eq!(records.len(), 2); // r1 and r2, not r3 (outside range) or r4 (different namespace)

    Ok(())
}

#[test]
fn test_batch_get_records_multiple_namespaces() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    // Insert records in multiple namespaces
    storage.records.put(&make_record(1, 1, "ns1-r1", 1500))?;
    storage.records.put(&make_record(2, 1, "ns2-r1", 1500))?;
    storage.records.put(&make_record(3, 1, "ns3-r1", 1500))?; // Not in batch

    // Create batch for namespaces 1 and 2
    let namespaces = vec![make_namespace(1), make_namespace(2)];
    let batch_id = generate_batch_id(&namespaces, 1000, 2000);
    storage.batches.create(batch_id, namespaces, 1000, 2000, 500)?;

    let records = storage.batches.get_records(&batch_id, &storage.records)?;
    assert_eq!(records.len(), 2); // ns1-r1 and ns2-r1

    Ok(())
}

#[test]
fn test_batch_deterministic_id() -> Result<()> {
    let ns1 = vec![make_namespace(1), make_namespace(2)];
    let ns2 = vec![make_namespace(2), make_namespace(1)]; // Same namespaces, different order

    let id1 = generate_batch_id(&ns1, 1000, 2000);
    let id2 = generate_batch_id(&ns2, 1000, 2000);

    // Should be the same because namespaces are sorted
    assert_eq!(id1, id2);

    // Different time range should produce different ID
    let id3 = generate_batch_id(&ns1, 1000, 3000);
    assert_ne!(id1, id3);

    Ok(())
}

// =============================================================================
// CommitmentStorage: Basic Operations
// =============================================================================

#[test]
fn test_commitment_store_and_get() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let root = vec![0xAB; 32];
    let namespaces = vec![make_namespace(1), make_namespace(2)];
    let batch_id = [0x01u8; 16];
    let proofs = HashMap::new();

    let commitment_id = storage.commitments.store(
        root.clone(),
        namespaces.clone(),
        batch_id,
        1000,
        2000,
        100,
        3000,
        proofs,
    )?;

    let commitment = storage.commitments.get(&commitment_id)?;
    assert!(commitment.is_some());

    let commitment = commitment.unwrap();
    assert_eq!(commitment.root, root);
    assert_eq!(commitment.namespaces.len(), 2);
    assert_eq!(commitment.batch_id, batch_id);
    assert_eq!(commitment.time_start, 1000);
    assert_eq!(commitment.time_end, 2000);
    assert_eq!(commitment.record_count, 100);
    assert_eq!(commitment.committed_at, 3000);

    Ok(())
}

#[test]
fn test_commitment_get_nonexistent() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let commitment_id = [0xFFu8; 32];
    let commitment = storage.commitments.get(&commitment_id)?;
    assert!(commitment.is_none());

    Ok(())
}

#[test]
fn test_commitment_exists() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let root = vec![0xAB; 32];
    let namespaces = vec![make_namespace(1)];
    let batch_id = [0x01u8; 16];

    let commitment_id = storage.commitments.store(
        root,
        namespaces,
        batch_id,
        1000,
        2000,
        100,
        3000,
        HashMap::new(),
    )?;

    assert!(storage.commitments.exists(&commitment_id)?);

    let nonexistent = [0xFFu8; 32];
    assert!(!storage.commitments.exists(&nonexistent)?);

    Ok(())
}

#[test]
fn test_commitment_get_by_namespace() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    // Create commitments for different namespaces
    let _id1 = storage.commitments.store(
        vec![0x01; 32],
        vec![make_namespace(1)],
        [0x01u8; 16],
        1000,
        2000,
        10,
        3000,
        HashMap::new(),
    )?;

    let _id2 = storage.commitments.store(
        vec![0x02; 32],
        vec![make_namespace(1), make_namespace(2)],
        [0x02u8; 16],
        2000,
        3000,
        20,
        4000,
        HashMap::new(),
    )?;

    let _id3 = storage.commitments.store(
        vec![0x03; 32],
        vec![make_namespace(2)],
        [0x03u8; 16],
        3000,
        4000,
        30,
        5000,
        HashMap::new(),
    )?;

    // Query by namespace 1
    let ns1_commitments = storage.commitments.get_by_namespace(&make_namespace(1))?;
    assert_eq!(ns1_commitments.len(), 2); // id1 and id2

    // Query by namespace 2
    let ns2_commitments = storage.commitments.get_by_namespace(&make_namespace(2))?;
    assert_eq!(ns2_commitments.len(), 2); // id2 and id3

    // Query by nonexistent namespace
    let ns3_commitments = storage.commitments.get_by_namespace(&make_namespace(3))?;
    assert!(ns3_commitments.is_empty());

    Ok(())
}

#[test]
fn test_commitment_query_by_time_range() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    // Create commitments at different times
    let _id1 = storage.commitments.store(
        vec![0x01; 32],
        vec![make_namespace(1)],
        [0x01u8; 16],
        0,
        100,
        10,
        1000, // committed_at = 1000
        HashMap::new(),
    )?;

    let _id2 = storage.commitments.store(
        vec![0x02; 32],
        vec![make_namespace(1)],
        [0x02u8; 16],
        0,
        100,
        10,
        2000, // committed_at = 2000
        HashMap::new(),
    )?;

    let _id3 = storage.commitments.store(
        vec![0x03; 32],
        vec![make_namespace(1)],
        [0x03u8; 16],
        0,
        100,
        10,
        3000, // committed_at = 3000
        HashMap::new(),
    )?;

    // Query range 1500-2500
    let results = storage.commitments.query_by_time_range(1500, 2500)?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1.committed_at, 2000);

    // Query range 0-5000 (all)
    let results = storage.commitments.query_by_time_range(0, 5000)?;
    assert_eq!(results.len(), 3);

    // Query range 5000-6000 (none)
    let results = storage.commitments.query_by_time_range(5000, 6000)?;
    assert!(results.is_empty());

    Ok(())
}

#[test]
fn test_commitment_with_proofs() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let key1 = make_key(1);
    let key2 = make_key(2);
    let proof1 = vec![0xAA; 64];
    let proof2 = vec![0xBB; 64];

    let mut proofs = HashMap::new();
    proofs.insert(key1, proof1.clone());
    proofs.insert(key2, proof2.clone());

    let commitment_id = storage.commitments.store(
        vec![0x01; 32],
        vec![make_namespace(1)],
        [0x01u8; 16],
        1000,
        2000,
        10,
        3000,
        proofs,
    )?;

    // Get proof for key1
    let retrieved_proof = storage.commitments.get_proof(&commitment_id, &key1)?;
    assert!(retrieved_proof.is_some());
    assert_eq!(retrieved_proof.unwrap(), proof1);

    // Get proof for key2
    let retrieved_proof = storage.commitments.get_proof(&commitment_id, &key2)?;
    assert!(retrieved_proof.is_some());
    assert_eq!(retrieved_proof.unwrap(), proof2);

    // Get proof for nonexistent key
    let retrieved_proof = storage.commitments.get_proof(&commitment_id, &make_key(99))?;
    assert!(retrieved_proof.is_none());

    Ok(())
}

#[test]
fn test_commitment_delete() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let commitment_id = storage.commitments.store(
        vec![0x01; 32],
        vec![make_namespace(1)],
        [0x01u8; 16],
        1000,
        2000,
        10,
        3000,
        HashMap::new(),
    )?;

    assert!(storage.commitments.exists(&commitment_id)?);

    let deleted = storage.commitments.delete(&commitment_id)?;
    assert!(deleted);

    assert!(!storage.commitments.exists(&commitment_id)?);

    // Delete again should return false
    let deleted = storage.commitments.delete(&commitment_id)?;
    assert!(!deleted);

    Ok(())
}

#[test]
fn test_commitment_delete_removes_indices() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let commitment_id = storage.commitments.store(
        vec![0x01; 32],
        vec![make_namespace(1)],
        [0x01u8; 16],
        1000,
        2000,
        10,
        3000,
        HashMap::new(),
    )?;

    // Verify it shows up in queries
    let ns_results = storage.commitments.get_by_namespace(&make_namespace(1))?;
    assert_eq!(ns_results.len(), 1);

    let time_results = storage.commitments.query_by_time_range(2000, 4000)?;
    assert_eq!(time_results.len(), 1);

    // Delete
    storage.commitments.delete(&commitment_id)?;

    // Verify indices are also deleted
    let ns_results = storage.commitments.get_by_namespace(&make_namespace(1))?;
    assert!(ns_results.is_empty());

    let time_results = storage.commitments.query_by_time_range(2000, 4000)?;
    assert!(time_results.is_empty());

    Ok(())
}

#[test]
fn test_commitment_list_all() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let _id1 = storage.commitments.store(
        vec![0x01; 32],
        vec![make_namespace(1)],
        [0x01u8; 16],
        1000,
        2000,
        10,
        3000,
        HashMap::new(),
    )?;

    let _id2 = storage.commitments.store(
        vec![0x02; 32],
        vec![make_namespace(2)],
        [0x02u8; 16],
        2000,
        3000,
        20,
        4000,
        HashMap::new(),
    )?;

    let all = storage.commitments.list_all()?;
    assert_eq!(all.len(), 2);

    Ok(())
}

// =============================================================================
// Cross-Storage Isolation Tests (Prefix Verification)
// =============================================================================

#[test]
fn test_storage_prefix_isolation() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    // Create data in all three storages
    storage.records.put(&make_record(1, 1, "record", 1000))?;

    let batch_id = generate_batch_id(&[make_namespace(1)], 1000, 2000);
    storage.batches.create(batch_id, vec![make_namespace(1)], 1000, 2000, 500)?;

    storage.commitments.store(
        vec![0x01; 32],
        vec![make_namespace(1)],
        [0x01u8; 16],
        1000,
        2000,
        10,
        3000,
        HashMap::new(),
    )?;

    // Verify each storage only sees its own data
    let records = storage.records.query_namespace(&make_namespace(1))?;
    assert_eq!(records.len(), 1);

    let batches = storage.batches.list_all()?;
    assert_eq!(batches.len(), 1);

    let commitments = storage.commitments.list_all()?;
    assert_eq!(commitments.len(), 1);

    // Delete from one storage shouldn't affect others
    storage.records.delete(&StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: None,
        key: None,
    })?;

    // Batches and commitments should still exist
    let batches = storage.batches.list_all()?;
    assert_eq!(batches.len(), 1);

    let commitments = storage.commitments.list_all()?;
    assert_eq!(commitments.len(), 1);

    Ok(())
}

#[test]
fn test_storage_manager_persistence() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path = temp_dir.path().to_str().unwrap();

    // Write data to all storages
    {
        let storage = StorageManager::open(path)?;

        storage.records.put(&make_record(1, 1, "record", 1000))?;

        let batch_id = generate_batch_id(&[make_namespace(1)], 1000, 2000);
        storage.batches.create(batch_id, vec![make_namespace(1)], 1000, 2000, 500)?;

        storage.commitments.store(
            vec![0x01; 32],
            vec![make_namespace(1)],
            [0x01u8; 16],
            1000,
            2000,
            10,
            3000,
            HashMap::new(),
        )?;
    }

    // Reopen and verify all data persisted
    {
        let storage = StorageManager::open(path)?;

        let records = storage.records.query_namespace(&make_namespace(1))?;
        assert_eq!(records.len(), 1);

        let batches = storage.batches.list_all()?;
        assert_eq!(batches.len(), 1);

        let commitments = storage.commitments.list_all()?;
        assert_eq!(commitments.len(), 1);
    }

    Ok(())
}

#[test]
fn test_high_volume_mixed_operations() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    // Insert many records
    for i in 0..100u8 {
        storage.records.put(&make_record(i % 10, i, &format!("record-{}", i), i as u64 * 100))?;
    }

    // Create multiple batches
    for i in 0..10u8 {
        let ns = vec![make_namespace(i)];
        let batch_id = generate_batch_id(&ns, i as u64 * 1000, (i as u64 + 1) * 1000);
        storage.batches.create(batch_id, ns, i as u64 * 1000, (i as u64 + 1) * 1000, 500)?;
    }

    // Create multiple commitments
    for i in 0..10u8 {
        storage.commitments.store(
            vec![i; 32],
            vec![make_namespace(i)],
            [i; 16],
            i as u64 * 1000,
            (i as u64 + 1) * 1000,
            10,
            i as u64 * 100,
            HashMap::new(),
        )?;
    }

    // Verify counts
    let mut total_records = 0;
    for i in 0..10u8 {
        total_records += storage.records.query_namespace(&make_namespace(i))?.len();
    }
    assert_eq!(total_records, 100);

    let batches = storage.batches.list_all()?;
    assert_eq!(batches.len(), 10);

    let commitments = storage.commitments.list_all()?;
    assert_eq!(commitments.len(), 10);

    Ok(())
}
