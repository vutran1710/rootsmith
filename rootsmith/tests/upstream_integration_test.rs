use anyhow::Result;

#[tokio::test]
async fn test_upstream_flow() -> Result<()> {
    // Send HTTP request to running server
    let client = reqwest::Client::new();
    let resp = client
        .post("http://127.0.0.1:8080")
        .header("Content-Type", "application/json")
        .body(r#"{"user_id": "alice", "event_type": "login", "timestamp": 1700000000, "data": "test"}"#)
        .send()
        .await?;

    assert_eq!(resp.status(), 202);
    println!("POST response: {}", resp.status());

    Ok(())
}
