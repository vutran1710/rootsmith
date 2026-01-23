use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use kanal::AsyncSender;

use crate::types::{CommitmentResult, Key32, Proof};
use super::client::ZkServiceClient;
use super::types::{DataSelection, InputData, JobStatusResponse, Operator, SelectionCount, SubmitJobRequest};

pub trait ZKTrait: Send + Sync {
    fn namespace(&self) -> [u8; 32];
    fn key(&self) -> [u8; 32];
    fn value(&self) -> [u8; 32];
    fn timestamp(&self) -> u64;
}

pub struct ZkAccumulator {
    client: ZkServiceClient,
    circuit_id: String,
    webhook_url: Option<String>,
    default_operator: Operator,
}

impl ZkAccumulator {
    pub fn new(base_url: impl Into<String>, circuit_id: impl Into<String>) -> Self {
        let client = ZkServiceClient::new(base_url);
        let default_operator = Operator::Merkle16 {
            selection: DataSelection {
                start: 0,
                offset: 1,
                count: SelectionCount::All,
            },
            handler: "0x0000000000000000000000000000000000000000".to_string(),
        };
        
        Self {
            client,
            circuit_id: circuit_id.into(),
            webhook_url: None,
            default_operator,
        }
    }
    
    pub fn with_webhook(mut self, url: impl Into<String>) -> Self {
        self.webhook_url = Some(url.into());
        self
    }
    
    pub fn with_operator(mut self, operator: Operator) -> Self {
        self.default_operator = operator;
        self
    }
    
    pub fn convert_to_input_data(records: &[Box<dyn ZKTrait>]) -> InputData {
        let mut data_rows = Vec::with_capacity(records.len());
        
        for record in records {
            let mut row = Vec::with_capacity(16);
            let value = record.value();
            row.extend_from_slice(&value[..16]);
            data_rows.push(row);
        }
        
        InputData::RawBytes(data_rows)
    }
    
    pub async fn commit_trait(
        &mut self,
        records: &[Box<dyn ZKTrait>],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        if records.is_empty() {
            return Err(anyhow::anyhow!("Cannot commit empty records"));
        }
        
        let input_data = Self::convert_to_input_data(records);
        
        let request = SubmitJobRequest {
            circuit_id: self.circuit_id.clone(),
            operators: vec![self.default_operator.clone()],
            data: input_data,
            webhook_url: self.webhook_url.clone()
                .unwrap_or_else(|| "http://localhost:8080/webhook".to_string()),
        };
        
        let response = self.client.submit_job(request).await?;
        
        tracing::info!("ZK job submitted: job_id={}", response.job_id);
        
        let job_id = response.job_id;
        
        let final_status = self.client
            .wait_for_job(
                job_id,
                Duration::from_millis(500),
                Duration::from_secs(300),
            )
            .await?;
        
        let commitment_result = Self::convert_to_commitment_result(final_status.clone())?;
        
        result_tx.send(commitment_result).await
            .map_err(|e| anyhow::anyhow!("Failed to send commitment result: {}", e))?;
        
        Ok(())
    }
    
    fn convert_to_commitment_result(
        status: JobStatusResponse,
    ) -> Result<CommitmentResult> {
        match status.status.as_str() {
            "completed" => {
                let result = status.result.ok_or_else(|| {
                    anyhow::anyhow!("Job completed but no result available")
                })?;
                
                let public_signals: serde_json::Value = serde_json::from_str(&result.public_signals)?;
                let root_hash = Self::extract_root_from_public_signals(&public_signals)?;
                
                let proof_json: serde_json::Value = serde_json::from_str(&result.proof)?;
                
                let committed_at = status.completed_at
                    .map(|dt| dt.timestamp() as u64)
                    .unwrap_or_else(|| {
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                    });
                
                Ok(CommitmentResult {
                    commitment: root_hash,
                    proofs: Some(Self::parse_proofs_from_result(&proof_json)?),
                    committed_at,
                })
            }
            "failed" => {
                Err(anyhow::anyhow!(
                    "ZK service job failed: {}",
                    status.error.unwrap_or_else(|| "Unknown error".to_string())
                ))
            }
            _ => {
                Err(anyhow::anyhow!(
                    "ZK service job in unexpected state: {}",
                    status.status
                ))
            }
        }
    }
    
    fn extract_root_from_public_signals(
        signals: &serde_json::Value,
    ) -> Result<Vec<u8>> {
        let array = signals.as_array()
            .ok_or_else(|| anyhow::anyhow!("Public signals must be an array"))?;
        
        if array.is_empty() {
            return Err(anyhow::anyhow!("Public signals array is empty"));
        }
        
        let root_value = &array[0];
        
        if let Some(hex_str) = root_value.as_str() {
            hex::decode(hex_str)
                .map_err(|e| anyhow::anyhow!("Failed to decode root hash hex: {}", e))
        } else if let Some(num) = root_value.as_u64() {
            Ok(num.to_be_bytes().to_vec())
        } else {
            Err(anyhow::anyhow!("Root hash must be a string or number"))
        }
    }
    
    fn parse_proofs_from_result(
        _proof_json: &serde_json::Value,
    ) -> Result<HashMap<Key32, Proof>> {
        Ok(HashMap::new())
    }
}
