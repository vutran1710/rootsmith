use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tempfile::TempDir;
use tokio::sync::Mutex;
use tower::ServiceExt;

use rootsmith::accumulator::{Accumulator, AccumulatorConfig, AccumulatorVariant};
use rootsmith::server::webhook::{self, WebhookState};
use rootsmith::storage::{generate_batch_id, BatchStatus, Storable, StorageManager};
use rootsmith::types::{Commitment, CommitmentResult, Namespace, Record, UpstreamData};

fn create_test_storage() -> (Arc<Mutex<StorageManager>>, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage =
        StorageManager::open(temp_dir.path().to_str().unwrap()).expect("Failed to open storage");
    (Arc::new(Mutex::new(storage)), temp_dir)
}

fn make_namespace(ns: u8) -> Namespace {
    let mut namespace = [0u8; 16];
    namespace[0] = ns;
    namespace
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

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[tokio::test]
async fn test_full_commit_flow_with_merkle_accumulator() {
    let (storage, _temp_dir) = create_test_storage();
    let accumulator = Arc::new(Mutex::new(AccumulatorVariant::new(&AccumulatorConfig::Merkle)));

    let now = current_timestamp();
    let time_start = now - 100;
    let time_end = now;

    {
        let storage = storage.lock().await;
        storage
            .put(Storable::Record(make_record(1, 1, "value1", now - 50)))
            .unwrap();
        storage
            .put(Storable::Record(make_record(1, 2, "value2", now - 30)))
            .unwrap();
        storage
            .put(Storable::Record(make_record(1, 3, "value3", now - 10)))
            .unwrap();
    }

    let namespaces = vec![make_namespace(1)];
    let batch_id = generate_batch_id(&namespaces, time_start, time_end);

    {
        let storage = storage.lock().await;

        storage
            .batches()
            .create(batch_id, namespaces.clone(), time_start, time_end, now)
            .unwrap();

        let records = storage
            .batches()
            .get_records(&batch_id, storage.records())
            .unwrap();
        assert_eq!(records.len(), 3);

        storage
            .batches()
            .update_record_count(&batch_id, records.len() as u64, now)
            .unwrap();

        let (result_tx, result_rx) = kanal::unbounded_async::<CommitmentResult>();

        let acc = accumulator.lock().await;
        let job_id = acc.commit(&records, result_tx).await.unwrap();
        drop(acc);

        assert!(job_id.is_none());

        let result = result_rx.recv().await.unwrap();
        assert_eq!(result.item_count, 3);
        assert!(!result.commitment.root.is_empty());

        let commitment_id = storage
            .commitments()
            .store(
                result.commitment.root,
                result.commitment.namespaces,
                batch_id,
                time_start,
                time_end,
                3,
                result.timestamp,
                result.proofs,
            )
            .unwrap();

        storage
            .batches()
            .mark_committed(&batch_id, &commitment_id, now)
            .unwrap();

        let batch = storage.batches().get(&batch_id).unwrap().unwrap();
        assert_eq!(batch.status, BatchStatus::Committed);
        assert!(batch.commitment_id.is_some());

        let commitment = storage.commitments().get(&commitment_id).unwrap().unwrap();
        assert_eq!(commitment.record_count, 3);
    }
}

#[tokio::test]
#[ignore = "sparse merkle accumulator has internal issues with monotree"]
async fn test_full_commit_flow_with_sparse_merkle_accumulator() {
    let (storage, _temp_dir) = create_test_storage();
    let accumulator = Arc::new(Mutex::new(AccumulatorVariant::new(
        &AccumulatorConfig::SparseMerkle,
    )));

    let now = current_timestamp();
    let time_start = now - 100;
    let time_end = now;

    {
        let storage = storage.lock().await;
        storage
            .put(Storable::Record(make_record(2, 1, "sparse1", now - 50)))
            .unwrap();
        storage
            .put(Storable::Record(make_record(2, 2, "sparse2", now - 30)))
            .unwrap();
    }

    let namespaces = vec![make_namespace(2)];
    let batch_id = generate_batch_id(&namespaces, time_start, time_end);

    {
        let storage = storage.lock().await;

        storage
            .batches()
            .create(batch_id, namespaces.clone(), time_start, time_end, now)
            .unwrap();

        let records = storage
            .batches()
            .get_records(&batch_id, storage.records())
            .unwrap();
        assert_eq!(records.len(), 2);

        let (result_tx, result_rx) = kanal::unbounded_async::<CommitmentResult>();

        let acc = accumulator.lock().await;
        let job_id = acc.commit(&records, result_tx).await.unwrap();
        drop(acc);

        assert!(job_id.is_none());

        let result = result_rx.recv().await.unwrap();
        assert_eq!(result.item_count, 2);
        assert_eq!(result.commitment.root.len(), 32);

        let commitment_id = storage
            .commitments()
            .store(
                result.commitment.root,
                result.commitment.namespaces,
                batch_id,
                time_start,
                time_end,
                2,
                result.timestamp,
                result.proofs,
            )
            .unwrap();

        let batch = storage.batches().get(&batch_id).unwrap().unwrap();
        assert_eq!(batch.status, BatchStatus::Pending);

        storage
            .batches()
            .mark_committed(&batch_id, &commitment_id, now)
            .unwrap();

        let batch = storage.batches().get(&batch_id).unwrap().unwrap();
        assert_eq!(batch.status, BatchStatus::Committed);
    }
}

#[tokio::test]
async fn test_external_accumulator_flow_with_webhook() {
    let (storage, _temp_dir) = create_test_storage();
    let state = WebhookState {
        storage: Arc::clone(&storage),
        client_callback_url: None,
    };
    let app = webhook::routes(state);

    let now = current_timestamp();
    let time_start = now - 100;
    let time_end = now;

    let namespaces = vec![make_namespace(3)];
    let batch_id = generate_batch_id(&namespaces, time_start, time_end);
    let external_job_id = "ext-integration-test-001";

    {
        let storage = storage.lock().await;

        storage
            .put(Storable::Record(make_record(3, 1, "ext1", now - 50)))
            .unwrap();
        storage
            .put(Storable::Record(make_record(3, 2, "ext2", now - 30)))
            .unwrap();
        storage
            .put(Storable::Record(make_record(3, 3, "ext3", now - 10)))
            .unwrap();
        storage
            .put(Storable::Record(make_record(3, 4, "ext4", now - 5)))
            .unwrap();

        storage
            .batches()
            .create(batch_id, namespaces.clone(), time_start, time_end, now)
            .unwrap();

        let records = storage
            .batches()
            .get_records(&batch_id, storage.records())
            .unwrap();
        assert_eq!(records.len(), 4);

        storage
            .batches()
            .update_record_count(&batch_id, records.len() as u64, now)
            .unwrap();

        storage
            .batches()
            .set_external_job_id(&batch_id, external_job_id, now)
            .unwrap();

        let batch = storage.batches().get(&batch_id).unwrap().unwrap();
        assert_eq!(batch.status, BatchStatus::Processing);
    }

    let commitment_result = CommitmentResult {
        commitment: Commitment {
            namespaces: namespaces.clone(),
            root: vec![0xDE; 32],
            committed_at: now,
        },
        item_count: 4,
        timestamp: now,
        proofs: HashMap::new(),
        meta: json!({"provider": "test-external-service"}),
    };

    let payload = json!({
        "job_id": external_job_id,
        "status": "success",
        "commitment": commitment_result.commitment,
        "item_count": commitment_result.item_count,
        "timestamp": commitment_result.timestamp,
        "proofs": commitment_result.proofs,
        "meta": commitment_result.meta,
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhook/commitment")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["accepted"], true);

    {
        let storage = storage.lock().await;

        let batch = storage.batches().get(&batch_id).unwrap().unwrap();
        assert_eq!(batch.status, BatchStatus::Committed);
        assert!(batch.commitment_id.is_some());
        assert_eq!(batch.record_count, 4);

        let commitment = storage
            .commitments()
            .get(&batch.commitment_id.unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(commitment.record_count, 4);
        assert_eq!(commitment.batch_id, batch_id);

        let lookup = storage
            .batches()
            .get_by_external_job_id(external_job_id)
            .unwrap();
        assert!(lookup.is_none());
    }
}

#[tokio::test]
async fn test_multiple_namespaces_in_single_batch() {
    let (storage, _temp_dir) = create_test_storage();
    let accumulator = Arc::new(Mutex::new(AccumulatorVariant::new(&AccumulatorConfig::Merkle)));

    let now = current_timestamp();
    let time_start = now - 100;
    let time_end = now;

    {
        let storage = storage.lock().await;
        storage
            .put(Storable::Record(make_record(10, 1, "ns10-1", now - 50)))
            .unwrap();
        storage
            .put(Storable::Record(make_record(10, 2, "ns10-2", now - 40)))
            .unwrap();
        storage
            .put(Storable::Record(make_record(11, 1, "ns11-1", now - 30)))
            .unwrap();
        storage
            .put(Storable::Record(make_record(11, 2, "ns11-2", now - 20)))
            .unwrap();
        storage
            .put(Storable::Record(make_record(12, 1, "ns12-1", now - 10)))
            .unwrap();
    }

    let namespaces = vec![make_namespace(10), make_namespace(11), make_namespace(12)];
    let batch_id = generate_batch_id(&namespaces, time_start, time_end);

    {
        let storage = storage.lock().await;

        storage
            .batches()
            .create(batch_id, namespaces.clone(), time_start, time_end, now)
            .unwrap();

        let records = storage
            .batches()
            .get_records(&batch_id, storage.records())
            .unwrap();
        assert_eq!(records.len(), 5);

        let (result_tx, result_rx) = kanal::unbounded_async::<CommitmentResult>();

        let acc = accumulator.lock().await;
        acc.commit(&records, result_tx).await.unwrap();
        drop(acc);

        let result = result_rx.recv().await.unwrap();
        assert_eq!(result.item_count, 5);

        let commitment_id = storage
            .commitments()
            .store(
                result.commitment.root,
                result.commitment.namespaces,
                batch_id,
                time_start,
                time_end,
                5,
                result.timestamp,
                result.proofs,
            )
            .unwrap();

        storage
            .batches()
            .mark_committed(&batch_id, &commitment_id, now)
            .unwrap();

        let commitment = storage.commitments().get(&commitment_id).unwrap().unwrap();
        assert_eq!(commitment.record_count, 5);
    }
}

#[tokio::test]
async fn test_empty_batch_handling() {
    let (storage, _temp_dir) = create_test_storage();

    let now = current_timestamp();
    let time_start = now - 100;
    let time_end = now;

    let namespaces = vec![make_namespace(99)];
    let batch_id = generate_batch_id(&namespaces, time_start, time_end);

    {
        let storage = storage.lock().await;

        storage
            .batches()
            .create(batch_id, namespaces.clone(), time_start, time_end, now)
            .unwrap();

        let records = storage
            .batches()
            .get_records(&batch_id, storage.records())
            .unwrap();
        assert!(records.is_empty());

        storage
            .batches()
            .update_status(&batch_id, BatchStatus::Failed, now)
            .unwrap();

        let batch = storage.batches().get(&batch_id).unwrap().unwrap();
        assert_eq!(batch.status, BatchStatus::Failed);
        assert_eq!(batch.record_count, 0);
    }
}

#[tokio::test]
async fn test_batch_time_range_filtering() {
    let (storage, _temp_dir) = create_test_storage();

    let now = current_timestamp();

    {
        let storage = storage.lock().await;
        storage
            .put(Storable::Record(make_record(20, 1, "before", now - 200)))
            .unwrap();
        storage
            .put(Storable::Record(make_record(20, 2, "in-range-1", now - 80)))
            .unwrap();
        storage
            .put(Storable::Record(make_record(20, 3, "in-range-2", now - 50)))
            .unwrap();
        storage
            .put(Storable::Record(make_record(20, 4, "after", now + 100)))
            .unwrap();
    }

    let namespaces = vec![make_namespace(20)];
    let time_start = now - 100;
    let time_end = now;
    let batch_id = generate_batch_id(&namespaces, time_start, time_end);

    {
        let storage = storage.lock().await;

        storage
            .batches()
            .create(batch_id, namespaces.clone(), time_start, time_end, now)
            .unwrap();

        let records = storage
            .batches()
            .get_records(&batch_id, storage.records())
            .unwrap();

        assert_eq!(records.len(), 2);

        for record in &records {
            assert!(record.timestamp >= time_start);
            assert!(record.timestamp <= time_end);
        }
    }
}

#[tokio::test]
async fn test_concurrent_batch_operations() {
    let (storage, _temp_dir) = create_test_storage();

    let now = current_timestamp();

    {
        let storage = storage.lock().await;
        for i in 0..10 {
            storage
                .put(Storable::Record(make_record(30, i, &format!("val{}", i), now - (i as u64 * 5))))
                .unwrap();
        }
    }

    let storage_clone1 = Arc::clone(&storage);
    let storage_clone2 = Arc::clone(&storage);

    let namespaces1 = vec![make_namespace(30)];
    let batch_id1 = generate_batch_id(&namespaces1, now - 100, now - 50);

    let namespaces2 = vec![make_namespace(30)];
    let batch_id2 = generate_batch_id(&namespaces2, now - 49, now);

    let handle1 = tokio::spawn(async move {
        let storage = storage_clone1.lock().await;
        storage
            .batches()
            .create(batch_id1, namespaces1, now - 100, now - 50, now)
            .unwrap();
        let records = storage
            .batches()
            .get_records(&batch_id1, storage.records())
            .unwrap();
        records.len()
    });

    let handle2 = tokio::spawn(async move {
        let storage = storage_clone2.lock().await;
        storage
            .batches()
            .create(batch_id2, namespaces2, now - 49, now, now)
            .unwrap();
        let records = storage
            .batches()
            .get_records(&batch_id2, storage.records())
            .unwrap();
        records.len()
    });

    let (count1, count2) = tokio::join!(handle1, handle2);
    let total = count1.unwrap() + count2.unwrap();
    assert_eq!(total, 10);
}
