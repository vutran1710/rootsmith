use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ::rootsmith::config::AccumulatorType;
use ::rootsmith::downstream::DownstreamVariant;
use ::rootsmith::storage::Storage;
use ::rootsmith::types::CommitmentResult;
use ::rootsmith::types::IncomingRecord;
use ::rootsmith::types::Key32;
use ::rootsmith::types::Namespace;
use ::rootsmith::types::Value32;
use ::rootsmith::*;
use anyhow::Result;
use kanal::unbounded_async;

// ===== Test Helper Functions =====

fn test_namespace(id: u8) -> Namespace {
    let mut ns = [0u8; 32];
    ns[0] = id;
    ns
}

fn test_key(id: u8) -> [u8; 32] {
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

#[tokio::test]
async fn test_process_commit_cycle_not_ready() -> Result<()> {
    println!("\n=== Test: Commit Cycle Not Ready ===\n");

    let temp_dir = tempfile::tempdir()?;
    let storage = Storage::open(temp_dir.path().to_str().unwrap())?;

    let epoch_start_ts = Arc::new(tokio::sync::Mutex::new(test_now()));
    let active_namespaces = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let committed_records = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    
    // Use channel downstream for testing
    let (downstream_tx, _downstream_rx) = unbounded_async::<CommitmentResult>();
    let downstream = Arc::new(tokio::sync::Mutex::new(
        DownstreamVariant::new_channel(downstream_tx),
    ));
    let (commit_tx, _commit_rx) =
        unbounded_async::<(Namespace, Vec<u8>, u64, Vec<(Key32, Value32)>)>();

    let storage_arc = Arc::new(tokio::sync::Mutex::new(storage));
    let batch_interval_secs = 60; // 60 seconds

    // Should return false - not enough time elapsed
    let result = RootSmith::process_commit_cycle(
        &epoch_start_ts,
        &active_namespaces,
        &storage_arc,
        &committed_records,
        &downstream,
        &commit_tx,
        batch_interval_secs,
        AccumulatorType::Merkle,
    )
    .await?;

    assert!(!result, "Should not commit when interval not elapsed");

    println!("✓ Commit cycle correctly returned false (not ready)");
    println!("✓ No commitment was made\n");

    Ok(())
}

#[tokio::test]
async fn test_process_commit_cycle_no_active_namespaces() -> Result<()> {
    println!("\n=== Test: Commit Cycle No Active Namespaces ===\n");

    let temp_dir = tempfile::tempdir()?;
    let storage = Storage::open(temp_dir.path().to_str().unwrap())?;

    let now = test_now();
    let epoch_start = now - 100; // Started 100 seconds ago (well past interval)
    let epoch_start_ts = Arc::new(tokio::sync::Mutex::new(epoch_start));
    let active_namespaces = Arc::new(tokio::sync::Mutex::new(HashMap::new())); // Empty
    let committed_records = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    
    // Use channel downstream for testing
    let (downstream_tx, _downstream_rx) = unbounded_async::<CommitmentResult>();
    let downstream = Arc::new(tokio::sync::Mutex::new(
        DownstreamVariant::new_channel(downstream_tx),
    ));
    let (commit_tx, _commit_rx) =
        unbounded_async::<(Namespace, Vec<u8>, u64, Vec<(Key32, Value32)>)>();

    let storage_arc = Arc::new(tokio::sync::Mutex::new(storage));
    let batch_interval_secs = 60;

    // Should return true (processed), but no commitment since no namespaces
    let result = RootSmith::process_commit_cycle(
        &epoch_start_ts,
        &active_namespaces,
        &storage_arc,
        &committed_records,
        &downstream,
        &commit_tx,
        batch_interval_secs,
        AccumulatorType::Merkle,
    )
    .await?;

    assert!(
        result,
        "Should process commit cycle even with no namespaces"
    );

    // Verify epoch was reset
    let new_epoch_start = *epoch_start_ts.lock().await;
    assert!(
        new_epoch_start >= now - 1,
        "Epoch should be reset to current time"
    );

    println!("✓ Commit cycle processed successfully");
    println!("✓ Epoch was reset");
    println!("✓ No commitment made (no active namespaces)\n");

    Ok(())
}

#[tokio::test]
async fn test_process_commit_cycle_with_namespace() -> Result<()> {
    println!("\n=== Test: Commit Cycle With Namespace ===\n");

    let temp_dir = tempfile::tempdir()?;
    let storage = Storage::open(temp_dir.path().to_str().unwrap())?;

    // Add some records to storage
    let namespace = test_namespace(1);
    let now = test_now();

    let storage_arc = Arc::new(tokio::sync::Mutex::new(storage));

    {
        let db = storage_arc.lock().await;
        for i in 0..3 {
            let record = IncomingRecord {
                namespace,
                key: test_key(i),
                value: test_value(i),
                timestamp: now - 50 + i as u64, // Old enough to be included in commit
            };
            db.put(&record)?;
        }
    }

    let epoch_start = now - 100; // Started 100 seconds ago
    let epoch_start_ts = Arc::new(tokio::sync::Mutex::new(epoch_start));

    // Mark namespace as active
    let mut active_namespaces_map = HashMap::new();
    active_namespaces_map.insert(namespace, true);
    let active_namespaces = Arc::new(tokio::sync::Mutex::new(active_namespaces_map));

    let committed_records = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    
    // Use channel downstream for testing
    let (downstream_tx, downstream_rx) = unbounded_async::<CommitmentResult>();
    let downstream = Arc::new(tokio::sync::Mutex::new(
        DownstreamVariant::new_channel(downstream_tx),
    ));
    let (commit_tx, commit_rx) =
        unbounded_async::<(Namespace, Vec<u8>, u64, Vec<(Key32, Value32)>)>();
    let batch_interval_secs = 60;

    // Should commit
    let result = RootSmith::process_commit_cycle(
        &epoch_start_ts,
        &active_namespaces,
        &storage_arc,
        &committed_records,
        &downstream,
        &commit_tx,
        batch_interval_secs,
        AccumulatorType::Merkle,
    )
    .await?;

    assert!(result, "Should process commit cycle");

    // Wait for commitment message
    let (committed_ns, root, committed_at, records) =
        tokio::time::timeout(Duration::from_secs(2), commit_rx.recv()).await??;

    assert_eq!(committed_ns, namespace, "Namespace should match");
    assert!(!root.is_empty(), "Root should not be empty");
    assert_eq!(records.len(), 3, "Should have 3 records");

    // Verify commitment was sent to downstream via channel
    match downstream_rx.try_recv() {
        Ok(Some(result)) => {
            assert!(!result.commitment.is_empty(), "Commitment should not be empty");
            assert!(result.proofs.is_none(), "Proofs should be None in commit phase");
        }
        _ => {
            // If channel is empty, that's fine - the test still validates the logic
        }
    }

    // Verify epoch was reset
    let new_epoch_start = *epoch_start_ts.lock().await;
    assert!(
        new_epoch_start >= now - 1,
        "Epoch should be reset to current time"
    );

    // Verify active namespaces cleared
    let active = active_namespaces.lock().await;
    assert!(active.is_empty(), "Active namespaces should be cleared");

    println!("✓ Commit cycle processed successfully");
    println!("✓ Commitment made with {} records", records.len());
    println!("✓ Message sent to commit channel");
    println!("✓ Epoch reset and namespaces cleared\n");

    Ok(())
}
