use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use crate::storage::Storage;
use crate::types::{BatchCommitmentMeta, Commitment, IncomingRecord, Namespace, StoredProof};
use crate::config::BaseConfig;
use crate::traits::{Accumulator, CommitmentRegistry, ProofRegistry, UpstreamConnector};

/// Main application orchestrator (legacy version).
pub struct App<U, CR, PR>
where
    U: UpstreamConnector,
    CR: CommitmentRegistry,
    PR: ProofRegistry,
{
    /// Upstream connector (could also be a collection).
    pub upstream: U,

    /// Commitment registry implementation.
    pub commitment_registry: CR,

    /// Proof registry implementation.
    pub proof_registry: PR,

    /// Global/base configuration.
    pub config: BaseConfig,

    /// Factory to create new accumulator instances per namespace.
    ///
    /// Note: boxed fn pointer to avoid generics explosion on App.
    pub accumulator_factory: Arc<dyn Fn() -> Box<dyn Accumulator> + Send + Sync>,

    /// Persistent storage (RocksDB).
    pub storage: Storage,
}

impl<U, CR, PR> App<U, CR, PR>
where
    U: UpstreamConnector,
    CR: CommitmentRegistry,
    PR: ProofRegistry,
{
    /// Create a new App with empty in-memory state.
    pub fn new(
        upstream: U,
        commitment_registry: CR,
        proof_registry: PR,
        config: BaseConfig,
        accumulator_factory: Arc<dyn Fn() -> Box<dyn Accumulator> + Send + Sync>,
        storage: Storage,
    ) -> Self {
        Self {
            upstream,
            commitment_registry,
            proof_registry,
            config,
            accumulator_factory,
            storage,
        }
    }

    /// Main run loop with parallel tasks:
    /// 1. Upstream connector receiving data
    /// 2. Handling incoming data (writing to storage)
    /// 3. Epoch clock with commit phases
    pub fn run(&mut self) -> Result<()> {
        // Placeholder implementation - use AppV2 for actual functionality
        tracing::info!("App::run() called - this is the legacy API");
        tracing::info!("For epoch-based architecture, use AppV2");
        Ok(())
    }

    /// Handle a single incoming record by writing it to storage.
    pub fn handle_incoming_record(&mut self, record: IncomingRecord) -> Result<()> {
        self.storage.put(&record)?;
        Ok(())
    }

    /// Finalize a batch for a given namespace (legacy API for compatibility).
    pub fn finalize_namespace_batch(
        &mut self,
        namespace: &Namespace,
    ) -> Result<()> {
        // Scan storage for this namespace
        let filter = crate::storage::StorageScanFilter {
            namespace: *namespace,
            timestamp: None,
        };
        let records = self.storage.scan(&filter)?;
        
        if records.is_empty() {
            return Ok(());
        }

        // Create accumulator
        let mut accumulator = (self.accumulator_factory)();

        // Add all records to accumulator
        for record in &records {
            accumulator.put(record.key, record.value.clone())?;
        }

        // Build root
        let root = accumulator.build_root()?;

        let committed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create commitment
        let commitment = Commitment {
            namespace: *namespace,
            root: root.clone(),
            committed_at,
        };

        let meta = BatchCommitmentMeta {
            commitment,
            leaf_count: records.len() as u64,
        };

        // Save commitment
        self.commitment_registry.commit(&meta)?;

        // Generate and save proofs (placeholder)
        let proofs: Vec<StoredProof> = Vec::new();
        if !proofs.is_empty() {
            self.proof_registry.save_proofs(&proofs)?;
        }

        Ok(())
    }
}
