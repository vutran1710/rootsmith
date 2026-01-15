use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use async_trait::async_trait;

use crate::traits::Downstream;
use crate::types::CommitmentResult;

/// Mock downstream for testing.
pub struct MockDownstream {
    pub calls: Arc<Mutex<Vec<CommitmentResult>>>,
}

impl MockDownstream {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_calls(&self) -> Vec<CommitmentResult> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl Downstream for MockDownstream {
    fn name(&self) -> &'static str {
        "mock"
    }

    async fn handle(&self, result: &CommitmentResult) -> Result<()> {
        self.calls.lock().unwrap().push(result.clone());
        Ok(())
    }
}
