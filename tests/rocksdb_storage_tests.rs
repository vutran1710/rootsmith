use anyhow::Result;
use rootsmith::storage::{Storage, StorageDeleteFilter, StorageScanFilter};
use rootsmith::types::IncomingRecord;
use std::fs;
use hex;

/// Load test data from sample-data.json
fn load_sample_data() -> Result<Vec<IncomingRecord>> {
    let data_path = "data/sample-data.json";
    let content = fs::read_to_string(data_path)?;
    let records: Vec<IncomingRecord> = serde_json::from_str(&content)?;
    Ok(records)
}

/// Helper to create test namespace
fn test_namespace(id: u8) -> [u8; 32] {
    let mut ns = [0u8; 32];
    ns[0] = id;
    ns
}

/// Helper to create test key
fn test_key(id: u8) -> [u8; 32] {
    let mut key = [0u8; 32];
    key[0] = id;
    key
}

/// Helper to create test value
fn test_value(id: u8) -> [u8; 32] {
    let mut val = [0u8; 32];
    val[0] = id;
    val
}

#[test]
fn test_rocksdb_with_sample_data() -> Result<()> {
    println!("\n=== Test: RocksDB with Sample Data ===\n");

    // Create temporary storage
    let temp_dir = tempfile::tempdir()?;
    let storage_path = temp_dir.path().join("test_sample_data_db");
    let storage = Storage::open(storage_path.to_str().unwrap())?;

    // Read sample-data.json
    let json_path = "tests/data/sample-data.json";
    let json_content = fs::read_to_string(json_path)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", json_path, e))?;

    // Parse JSON
    let data: serde_json::Value = serde_json::from_str(&json_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e))?;

    let array = data.as_array()
        .ok_or_else(|| anyhow::anyhow!("Expected JSON array"))?;

    println!("📦 Loaded {} records from sample-data.json\n", array.len());

    // Create a test namespace
    let namespace = test_namespace(1);
    
    // Use current timestamp
    use std::time::{SystemTime, UNIX_EPOCH};
    let base_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time before UNIX_EPOCH")
        .as_secs();

    // Convert JSON elements to IncomingRecord and put into storage
    let mut keys = Vec::new();
    for (index, element) in array.iter().enumerate() {
        // Extract metadataHash as key (remove 0x prefix if present)
        //TODO: print the element here
        println!("Element: {:?}", element);
        let metadata_hash_str = element["metadataHash"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Expected metadataHash string at index {}", index))?;
        
        let hash_str = metadata_hash_str.strip_prefix("0x").unwrap_or(metadata_hash_str);
        let hash_bytes = hex::decode(hash_str)
            .map_err(|e| anyhow::anyhow!("Failed to decode hash at index {}: {}", index, e))?;
        
        if hash_bytes.len() != 32 {
            return Err(anyhow::anyhow!("Hash at index {} is not 32 bytes", index));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&hash_bytes);

        // Use the hash as both key and value (or hash the entire JSON element)
        let value = key; // Using the same hash as value, or you could hash the entire element

        // Create IncomingRecord
        let record = IncomingRecord {
            namespace,
            key,
            value,
            timestamp: base_timestamp + index as u64,
        };

        // Put into storage
        storage.put(&record)?;
        keys.push(key);

        println!("✓ Inserted record {}: key={}", index + 1, metadata_hash_str);
    }

    println!("\n📋 Keys inserted (first 8 bytes of each):");
    for (i, key) in keys.iter().enumerate() {
        println!("  {}: {:?}", i + 1, &key[..8]);
    }

    println!("\n✅ Successfully inserted {} records into RocksDB", keys.len());

    Ok(())
}