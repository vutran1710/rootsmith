//! Core RootSmith struct and initialization - no business logic.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use tracing::info;

use crate::accumulator::AccumulatorVariant;
use crate::archiver::ArchiveVariant;
use crate::config::Config;
use crate::downstream::DownstreamVariant;
use crate::storage::Storage;
use crate::types::Namespace;
use crate::types::Value32;
use crate::upstream::UpstreamVariant;
use crate::wasm_host::WasmPluginHost;

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
    pub archive_storage: ArchiveVariant,

    /// Global/base configuration.
    pub config: Config,

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

    /// Accumulator for processing records (Merkle, SparseMerkle, or ZK).
    accumulator: Option<Arc<tokio::sync::Mutex<AccumulatorVariant>>>,
}

impl RootSmith {
    /// Create a new RootSmith.
    pub fn new(
        upstream: UpstreamVariant,
        downstream: DownstreamVariant,
        archive_storage: ArchiveVariant,
        config: Config,
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
            accumulator: None,
        }
    }

    /// Initialize RootSmith with default Noop implementations.
    pub async fn initialize(config: Config) -> Result<Self> {
        let storage = Storage::open(&config.storage_path)?;
        info!("Storage opened at: {}", config.storage_path);

        let upstream = UpstreamVariant::new(config.upstream.clone());
        let downstream = DownstreamVariant::new(config.downstream.clone());
        let archive_storage = ArchiveVariant::new(config.archive.clone());

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

    pub fn with_accumulator(mut self, accumulator: AccumulatorVariant) -> Self {
        self.accumulator = Some(Arc::new(tokio::sync::Mutex::new(accumulator)));
        self
    }
}
