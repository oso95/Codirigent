//! Core service traits for the Codirigent application.
//!
//! This module defines the trait contracts that govern inter-crate
//! communication. Each trait represents a service that can be
//! implemented by different crates.
//!
//! ## Core Traits
//!
//! - [`EventBus`]: Cross-module event publication and subscription
//! - [`SessionManager`]: Session lifecycle management
//! - [`ProcessMonitor`]: Process state monitoring
//! - [`StorageService`]: File-based persistence
//! - [`BroadcastService`]: Broadcast messaging to multiple sessions
//!
//! ## Skill Traits
//!
//! - [`SkillManager`]: Skill management and token budget tracking
//!
//! ## Verification Traits
//!
//! - [`Verifier`]: Run verification checks on task completions
//! - [`VerificationDetector`]: Auto-detect verification commands
//! - [`FailureFormatter`]: Format verification failures for display
//!
//! ## Project Types
//!
//! - [`ProjectType`]: Known project types for auto-detection

pub mod ralph_controller;
mod services;
pub mod skill_manager;
pub mod verifier;

// Re-export core service traits
pub use services::*;

// Re-export skill manager trait
pub use skill_manager::SkillManager;

// Re-export Ralph Loop controller trait
pub use ralph_controller::RalphLoopController;

// Re-export verification traits
pub use verifier::{FailureFormatter, ProjectType, VerificationDetector, Verifier};
