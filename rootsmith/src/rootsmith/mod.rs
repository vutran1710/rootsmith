//! RootSmith application orchestrator with clean module layout.
//!
//! This module provides:
//! - `core`: RootSmith struct and business logic

pub mod core;

// Re-export main types and structs
pub use core::CommittedRecord;
pub use core::EpochPhase;
pub use core::RootSmith;
