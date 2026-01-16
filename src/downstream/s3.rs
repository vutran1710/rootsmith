use anyhow::Result;
use async_trait::async_trait;

use crate::traits::Downstream;
use crate::types::CommitmentResult;

/// S3 downstream that uploads commitment results to S3.
pub struct S3Downstream {
    bucket: String,
    region: String,
}

impl S3Downstream {
    pub fn new(bucket: String, region: String) -> Self {
        Self { bucket, region }
    }
}

#[async_trait]
impl Downstream for S3Downstream {
    fn name(&self) -> &'static str {
        "s3"
    }

    async fn handle(&self, result: &CommitmentResult) -> Result<()> {
        // TODO: Implement S3 upload logic
        // For now, just log the intent
        tracing::info!(
            "S3 downstream: would upload commitment ({} bytes) to bucket {} in region {}",
            result.commitment.len(),
            self.bucket,
            self.region
        );
        Ok(())
    }
}
