//! RootSmith application orchestrator with clean module layout.
//!
//! This module provides:
//! - `core`: RootSmith struct and business logic (testable "*_once" functions)
//! - `tasks`: Async task orchestration with tokio::spawn
//! - `tests`: Unit tests for business logic

pub mod core;
pub mod tasks;

// Re-export main types and structs
pub use core::{CommittedRecord, EpochPhase, RootSmith};

#[cfg(test)]
mod tests;

