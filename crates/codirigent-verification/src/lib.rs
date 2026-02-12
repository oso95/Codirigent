//! Codirigent Verification
//!
//! Verification gate for running tests and checks on completed tasks.
//!
//! This crate provides the implementation of the verification system that
//! validates task completions by running automated checks (tests, linting,
//! type checking, etc.).
//!
//! ## Key Components
//!
//! - [`DefaultDetector`] - Auto-detects project type and verification commands
//! - [`CommandExecutor`] - Executes verification commands with timeout support
//! - [`OutputParser`] - Parses test output into structured results
//! - [`VerificationGate`] - Main implementation of the [`Verifier`] trait
//! - [`GitChangeDetector`] - Detects file changes using git
//! - [`RuleBasedRiskAssessor`] - Assesses risk levels for file changes
//! - [`DefaultFailureFormatter`] - Formats verification failures for session retry
//! - [`PipelineOrchestrator`] - Coordinates the complete verification pipeline
//!
//! ## Example
//!
//! ```no_run
//! use codirigent_verification::{DefaultDetector, VerificationGate};
//! use codirigent_core::VerificationDetector;
//! use std::path::Path;
//!
//! // Auto-detect project type and commands
//! let detector = DefaultDetector::new();
//! let commands = detector.detect(Path::new("/path/to/project"));
//!
//! // Create verification gate with default config
//! let gate = VerificationGate::new();
//! ```
//!
//! ## Change Detection Example
//!
//! ```no_run
//! use codirigent_verification::{GitChangeDetector, RuleBasedRiskAssessor};
//! use codirigent_core::{ChangeDetector, RiskAssessor, SessionId, TaskId};
//! use std::path::Path;
//!
//! // Create a change detector
//! let detector = GitChangeDetector::new();
//!
//! // Detect changes since last commit
//! let changes = detector.detect_changes(Path::new("/path/to/repo"), None).unwrap();
//!
//! // Generate a full change summary
//! let summary = detector.generate_summary(
//!     TaskId::from("task-001"),
//!     SessionId(1),
//!     Path::new("/path/to/repo"),
//!     Some("HEAD~1"),
//! ).unwrap();
//!
//! println!("Risk level: {:?}", summary.risk_assessment.overall_risk);
//! ```
//!
//! ## Pipeline Example
//!
//! ```no_run
//! use codirigent_verification::PipelineOrchestrator;
//! use codirigent_core::pipeline::{VerificationPipeline, ReviewDecision};
//! use codirigent_core::{SessionId, TaskId};
//! use std::path::PathBuf;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Create pipeline orchestrator
//! let pipeline = PipelineOrchestrator::new(PathBuf::from(".codirigent/notes"));
//!
//! // Start pipeline for a completed task
//! pipeline.start(
//!     TaskId::from("task-001"),
//!     SessionId(1),
//!     PathBuf::from("/project"),
//! ).await?;
//!
//! // Submit review
//! pipeline.submit_review(
//!     &TaskId::from("task-001"),
//!     ReviewDecision::Approve,
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! [`Verifier`]: codirigent_core::Verifier

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod change_detector;
pub mod detector;
pub mod executor;
pub mod formatter;
pub mod notes;
pub mod parser;
pub mod pipeline;
pub mod risk_assessor;
pub mod verifier;

pub use change_detector::GitChangeDetector;
pub use detector::DefaultDetector;
pub use executor::{CommandExecutor, ExecutionResult};
pub use formatter::DefaultFailureFormatter;
pub use notes::{DefaultNotesGenerator, PatternLearningsExtractor};
pub use parser::{CargoTestParser, GenericParser, JestParser, OutputParser, PytestParser};
pub use pipeline::PipelineOrchestrator;
pub use risk_assessor::RuleBasedRiskAssessor;
pub use verifier::VerificationGate;
