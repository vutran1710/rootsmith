use anyhow::Result;
use async_trait::async_trait;
use kanal::AsyncSender;

use crate::traits::Accumulator;
use crate::types::{CommitmentResult, RawRecord};
use super::zk_accumulator::{ZkAccumulator, ZKTrait};

pub struct ZkAccumulatorAdapter {
    inner: ZkAccumulator,
    default_namespace: [u8; 32],
}

impl ZkAccumulatorAdapter {
    pub fn new(zk_accumulator: ZkAccumulator) -> Self {
        Self {
            inner: zk_accumulator,
            default_namespace: [0u8; 32],
        }
    }
    
    pub fn with_namespace(mut self, namespace: [u8; 32]) -> Self {
        self.default_namespace = namespace;
        self
    }
}

#[async_trait]
impl Accumulator for ZkAccumulatorAdapter {
    fn id(&self) -> &'static str {
        "zk"
    }

    async fn commit(
        &mut self,
        records: &[RawRecord],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        if records.is_empty() {
            return Err(anyhow::anyhow!("Cannot commit empty records"));
        }

        struct ZKRecordAdapter {
            namespace: [u8; 32],
            key: [u8; 32],
            value: [u8; 32],
            timestamp: u64,
        }

        impl ZKTrait for ZKRecordAdapter {
            fn namespace(&self) -> [u8; 32] { self.namespace }
            fn key(&self) -> [u8; 32] { self.key }
            fn value(&self) -> [u8; 32] { self.value }
            fn timestamp(&self) -> u64 { self.timestamp }
        }

        let zk_records: Vec<Box<dyn ZKTrait>> = records.iter()
            .map(|record| {
                let mut value = [0u8; 32];
                let value_len = record.value.len().min(32);
                value[..value_len].copy_from_slice(&record.value[..value_len]);

                Box::new(ZKRecordAdapter {
                    namespace: self.default_namespace,
                    key: record.key,
                    value,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                }) as Box<dyn ZKTrait>
            })
            .collect();

        self.inner.commit_trait(&zk_records, result_tx).await
    }
}
