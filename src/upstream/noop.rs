use anyhow::Result;
use crossbeam_channel::Sender;
use crate::traits::UpstreamConnector;
use crate::types::IncomingRecord;

/// Noop upstream connector for demonstration purposes.
pub struct NoopUpstream;

impl UpstreamConnector for NoopUpstream {
    fn name(&self) -> &'static str {
        "noop-upstream"
    }
    
    fn open(&mut self, _tx: Sender<IncomingRecord>) -> Result<()> {
        tracing::info!("NoopUpstream: open() called - no data to send");
        Ok(())
    }
    
    fn close(&mut self) -> Result<()> {
        tracing::info!("NoopUpstream: close() called");
        Ok(())
    }
}
