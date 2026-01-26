use std::time::Duration;

use hex;
use reqwest::Client;
use tracing;
use uuid::Uuid;

use super::error::Result;
use super::error::ZkServiceError;
use super::types::*;

/// ZK Service HTTP client
#[derive(Debug, Clone)]
pub struct ZkServiceClient {
    client: Client,
    base_url: String,
}

impl ZkServiceClient {
    /// Create a new client with the given base URL
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
        }
    }

    /// Create a new client with custom timeout
    pub fn with_timeout(base_url: impl Into<String>, timeout: Duration) -> Result<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| ZkServiceError::Http(e))?;

        Ok(Self {
            client,
            base_url: base_url.into(),
        })
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Submit a job using JSON format
    pub async fn submit_job(&self, request: SubmitJobRequest) -> Result<SubmitJobResponse> {
        // Convert InputData to JSON value
        let data = match &request.data {
            InputData::RawBytes(bytes) => {
                serde_json::json!(bytes)
            }
            InputData::Table {
                columns,
                column_order,
            } => {
                serde_json::json!({
                    "columns": columns,
                    "column_order": column_order
                })
            }
        };

        // Determine data_type
        let data_type = match request.data {
            InputData::RawBytes(_) => "raw_bytes",
            InputData::Table { .. } => "table",
        };

        // Build request body
        let body = serde_json::json!({
            "circuit_id": request.circuit_id,
            "operators": request.operators,
            "data_type": data_type,
            "data": data,
            "webhook_url": request.webhook_url,
        });

        let url = format!("{}/job/submit", self.base_url);

        tracing::info!("ZK Service Request:");
        tracing::info!("  URL: {}", url);
        tracing::info!("  Circuit ID: {}", request.circuit_id);
        tracing::info!("  Operators: {:?}", request.operators);
        tracing::info!("  Data Type: {}", data_type);
        match &data {
            serde_json::Value::Array(rows) => {
                tracing::info!("  Data: {} rows", rows.len());
                for (idx, row) in rows.iter().enumerate() {
                    if let Some(bytes) = row.as_array() {
                        let byte_vec: Vec<u8> = bytes
                            .iter()
                            .filter_map(|v| v.as_u64().map(|n| n as u8))
                            .collect();
                        tracing::info!(
                            "    Row {}: {} bytes - {}",
                            idx,
                            byte_vec.len(),
                            hex::encode(&byte_vec)
                        );
                    }
                }
            }
            _ => {
                tracing::info!(
                    "  Data: {}",
                    serde_json::to_string(&data)
                        .unwrap_or_else(|_| "failed to serialize".to_string())
                );
            }
        }
        tracing::info!("  Webhook URL: {}", request.webhook_url);
        tracing::info!(
            "  Full Request Body: {}",
            serde_json::to_string_pretty(&body)
                .unwrap_or_else(|_| "failed to serialize".to_string())
        );

        let response = self.client.post(&url).json(&body).send().await?;

        // Handle errors
        let status = response.status();
        if !status.is_success() {
            return self
                .handle_error_response(response)
                .await
                .map(|_| unreachable!());
        }

        let result: SubmitJobResponse = response.json().await?;
        Ok(result)
    }

    /// Submit a job using multipart format
    pub async fn submit_job_multipart(
        &self,
        request: SubmitJobRequest,
    ) -> Result<SubmitJobResponse> {
        use reqwest::multipart;

        tracing::info!("ZK Service Request (Multipart):");
        tracing::info!("  URL: {}/job/submit", self.base_url);
        tracing::info!("  Circuit ID: {}", request.circuit_id);
        tracing::info!("  Operators: {:?}", request.operators);

        // Serialize operators to JSON string
        let operators_json = serde_json::to_string(&request.operators)?;

        // Build multipart form
        let mut form = multipart::Form::new()
            .text("circuit_id", request.circuit_id.clone())
            .text("webhook_url", request.webhook_url.clone())
            .text("operators", operators_json.clone());

        // Add data fields based on type
        match &request.data {
            InputData::RawBytes(rows) => {
                tracing::info!("  Data Type: raw_bytes");
                tracing::info!("  Data: {} rows", rows.len());
                for (index, row) in rows.iter().enumerate() {
                    tracing::info!(
                        "    Row {}: {} bytes - {}",
                        index,
                        row.len(),
                        hex::encode(row)
                    );
                }
            }
            InputData::Table {
                columns,
                column_order,
            } => {
                tracing::info!("  Data Type: table");
                tracing::info!("  Columns: {} ({:?})", column_order.len(), column_order);
            }
        }

        match request.data {
            InputData::RawBytes(rows) => {
                for (index, row) in rows.into_iter().enumerate() {
                    let field_name = format!("data[{}]", index);
                    tracing::info!(
                        "  Adding form part: {} ({} bytes) - {}",
                        field_name,
                        row.len(),
                        hex::encode(&row)
                    );
                    form = form.part(field_name, multipart::Part::bytes(row));
                }
            }
            InputData::Table {
                columns,
                column_order,
            } => {
                // Build row index map
                let max_rows = columns.values().map(|v| v.len()).max().unwrap_or(0);

                for row_idx in 0..max_rows {
                    for col_name in &column_order {
                        if let Some(col_values) = columns.get(col_name) {
                            if let Some(value) = col_values.get(row_idx) {
                                // Convert value to string
                                let value_str = match value {
                                    serde_json::Value::Number(n) => n.to_string(),
                                    serde_json::Value::String(s) => s.clone(),
                                    _ => value.to_string(),
                                };
                                form = form
                                    .text(format!("data[{}][{}]", row_idx, col_name), value_str);
                            }
                        }
                    }
                }
            }
        }

        let url = format!("{}/job/submit", self.base_url);
        tracing::info!("  Webhook URL: {}", request.webhook_url);
        tracing::info!("  Sending multipart form");

        let response = self.client.post(&url).multipart(form).send().await?;

        let status = response.status();
        if !status.is_success() {
            return self
                .handle_error_response(response)
                .await
                .map(|_| unreachable!());
        }

        let result: SubmitJobResponse = response.json().await?;
        Ok(result)
    }

    /// Get job status
    pub async fn get_job_status(&self, job_id: Uuid) -> Result<JobStatusResponse> {
        let url = format!("{}/job/{}", self.base_url, job_id);

        let response = self.client.get(&url).send().await?;

        let status = response.status();
        if status == 404 {
            return Err(ZkServiceError::JobNotFound(job_id.to_string()));
        }

        if !status.is_success() {
            return self
                .handle_error_response(response)
                .await
                .map(|_| unreachable!());
        }

        let result: JobStatusResponse = response.json().await?;
        Ok(result)
    }

    /// List available circuits
    pub async fn list_circuits(&self) -> Result<CircuitsResponse> {
        let url = format!("{}/circuits", self.base_url);

        let response = self.client.get(&url).send().await?;

        let status = response.status();
        if !status.is_success() {
            return self
                .handle_error_response(response)
                .await
                .map(|_| unreachable!());
        }

        let result: CircuitsResponse = response.json().await?;
        Ok(result)
    }

    /// Health check
    pub async fn health(&self) -> Result<HealthResponse> {
        let url = format!("{}/health", self.base_url);

        let response = self.client.get(&url).send().await?;

        let status = response.status();
        if !status.is_success() {
            return self
                .handle_error_response(response)
                .await
                .map(|_| unreachable!());
        }

        let result: HealthResponse = response.json().await?;
        Ok(result)
    }

    /// Wait for job completion (polling)
    pub async fn wait_for_job(
        &self,
        job_id: Uuid,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<JobStatusResponse> {
        let start = std::time::Instant::now();

        loop {
            let status = self.get_job_status(job_id).await?;

            match status.status.as_str() {
                "completed" | "failed" => {
                    return Ok(status);
                }
                _ => {
                    if start.elapsed() > timeout {
                        return Err(ZkServiceError::InvalidInput(format!(
                            "Job {} did not complete within timeout",
                            job_id
                        )));
                    }
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }

    /// Helper to handle error responses
    async fn handle_error_response(&self, response: reqwest::Response) -> Result<()> {
        let status = response.status();
        let error_text = response.text().await?;

        // Try to parse as ErrorResponse
        if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&error_text) {
            let error_msg = error_response.error;

            return match status.as_u16() {
                400 => Err(ZkServiceError::InvalidInput(error_msg)),
                404 if error_msg.contains("Circuit") => {
                    Err(ZkServiceError::CircuitNotFound(error_msg))
                }
                404 if error_msg.contains("Job") => Err(ZkServiceError::JobNotFound(error_msg)),
                503 if error_msg.contains("Queue") => Err(ZkServiceError::QueueFull),
                _ => Err(ZkServiceError::Api(error_msg)),
            };
        }

        Err(ZkServiceError::Api(format!(
            "HTTP {}: {}",
            status, error_text
        )))
    }
}
