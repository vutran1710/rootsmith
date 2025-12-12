use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;
use crate::traits::UpstreamConnector;
use crate::types::IncomingRecord;

/// Noop upstream connector for demonstration purposes.
pub struct NoopUpstream;

#[async_trait]
impl UpstreamConnector for NoopUpstream {
    fn name(&self) -> &'static str {
        "noop-upstream"
    }
    
    async fn open(&mut self, _tx: AsyncSender<IncomingRecord>) -> Result<()> {
        tracing::info!("NoopUpstream: open() called - no data to send");
        Ok(())
    }
    
    async fn close(&mut self) -> Result<()> {
        tracing::info!("NoopUpstream: close() called");
        Ok(())
    }
}
