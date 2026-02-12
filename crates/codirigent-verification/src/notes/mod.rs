//! Session notes generation and management.
//!
//! This module provides implementations for generating session notes
//! from task completions. Notes are markdown documents designed for
//! human review (not AI context) that summarize what was accomplished.
//!
//! ## Components
//!
//! - [`DefaultNotesGenerator`] - Generates and saves session notes
//! - [`PatternLearningsExtractor`] - Extracts learnings from session output
//!
//! ## Example
//!
//! ```
//! use codirigent_verification::notes::{DefaultNotesGenerator, PatternLearningsExtractor};
//! use codirigent_core::session_notes::{
//!     NotesGenerator, LearningsExtractor, CompletionStatus, GenerateNoteParams,
//! };
//! use codirigent_core::{SessionId, TaskId};
//!
//! // Generate a note
//! let generator = DefaultNotesGenerator::new();
//! let params = GenerateNoteParams {
//!     task_id: TaskId::from("task-001"),
//!     session_id: SessionId(1),
//!     title: "Refactor Auth".to_string(),
//!     duration_minutes: 45,
//!     completion_status: CompletionStatus::Completed,
//!     change_summary: None,
//!     verification: None,
//! };
//! let note = generator.generate(params).unwrap();
//!
//! // Render to markdown
//! let markdown = generator.render_markdown(&note);
//! assert!(markdown.contains("Refactor Auth"));
//!
//! // Extract learnings from output
//! let extractor = PatternLearningsExtractor::new();
//! let learnings = extractor.extract("I recommend using jose instead of jsonwebtoken");
//! ```

pub mod extractor;
pub mod generator;

pub use extractor::PatternLearningsExtractor;
pub use generator::DefaultNotesGenerator;
