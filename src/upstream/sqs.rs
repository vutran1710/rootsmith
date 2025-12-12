use anyhow::Result;
use crossbeam_channel::Sender;
use crate::traits::UpstreamConnector;
use crate::types::IncomingRecord;

pub struct SqsSource {
    queue_url: String,
    region: String,
}

impl SqsSource {
    pub fn new(queue_url: String, region: String) -> Self {
        Self {
            queue_url,
            region,
        }
    }
}

impl UpstreamConnector for SqsSource {
    fn name(&self) -> &'static str {
        "sqs"
    }

    fn open(&mut self, _tx: Sender<IncomingRecord>) -> Result<()> {
        tracing::info!("Opening SQS connection: {} region: {}", self.queue_url, self.region);
        // TODO: Implement actual SQS connection
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        tracing::info!("Closing SQS connection");
        // TODO: Implement actual SQS disconnection
        Ok(())
    }
}
