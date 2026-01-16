use ::rootsmith::storage::Storage;
use ::rootsmith::types::IncomingRecord;
use ::rootsmith::types::Key32;
use ::rootsmith::types::Namespace;
use ::rootsmith::types::Value32;
use anyhow::Result;

// ===== Test Helper Functions =====

fn test_namespace(id: u8) -> Namespace {
    let mut ns = [0u8; 32];
    ns[0] = id;
    ns
}

fn test_key(id: u8) -> Key32 {
    let mut key = [0u8; 32];
    key[0] = id;
    key
}

fn test_value(id: u8) -> Value32 {
    let mut value = [0u8; 32];
    value[0] = id;
    value
}

fn test_now() -> u64 {
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time before UNIX_EPOCH")
        .as_secs()
}

// ===== Unit Tests =====

#[test]
fn test_put_many_empty() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let storage = Storage::open(temp_dir.path().to_str().unwrap())?;

    // Test with empty slice
    let records: Vec<IncomingRecord> = vec![];
    storage.put_many(&records)?;

    Ok(())
}

#[test]
fn test_put_many_single_record() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let storage = Storage::open(temp_dir.path().to_str().unwrap())?;

    let namespace = test_namespace(1);
    let now = test_now();

    let records = vec![IncomingRecord {
        namespace,
        key: test_key(1),
        value: test_value(1),
        timestamp: now,
    }];

    storage.put_many(&records)?;

    // Verify the record was inserted
    let result = storage.get(&namespace, &test_key(1), None)?;
    assert!(result.is_some());
    let record = result.unwrap();
    assert_eq!(record.namespace, namespace);
    assert_eq!(record.key, test_key(1));
    assert_eq!(record.value, test_value(1));
    assert_eq!(record.timestamp, now);

    Ok(())
}

#[test]
fn test_put_many_multiple_records() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let storage = Storage::open(temp_dir.path().to_str().unwrap())?;

    let namespace = test_namespace(1);
    let now = test_now();

    let mut records = vec![];
    for i in 0..10 {
        records.push(IncomingRecord {
            namespace,
            key: test_key(i),
            value: test_value(i),
            timestamp: now + i as u64,
        });
    }

    storage.put_many(&records)?;

    // Verify all records were inserted
    for i in 0..10 {
        let result = storage.get(&namespace, &test_key(i), None)?;
        assert!(result.is_some());
        let record = result.unwrap();
        assert_eq!(record.namespace, namespace);
        assert_eq!(record.key, test_key(i));
        assert_eq!(record.value, test_value(i));
        assert_eq!(record.timestamp, now + i as u64);
    }

    Ok(())
}

#[test]
fn test_put_many_different_namespaces() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let storage = Storage::open(temp_dir.path().to_str().unwrap())?;

    let now = test_now();

    let mut records = vec![];
    for i in 0..5 {
        records.push(IncomingRecord {
            namespace: test_namespace(i),
            key: test_key(1),
            value: test_value(i),
            timestamp: now,
        });
    }

    storage.put_many(&records)?;

    // Verify all records were inserted with correct namespaces
    for i in 0..5 {
        let result = storage.get(&test_namespace(i), &test_key(1), None)?;
        assert!(result.is_some());
        let record = result.unwrap();
        assert_eq!(record.namespace, test_namespace(i));
        assert_eq!(record.value, test_value(i));
    }

    Ok(())
}

#[test]
fn test_put_many_same_key_different_timestamps() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let storage = Storage::open(temp_dir.path().to_str().unwrap())?;

    let namespace = test_namespace(1);
    let base_time = test_now();

    let mut records = vec![];
    for i in 0..5 {
        records.push(IncomingRecord {
            namespace,
            key: test_key(1),
            value: test_value(i),
            timestamp: base_time + i as u64 * 100,
        });
    }

    storage.put_many(&records)?;

    // Get should return the latest record (highest timestamp)
    let result = storage.get(&namespace, &test_key(1), None)?;
    assert!(result.is_some());
    let record = result.unwrap();
    assert_eq!(record.value, test_value(4)); // Latest value
    assert_eq!(record.timestamp, base_time + 400);

    Ok(())
}

#[test]
fn test_put_many_vs_put_consistency() -> Result<()> {
    let temp_dir1 = tempfile::tempdir()?;
    let temp_dir2 = tempfile::tempdir()?;
    let storage1 = Storage::open(temp_dir1.path().to_str().unwrap())?;
    let storage2 = Storage::open(temp_dir2.path().to_str().unwrap())?;

    let namespace = test_namespace(1);
    let now = test_now();

    let mut records = vec![];
    for i in 0..5 {
        records.push(IncomingRecord {
            namespace,
            key: test_key(i),
            value: test_value(i),
            timestamp: now + i as u64,
        });
    }

    // Insert using put_many in storage1
    storage1.put_many(&records)?;

    // Insert using put in storage2
    for record in &records {
        storage2.put(record)?;
    }

    // Verify both storages have the same data
    for i in 0..5 {
        let result1 = storage1.get(&namespace, &test_key(i), None)?;
        let result2 = storage2.get(&namespace, &test_key(i), None)?;

        assert!(result1.is_some());
        assert!(result2.is_some());

        let record1 = result1.unwrap();
        let record2 = result2.unwrap();

        assert_eq!(record1.namespace, record2.namespace);
        assert_eq!(record1.key, record2.key);
        assert_eq!(record1.value, record2.value);
        assert_eq!(record1.timestamp, record2.timestamp);
    }

    Ok(())
}
