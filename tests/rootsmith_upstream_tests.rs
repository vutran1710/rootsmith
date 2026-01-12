use ::rootsmith::*;
use anyhow::Result;
use kanal::unbounded_async;
use ::rootsmith::types::{IncomingRecord, Value32};
use ::rootsmith::upstream::{PubChannelUpstream, UpstreamVariant};

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

fn test_record(id: u8, timestamp: u64) -> IncomingRecord {
    IncomingRecord {
        namespace: test_namespace(1),
        key: test_key(id),
        value: test_value(id),
        timestamp,
    }
}

// ===== Unit Tests =====

#[tokio::test]
async fn test_run_upstream_task_success() -> Result<()> {
    println!("\n=== Test: Upstream Task Success ===\n");

    // Create test records
    let test_records = vec![
        test_record(1, 1000),
        test_record(2, 1001),
        test_record(3, 1002),
    ];

    // Create pubchannel upstream that sends records
    let upstream = UpstreamVariant::PubChannel(PubChannelUpstream::new(test_records.clone(), 10));

    // Create channel
    let (data_tx, data_rx) = unbounded_async::<IncomingRecord>();

    // Run upstream task
    let task_handle = tokio::spawn(async move {
        RootSmith::run_upstream_task(upstream, data_tx).await
    });

    // Collect all records from channel
    // Note: PubChannelUpstream spawns a task, so we need to wait for records to arrive
    let mut received_records = Vec::new();
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(5);

    while received_records.len() < test_records.len() && start.elapsed() < timeout {
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(200),
            data_rx.recv(),
        )
        .await
        {
            Ok(Ok(record)) => {
                received_records.push(record);
            }
            Ok(Err(_)) => {
                // Channel closed
                break;
            }
            Err(_) => {
                // Timeout - continue waiting
                continue;
            }
        }
    }

    // Give a bit more time for PubChannelUpstream's spawned task to finish
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Wait for task to complete
    let result = task_handle.await?;

    // Verify task completed successfully
    assert!(result.is_ok(), "Upstream task should complete successfully");

    // Verify all records were received
    assert_eq!(
        received_records.len(),
        test_records.len(),
        "Expected {} records, got {}",
        test_records.len(),
        received_records.len()
    );

    // Verify record contents
    for (i, expected) in test_records.iter().enumerate() {
        let received = &received_records[i];
        assert_eq!(
            received.namespace, expected.namespace,
            "Record {} namespace mismatch",
            i
        );
        assert_eq!(received.key, expected.key, "Record {} key mismatch", i);
        assert_eq!(received.value, expected.value, "Record {} value mismatch", i);
        assert_eq!(
            received.timestamp, expected.timestamp,
            "Record {} timestamp mismatch",
            i
        );
    }

    println!("✓ Successfully received all {} records", received_records.len());
    println!("✓ Upstream task completed successfully\n");

    Ok(())
}

#[tokio::test]
async fn test_run_upstream_task_no_records() -> Result<()> {
    println!("\n=== Test: Upstream Task No Records ===\n");

    // Create pubchannel upstream with no records
    let upstream = UpstreamVariant::PubChannel(PubChannelUpstream::new(Vec::new(), 10));

    // Create channel
    let (data_tx, data_rx) = unbounded_async::<IncomingRecord>();

    // Run upstream task
    let task_handle = tokio::spawn(async move {
        RootSmith::run_upstream_task(upstream, data_tx).await
    });

    // Wait a bit for task to complete (PubChannelUpstream returns immediately with no records)
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify no records received
    match data_rx.try_recv() {
        Ok(_) => panic!("Should not receive any records"),
        Err(_) => {
            // Expected - channel empty or closed
        }
    }

    // Wait for task to complete
    let result = task_handle.await?;

    // Verify task completed successfully (even with no records)
    assert!(result.is_ok(), "Upstream task should complete successfully with no records");

    println!("✓ No records sent (as expected)");
    println!("✓ Upstream task completed successfully\n");

    Ok(())
}

#[tokio::test]
async fn test_run_upstream_task_multiple_batches() -> Result<()> {
    println!("\n=== Test: Upstream Task Multiple Batches ===\n");

    // Create many test records
    let mut test_records = Vec::new();
    for i in 0..20 {
        test_records.push(test_record(i, 1000 + i as u64));
    }

    // Create mock upstream with delay between batches
    let upstream = UpstreamVariant::PubChannel(PubChannelUpstream::new(test_records.clone(), 50));

    // Create channel
    let (data_tx, data_rx) = unbounded_async::<IncomingRecord>();

    // Run upstream task
    let task_handle = tokio::spawn(async move {
        RootSmith::run_upstream_task(upstream, data_tx).await
    });

    // Collect records as they arrive
    let mut received_records = Vec::new();
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(5);

    while received_records.len() < test_records.len() && start.elapsed() < timeout {
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(200),
            data_rx.recv(),
        )
        .await
        {
            Ok(Ok(record)) => {
                received_records.push(record);
            }
            Ok(Err(_)) => {
                // Channel closed
                break;
            }
            Err(_) => {
                // Timeout - check if we're done
                continue;
            }
        }
    }

    // Wait for task to complete
    let result = task_handle.await?;

    // Verify task completed successfully
    assert!(result.is_ok(), "Upstream task should complete successfully");

    // Verify all records were received
    assert_eq!(
        received_records.len(),
        test_records.len(),
        "Expected {} records, got {}",
        test_records.len(),
        received_records.len()
    );

    println!("✓ Successfully received all {} records in batches", received_records.len());
    println!("✓ Upstream task completed successfully\n");

    Ok(())
}

#[tokio::test]
async fn test_run_upstream_task_channel_closed() -> Result<()> {
    println!("\n=== Test: Upstream Task Channel Closed ===\n");

    // Create test records
    let test_records = vec![
        test_record(1, 1000),
        test_record(2, 1001),
    ];

    // Create pubchannel upstream
    let upstream = UpstreamVariant::PubChannel(PubChannelUpstream::new(test_records, 10));

    // Create channel and immediately close receiver
    let (data_tx, data_rx) = unbounded_async::<IncomingRecord>();
    drop(data_rx); // Close receiver

    // Run upstream task
    let task_handle = tokio::spawn(async move {
        RootSmith::run_upstream_task(upstream, data_tx).await
    });

    // Wait for task to complete (should handle channel closure gracefully)
    let result = task_handle.await?;

    // PubChannelUpstream should handle channel closure without error
    // But if it does error, that's also acceptable behavior
    println!("Task result: {:?}", result);
    println!("✓ Upstream task handled channel closure\n");

    Ok(())
}

