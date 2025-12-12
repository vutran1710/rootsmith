use anyhow::Result;
use crossbeam_channel::Sender;
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

impl UpstreamConnector for MockUpstream {
    fn name(&self) -> &'static str {
        "mock-upstream"
    }

    fn open(&mut self, tx: Sender<IncomingRecord>) -> Result<()> {
        let records = self.records.clone();
        let delay = self.delay_ms;

        std::thread::spawn(move || {
            for record in records {
                if delay > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                }
                if tx.send(record).is_err() {
                    break;
                }
            }
        });

        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
