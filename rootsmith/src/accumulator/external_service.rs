use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde::Serialize;

use super::Accumulator;
use crate::types::CommitmentResult;
use crate::types::Record;
use crate::utils::http_client::MultipartValue;
use crate::utils::HttpClient;

pub enum Transport {
    Http(HttpClient),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum WireFormat {
    Protobuf,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct HttpTransportConfig {
    pub base_url: String,
    pub headers: HashMap<String, String>,
    pub endpoint: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum TransportConfig {
    #[serde(rename = "http")]
    Http(HttpTransportConfig),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ExternalServiceConfig {
    pub transport: TransportConfig,
    pub wire_format: WireFormat,
}

pub struct ExternalServiceAccumulator {
    transport: Transport,
    wire_format: WireFormat,
    config: ExternalServiceConfig,
}

impl ExternalServiceAccumulator {
    pub fn new(config: ExternalServiceConfig) -> Self {
        let transport = match &config.transport {
            TransportConfig::Http(http_config) => {
                let headers = HeaderMap::from_iter(
                    http_config
                        .headers
                        .iter()
                        .map(|(k, v)| (k.parse().unwrap(), v.parse().unwrap())),
                );
                let client = HttpClient::new(&http_config.base_url, headers)
                    .expect("Failed to create HTTP client");
                Transport::Http(client)
            }
        };

        Self {
            transport,
            wire_format: config.wire_format.clone(),
            config,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ResponseProtobufMessage {
    job_id: String,
}

#[async_trait]
impl Accumulator for ExternalServiceAccumulator {
    async fn commit(
        &self,
        records: &[Record],
        _result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<Option<String>> {
        match (&self.transport, &self.wire_format) {
            (Transport::Http(ref client), &WireFormat::Protobuf) => {
                tracing::info_span!("ExternalServiceAccumulator::commit");

                // TODO: this is not correct, but just a placeholder for now
                // we need to define the protobuf message format for the records
                let data = records
                    .iter()
                    .flat_map(|record| record.value.as_bytes())
                    .collect::<Vec<u8>>();

                let endpoint = match &self.config.transport {
                    TransportConfig::Http(cfg) => &cfg.endpoint,
                };

                // TODO: register webhook URL or other metadata if needed with fields
                let mut fields = HashMap::new();
                fields.insert(
                    "webhook_url".to_string(),
                    MultipartValue::Text("http://localhost:8081/webhook".to_string()),
                );

                let ResponseProtobufMessage { job_id } = client
                    .post_protobuf(endpoint, fields, data.to_vec(), "v1.0", None, None)
                    .await?
                    .json()
                    .await?;

                tracing::info!("Submitted job to external service: job_id={}", job_id);

                // Return job_id so caller can link it to batch for webhook tracking
                Ok(Some(job_id))
            }
        }
    }
}
