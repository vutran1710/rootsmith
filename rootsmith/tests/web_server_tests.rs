use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use rootsmith::server::admin;

#[tokio::test]
async fn test_health_endpoint() {
    let app = admin::routes();

    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let app = admin::routes();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn test_methods_endpoint() {
    let app = admin::routes();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/methods")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let methods = json.as_array().unwrap();
    assert_eq!(methods.len(), 2);
    assert!(methods.contains(&serde_json::json!("merkle")));
    assert!(methods.contains(&serde_json::json!("zero knowledge")));
}

#[tokio::test]
async fn test_not_found() {
    let app = admin::routes();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_webserver_with_admin_routes() {
    use rootsmith::server::Webserver;
    use std::net::TcpListener;
    use std::time::Duration;

    // Find an available port
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Start server in background
    let server = Webserver::new(port).register(admin::routes());
    let handle = tokio::spawn(async move {
        server.run().await;
    });

    // Give the server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();

    // Test health endpoint
    let resp = client
        .get(format!("http://127.0.0.1:{}/health", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Test metrics endpoint
    let resp = client
        .get(format!("http://127.0.0.1:{}/metrics", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["status"], "ok");

    // Test methods endpoint
    let resp = client
        .get(format!("http://127.0.0.1:{}/methods", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let methods: Vec<String> = resp.json().await.unwrap();
    assert_eq!(methods.len(), 2);

    handle.abort();
}

#[tokio::test]
async fn test_webserver_multiple_route_registrations() {
    use axum::{routing::get, Router};
    use rootsmith::server::Webserver;
    use std::net::TcpListener;
    use std::time::Duration;

    // Find an available port
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Create custom routes
    let custom_routes = Router::new().route("/custom", get(|| async { "custom response" }));

    // Start server with both admin and custom routes
    let server = Webserver::new(port)
        .register(admin::routes())
        .register(custom_routes);

    let handle = tokio::spawn(async move {
        server.run().await;
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();

    // Test admin route still works
    let resp = client
        .get(format!("http://127.0.0.1:{}/health", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Test custom route
    let resp = client
        .get(format!("http://127.0.0.1:{}/custom", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "custom response");

    handle.abort();
}
