//! Core RootSmith struct and initialization - no business logic.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::accumulator::Accumulator;
use crate::accumulator::AccumulatorVariant;
use crate::archiver::ArchiveVariant;
use crate::config::Config;
use crate::downstream::DownstreamVariant;
use crate::server::{admin, webhook, Webserver};
use crate::storage::{generate_batch_id, BatchStatus, Storable, StorageManager};
use crate::types::CommitmentResult;
use crate::types::Namespace;
use crate::types::UpstreamData;
use crate::upstream::UpstreamConnector;
use crate::upstream::UpstreamVariant;
use crate::wasm_host::{WasmBuilder, WasmPluginHost};

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
    /// HTTP server for admin endpoints.
    pub webserver: Option<Webserver>,

    /// Upstream connector.
    pub upstream: UpstreamVariant,

    /// Downstream handler for commitment results.
    pub downstream: DownstreamVariant,

    /// Archive storage implementation.
    pub archive_storage: ArchiveVariant,

    /// Global/base configuration.
    pub config: Config,

    /// Persistent storage (RocksDB).
    pub storage: Arc<tokio::sync::Mutex<StorageManager>>,

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
    /// Initialize RootSmith with configuration only.
    /// Builds the WASM plugin from source_path specified in config.
    pub async fn initialize(config: Config) -> anyhow::Result<Self> {
        let source_path = PathBuf::from(&config.source_path);
        let output_dir = PathBuf::from(&config.wasm_output_dir);

        if !source_path.exists() {
            anyhow::bail!("Source file not found: {:?}", source_path);
        }

        tracing::info!("Building WASM plugin from: {:?}", source_path);
        let wasm_path = WasmBuilder::build(&source_path, &output_dir)?;
        tracing::info!("Built WASM plugin: {:?}", wasm_path);

        let storage = StorageManager::open(&config.storage_path).expect("Failed to open storage");
        tracing::info!("Storage opened at: {}", config.storage_path);
        let storage = Arc::new(tokio::sync::Mutex::new(storage));

        // Create webhook state with shared storage
        let webhook_state = webhook::WebhookState {
            storage: Arc::clone(&storage),
            client_callback_url: config.client_callback_url.clone(),
        };

        // Register both admin and webhook routes on the same server
        let webserver = Webserver::new(config.http_port)
            .register(admin::routes())
            .register(webhook::routes(webhook_state));

        let upstream = UpstreamVariant::new(config.upstream.clone());
        let downstream = DownstreamVariant::new(config.downstream.clone());
        let archive_storage = ArchiveVariant::new(config.archive.clone());
        let wasm_host = WasmPluginHost::load(wasm_path.to_str().unwrap(), None)
            .expect("Failed to load WASM plugin");
        let accumulator = AccumulatorVariant::new(&config.accumulator);

        Ok(Self {
            webserver: Some(webserver),
            upstream,
            downstream,
            archive_storage,
            config,
            storage,
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
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        if let Some(webserver) = self.webserver.take() {
            tokio::spawn(webserver.run());
        }

        let epoch_duration = Duration::from_secs(self.config.epoch_duration_secs);
        let storage = Arc::clone(&self.storage);
        let accumulator = Arc::clone(&self.accumulator);
        let epoch_start_ts = Arc::clone(&self.epoch_start_ts);
        let active_namespaces = Arc::clone(&self.active_namespaces);

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(epoch_duration).await;

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("System time before UNIX_EPOCH")
                    .as_secs();

                let mut epoch_start = epoch_start_ts.lock().await;
                let mut namespaces_map = active_namespaces.lock().await;

                let namespaces: Vec<Namespace> = namespaces_map.keys().cloned().collect();

                if namespaces.is_empty() {
                    tracing::debug!("No active namespaces, skipping epoch commit");
                    *epoch_start = now;
                    continue;
                }

                let time_start = *epoch_start;
                let time_end = now;

                tracing::info!(
                    "Epoch commit: namespaces={} time_range=[{}, {}]",
                    namespaces.len(),
                    time_start,
                    time_end
                );

                if let Err(e) = Self::do_commit_batch(
                    &storage,
                    &accumulator,
                    namespaces,
                    time_start,
                    time_end,
                )
                .await
                {
                    tracing::error!("Epoch commit failed: {}", e);
                }

                namespaces_map.clear();
                *epoch_start = now;
            }
        });

        let (tx, rx) = kanal::unbounded_async::<UpstreamData>();

        self.upstream.open(tx).await?;
        tracing::info!("Upstream started");

        while let Ok(data) = rx.recv().await {
            tracing::info!("Received: {:?}", data);

            let mut wasm = self.wasm_host.lock().await;
            match wasm.process_to_record(data) {
                Ok(record) => {
                    tracing::info!(
                        "WASM output: ns={} key={}",
                        hex::encode(&record.namespace[..8]),
                        hex::encode(&record.key[..8])
                    );

                    {
                        let mut namespaces = self.active_namespaces.lock().await;
                        namespaces.insert(record.namespace, true);
                    }

                    let storage = self.storage.lock().await;
                    storage.put(Storable::Record(record))?;
                    tracing::info!("Stored in RocksDB");
                }
                Err(e) => tracing::error!("Plugin error: {}", e),
            }
        }

        self.upstream.close().await
    }

    async fn do_commit_batch(
        storage: &Arc<tokio::sync::Mutex<StorageManager>>,
        accumulator: &Arc<tokio::sync::Mutex<AccumulatorVariant>>,
        namespaces: Vec<Namespace>,
        time_start: u64,
        time_end: u64,
    ) -> anyhow::Result<[u8; 16]> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before UNIX_EPOCH")
            .as_secs();

        let batch_id = generate_batch_id(&namespaces, time_start, time_end);
        tracing::info!(
            "Creating batch: id={} namespaces={} time_range=[{}, {}]",
            hex::encode(&batch_id[..8]),
            namespaces.len(),
            time_start,
            time_end
        );

        let storage = storage.lock().await;

        storage
            .batches()
            .create(batch_id, namespaces.clone(), time_start, time_end, now)?;

        let records = storage.batches().get_records(&batch_id, storage.records())?;
        let record_count = records.len() as u64;

        tracing::info!(
            "Batch {} has {} records",
            hex::encode(&batch_id[..8]),
            record_count
        );

        if records.is_empty() {
            tracing::warn!("No records found for batch, skipping commit");
            storage
                .batches()
                .update_status(&batch_id, BatchStatus::Failed, now)?;
            return Ok(batch_id);
        }

        storage
            .batches()
            .update_record_count(&batch_id, record_count, now)?;

        let (result_tx, result_rx) = kanal::unbounded_async::<CommitmentResult>();

        let acc = accumulator.lock().await;
        let job_id = acc.commit(&records, result_tx).await?;
        drop(acc);

        match job_id {
            Some(external_job_id) => {
                tracing::info!(
                    "External job submitted: job_id={} batch_id={}",
                    external_job_id,
                    hex::encode(&batch_id[..8])
                );

                storage
                    .batches()
                    .set_external_job_id(&batch_id, &external_job_id, now)?;
            }
            None => {
                let result = result_rx
                    .recv()
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to receive commitment result: {}", e))?;

                tracing::info!(
                    "Local commitment complete: root={} items={}",
                    hex::encode(&result.commitment.root[..8]),
                    result.item_count
                );

                let commitment_id = storage.commitments().store(
                    result.commitment.root,
                    result.commitment.namespaces,
                    batch_id,
                    time_start,
                    time_end,
                    record_count,
                    result.timestamp,
                    result.proofs,
                )?;

                storage
                    .batches()
                    .mark_committed(&batch_id, &commitment_id, now)?;
            }
        }

        Ok(batch_id)
    }

    pub async fn commit_batch(
        &self,
        namespaces: Vec<Namespace>,
        time_start: u64,
        time_end: u64,
    ) -> anyhow::Result<[u8; 16]> {
        Self::do_commit_batch(&self.storage, &self.accumulator, namespaces, time_start, time_end)
            .await
    }
}
