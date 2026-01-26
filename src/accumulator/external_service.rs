use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde::Serialize;

use crate::config::AccumulatorType;
use crate::traits::Accumulator;
use crate::types::CommitmentResult;
use crate::types::Record;
use crate::utils::HttpClient;
use crate::MultipartValue;

pub enum Transport {
    Http(HttpClient),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum WireProtocol {
    Protobuf,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HttpTransportConfig {
    pub base_url: String,
    pub headers: HashMap<String, String>,
    pub endpoint: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum TransportConfig {
    HttpConfig(HttpTransportConfig),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ExternalServiceConfig {
    pub transport: TransportConfig,
    pub wire_protocol: WireProtocol,
}

pub struct ExternalServiceAccumulator {
    transport: Transport,
    wire_protocol: WireProtocol,
    config: ExternalServiceConfig,
}

impl ExternalServiceAccumulator {
    pub fn new(config: ExternalServiceConfig) -> Self {
        let transport = match &config.transport {
            TransportConfig::HttpConfig(http_config) => {
                let headers = HeaderMap::from_iter(
                    http_config
                        .headers
                        .iter()
                        .map(|(k, v)| (k.parse().unwrap(), v.parse().unwrap())),
                );
                let client = HttpClient::new(&http_config.endpoint, headers)
                    .expect("Failed to create HTTP client");
                Transport::Http(client)
            }
        };

        Self {
            transport,
            wire_protocol: config.wire_protocol.clone(),
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
    fn accumulator_type(&self) -> AccumulatorType {
        AccumulatorType::External
    }

    async fn commit(
        &self,
        records: &[Record],
        _result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        match (&self.transport, &self.wire_protocol) {
            (Transport::Http(ref client), &WireProtocol::Protobuf) => {
                tracing::info_span!("ExternalServiceAccumulator::commit");

                // TODO: this is not correct, but just a placeholder for now
                // we need to define the protobuf message format for the records
                let data = records
                    .iter()
                    .flat_map(|record| record.value.as_bytes())
                    .collect::<Vec<u8>>();

                let endpoint = match &self.config.transport {
                    TransportConfig::HttpConfig(http_config) => &http_config.endpoint,
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

                /// TODO: register job_id to track status and get commitment result later
                Ok(())
            }
        }
    }
}
