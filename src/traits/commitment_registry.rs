use crate::types::{BatchCommitmentMeta, Commitment, CommitmentFilterOptions};
use anyhow::Result;
use async_trait::async_trait;

/// Where commitments are recorded (e.g. smart contract, DB, ledger).
#[async_trait]
pub trait CommitmentRegistry: Send + Sync {
    /// Registry name for logging and metrics.
    fn name(&self) -> &'static str;

    /// Persist a single batch commitment meta.
    async fn commit(&self, meta: &BatchCommitmentMeta) -> Result<()>;

    /// Fetch a previous commitment according to filter options.
    ///
    /// Implementations define semantics of `time`, e.g.:
    /// - "latest commitment for `namespace` with `committed_at <= time`".
    async fn get_prev_commitment(
        &self,
        filter: &CommitmentFilterOptions,
    ) -> Result<Option<Commitment>>;
}
