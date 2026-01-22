use crate::rootsmith::core::ZkAccumulatorTrait;
use crate::rootsmith::core::ZKTraitData;
use crate::zk_service::ZkAccumulator;
use crate::zk_service::ZKTrait;
use crate::types::CommitmentResult;
use kanal::AsyncSender;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
impl ZkAccumulatorTrait for ZkAccumulator {
    async fn commit_trait(
        &mut self,
        records: &[Box<dyn ZKTraitData>],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        struct Adapter {
            namespace: [u8; 32],
            key: [u8; 32],
            value: [u8; 32],
            timestamp: u64,
        }
        
        impl ZKTrait for Adapter {
            fn namespace(&self) -> [u8; 32] { self.namespace }
            fn key(&self) -> [u8; 32] { self.key }
            fn value(&self) -> [u8; 32] { self.value }
            fn timestamp(&self) -> u64 { self.timestamp }
        }
        
        let zk_records: Vec<Box<dyn ZKTrait>> = records.iter()
            .map(|r| {
                let adapter = Adapter {
                    namespace: r.namespace(),
                    key: r.key(),
                    value: r.value(),
                    timestamp: r.timestamp(),
                };
                Box::new(adapter) as Box<dyn ZKTrait>
            })
            .collect();
        
        self.commit_trait(&zk_records, result_tx).await
    }
}
