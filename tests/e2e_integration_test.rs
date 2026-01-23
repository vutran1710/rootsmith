use std::path::Path;
use std::time::Duration;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct TestData {
    records: Vec<TestRecord>,
}

#[derive(Deserialize)]
struct TestRecord {
    id: String,
    ts_ms: u64,
    user_id: String,
    action: String,
}

#[tokio::test]
#[ignore]
async fn test_e2e_send_all_zk_data() {
    println!("\n═══════════════════════════════════════════════════════════");
    println!("🔄 E2E Integration Test: Send All Data from zk-data.json");
    println!("═══════════════════════════════════════════════════════════\n");

    let data_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("zk-data.json");

    if !data_path.exists() {
        panic!("Test data file not found: {:?}", data_path);
    }

    let json_content = std::fs::read_to_string(&data_path)
        .expect("Failed to read zk-data.json");
    let test_data: TestData = serde_json::from_str(&json_content)
        .expect("Failed to parse zk-data.json");

    println!("📊 Loaded {} records from zk-data.json", test_data.records.len());

    let base_url = std::env::var("ROOTSMITH_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string());

    println!("🌐 Sending data to: {}/ingest/raw", base_url);

    let client = reqwest::Client::new();
    let mut success_count = 0;
    let mut error_count = 0;

    for (index, record) in test_data.records.iter().enumerate() {
        let payload = json!({
            "id": record.id,
            "ts_ms": record.ts_ms,
            "user_id": record.user_id,
            "action": record.action
        });

        let url = format!("{}/ingest/raw", base_url);

        match client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    success_count += 1;
                    println!("✅ [{}] Record {} sent successfully (status: {})", 
                        index + 1, record.id, status);
                } else {
                    error_count += 1;
                    let error_text = response.text().await.unwrap_or_default();
                    println!("❌ [{}] Record {} failed (status: {}, error: {})", 
                        index + 1, record.id, status, error_text);
                }
            }
            Err(e) => {
                error_count += 1;
                println!("❌ [{}] Record {} failed to send: {}", index + 1, record.id, e);
            }
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    println!("\n═══════════════════════════════════════════════════════════");
    println!("📈 Test Summary");
    println!("═══════════════════════════════════════════════════════════");
    println!("Total records: {}", test_data.records.len());
    println!("✅ Successful: {}", success_count);
    println!("❌ Failed: {}", error_count);
    println!("═══════════════════════════════════════════════════════════\n");

    assert_eq!(error_count, 0, "All records should be sent successfully");
    assert_eq!(success_count, test_data.records.len(), 
        "All records should be processed");
}

#[tokio::test]
#[ignore]
async fn test_e2e_health_check() {
    let base_url = std::env::var("ROOTSMITH_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string());

    let client = reqwest::Client::new();
    let url = format!("{}/health", base_url);

    match client.get(&url).send().await {
        Ok(response) => {
            assert!(response.status().is_success(), 
                "Health check should return success");
            println!("✅ Health check passed: {}", url);
        }
        Err(e) => {
            panic!("Health check failed: {}", e);
        }
    }
}
