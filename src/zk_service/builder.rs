use super::types::*;

/// Builder for job submission requests
pub struct JobRequestBuilder {
    circuit_id: Option<String>,
    operators: Vec<Operator>,
    data: Option<InputData>,
    webhook_url: Option<String>,
}

impl JobRequestBuilder {
    pub fn new() -> Self {
        Self {
            circuit_id: None,
            operators: Vec::new(),
            data: None,
            webhook_url: None,
        }
    }

    pub fn circuit_id(mut self, circuit_id: impl Into<String>) -> Self {
        self.circuit_id = Some(circuit_id.into());
        self
    }

    pub fn operator(mut self, operator: Operator) -> Self {
        self.operators.push(operator);
        self
    }

    pub fn raw_bytes(mut self, data: Vec<Vec<u8>>) -> Self {
        self.data = Some(InputData::RawBytes(data));
        self
    }

    pub fn table(
        mut self,
        columns: std::collections::HashMap<String, Vec<serde_json::Value>>,
        column_order: Vec<String>,
    ) -> Self {
        self.data = Some(InputData::Table {
            columns,
            column_order,
        });
        self
    }

    pub fn webhook_url(mut self, url: impl Into<String>) -> Self {
        self.webhook_url = Some(url.into());
        self
    }

    pub fn build(self) -> std::result::Result<SubmitJobRequest, String> {
        Ok(SubmitJobRequest {
            circuit_id: self.circuit_id.ok_or("circuit_id is required")?,
            operators: self.operators,
            data: self.data.ok_or("data is required")?,
            webhook_url: self.webhook_url.ok_or("webhook_url is required")?,
        })
    }
}

impl Default for JobRequestBuilder {
    fn default() -> Self {
        Self::new()
    }
}
