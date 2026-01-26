//! Core RootSmith struct and initialization - no business logic.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use kanal::unbounded_async;
use kanal::AsyncSender;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::accumulator::AccumulatorVariant;
use crate::archiver::ArchiveVariant;
use crate::config::BaseConfig;
use crate::downstream::DownstreamVariant;
use crate::storage::Storage;
use crate::traits::Accumulator;
use crate::traits::Downstream;
use crate::traits::UpstreamConnector;
use crate::types::CommitmentResult;
use crate::types::Namespace;
use crate::types::RawRecord;
use crate::types::UpstreamData;
use crate::types::Value32;
use crate::upstream::UpstreamVariant;
use crate::wasm_host::ToStandardData;
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

    /// Accumulator for processing records (Merkle, SparseMerkle, or ZK).
    accumulator: Option<Arc<tokio::sync::Mutex<AccumulatorVariant>>>,
}

impl RootSmith {
    /// Create a new RootSmith.
    pub fn new(
        upstream: UpstreamVariant,
        downstream: DownstreamVariant,
        archive_storage: ArchiveVariant,
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
            accumulator: None,
        }
    }

    /// Initialize RootSmith with default Noop implementations.
    pub async fn initialize(config: BaseConfig) -> Result<Self> {
        use crate::archiver::ArchiveVariant;
        use crate::archiver::NoopArchive;
        use crate::downstream::BlackholeDownstream;
        use crate::downstream::DownstreamVariant;
        use crate::upstream::NoopUpstream;

        let storage = Storage::open(&config.storage_path)?;
        info!("Storage opened at: {}", config.storage_path);

        let upstream = UpstreamVariant::Noop(NoopUpstream);
        let downstream = DownstreamVariant::Blackhole(BlackholeDownstream::new());
        let archive_storage = ArchiveVariant::Noop(NoopArchive);

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

    pub async fn process_partner_data_to_zk(
        &self,
        partner_data: &[u8],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        let wasm_host = self
            .wasm_host
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("WASM host not configured"))?;
        let accumulator = self
            .accumulator
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Accumulator not configured"))?;

        let mut host_guard = wasm_host.lock().await;
        let output: Box<dyn ToStandardData> = host_guard.process_input(partner_data)?;

        let key = output.key();
        let value = output.value();

        drop(host_guard);

        let raw_record = RawRecord {
            key,
            value: value.to_vec(),
        };

        let mut acc_guard = accumulator.lock().await;
        acc_guard.commit(&[raw_record], result_tx).await?;

        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        let (tx, rx) = unbounded_async();

        info!("Opening upstream connector: {}", self.upstream.name());
        self.upstream.open(tx).await?;

        info!("RootSmith run loop started");
        info!("Waiting for incoming data...");

        while let Ok(data) = rx.recv().await {
            match data {
                UpstreamData::Raw(raw_bytes) => {
                    info!("Received raw data ({} bytes)", raw_bytes.len());

                    let (result_tx, result_rx) = unbounded_async();

                    match self.process_partner_data_to_zk(&raw_bytes, result_tx).await {
                        Ok(_) => {
                            info!("Data processed through WASM → ZK accumulator");

                            match result_rx.recv().await {
                                Ok(result) => {
                                    info!("Commitment result received");
                                    info!("  Commitment: {} bytes", result.commitment.len());
                                    info!("  Committed at: {}", result.committed_at);

                                    if let Err(e) = self.downstream.handle(&result).await {
                                        error!("Failed to send result to downstream: {}", e);
                                    } else {
                                        info!(
                                            "Result sent to downstream: {}",
                                            self.downstream.name()
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to receive commitment result: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to process data: {}", e);
                        }
                    }
                }
                UpstreamData::Record(record) => {
                    warn!("Received IncomingRecord format - not processing (use /ingest/raw for partner data)");
                }
            }
        }

        info!("RootSmith run loop ended");
        Ok(())
    }
}
