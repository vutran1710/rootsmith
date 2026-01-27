//! Core RootSmith struct and initialization - no business logic.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommittedRecord {
    pub namespace: Namespace,
    pub key: [u8; 32],
    pub value: Vec<u8>,
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
    pub wasm_host: tokio::sync::Mutex<WasmPluginHost>,

    /// Accumulator for processing records (Merkle, SparseMerkle, or ZK).
    pub accumulator: Arc<tokio::sync::Mutex<AccumulatorVariant>>,
}

impl RootSmith {
    /// Initialize RootSmith with default Noop implementations.
    pub async fn initialize(config: Config) -> Self {
        let storage = Storage::open(&config.storage_path).expect("Failed to open storage");
        tracing::info!("Storage opened at: {}", config.storage_path);

        let upstream = UpstreamVariant::new(config.upstream.clone());
        let downstream = DownstreamVariant::new(config.downstream.clone());
        let archive_storage = ArchiveVariant::new(config.archive.clone());
        let wasm_host =
            WasmPluginHost::load(&config.plugin_path, None).expect("Failed to load WASM plugins");
        let accumulator = AccumulatorVariant::new(&config.accumulator);

        Self {
            upstream,
            downstream,
            archive_storage,
            config,
            storage: Arc::new(tokio::sync::Mutex::new(storage)),
            epoch_start_ts: Arc::new(tokio::sync::Mutex::new(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("System time before UNIX_EPOCH")
                    .as_secs(),
            )),
            active_namespaces: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            committed_records: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            wasm_host: tokio::sync::Mutex::new(wasm_host),
            accumulator: Arc::new(tokio::sync::Mutex::new(accumulator)),
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        // Main run loop placeholder
        Ok(())
    }
}
