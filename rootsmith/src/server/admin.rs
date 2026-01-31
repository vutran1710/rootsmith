use axum::{http::StatusCode, routing::get, Json, Router};
use serde::Serialize;

pub fn routes() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/methods", get(methods))
}

async fn health() -> StatusCode {
    StatusCode::OK
}

#[derive(Serialize)]
struct Metrics {
    status: &'static str,
}

async fn metrics() -> Json<Metrics> {
    Json(Metrics { status: "ok" })
}

async fn methods() -> Json<Vec<&'static str>> {
    Json(vec!["merkle", "zero knowledge"])
}
