use rootsmith::storage::Storage;
use rootsmith::types::Block;

#[test]
fn test_storage_scan() {
    let temp_dir = std::env::temp_dir().join(format!("rootsmith_test_{}", uuid::Uuid::new_v4()));
    let storage = Storage::new(temp_dir.to_str().unwrap()).expect("Failed to create storage");

    let block1 = Block {
        number: 1,
        hash: "hash1".to_string(),
        timestamp: 1000,
        data: vec![1, 2, 3],
    };

    let block2 = Block {
        number: 2,
        hash: "hash2".to_string(),
        timestamp: 2000,
        data: vec![4, 5, 6],
    };

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_scan_empty_storage() {
    let temp_dir = std::env::temp_dir().join(format!("rootsmith_test_{}", uuid::Uuid::new_v4()));
    let _storage = Storage::new(temp_dir.to_str().unwrap()).expect("Failed to create storage");
    std::fs::remove_dir_all(&temp_dir).ok();
}
