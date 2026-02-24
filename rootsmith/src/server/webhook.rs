//! Webhook handler for receiving commitment results from external services.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::storage::{BatchStatus, StorageManager};
use crate::types::CommitmentResult;

/// Shared state for webhook handlers.
#[derive(Clone)]
pub struct WebhookState {
    pub storage: Arc<Mutex<StorageManager>>,
    /// Optional URL to POST commitment result when webhook is processed (e.g. for integration testing).
    pub client_callback_url: Option<String>,
}

/// Job status reported by external service.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Success,
    Failed,
}

/// Webhook payload from external service.
#[derive(Debug, Deserialize)]
pub struct WebhookPayload {
    pub job_id: String,
    pub status: JobStatus,
    #[serde(flatten)]
    pub result: Option<CommitmentResult>,
    pub error: Option<String>,
}

/// Response sent back to external service.
#[derive(Serialize)]
pub struct WebhookResponse {
    pub accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Create webhook routes.
pub fn routes(state: WebhookState) -> Router {
    Router::new()
        .route("/webhook/commitment", post(handle_commitment))
        .with_state(state)
}

/// Handle commitment webhook from external service.
async fn handle_commitment(
    State(state): State<WebhookState>,
    Json(payload): Json<WebhookPayload>,
) -> Result<Json<WebhookResponse>, (StatusCode, Json<WebhookResponse>)> {
    tracing::info!("Webhook received: job_id={}", payload.job_id);

    let storage = state.storage.lock().await;

    // Lookup batch by external job_id
    let batch = storage
        .batches()
        .get_by_external_job_id(&payload.job_id)
        .map_err(|e| storage_error(&e))?
        .ok_or_else(|| not_found(&payload.job_id))?;

    // Verify batch is in Processing state
    if batch.status != BatchStatus::Processing {
        return Err((
            StatusCode::CONFLICT,
            Json(WebhookResponse {
                accepted: false,
                error: Some(format!(
                    "Batch status is {:?}, expected Processing",
                    batch.status
                )),
            }),
        ));
    }

    let now = current_timestamp();

    match payload.status {
        JobStatus::Success => {
            let result = payload.result.ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(WebhookResponse {
                        accepted: false,
                        error: Some("Missing result for success status".into()),
                    }),
                )
            })?;

            // Store commitment
            let commitment_id = storage
                .commitments()
                .store(
                    result.commitment.root.clone(),
                    batch.namespaces.clone(),
                    batch.batch_id,
                    batch.time_start,
                    batch.time_end,
                    result.item_count,
                    now,
                    result.proofs.clone(),
                )
                .map_err(|e| storage_error(&e))?;

            // Mark batch committed (also removes job index)
            storage
                .batches()
                .mark_committed(&batch.batch_id, &commitment_id, now)
                .map_err(|e| storage_error(&e))?;

            tracing::info!(
                "Commitment stored: job_id={} batch_id={} commitment_id={}",
                payload.job_id,
                hex::encode(&batch.batch_id),
                hex::encode(&commitment_id[..8])
            );

            // Notify client if callback URL is configured
            if let Some(ref callback_url) = state.client_callback_url {
                let job_id = payload.job_id.clone();
                let batch_id = batch.batch_id;
                let commitment_id_copy = commitment_id;
                let result_clone = result.clone();
                let url = callback_url.clone();
                tokio::spawn(async move {
                    if let Err(e) =
                        notify_client(&url, &job_id, &batch_id, &commitment_id_copy, &result_clone)
                            .await
                    {
                        tracing::warn!("Failed to notify client at {}: {}", url, e);
                    }
                });
            }

            Ok(Json(WebhookResponse {
                accepted: true,
                error: None,
            }))
        }

        JobStatus::Failed => {
            // Update batch status to Failed
            storage
                .batches()
                .update_status(&batch.batch_id, BatchStatus::Failed, now)
                .map_err(|e| storage_error(&e))?;

            // Remove job index
            if let Some(ref job_id) = batch.external_job_id {
                storage
                    .batches()
                    .remove_job_index(job_id)
                    .map_err(|e| storage_error(&e))?;
            }

            tracing::error!(
                "External service failed: job_id={} error={:?}",
                payload.job_id,
                payload.error
            );

            Ok(Json(WebhookResponse {
                accepted: true,
                error: None,
            }))
        }
    }
}

/// Payload sent to client callback URL when commitment is processed.
#[derive(Serialize)]
struct ClientCallbackPayload {
    job_id: String,
    batch_id: Vec<u8>,
    commitment_id: Vec<u8>,
    commitment: CommitmentResult,
}

async fn notify_client(
    url: &str,
    job_id: &str,
    batch_id: &[u8; 16],
    commitment_id: &[u8; 32],
    result: &CommitmentResult,
) -> Result<(), reqwest::Error> {
    let payload = ClientCallbackPayload {
        job_id: job_id.to_string(),
        batch_id: batch_id.to_vec(),
        commitment_id: commitment_id.to_vec(),
        commitment: result.clone(),
    };
    let client = reqwest::Client::new();
    client.post(url).json(&payload).send().await?;
    Ok(())
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn not_found(job_id: &str) -> (StatusCode, Json<WebhookResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(WebhookResponse {
            accepted: false,
            error: Some(format!("Unknown job_id: {}", job_id)),
        }),
    )
}

fn storage_error(e: &anyhow::Error) -> (StatusCode, Json<WebhookResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(WebhookResponse {
            accepted: false,
            error: Some(format!("Storage error: {}", e)),
        }),
    )
}
