use anyhow::Result;
use tempfile::TempDir;

use rootsmith::storage::{Storage, StorageQueryFilter};
use rootsmith::types::{Key16, Namespace, Record, UpstreamData};

fn create_test_storage() -> (Storage, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage = Storage::open(temp_dir.path().to_str().unwrap()).expect("Failed to open storage");
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
// Basic CRUD Operations
// =============================================================================

#[test]
fn test_put_and_get_version() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let record = make_record(1, 1, "hello", 1000);
    storage.put(&record)?;

    let retrieved = storage.get_version(&record.namespace, &record.key, 1000)?;
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
fn test_put_and_get_latest() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let ns = make_namespace(1);
    let key = make_key(1);

    // Insert multiple versions
    storage.put(&make_record(1, 1, "v1", 1000))?;
    storage.put(&make_record(1, 1, "v2", 2000))?;
    storage.put(&make_record(1, 1, "v3", 1500))?; // Out of order

    let latest = storage.get_latest(&ns, &key)?;
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
fn test_get_all_versions() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let ns = make_namespace(1);
    let key = make_key(1);

    storage.put(&make_record(1, 1, "v1", 1000))?;
    storage.put(&make_record(1, 1, "v2", 2000))?;
    storage.put(&make_record(1, 1, "v3", 3000))?;

    let versions = storage.get_all_versions(&ns, &key)?;
    assert_eq!(versions.len(), 3);

    let timestamps: Vec<u64> = versions.iter().map(|r| r.timestamp).collect();
    assert!(timestamps.contains(&1000));
    assert!(timestamps.contains(&2000));
    assert!(timestamps.contains(&3000));

    Ok(())
}

#[test]
fn test_get_nonexistent() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let ns = make_namespace(99);
    let key = make_key(99);

    let result = storage.get_version(&ns, &key, 1000)?;
    assert!(result.is_none());

    let result = storage.get_latest(&ns, &key)?;
    assert!(result.is_none());

    let versions = storage.get_all_versions(&ns, &key)?;
    assert!(versions.is_empty());

    Ok(())
}

// =============================================================================
// Batch Operations
// =============================================================================

#[test]
fn test_put_batch() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let records = vec![
        make_record(1, 1, "r1", 1000),
        make_record(1, 2, "r2", 1000),
        make_record(2, 1, "r3", 1000),
    ];

    storage.put_batch(&records)?;

    // Verify all records were stored
    assert!(storage
        .get_version(&make_namespace(1), &make_key(1), 1000)?
        .is_some());
    assert!(storage
        .get_version(&make_namespace(1), &make_key(2), 1000)?
        .is_some());
    assert!(storage
        .get_version(&make_namespace(2), &make_key(1), 1000)?
        .is_some());

    Ok(())
}

#[test]
fn test_batch_same_key_different_timestamps() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let records = vec![
        make_record(1, 1, "v1", 1000),
        make_record(1, 1, "v2", 2000),
        make_record(1, 1, "v3", 3000),
    ];

    storage.put_batch(&records)?;

    let versions = storage.get_all_versions(&make_namespace(1), &make_key(1))?;
    assert_eq!(versions.len(), 3);

    Ok(())
}

// =============================================================================
// Query Operations
// =============================================================================

#[test]
fn test_query_namespace() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    // Insert records in different namespaces
    storage.put(&make_record(1, 1, "ns1-k1", 1000))?;
    storage.put(&make_record(1, 2, "ns1-k2", 1000))?;
    storage.put(&make_record(2, 1, "ns2-k1", 1000))?;

    let ns1_records = storage.query_namespace(&make_namespace(1))?;
    assert_eq!(ns1_records.len(), 2);

    let ns2_records = storage.query_namespace(&make_namespace(2))?;
    assert_eq!(ns2_records.len(), 1);

    Ok(())
}

#[test]
fn test_empty_namespace_query() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let records = storage.query_namespace(&make_namespace(99))?;
    assert!(records.is_empty());

    Ok(())
}

#[test]
fn test_query_with_time_range() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(&make_record(1, 1, "early", 1000))?;
    storage.put(&make_record(1, 2, "middle", 2000))?;
    storage.put(&make_record(1, 3, "late", 3000))?;

    let filter = StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: Some((1500, 2500)),
        key: None,
    };

    let results = storage.query(&filter)?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].timestamp, 2000);

    Ok(())
}

#[test]
fn test_query_with_key_filter() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(&make_record(1, 1, "k1-v1", 1000))?;
    storage.put(&make_record(1, 1, "k1-v2", 2000))?;
    storage.put(&make_record(1, 2, "k2-v1", 1000))?;

    let filter = StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: None,
        key: Some(make_key(1)),
    };

    let results = storage.query(&filter)?;
    assert_eq!(results.len(), 2);

    for r in &results {
        assert_eq!(r.key, make_key(1));
    }

    Ok(())
}

#[test]
fn test_query_combined_filters() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(&make_record(1, 1, "match", 2000))?;
    storage.put(&make_record(1, 1, "too-early", 1000))?;
    storage.put(&make_record(1, 2, "wrong-key", 2000))?;
    storage.put(&make_record(2, 1, "wrong-ns", 2000))?;

    let filter = StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: Some((1500, 2500)),
        key: Some(make_key(1)),
    };

    let results = storage.query(&filter)?;
    assert_eq!(results.len(), 1);

    if let UpstreamData::Text(text) = &results[0].value {
        assert_eq!(text, "match");
    } else {
        panic!("Expected Text value");
    }

    Ok(())
}

// =============================================================================
// Delete Operations
// =============================================================================

#[test]
fn test_delete_by_namespace() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(&make_record(1, 1, "ns1-r1", 1000))?;
    storage.put(&make_record(1, 2, "ns1-r2", 1000))?;
    storage.put(&make_record(2, 1, "ns2-r1", 1000))?;

    let filter = StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: None,
        key: None,
    };

    let deleted = storage.delete(&filter)?;
    assert_eq!(deleted, 2);

    // Verify ns1 is empty
    let ns1_records = storage.query_namespace(&make_namespace(1))?;
    assert!(ns1_records.is_empty());

    // Verify ns2 still has records
    let ns2_records = storage.query_namespace(&make_namespace(2))?;
    assert_eq!(ns2_records.len(), 1);

    Ok(())
}

#[test]
fn test_delete_by_key() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(&make_record(1, 1, "k1-v1", 1000))?;
    storage.put(&make_record(1, 1, "k1-v2", 2000))?;
    storage.put(&make_record(1, 2, "k2-v1", 1000))?;

    let filter = StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: None,
        key: Some(make_key(1)),
    };

    let deleted = storage.delete(&filter)?;
    assert_eq!(deleted, 2);

    // Key 1 should be gone
    let k1_versions = storage.get_all_versions(&make_namespace(1), &make_key(1))?;
    assert!(k1_versions.is_empty());

    // Key 2 should remain
    let k2_versions = storage.get_all_versions(&make_namespace(1), &make_key(2))?;
    assert_eq!(k2_versions.len(), 1);

    Ok(())
}

#[test]
fn test_delete_with_time_range() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    storage.put(&make_record(1, 1, "early", 1000))?;
    storage.put(&make_record(1, 1, "middle", 2000))?;
    storage.put(&make_record(1, 1, "late", 3000))?;

    let filter = StorageQueryFilter {
        namespace: make_namespace(1),
        time_range: Some((1500, 2500)),
        key: None,
    };

    let deleted = storage.delete(&filter)?;
    assert_eq!(deleted, 1);

    let remaining = storage.get_all_versions(&make_namespace(1), &make_key(1))?;
    assert_eq!(remaining.len(), 2);

    let timestamps: Vec<u64> = remaining.iter().map(|r| r.timestamp).collect();
    assert!(timestamps.contains(&1000));
    assert!(timestamps.contains(&3000));
    assert!(!timestamps.contains(&2000));

    Ok(())
}

#[test]
fn test_delete_nonexistent() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let filter = StorageQueryFilter {
        namespace: make_namespace(99),
        time_range: None,
        key: None,
    };

    let deleted = storage.delete(&filter)?;
    assert_eq!(deleted, 0);

    Ok(())
}

// =============================================================================
// Data Format Tests
// =============================================================================

#[test]
fn test_bytes_format_roundtrip() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let mut record = make_record(1, 1, "", 1000);
    record.value = UpstreamData::Bytes(vec![0x00, 0x01, 0x02, 0xFF]);

    storage.put(&record)?;

    let retrieved = storage.get_version(&record.namespace, &record.key, 1000)?;
    assert!(retrieved.is_some());

    if let UpstreamData::Bytes(data) = &retrieved.unwrap().value {
        assert_eq!(data, &vec![0x00, 0x01, 0x02, 0xFF]);
    } else {
        panic!("Expected Bytes value");
    }

    Ok(())
}

#[test]
fn test_json_format_roundtrip() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let json_value = serde_json::json!({
        "name": "test",
        "count": 42,
        "nested": { "foo": "bar" }
    });

    let mut record = make_record(1, 1, "", 1000);
    record.value = UpstreamData::Json(json_value.clone());

    storage.put(&record)?;

    let retrieved = storage.get_version(&record.namespace, &record.key, 1000)?;
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
fn test_text_format_roundtrip() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let record = make_record(1, 1, "Hello, World! Unicode: \u{1F600}", 1000);
    storage.put(&record)?;

    let retrieved = storage.get_version(&record.namespace, &record.key, 1000)?;
    assert!(retrieved.is_some());

    if let UpstreamData::Text(text) = &retrieved.unwrap().value {
        assert_eq!(text, "Hello, World! Unicode: \u{1F600}");
    } else {
        panic!("Expected Text value");
    }

    Ok(())
}

#[test]
fn test_metadata_roundtrip() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let mut record = make_record(1, 1, "with-metadata", 1000);
    record.metadata = Some(serde_json::json!({
        "source": "test",
        "version": 1
    }));

    storage.put(&record)?;

    let retrieved = storage.get_version(&record.namespace, &record.key, 1000)?;
    assert!(retrieved.is_some());

    let retrieved = retrieved.unwrap();
    assert!(retrieved.metadata.is_some());

    let meta = retrieved.metadata.unwrap();
    assert_eq!(meta["source"], "test");
    assert_eq!(meta["version"], 1);

    Ok(())
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_max_key_values() -> Result<()> {
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

    storage.put(&record)?;

    let retrieved = storage.get_version(&namespace, &key, timestamp)?;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().timestamp, u64::MAX);

    Ok(())
}

#[test]
fn test_zero_key_values() -> Result<()> {
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

    storage.put(&record)?;

    let retrieved = storage.get_version(&namespace, &key, timestamp)?;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().timestamp, 0);

    Ok(())
}

#[test]
fn test_large_value() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    // 1MB of data
    let large_data = vec![0xAB; 1024 * 1024];
    let mut record = make_record(1, 1, "", 1000);
    record.value = UpstreamData::Bytes(large_data.clone());

    storage.put(&record)?;

    let retrieved = storage.get_version(&record.namespace, &record.key, 1000)?;
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
fn test_empty_value() -> Result<()> {
    let (storage, _temp) = create_test_storage();

    let mut record = make_record(1, 1, "", 1000);
    record.value = UpstreamData::Bytes(vec![]);

    storage.put(&record)?;

    let retrieved = storage.get_version(&record.namespace, &record.key, 1000)?;
    assert!(retrieved.is_some());

    if let UpstreamData::Bytes(data) = &retrieved.unwrap().value {
        assert!(data.is_empty());
    } else {
        panic!("Expected Bytes value");
    }

    Ok(())
}

// =============================================================================
// Persistence Tests
// =============================================================================

#[test]
fn test_reopen_persistence() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path = temp_dir.path().to_str().unwrap();

    // Write data
    {
        let storage = Storage::open(path)?;
        storage.put(&make_record(1, 1, "persisted", 1000))?;
    }

    // Reopen and verify
    {
        let storage = Storage::open(path)?;
        let retrieved = storage.get_version(&make_namespace(1), &make_key(1), 1000)?;
        assert!(retrieved.is_some());

        if let UpstreamData::Text(text) = &retrieved.unwrap().value {
            assert_eq!(text, "persisted");
        } else {
            panic!("Expected Text value");
        }
    }

    Ok(())
}
