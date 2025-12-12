use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use crate::types::{BatchCommitmentMeta, Commitment, IncomingRecord, Namespace, StoredProof};
use crate::config::BaseConfig;
use crate::traits::{Accumulator, CommitmentRegistry, ProofRegistry, UpstreamConnector};

/// Per-namespace in-memory batch state.
struct NamespaceBatchState {
    /// Accumulator instance for this namespace.
    accumulator: Box<dyn Accumulator>,
    /// Number of leaves in this batch.
    leaf_count: u64,
    /// Start time of the current batch window (unix seconds).
    batch_start_ts: u64,
}

/// Main application orchestrator.
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

    /// In-memory state keyed by namespace.
    batches: HashMap<Namespace, NamespaceBatchState>,
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
    ) -> Self {
        Self {
            upstream,
            commitment_registry,
            proof_registry,
            config,
            accumulator_factory,
            batches: HashMap::new(),
        }
    }

    /// Main run loop (high-level sketch; concrete implementation
    /// will depend on how `UpstreamConnector` delivers records).
    pub fn run(&mut self) -> Result<()> {
        // 1. open upstream
        self.upstream.open()?;

        // 2. loop over incoming records (pseudo code):
        // while let Some(record) = self.next_incoming_record()? {
        //     self.handle_record(record)?;
        // }

        // 3. on shutdown: flush/commit remaining batches if needed
        // for (ns, state) in self.batches.iter_mut() {
        //     self.finalize_namespace_batch(ns, state)?;
        // }

        // 4. close upstream
        self.upstream.close()?;
        Ok(())
    }

    /// Handle a single incoming record:
    /// - route to per-namespace accumulator,
    /// - check batch window / size and finalize if needed.
    pub fn handle_record(&mut self, record: IncomingRecord) -> Result<()> {
        let namespace = record.namespace;
        let now = record.timestamp;

        // Check if we need to finalize before processing
        let should_finalize_time = if let Some(state) = self.batches.get(&namespace) {
            self.config.auto_commit && now >= state.batch_start_ts + self.config.batch_interval_secs
        } else {
            false
        };

        if should_finalize_time {
            if let Some(state) = self.batches.get_mut(&namespace) {
                let committed_at = state.batch_start_ts + self.config.batch_interval_secs;
                
                // Perform finalization inline to avoid borrow issues
                if state.leaf_count > 0 {
                    let root = state.accumulator.build_root()?;
                    let commitment = Commitment {
                        namespace,
                        root: root.clone(),
                        committed_at,
                    };
                    let meta = BatchCommitmentMeta {
                        commitment,
                        leaf_count: state.leaf_count,
                    };
                    self.commitment_registry.commit(&meta)?;
                    state.accumulator.flush()?;
                }
                
                state.accumulator = (self.accumulator_factory)();
                state.leaf_count = 0;
                state.batch_start_ts = now;
            }
        }

        // Get or create state
        let state = self
            .batches
            .entry(namespace)
            .or_insert_with(|| NamespaceBatchState {
                accumulator: (self.accumulator_factory)(),
                leaf_count: 0,
                batch_start_ts: now,
            });

        // Add leaf to accumulator
        state.accumulator.put(record.key, record.value)?;
        state.leaf_count += 1;

        // Check if we need to finalize due to size
        let should_finalize_size = if let Some(max_leaves) = self.config.max_batch_leaves {
            state.leaf_count >= max_leaves
        } else {
            false
        };

        if should_finalize_size {
            let committed_at = state.batch_start_ts + self.config.batch_interval_secs;
            
            // Perform finalization inline to avoid borrow issues
            if state.leaf_count > 0 {
                let root = state.accumulator.build_root()?;
                let commitment = Commitment {
                    namespace,
                    root: root.clone(),
                    committed_at,
                };
                let meta = BatchCommitmentMeta {
                    commitment,
                    leaf_count: state.leaf_count,
                };
                self.commitment_registry.commit(&meta)?;
                state.accumulator.flush()?;
            }
            
            state.accumulator = (self.accumulator_factory)();
            state.leaf_count = 0;
            state.batch_start_ts = now;
        }

        Ok(())
    }

    /// Finalize a batch for a given namespace (public API).
    /// - build root from accumulator,
    /// - build commitment + meta,
    /// - optionally generate proofs and save via proof registry,
    /// - commit meta via commitment registry,
    /// - flush accumulator state.
    pub fn finalize_namespace_batch(
        &mut self,
        namespace: &Namespace,
    ) -> Result<()> {
        if let Some(state) = self.batches.get_mut(namespace) {
            if state.leaf_count == 0 {
                return Ok(());
            }

            let committed_at = state.batch_start_ts + self.config.batch_interval_secs;
            let root = state.accumulator.build_root()?;

            let commitment = Commitment {
                namespace: *namespace,
                root: root.clone(),
                committed_at,
            };

            let meta = BatchCommitmentMeta {
                commitment,
                leaf_count: state.leaf_count,
            };

            // Here is where a concrete implementation may:
            // - reconstruct per-key proofs from the accumulator,
            // - serialize them as `StoredProof`,
            // - call `self.proof_registry.save_proofs(&proofs)`.

            // For now, assume proofs are not generated here (or done elsewhere).
            // Example empty proofs vector:
            let proofs: Vec<StoredProof> = Vec::new();

            // Save commitment
            self.commitment_registry.commit(&meta)?;

            // Save proofs (if any)
            if !proofs.is_empty() {
                self.proof_registry.save_proofs(&proofs)?;
            }

            // Reset accumulator state for this namespace
            state.accumulator.flush()?;
        }

        Ok(())
    }
}
