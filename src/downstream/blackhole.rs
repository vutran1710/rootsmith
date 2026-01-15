use anyhow::Result;
use async_trait::async_trait;

use crate::traits::Downstream;
use crate::types::CommitmentResult;

/// Blackhole downstream that discards all data (no-op).
pub struct BlackholeDownstream;

impl BlackholeDownstream {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Downstream for BlackholeDownstream {
    fn name(&self) -> &'static str {
        "blackhole"
    }

    async fn handle(&self, _result: &CommitmentResult) -> Result<()> {
        // Discard all data
        Ok(())
    }
}
