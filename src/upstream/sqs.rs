use crate::traits::UpstreamConnector;
use crate::types::IncomingRecord;
use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;

pub struct SqsSource {
    queue_url: String,
    region: String,
}

impl SqsSource {
    pub fn new(queue_url: String, region: String) -> Self {
        Self { queue_url, region }
    }
}

#[async_trait]
impl UpstreamConnector for SqsSource {
    fn name(&self) -> &'static str {
        "sqs"
    }

    async fn open(&mut self, _tx: AsyncSender<IncomingRecord>) -> Result<()> {
        tracing::info!(
            "Opening SQS connection: {} region: {}",
            self.queue_url,
            self.region
        );
        // TODO: Implement actual SQS connection
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        tracing::info!("Closing SQS connection");
        // TODO: Implement actual SQS disconnection
        Ok(())
    }
}
