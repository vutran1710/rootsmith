//! Core RootSmith struct and initialization - no business logic.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use tracing::info;

use crate::archiver::ArchiveStorageVariant;
use crate::config::BaseConfig;
use crate::downstream::DownstreamVariant;
use crate::storage::Storage;
use crate::types::Namespace;
use crate::types::Value32;
use crate::upstream::UpstreamVariant;
use crate::wasm_host::{WasmPluginHost, ToStandardData};
use kanal::AsyncSender;
use crate::types::CommitmentResult;
use async_trait::async_trait;

/// Trait for ZK accumulator to avoid circular dependencies
#[async_trait]
pub(crate) trait ZkAccumulatorTrait: Send + Sync {
    async fn commit_trait(
        &mut self,
        records: &[Box<dyn ZKTraitData>],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()>;
}

/// Trait for ZK data to avoid importing ZKTrait directly
pub(crate) trait ZKTraitData: Send + Sync {
    fn namespace(&self) -> [u8; 32];
    fn key(&self) -> [u8; 32];
    fn value(&self) -> [u8; 32];
    fn timestamp(&self) -> u64;
}

/// Epoch phase for the commit cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpochPhase {
    /// Waiting for the next commit window.
    Pending,
    /// Actively committing batches.
    Commit,
}

/// Record tracked in memory for pruning after commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommittedRecord {
    pub namespace: Namespace,
    pub key: [u8; 32],
    pub value: Value32,
    pub timestamp: u64,
}

/// Main application orchestrator with epoch-based architecture.
pub struct RootSmith {
    /// Upstream connector.
    pub upstream: UpstreamVariant,

    /// Downstream handler for commitment results.
    pub downstream: DownstreamVariant,

    /// Archive storage implementation.
    pub archive_storage: ArchiveStorageVariant,

    /// Global/base configuration.
    pub config: BaseConfig,

    /// Persistent storage (RocksDB).
    pub storage: Arc<tokio::sync::Mutex<Storage>>,

    /// Start time of the current epoch (unix seconds).
    pub epoch_start_ts: Arc<tokio::sync::Mutex<u64>>,

    /// Track active namespaces for efficient commit.
    pub active_namespaces: Arc<tokio::sync::Mutex<HashMap<Namespace, bool>>>,

    /// Track committed records for pruning in pending phase.
    pub committed_records: Arc<tokio::sync::Mutex<Vec<CommittedRecord>>>,

    /// WASM plugin host for processing partner data.
    pub wasm_host: Option<Arc<tokio::sync::Mutex<WasmPluginHost>>>,

    /// ZK accumulator for sending data to ZK service.
    zk_accumulator: Option<Arc<tokio::sync::Mutex<Box<dyn ZkAccumulatorTrait>>>>,
}

impl RootSmith {
    /// Create a new RootSmith.
    pub fn new(
        upstream: UpstreamVariant,
        downstream: DownstreamVariant,
        archive_storage: ArchiveStorageVariant,
        config: BaseConfig,
        storage: Storage,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time is before UNIX_EPOCH")
            .as_secs();

        Self {
            upstream,
            downstream,
            archive_storage,
            config,
            storage: Arc::new(tokio::sync::Mutex::new(storage)),
            epoch_start_ts: Arc::new(tokio::sync::Mutex::new(now)),
            active_namespaces: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            committed_records: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            wasm_host: None,
            zk_accumulator: None,
        }
    }

    /// Initialize RootSmith with default Noop implementations.
    pub async fn initialize(config: BaseConfig) -> Result<Self> {
        use crate::archiver::ArchiveStorageVariant;
        use crate::archiver::NoopArchive;
        use crate::downstream::BlackholeDownstream;
        use crate::downstream::DownstreamVariant;
        use crate::upstream::NoopUpstream;

        let storage = Storage::open(&config.storage_path)?;
        info!("Storage opened at: {}", config.storage_path);

        let upstream = UpstreamVariant::Noop(NoopUpstream);
        let downstream = DownstreamVariant::Blackhole(BlackholeDownstream::new());
        let archive_storage = ArchiveStorageVariant::Noop(NoopArchive);

        Ok(Self::new(
            upstream,
            downstream,
            archive_storage,
            config,
            storage,
        ))
    }

    pub fn with_wasm_host(mut self, wasm_host: WasmPluginHost) -> Self {
        self.wasm_host = Some(Arc::new(tokio::sync::Mutex::new(wasm_host)));
        self
    }

    pub fn with_zk_accumulator(mut self, zk_accumulator: impl ZkAccumulatorTrait + 'static) -> Self {
        self.zk_accumulator = Some(Arc::new(tokio::sync::Mutex::new(Box::new(zk_accumulator))));
        self
    }

    pub async fn process_partner_data_to_zk(
        &self,
        partner_data: &[u8],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        let wasm_host = self.wasm_host.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WASM host not configured"))?;
        let zk_accumulator = self.zk_accumulator.as_ref()
            .ok_or_else(|| anyhow::anyhow!("ZK accumulator not configured"))?;

        let mut host_guard = wasm_host.lock().await;
        let output: Box<dyn ToStandardData> = host_guard.process_input(partner_data)?;

        let namespace = output.namespace();
        let key = output.key();
        let value = output.value();
        let timestamp = output.timestamp();

        drop(host_guard);

        struct ZKDataWrapper {
            namespace: [u8; 32],
            key: [u8; 32],
            value: [u8; 32],
            timestamp: u64,
        }

        impl ZKTraitData for ZKDataWrapper {
            fn namespace(&self) -> [u8; 32] { self.namespace }
            fn key(&self) -> [u8; 32] { self.key }
            fn value(&self) -> [u8; 32] { self.value }
            fn timestamp(&self) -> u64 { self.timestamp }
        }

        let zk_data = ZKDataWrapper {
            namespace,
            key,
            value,
            timestamp,
        };

        let trait_objects: Vec<Box<dyn ZKTraitData>> = vec![Box::new(zk_data)];

        let mut zk_guard = zk_accumulator.lock().await;
        zk_guard.commit_trait(&trait_objects, result_tx).await?;

        Ok(())
    }
}
