//! Core RootSmith struct and initialization - no business logic.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tracing::info;

use crate::archive::ArchiveStorageVariant;
use crate::config::BaseConfig;
use crate::downstream::DownstreamVariant;
use crate::proof_delivery::ProofDeliveryVariant;
use crate::storage::Storage;
use crate::types::Namespace;
use crate::types::Value32;
use crate::upstream::UpstreamVariant;

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

    /// Proof delivery implementation.
    pub proof_delivery: ProofDeliveryVariant,

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
}

impl RootSmith {
    /// Create a new RootSmith.
    pub fn new(
        upstream: UpstreamVariant,
        downstream: DownstreamVariant,
        proof_delivery: ProofDeliveryVariant,
        archive_storage: ArchiveStorageVariant,
        config: BaseConfig,
        storage: Storage,
    ) -> Self {
        let now = crate::rootsmith::tasks::now_secs();

        Self {
            upstream,
            downstream,
            proof_delivery,
            archive_storage,
            config,
            storage: Arc::new(tokio::sync::Mutex::new(storage)),
            epoch_start_ts: Arc::new(tokio::sync::Mutex::new(now)),
            active_namespaces: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            committed_records: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    /// Initialize RootSmith with default Noop implementations.
    pub async fn initialize(config: BaseConfig) -> Result<Self> {
        use crate::archive::ArchiveStorageVariant;
        use crate::archive::NoopArchive;
        use crate::downstream::BlackholeDownstream;
        use crate::downstream::DownstreamVariant;
        use crate::proof_delivery::NoopDelivery;
        use crate::proof_delivery::ProofDeliveryVariant;
        use crate::upstream::NoopUpstream;

        let storage = Storage::open(&config.storage_path)?;
        info!("Storage opened at: {}", config.storage_path);

        let upstream = UpstreamVariant::Noop(NoopUpstream);
        let downstream = DownstreamVariant::Blackhole(BlackholeDownstream::new());
        let proof_delivery = ProofDeliveryVariant::Noop(NoopDelivery);
        let archive_storage = ArchiveStorageVariant::Noop(NoopArchive);

        Ok(Self::new(
            upstream,
            downstream,
            proof_delivery,
            archive_storage,
            config,
            storage,
        ))
    }
}
