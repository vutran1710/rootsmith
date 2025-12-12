use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;
use crate::traits::UpstreamConnector;
use crate::types::IncomingRecord;

/// Mock upstream connector for testing.
pub struct MockUpstream {
    pub records: Vec<IncomingRecord>,
    pub delay_ms: u64,
}

impl MockUpstream {
    pub fn new(records: Vec<IncomingRecord>, delay_ms: u64) -> Self {
        Self { records, delay_ms }
    }
}

impl Default for MockUpstream {
    fn default() -> Self {
        Self {
            records: Vec::new(),
            delay_ms: 0,
        }
    }
}

#[async_trait]
impl UpstreamConnector for MockUpstream {
    fn name(&self) -> &'static str {
        "mock-upstream"
    }

    async fn open(&mut self, tx: AsyncSender<IncomingRecord>) -> Result<()> {
        let records = self.records.clone();
        let delay = self.delay_ms;

        tokio::spawn(async move {
            for record in records {
                if delay > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                }
                if tx.send(record).await.is_err() {
                    break;
                }
            }
        });

        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
