//! Session notes types for tracking task completion documentation.
//!
//! This module provides types for generating and managing session notes,
//! which are markdown documents summarizing what was accomplished during
//! a task. Notes are designed for human review (not AI context) and include
//! file changes, verification results, and optional learnings extraction.
//!
//! ## Overview
//!
//! The session notes system supports:
//! - Automatic note generation on task completion
//! - Structured data storage (changes, verification results)
//! - Optional AI-generated summaries
//! - Learnings extraction for CLAUDE.md suggestions
//!
//! ## Example
//!
//! ```
//! use codirigent_core::session_notes::{
//!     SessionNotesConfig, SummaryMode, SessionNote, CompletionStatus,
//!     Learning, LearningCategory,
//! };
//! use codirigent_core::{SessionId, TaskId};
//!
//! // Configure session notes
//! let config = SessionNotesConfig::default();
//! assert!(config.enabled);
//! assert_eq!(config.summary_mode, SummaryMode::Manual);
//!
//! // Create a learning
//! let learning = Learning {
//!     category: LearningCategory::Preference,
//!     content: "Use jose instead of jsonwebtoken for ESM compatibility".to_string(),
//!     suggested_for_claude_md: true,
//! };
//! assert!(learning.suggested_for_claude_md);
//! ```

use crate::change_summary::ChangeSummary;
use crate::types::{SessionId, TaskId};
use crate::verification::VerificationStatus;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Configuration for session notes generation.
///
/// Controls whether notes are generated, what summary mode to use,
/// and where to store the generated notes.
///
/// # Example
///
/// ```
/// use codirigent_core::session_notes::{SessionNotesConfig, SummaryMode};
/// use std::path::PathBuf;
///
/// let config = SessionNotesConfig {
///     enabled: true,
///     summary_mode: SummaryMode::Auto,
///     structured_data_only: false,
///     output_dir: Some(PathBuf::from(".codirigent/notes")),
/// };
/// assert!(!config.structured_data_only);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionNotesConfig {
    /// Whether session notes are enabled.
    pub enabled: bool,
    /// Summary generation mode.
    pub summary_mode: SummaryMode,
    /// Whether to only include structured data (no AI summary).
    pub structured_data_only: bool,
    /// Output directory for notes.
    pub output_dir: Option<PathBuf>,
}

impl Default for SessionNotesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            summary_mode: SummaryMode::Manual,
            structured_data_only: true,
            output_dir: None, // Uses .codirigent/sessions/ by default
        }
    }
}

/// Mode for generating summary content.
///
/// Controls whether AI-generated summaries are created automatically,
/// on user request, or never.
///
/// # Example
///
/// ```
/// use codirigent_core::session_notes::SummaryMode;
///
/// assert_eq!(SummaryMode::default(), SummaryMode::Manual);
///
/// let mode = SummaryMode::Auto;
/// let json = serde_json::to_string(&mode).unwrap();
/// assert_eq!(json, "\"Auto\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SummaryMode {
    /// Automatically generate summary using Claude (costs tokens).
    Auto,
    /// Only generate on user request (default, zero tokens).
    #[default]
    Manual,
    /// Never generate summaries.
    None,
}

impl std::fmt::Display for SummaryMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SummaryMode::Auto => write!(f, "Auto"),
            SummaryMode::Manual => write!(f, "Manual"),
            SummaryMode::None => write!(f, "None"),
        }
    }
}

/// Completion status of a task.
///
/// Represents the final state when a task completes and notes are generated.
///
/// # Example
///
/// ```
/// use codirigent_core::session_notes::CompletionStatus;
///
/// let status = CompletionStatus::Completed;
/// let json = serde_json::to_string(&status).unwrap();
/// assert_eq!(json, "\"Completed\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompletionStatus {
    /// Task completed successfully.
    Completed,
    /// Task failed and was abandoned.
    Failed,
    /// Task was blocked and requires human intervention.
    Blocked,
    /// Task was manually stopped.
    Stopped,
}

impl std::fmt::Display for CompletionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompletionStatus::Completed => write!(f, "Completed"),
            CompletionStatus::Failed => write!(f, "Failed"),
            CompletionStatus::Blocked => write!(f, "Blocked"),
            CompletionStatus::Stopped => write!(f, "Stopped"),
        }
    }
}

/// A session note document.
///
/// Contains all information about a completed task session including
/// file changes, verification results, summaries, and learnings.
///
/// # Example
///
/// ```
/// use codirigent_core::session_notes::{SessionNote, CompletionStatus};
/// use codirigent_core::{SessionId, TaskId};
///
/// let note = SessionNote {
///     task_id: TaskId::from("task-001"),
///     title: "Refactor Auth Module".to_string(),
///     session_id: SessionId(1),
///     duration_minutes: 45,
///     completion_status: CompletionStatus::Completed,
///     change_summary: None,
///     verification: None,
///     summary: None,
///     learnings: vec![],
///     generated_at: chrono::Utc::now(),
/// };
/// assert_eq!(note.duration_minutes, 45);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionNote {
    /// Task this note is for.
    pub task_id: TaskId,
    /// Title for the note.
    pub title: String,
    /// Session that worked on this task.
    pub session_id: SessionId,
    /// Duration in minutes.
    pub duration_minutes: u32,
    /// Completion status.
    pub completion_status: CompletionStatus,
    /// Change summary (files modified, etc.).
    pub change_summary: Option<ChangeSummary>,
    /// Verification results.
    pub verification: Option<VerificationStatus>,
    /// AI-generated summary (if auto mode).
    pub summary: Option<String>,
    /// Extracted learnings.
    pub learnings: Vec<Learning>,
    /// When the note was generated.
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

impl SessionNote {
    /// Create a new session note with required fields.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task this note documents
    /// * `title` - Human-readable title for the note
    /// * `session_id` - The session that completed the task
    /// * `duration_minutes` - How long the task took
    /// * `completion_status` - Final status of the task
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::session_notes::{SessionNote, CompletionStatus};
    /// use codirigent_core::{SessionId, TaskId};
    ///
    /// let note = SessionNote::new(
    ///     TaskId::from("task-001"),
    ///     "Test Task".to_string(),
    ///     SessionId(1),
    ///     30,
    ///     CompletionStatus::Completed,
    /// );
    /// assert!(note.change_summary.is_none());
    /// assert!(note.learnings.is_empty());
    /// ```
    pub fn new(
        task_id: TaskId,
        title: String,
        session_id: SessionId,
        duration_minutes: u32,
        completion_status: CompletionStatus,
    ) -> Self {
        Self {
            task_id,
            title,
            session_id,
            duration_minutes,
            completion_status,
            change_summary: None,
            verification: None,
            summary: None,
            learnings: vec![],
            generated_at: chrono::Utc::now(),
        }
    }

    /// Set the change summary for this note.
    pub fn with_change_summary(mut self, summary: ChangeSummary) -> Self {
        self.change_summary = Some(summary);
        self
    }

    /// Set the verification status for this note.
    pub fn with_verification(mut self, verification: VerificationStatus) -> Self {
        self.verification = Some(verification);
        self
    }

    /// Set the AI-generated summary for this note.
    pub fn with_summary(mut self, summary: String) -> Self {
        self.summary = Some(summary);
        self
    }

    /// Add learnings to this note.
    pub fn with_learnings(mut self, learnings: Vec<Learning>) -> Self {
        self.learnings = learnings;
        self
    }
}

/// A learning extracted from the session.
///
/// Represents a piece of knowledge gained during the task that may
/// be useful to document in CLAUDE.md for future sessions.
///
/// # Example
///
/// ```
/// use codirigent_core::session_notes::{Learning, LearningCategory};
///
/// let learning = Learning {
///     category: LearningCategory::Preference,
///     content: "Use jose instead of jsonwebtoken for ESM compatibility".to_string(),
///     suggested_for_claude_md: true,
/// };
/// assert!(learning.suggested_for_claude_md);
/// assert_eq!(learning.category, LearningCategory::Preference);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Learning {
    /// Category of the learning.
    pub category: LearningCategory,
    /// The learning content.
    pub content: String,
    /// Whether this was suggested for CLAUDE.md.
    pub suggested_for_claude_md: bool,
}

impl Learning {
    /// Create a new learning.
    ///
    /// # Arguments
    ///
    /// * `category` - The type of learning
    /// * `content` - The learning content
    /// * `suggested_for_claude_md` - Whether to suggest adding to CLAUDE.md
    pub fn new(
        category: LearningCategory,
        content: impl Into<String>,
        suggested_for_claude_md: bool,
    ) -> Self {
        Self {
            category,
            content: content.into(),
            suggested_for_claude_md,
        }
    }
}

/// Category of a learning.
///
/// Classifies the type of knowledge gained during a session to help
/// organize and filter learnings.
///
/// # Example
///
/// ```
/// use codirigent_core::session_notes::LearningCategory;
///
/// let category = LearningCategory::AntiPattern;
/// let json = serde_json::to_string(&category).unwrap();
/// assert_eq!(json, "\"AntiPattern\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LearningCategory {
    /// Library/tool preference.
    Preference,
    /// Pattern to follow.
    Pattern,
    /// Anti-pattern to avoid.
    AntiPattern,
    /// Gotcha or edge case.
    Gotcha,
    /// Project convention.
    Convention,
}

impl std::fmt::Display for LearningCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LearningCategory::Preference => write!(f, "Preference"),
            LearningCategory::Pattern => write!(f, "Pattern"),
            LearningCategory::AntiPattern => write!(f, "AntiPattern"),
            LearningCategory::Gotcha => write!(f, "Gotcha"),
            LearningCategory::Convention => write!(f, "Convention"),
        }
    }
}

/// Parameters for generating a session note.
///
/// Groups all the parameters needed to generate a session note
/// into a single struct to avoid functions with too many arguments.
///
/// # Example
///
/// ```
/// use codirigent_core::session_notes::{GenerateNoteParams, CompletionStatus};
/// use codirigent_core::{SessionId, TaskId};
///
/// let params = GenerateNoteParams {
///     task_id: TaskId::from("task-001"),
///     session_id: SessionId(1),
///     title: "Fix authentication bug".to_string(),
///     duration_minutes: 45,
///     completion_status: CompletionStatus::Completed,
///     change_summary: None,
///     verification: None,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct GenerateNoteParams {
    /// The task to document
    pub task_id: TaskId,
    /// The session that completed the task
    pub session_id: SessionId,
    /// Human-readable title
    pub title: String,
    /// How long the task took
    pub duration_minutes: u32,
    /// Final status
    pub completion_status: CompletionStatus,
    /// Optional file changes
    pub change_summary: Option<ChangeSummary>,
    /// Optional verification results
    pub verification: Option<VerificationStatus>,
}

/// Trait for generating session notes.
///
/// Implementors create session note documents, render them to markdown,
/// and save them to disk.
///
/// # Example
///
/// ```
/// use codirigent_core::session_notes::{
///     NotesGenerator, SessionNote, CompletionStatus, GenerateNoteParams,
/// };
/// use codirigent_core::{SessionId, TaskId, ChangeSummary};
/// use codirigent_core::verification::VerificationStatus;
/// use std::path::Path;
///
/// // Trait is typically implemented by dirigent-verification crate
/// fn example_usage<T: NotesGenerator>(generator: &T) {
///     let params = GenerateNoteParams {
///         task_id: TaskId::from("task-001"),
///         session_id: SessionId(1),
///         title: "Test Task".to_string(),
///         duration_minutes: 30,
///         completion_status: CompletionStatus::Completed,
///         change_summary: None,
///         verification: None,
///     };
///     let note = generator.generate(params).unwrap();
///
///     let markdown = generator.render_markdown(&note);
///     println!("Note:\n{}", markdown);
/// }
/// ```
pub trait NotesGenerator: Send + Sync {
    /// Generate a session note.
    ///
    /// # Arguments
    ///
    /// * `params` - Parameters for note generation
    ///
    /// # Returns
    ///
    /// A session note ready for rendering or saving.
    fn generate(&self, params: GenerateNoteParams) -> anyhow::Result<SessionNote>;

    /// Render a session note to markdown.
    ///
    /// # Arguments
    ///
    /// * `note` - The note to render
    ///
    /// # Returns
    ///
    /// Markdown-formatted string.
    fn render_markdown(&self, note: &SessionNote) -> String;

    /// Save a session note to disk.
    ///
    /// # Arguments
    ///
    /// * `note` - The note to save
    /// * `output_dir` - Directory to save the note in
    ///
    /// # Returns
    ///
    /// Path to the saved file.
    fn save(&self, note: &SessionNote, output_dir: &Path) -> anyhow::Result<PathBuf>;
}

/// Trait for extracting learnings from session output.
///
/// Implementors analyze session output text to identify patterns
/// that represent learnings (preferences, gotchas, conventions, etc.).
///
/// # Example
///
/// ```
/// use codirigent_core::session_notes::{LearningsExtractor, Learning};
///
/// // Trait is typically implemented by dirigent-verification crate
/// fn example_usage<T: LearningsExtractor>(extractor: &T) {
///     let output = "I recommend using jose instead of jsonwebtoken";
///     let learnings = extractor.extract(output);
///     for learning in learnings {
///         println!("Found: {}", learning.content);
///     }
/// }
/// ```
pub trait LearningsExtractor: Send + Sync {
    /// Extract learnings from session output.
    ///
    /// # Arguments
    ///
    /// * `output` - Session output text to analyze
    ///
    /// # Returns
    ///
    /// List of extracted learnings.
    fn extract(&self, output: &str) -> Vec<Learning>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // SessionNotesConfig tests
    #[test]
    fn test_session_notes_config_default() {
        let config = SessionNotesConfig::default();
        assert!(config.enabled);
        assert_eq!(config.summary_mode, SummaryMode::Manual);
        assert!(config.structured_data_only);
        assert!(config.output_dir.is_none());
    }

    #[test]
    fn test_session_notes_config_custom() {
        let config = SessionNotesConfig {
            enabled: false,
            summary_mode: SummaryMode::Auto,
            structured_data_only: false,
            output_dir: Some(PathBuf::from(".codirigent/notes")),
        };
        assert!(!config.enabled);
        assert_eq!(config.summary_mode, SummaryMode::Auto);
        assert!(!config.structured_data_only);
        assert_eq!(config.output_dir, Some(PathBuf::from(".codirigent/notes")));
    }

    #[test]
    fn test_config_serialization() {
        let config = SessionNotesConfig {
            enabled: true,
            summary_mode: SummaryMode::Auto,
            structured_data_only: false,
            output_dir: Some(PathBuf::from(".codirigent/notes")),
        };
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: SessionNotesConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.summary_mode, parsed.summary_mode);
        assert_eq!(config.enabled, parsed.enabled);
        assert_eq!(config.structured_data_only, parsed.structured_data_only);
        assert_eq!(config.output_dir, parsed.output_dir);
    }

    #[test]
    fn test_config_equality() {
        let config1 = SessionNotesConfig::default();
        let config2 = SessionNotesConfig::default();
        let config3 = SessionNotesConfig {
            enabled: false,
            ..Default::default()
        };
        assert_eq!(config1, config2);
        assert_ne!(config1, config3);
    }

    #[test]
    fn test_config_clone() {
        let config = SessionNotesConfig::default();
        let cloned = config.clone();
        assert_eq!(config, cloned);
    }

    #[test]
    fn test_config_debug() {
        let config = SessionNotesConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("SessionNotesConfig"));
        assert!(debug_str.contains("enabled"));
    }

    // SummaryMode tests
    #[test]
    fn test_summary_mode_default() {
        assert_eq!(SummaryMode::default(), SummaryMode::Manual);
    }

    #[test]
    fn test_summary_mode_serialization() {
        let modes = [SummaryMode::Auto, SummaryMode::Manual, SummaryMode::None];
        for mode in modes {
            let json = serde_json::to_string(&mode).unwrap();
            let parsed: SummaryMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, parsed);
        }
    }

    #[test]
    fn test_summary_mode_display() {
        assert_eq!(format!("{}", SummaryMode::Auto), "Auto");
        assert_eq!(format!("{}", SummaryMode::Manual), "Manual");
        assert_eq!(format!("{}", SummaryMode::None), "None");
    }

    #[test]
    fn test_summary_mode_equality() {
        assert_eq!(SummaryMode::Auto, SummaryMode::Auto);
        assert_ne!(SummaryMode::Auto, SummaryMode::Manual);
        assert_ne!(SummaryMode::Manual, SummaryMode::None);
    }

    #[test]
    fn test_summary_mode_clone_copy() {
        let mode = SummaryMode::Auto;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    // CompletionStatus tests
    #[test]
    fn test_completion_status_variants() {
        assert!(matches!(
            CompletionStatus::Completed,
            CompletionStatus::Completed
        ));
        assert!(matches!(CompletionStatus::Failed, CompletionStatus::Failed));
        assert!(matches!(
            CompletionStatus::Blocked,
            CompletionStatus::Blocked
        ));
        assert!(matches!(
            CompletionStatus::Stopped,
            CompletionStatus::Stopped
        ));
    }

    #[test]
    fn test_completion_status_serialization() {
        let statuses = [
            CompletionStatus::Completed,
            CompletionStatus::Failed,
            CompletionStatus::Blocked,
            CompletionStatus::Stopped,
        ];
        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: CompletionStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn test_completion_status_display() {
        assert_eq!(format!("{}", CompletionStatus::Completed), "Completed");
        assert_eq!(format!("{}", CompletionStatus::Failed), "Failed");
        assert_eq!(format!("{}", CompletionStatus::Blocked), "Blocked");
        assert_eq!(format!("{}", CompletionStatus::Stopped), "Stopped");
    }

    #[test]
    fn test_completion_status_equality() {
        assert_eq!(CompletionStatus::Completed, CompletionStatus::Completed);
        assert_ne!(CompletionStatus::Completed, CompletionStatus::Failed);
    }

    #[test]
    fn test_completion_status_clone_copy() {
        let status = CompletionStatus::Completed;
        let cloned = status;
        assert_eq!(status, cloned);
    }

    // Learning tests
    #[test]
    fn test_learning_creation() {
        let learning = Learning {
            category: LearningCategory::Preference,
            content: "Use jose instead of jsonwebtoken for ESM compatibility".to_string(),
            suggested_for_claude_md: true,
        };
        assert!(learning.suggested_for_claude_md);
        assert_eq!(learning.category, LearningCategory::Preference);
    }

    #[test]
    fn test_learning_new() {
        let learning = Learning::new(
            LearningCategory::Gotcha,
            "API returns null for empty arrays",
            true,
        );
        assert_eq!(learning.category, LearningCategory::Gotcha);
        assert_eq!(learning.content, "API returns null for empty arrays");
        assert!(learning.suggested_for_claude_md);
    }

    #[test]
    fn test_learning_serialization() {
        let learning = Learning {
            category: LearningCategory::Convention,
            content: "All API routes should start with /api/v1".to_string(),
            suggested_for_claude_md: false,
        };
        let json = serde_json::to_string(&learning).unwrap();
        let parsed: Learning = serde_json::from_str(&json).unwrap();
        assert_eq!(learning, parsed);
    }

    #[test]
    fn test_learning_equality() {
        let l1 = Learning::new(LearningCategory::Pattern, "test", true);
        let l2 = Learning::new(LearningCategory::Pattern, "test", true);
        let l3 = Learning::new(LearningCategory::Pattern, "different", true);
        assert_eq!(l1, l2);
        assert_ne!(l1, l3);
    }

    #[test]
    fn test_learning_clone() {
        let learning = Learning::new(LearningCategory::AntiPattern, "test", true);
        let cloned = learning.clone();
        assert_eq!(learning, cloned);
    }

    // LearningCategory tests
    #[test]
    fn test_learning_category_serialization() {
        let category = LearningCategory::AntiPattern;
        let json = serde_json::to_string(&category).unwrap();
        assert_eq!(json, "\"AntiPattern\"");
    }

    #[test]
    fn test_learning_category_all_variants() {
        let categories = [
            LearningCategory::Preference,
            LearningCategory::Pattern,
            LearningCategory::AntiPattern,
            LearningCategory::Gotcha,
            LearningCategory::Convention,
        ];
        for category in categories {
            let json = serde_json::to_string(&category).unwrap();
            let parsed: LearningCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(category, parsed);
        }
    }

    #[test]
    fn test_learning_category_display() {
        assert_eq!(format!("{}", LearningCategory::Preference), "Preference");
        assert_eq!(format!("{}", LearningCategory::Pattern), "Pattern");
        assert_eq!(format!("{}", LearningCategory::AntiPattern), "AntiPattern");
        assert_eq!(format!("{}", LearningCategory::Gotcha), "Gotcha");
        assert_eq!(format!("{}", LearningCategory::Convention), "Convention");
    }

    #[test]
    fn test_learning_category_equality() {
        assert_eq!(LearningCategory::Gotcha, LearningCategory::Gotcha);
        assert_ne!(LearningCategory::Gotcha, LearningCategory::Pattern);
    }

    #[test]
    fn test_learning_category_clone_copy() {
        let category = LearningCategory::Convention;
        let cloned = category;
        assert_eq!(category, cloned);
    }

    // SessionNote tests
    #[test]
    fn test_session_note_creation() {
        let note = SessionNote {
            task_id: TaskId::from("task-001"),
            title: "Refactor Auth Module".to_string(),
            session_id: SessionId(1),
            duration_minutes: 45,
            completion_status: CompletionStatus::Completed,
            change_summary: None,
            verification: None,
            summary: None,
            learnings: vec![],
            generated_at: chrono::Utc::now(),
        };
        assert_eq!(note.duration_minutes, 45);
        assert_eq!(note.task_id, TaskId::from("task-001"));
        assert_eq!(note.completion_status, CompletionStatus::Completed);
    }

    #[test]
    fn test_session_note_new() {
        let note = SessionNote::new(
            TaskId::from("task-001"),
            "Test Task".to_string(),
            SessionId(1),
            30,
            CompletionStatus::Completed,
        );
        assert_eq!(note.duration_minutes, 30);
        assert!(note.change_summary.is_none());
        assert!(note.verification.is_none());
        assert!(note.summary.is_none());
        assert!(note.learnings.is_empty());
    }

    #[test]
    fn test_session_note_with_summary() {
        let note = SessionNote::new(
            TaskId::from("task-001"),
            "Test".to_string(),
            SessionId(1),
            30,
            CompletionStatus::Completed,
        )
        .with_summary("This task refactored the auth module.".to_string());

        assert_eq!(
            note.summary,
            Some("This task refactored the auth module.".to_string())
        );
    }

    #[test]
    fn test_session_note_with_learnings() {
        let learnings = vec![
            Learning::new(LearningCategory::Preference, "Use X over Y", true),
            Learning::new(LearningCategory::Gotcha, "Watch out for Z", false),
        ];
        let note = SessionNote::new(
            TaskId::from("task-001"),
            "Test".to_string(),
            SessionId(1),
            30,
            CompletionStatus::Completed,
        )
        .with_learnings(learnings);

        assert_eq!(note.learnings.len(), 2);
    }

    #[test]
    fn test_session_note_serialization() {
        let note = SessionNote {
            task_id: TaskId::from("task-001"),
            title: "Test Task".to_string(),
            session_id: SessionId(1),
            duration_minutes: 30,
            completion_status: CompletionStatus::Completed,
            change_summary: None,
            verification: None,
            summary: Some("Summary text".to_string()),
            learnings: vec![Learning::new(
                LearningCategory::Pattern,
                "Test pattern",
                true,
            )],
            generated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string_pretty(&note).unwrap();
        let parsed: SessionNote = serde_json::from_str(&json).unwrap();

        assert_eq!(note.task_id, parsed.task_id);
        assert_eq!(note.session_id, parsed.session_id);
        assert_eq!(note.duration_minutes, parsed.duration_minutes);
        assert_eq!(note.completion_status, parsed.completion_status);
        assert_eq!(note.summary, parsed.summary);
        assert_eq!(note.learnings.len(), parsed.learnings.len());
    }

    #[test]
    fn test_session_note_clone() {
        let note = SessionNote::new(
            TaskId::from("task-001"),
            "Test".to_string(),
            SessionId(1),
            30,
            CompletionStatus::Completed,
        );
        let cloned = note.clone();
        assert_eq!(note.task_id, cloned.task_id);
        assert_eq!(note.duration_minutes, cloned.duration_minutes);
    }

    #[test]
    fn test_session_note_debug() {
        let note = SessionNote::new(
            TaskId::from("task-001"),
            "Test".to_string(),
            SessionId(1),
            30,
            CompletionStatus::Completed,
        );
        let debug_str = format!("{:?}", note);
        assert!(debug_str.contains("SessionNote"));
        assert!(debug_str.contains("task-001"));
    }

    #[test]
    fn test_session_note_all_statuses() {
        let statuses = [
            CompletionStatus::Completed,
            CompletionStatus::Failed,
            CompletionStatus::Blocked,
            CompletionStatus::Stopped,
        ];
        for status in statuses {
            let note = SessionNote::new(
                TaskId::from("task"),
                "Test".to_string(),
                SessionId(1),
                10,
                status,
            );
            assert_eq!(note.completion_status, status);
        }
    }
}
