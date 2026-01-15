use anyhow::Result;
use async_trait::async_trait;

use crate::types::CommitmentResult;

/// Downstream handler for commitment results.
/// Receives batched commitments for multiple namespaces with optional proofs.
#[async_trait]
pub trait Downstream: Send + Sync {
    /// Downstream name for logging and metrics.
    fn name(&self) -> &'static str;

    /// Handle a commitment result containing multiple namespace commitments and optional proofs.
    async fn handle(&self, result: &CommitmentResult) -> Result<()>;
}
