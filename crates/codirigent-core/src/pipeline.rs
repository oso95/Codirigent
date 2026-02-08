//! Verification pipeline types and traits.
//!
//! This module provides the types and traits for orchestrating the complete
//! verification workflow from task completion through human review to notes
//! generation.
//!
//! ## Overview
//!
//! The verification pipeline coordinates:
//! - Verification checks (tests, lint, type check)
//! - Change summary generation
//! - Human review workflow
//! - Session notes generation
//!
//! ## Pipeline Stages
//!
//! ```text
//! TaskCompleted -> Verifying -> GeneratingChangeSummary -> AwaitingReview
//!                      |                                        |
//!                      v                                  +-----------+
//!              RetryingInSession                          |           |
//!                                                   Approve      RequestChanges
//!                                                         |           |
//!                                                         v           v
//!                                               GeneratingNotes  RetryingInSession
//!                                                         |
//!                                                         v
//!                                                    Complete
//! ```
//!
//! ## Example
//!
//! ```
//! use codirigent_core::pipeline::{PipelineStage, ReviewDecision, PipelineState};
//! use codirigent_core::{SessionId, TaskId};
//! use std::path::PathBuf;
//!
//! let stage = PipelineStage::Verifying;
//! assert_eq!(format!("{}", stage), "Verifying");
//!
//! let decision = ReviewDecision::Approve;
//! assert!(matches!(decision, ReviewDecision::Approve));
//! ```

use crate::change_summary::ChangeSummary;
use crate::session_notes::SessionNote;
use crate::types::{SessionId, TaskId};
use crate::verification::VerificationStatus;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Pipeline stage in the verification workflow.
///
/// Represents the current state of the verification pipeline state machine.
///
/// # Example
///
/// ```
/// use codirigent_core::pipeline::PipelineStage;
///
/// let stage = PipelineStage::TaskCompleted;
/// assert_eq!(format!("{}", stage), "TaskCompleted");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PipelineStage {
    /// Task just completed by session.
    #[default]
    TaskCompleted,
    /// Verification checks running.
    Verifying,
    /// Verification passed, generating change summary.
    GeneratingChangeSummary,
    /// Awaiting human review.
    AwaitingReview,
    /// Human approved, generating notes.
    GeneratingNotes,
    /// Pipeline complete.
    Complete,
    /// Verification failed, sent back to session.
    RetryingInSession,
    /// Blocked, requires human intervention.
    Blocked,
}

impl std::fmt::Display for PipelineStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineStage::TaskCompleted => write!(f, "TaskCompleted"),
            PipelineStage::Verifying => write!(f, "Verifying"),
            PipelineStage::GeneratingChangeSummary => write!(f, "GeneratingChangeSummary"),
            PipelineStage::AwaitingReview => write!(f, "AwaitingReview"),
            PipelineStage::GeneratingNotes => write!(f, "GeneratingNotes"),
            PipelineStage::Complete => write!(f, "Complete"),
            PipelineStage::RetryingInSession => write!(f, "RetryingInSession"),
            PipelineStage::Blocked => write!(f, "Blocked"),
        }
    }
}

/// Result of a human review.
///
/// Represents the decision made by a human reviewer after examining
/// the verification results and change summary.
///
/// # Example
///
/// ```
/// use codirigent_core::pipeline::ReviewDecision;
///
/// let decision = ReviewDecision::RequestChanges {
///     feedback: "Please add error handling".to_string(),
/// };
/// if let ReviewDecision::RequestChanges { feedback } = decision {
///     assert!(feedback.contains("error"));
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReviewDecision {
    /// Approved, proceed to completion.
    Approve,
    /// Rejected, task failed.
    Reject {
        /// Reason for rejection.
        reason: String,
    },
    /// Request changes, send back to session.
    RequestChanges {
        /// Feedback for the session.
        feedback: String,
    },
}

impl std::fmt::Display for ReviewDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReviewDecision::Approve => write!(f, "Approved"),
            ReviewDecision::Reject { reason } => write!(f, "Rejected: {}", reason),
            ReviewDecision::RequestChanges { feedback } => {
                write!(f, "Changes Requested: {}", feedback)
            }
        }
    }
}

/// Current state of the verification pipeline.
///
/// Contains all information about the progress of a task through
/// the verification pipeline, including intermediate results.
///
/// # Example
///
/// ```
/// use codirigent_core::pipeline::{PipelineState, PipelineStage};
/// use codirigent_core::{SessionId, TaskId};
/// use std::path::PathBuf;
///
/// let state = PipelineState::new(
///     TaskId("task-001".to_string()),
///     SessionId(1),
///     PathBuf::from("/project"),
/// );
/// assert_eq!(state.stage, PipelineStage::TaskCompleted);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineState {
    /// Task being processed.
    pub task_id: TaskId,
    /// Session that completed the task.
    pub session_id: SessionId,
    /// Working directory for the task.
    pub working_dir: PathBuf,
    /// Current stage.
    pub stage: PipelineStage,
    /// Verification status (if completed).
    pub verification: Option<VerificationStatus>,
    /// Change summary (if generated).
    pub change_summary: Option<ChangeSummary>,
    /// Human review decision (if made).
    pub review_decision: Option<ReviewDecision>,
    /// Generated session note (if complete).
    pub session_note: Option<SessionNote>,
    /// When the pipeline started.
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// When the pipeline completed.
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Error message if blocked.
    pub error: Option<String>,
}

impl PipelineState {
    /// Create a new pipeline state for a task.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task being processed
    /// * `session_id` - The session that completed the task
    /// * `working_dir` - Working directory for the task
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::pipeline::PipelineState;
    /// use codirigent_core::{SessionId, TaskId};
    /// use std::path::PathBuf;
    ///
    /// let state = PipelineState::new(
    ///     TaskId("task-001".to_string()),
    ///     SessionId(1),
    ///     PathBuf::from("/project"),
    /// );
    /// assert!(state.verification.is_none());
    /// ```
    pub fn new(task_id: TaskId, session_id: SessionId, working_dir: PathBuf) -> Self {
        Self {
            task_id,
            session_id,
            working_dir,
            stage: PipelineStage::TaskCompleted,
            verification: None,
            change_summary: None,
            review_decision: None,
            session_note: None,
            started_at: chrono::Utc::now(),
            completed_at: None,
            error: None,
        }
    }

    /// Check if the pipeline is in a terminal state.
    ///
    /// Terminal states are: Complete, Blocked, RetryingInSession.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::pipeline::{PipelineState, PipelineStage};
    /// use codirigent_core::{SessionId, TaskId};
    /// use std::path::PathBuf;
    ///
    /// let mut state = PipelineState::new(
    ///     TaskId("task-001".to_string()),
    ///     SessionId(1),
    ///     PathBuf::from("/project"),
    /// );
    /// assert!(!state.is_terminal());
    ///
    /// state.stage = PipelineStage::Complete;
    /// assert!(state.is_terminal());
    /// ```
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.stage,
            PipelineStage::Complete | PipelineStage::Blocked | PipelineStage::RetryingInSession
        )
    }

    /// Check if the pipeline is waiting for human input.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::pipeline::{PipelineState, PipelineStage};
    /// use codirigent_core::{SessionId, TaskId};
    /// use std::path::PathBuf;
    ///
    /// let mut state = PipelineState::new(
    ///     TaskId("task-001".to_string()),
    ///     SessionId(1),
    ///     PathBuf::from("/project"),
    /// );
    /// state.stage = PipelineStage::AwaitingReview;
    /// assert!(state.is_awaiting_human());
    /// ```
    pub fn is_awaiting_human(&self) -> bool {
        matches!(
            self.stage,
            PipelineStage::AwaitingReview | PipelineStage::Blocked
        )
    }

    /// Mark the pipeline as complete.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::pipeline::PipelineState;
    /// use codirigent_core::{SessionId, TaskId};
    /// use std::path::PathBuf;
    ///
    /// let mut state = PipelineState::new(
    ///     TaskId("task-001".to_string()),
    ///     SessionId(1),
    ///     PathBuf::from("/project"),
    /// );
    /// state.complete();
    /// assert!(state.completed_at.is_some());
    /// ```
    pub fn complete(&mut self) {
        self.stage = PipelineStage::Complete;
        self.completed_at = Some(chrono::Utc::now());
    }

    /// Mark the pipeline as blocked with an error.
    ///
    /// # Arguments
    ///
    /// * `error` - The error message
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::pipeline::{PipelineState, PipelineStage};
    /// use codirigent_core::{SessionId, TaskId};
    /// use std::path::PathBuf;
    ///
    /// let mut state = PipelineState::new(
    ///     TaskId("task-001".to_string()),
    ///     SessionId(1),
    ///     PathBuf::from("/project"),
    /// );
    /// state.block("Max retries exceeded");
    /// assert_eq!(state.stage, PipelineStage::Blocked);
    /// assert_eq!(state.error, Some("Max retries exceeded".to_string()));
    /// ```
    pub fn block(&mut self, error: impl Into<String>) {
        self.stage = PipelineStage::Blocked;
        self.error = Some(error.into());
    }

    /// Get the duration of the pipeline so far.
    ///
    /// # Returns
    ///
    /// Duration since the pipeline started, or time to completion if complete.
    pub fn duration(&self) -> chrono::Duration {
        let end = self.completed_at.unwrap_or_else(chrono::Utc::now);
        end - self.started_at
    }
}

/// Events emitted by the pipeline.
///
/// These events notify listeners about pipeline progress and state changes.
///
/// # Example
///
/// ```
/// use codirigent_core::pipeline::{PipelineEvent, PipelineStage};
/// use codirigent_core::{SessionId, TaskId};
///
/// let event = PipelineEvent::StageChanged {
///     task_id: TaskId("task-001".to_string()),
///     stage: PipelineStage::Verifying,
/// };
/// assert!(matches!(event, PipelineEvent::StageChanged { .. }));
/// ```
#[derive(Debug, Clone)]
pub enum PipelineEvent {
    /// Pipeline started for a task.
    Started {
        /// The task ID.
        task_id: TaskId,
        /// The session ID.
        session_id: SessionId,
    },
    /// Stage changed.
    StageChanged {
        /// The task ID.
        task_id: TaskId,
        /// The new stage.
        stage: PipelineStage,
    },
    /// Verification completed.
    VerificationCompleted {
        /// The task ID.
        task_id: TaskId,
        /// Whether verification passed.
        passed: bool,
    },
    /// Change summary generated.
    ChangeSummaryGenerated {
        /// The task ID.
        task_id: TaskId,
        /// The generated summary.
        summary: ChangeSummary,
    },
    /// Awaiting human review.
    AwaitingReview {
        /// The task ID.
        task_id: TaskId,
    },
    /// Human reviewed.
    Reviewed {
        /// The task ID.
        task_id: TaskId,
        /// The review decision.
        decision: ReviewDecision,
    },
    /// Session note generated.
    NoteGenerated {
        /// The task ID.
        task_id: TaskId,
        /// Path to the generated note.
        note_path: PathBuf,
    },
    /// Pipeline completed successfully.
    Completed {
        /// The task ID.
        task_id: TaskId,
    },
    /// Pipeline failed.
    Failed {
        /// The task ID.
        task_id: TaskId,
        /// Error message.
        error: String,
    },
    /// Sent back to session for retry.
    SentToSession {
        /// The task ID.
        task_id: TaskId,
        /// Feedback message.
        feedback: String,
    },
}

/// Trait for the verification pipeline.
///
/// Implementors orchestrate the complete verification workflow from
/// task completion through human review to notes generation.
///
/// # Async
///
/// This trait uses `async_trait` because pipeline operations may involve
/// I/O and external command execution.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to allow sharing across threads.
///
/// # Example
///
/// ```ignore
/// use codirigent_core::pipeline::{VerificationPipeline, ReviewDecision};
/// use codirigent_core::{SessionId, TaskId};
/// use std::path::PathBuf;
///
/// async fn example<P: VerificationPipeline>(pipeline: &P) -> anyhow::Result<()> {
///     // Start pipeline for a completed task
///     pipeline.start(
///         TaskId("task-001".to_string()),
///         SessionId(1),
///         PathBuf::from("/project"),
///     ).await?;
///
///     // Later, submit review
///     pipeline.submit_review(
///         &TaskId("task-001".to_string()),
///         ReviewDecision::Approve,
///     ).await?;
///
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait VerificationPipeline: Send + Sync {
    /// Start the pipeline for a completed task.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task to process
    /// * `session_id` - The session that completed the task
    /// * `working_dir` - Working directory for the task
    ///
    /// # Returns
    ///
    /// Result indicating success or failure of starting the pipeline.
    async fn start(
        &self,
        task_id: TaskId,
        session_id: SessionId,
        working_dir: PathBuf,
    ) -> Result<()>;

    /// Get current pipeline state.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task to get state for
    ///
    /// # Returns
    ///
    /// The current pipeline state, if it exists.
    fn get_state(&self, task_id: &TaskId) -> Option<PipelineState>;

    /// Submit human review decision.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task to review
    /// * `decision` - The review decision
    ///
    /// # Returns
    ///
    /// Result indicating success or failure.
    async fn submit_review(&self, task_id: &TaskId, decision: ReviewDecision) -> Result<()>;

    /// Skip verification and go directly to review.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task to skip verification for
    ///
    /// # Returns
    ///
    /// Result indicating success or failure.
    async fn skip_verification(&self, task_id: &TaskId) -> Result<()>;

    /// Cancel the pipeline.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task to cancel
    ///
    /// # Returns
    ///
    /// Result indicating success or failure.
    fn cancel(&self, task_id: &TaskId) -> Result<()>;
}

/// Format failure message to send back to session.
///
/// Implementations convert verification failures and review feedback into
/// human-readable messages that can be sent back to an AI session for fixing.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to allow sharing across threads.
///
/// # Example
///
/// ```
/// use codirigent_core::pipeline::FailureMessageFormatter;
/// use codirigent_core::verification::{VerificationStatus, VerificationState};
/// use codirigent_core::{SessionId, TaskId};
///
/// struct MyFormatter;
///
/// impl FailureMessageFormatter for MyFormatter {
///     fn format_verification_failure(&self, status: &VerificationStatus) -> String {
///         format!("Verification failed with {} results", status.results.len())
///     }
///
///     fn format_review_feedback(&self, feedback: &str) -> String {
///         format!("Review feedback: {}", feedback)
///     }
/// }
/// ```
pub trait FailureMessageFormatter: Send + Sync {
    /// Format verification failures for session retry.
    ///
    /// # Arguments
    ///
    /// * `status` - The verification status containing failures
    ///
    /// # Returns
    ///
    /// A formatted string describing the failures.
    fn format_verification_failure(&self, status: &VerificationStatus) -> String;

    /// Format review feedback for session retry.
    ///
    /// # Arguments
    ///
    /// * `feedback` - The reviewer's feedback
    ///
    /// # Returns
    ///
    /// A formatted string with the feedback.
    fn format_review_feedback(&self, feedback: &str) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    // PipelineStage tests

    #[test]
    fn test_pipeline_stage_variants() {
        let stages = [
            PipelineStage::TaskCompleted,
            PipelineStage::Verifying,
            PipelineStage::GeneratingChangeSummary,
            PipelineStage::AwaitingReview,
            PipelineStage::GeneratingNotes,
            PipelineStage::Complete,
            PipelineStage::RetryingInSession,
            PipelineStage::Blocked,
        ];
        assert_eq!(stages.len(), 8);
    }

    #[test]
    fn test_pipeline_stage_default() {
        let stage = PipelineStage::default();
        assert_eq!(stage, PipelineStage::TaskCompleted);
    }

    #[test]
    fn test_pipeline_stage_display() {
        assert_eq!(format!("{}", PipelineStage::TaskCompleted), "TaskCompleted");
        assert_eq!(format!("{}", PipelineStage::Verifying), "Verifying");
        assert_eq!(
            format!("{}", PipelineStage::GeneratingChangeSummary),
            "GeneratingChangeSummary"
        );
        assert_eq!(
            format!("{}", PipelineStage::AwaitingReview),
            "AwaitingReview"
        );
        assert_eq!(
            format!("{}", PipelineStage::GeneratingNotes),
            "GeneratingNotes"
        );
        assert_eq!(format!("{}", PipelineStage::Complete), "Complete");
        assert_eq!(
            format!("{}", PipelineStage::RetryingInSession),
            "RetryingInSession"
        );
        assert_eq!(format!("{}", PipelineStage::Blocked), "Blocked");
    }

    #[test]
    fn test_pipeline_stage_serialization() {
        let stages = [
            PipelineStage::TaskCompleted,
            PipelineStage::Verifying,
            PipelineStage::GeneratingChangeSummary,
            PipelineStage::AwaitingReview,
            PipelineStage::GeneratingNotes,
            PipelineStage::Complete,
            PipelineStage::RetryingInSession,
            PipelineStage::Blocked,
        ];
        for stage in stages {
            let json = serde_json::to_string(&stage).unwrap();
            let parsed: PipelineStage = serde_json::from_str(&json).unwrap();
            assert_eq!(stage, parsed);
        }
    }

    #[test]
    fn test_pipeline_stage_equality() {
        assert_eq!(PipelineStage::Verifying, PipelineStage::Verifying);
        assert_ne!(PipelineStage::Verifying, PipelineStage::Complete);
    }

    #[test]
    fn test_pipeline_stage_clone_copy() {
        let stage = PipelineStage::AwaitingReview;
        let cloned = stage;
        assert_eq!(stage, cloned);
    }

    #[test]
    fn test_pipeline_stage_debug() {
        let stage = PipelineStage::Complete;
        let debug_str = format!("{:?}", stage);
        assert!(debug_str.contains("Complete"));
    }

    // ReviewDecision tests

    #[test]
    fn test_review_decision_approve() {
        let decision = ReviewDecision::Approve;
        assert!(matches!(decision, ReviewDecision::Approve));
    }

    #[test]
    fn test_review_decision_reject() {
        let decision = ReviewDecision::Reject {
            reason: "Code quality issues".to_string(),
        };
        if let ReviewDecision::Reject { reason } = decision {
            assert!(reason.contains("quality"));
        } else {
            panic!("Expected Reject variant");
        }
    }

    #[test]
    fn test_review_decision_request_changes() {
        let decision = ReviewDecision::RequestChanges {
            feedback: "Please add more tests".to_string(),
        };
        if let ReviewDecision::RequestChanges { feedback } = decision {
            assert!(feedback.contains("tests"));
        } else {
            panic!("Expected RequestChanges variant");
        }
    }

    #[test]
    fn test_review_decision_display() {
        assert_eq!(format!("{}", ReviewDecision::Approve), "Approved");
        assert_eq!(
            format!(
                "{}",
                ReviewDecision::Reject {
                    reason: "bad".to_string()
                }
            ),
            "Rejected: bad"
        );
        assert_eq!(
            format!(
                "{}",
                ReviewDecision::RequestChanges {
                    feedback: "fix".to_string()
                }
            ),
            "Changes Requested: fix"
        );
    }

    #[test]
    fn test_review_decision_serialization() {
        let decisions = [
            ReviewDecision::Approve,
            ReviewDecision::Reject {
                reason: "test".to_string(),
            },
            ReviewDecision::RequestChanges {
                feedback: "test".to_string(),
            },
        ];
        for decision in decisions {
            let json = serde_json::to_string(&decision).unwrap();
            let parsed: ReviewDecision = serde_json::from_str(&json).unwrap();
            assert_eq!(decision, parsed);
        }
    }

    #[test]
    fn test_review_decision_equality() {
        assert_eq!(ReviewDecision::Approve, ReviewDecision::Approve);
        assert_ne!(
            ReviewDecision::Approve,
            ReviewDecision::Reject {
                reason: "x".to_string()
            }
        );
    }

    #[test]
    fn test_review_decision_clone() {
        let decision = ReviewDecision::RequestChanges {
            feedback: "fix this".to_string(),
        };
        let cloned = decision.clone();
        assert_eq!(decision, cloned);
    }

    // PipelineState tests

    #[test]
    fn test_pipeline_state_new() {
        let state = PipelineState::new(
            TaskId("task-001".to_string()),
            SessionId(1),
            PathBuf::from("/tmp/project"),
        );
        assert_eq!(state.task_id, TaskId("task-001".to_string()));
        assert_eq!(state.session_id, SessionId(1));
        assert_eq!(state.working_dir, PathBuf::from("/tmp/project"));
        assert_eq!(state.stage, PipelineStage::TaskCompleted);
        assert!(state.verification.is_none());
        assert!(state.change_summary.is_none());
        assert!(state.review_decision.is_none());
        assert!(state.session_note.is_none());
        assert!(state.completed_at.is_none());
        assert!(state.error.is_none());
    }

    #[test]
    fn test_pipeline_state_is_terminal() {
        let mut state = PipelineState::new(
            TaskId("task-001".to_string()),
            SessionId(1),
            PathBuf::from("/tmp"),
        );

        assert!(!state.is_terminal());

        state.stage = PipelineStage::Verifying;
        assert!(!state.is_terminal());

        state.stage = PipelineStage::AwaitingReview;
        assert!(!state.is_terminal());

        state.stage = PipelineStage::Complete;
        assert!(state.is_terminal());

        state.stage = PipelineStage::Blocked;
        assert!(state.is_terminal());

        state.stage = PipelineStage::RetryingInSession;
        assert!(state.is_terminal());
    }

    #[test]
    fn test_pipeline_state_is_awaiting_human() {
        let mut state = PipelineState::new(
            TaskId("task-001".to_string()),
            SessionId(1),
            PathBuf::from("/tmp"),
        );

        assert!(!state.is_awaiting_human());

        state.stage = PipelineStage::AwaitingReview;
        assert!(state.is_awaiting_human());

        state.stage = PipelineStage::Blocked;
        assert!(state.is_awaiting_human());

        state.stage = PipelineStage::Complete;
        assert!(!state.is_awaiting_human());
    }

    #[test]
    fn test_pipeline_state_complete() {
        let mut state = PipelineState::new(
            TaskId("task-001".to_string()),
            SessionId(1),
            PathBuf::from("/tmp"),
        );
        assert!(state.completed_at.is_none());

        state.complete();
        assert_eq!(state.stage, PipelineStage::Complete);
        assert!(state.completed_at.is_some());
    }

    #[test]
    fn test_pipeline_state_block() {
        let mut state = PipelineState::new(
            TaskId("task-001".to_string()),
            SessionId(1),
            PathBuf::from("/tmp"),
        );

        state.block("Max retries exceeded");
        assert_eq!(state.stage, PipelineStage::Blocked);
        assert_eq!(state.error, Some("Max retries exceeded".to_string()));
    }

    #[test]
    fn test_pipeline_state_duration() {
        let state = PipelineState::new(
            TaskId("task-001".to_string()),
            SessionId(1),
            PathBuf::from("/tmp"),
        );
        let duration = state.duration();
        assert!(duration.num_milliseconds() >= 0);
    }

    #[test]
    fn test_pipeline_state_serialization() {
        let mut state = PipelineState::new(
            TaskId("task-001".to_string()),
            SessionId(1),
            PathBuf::from("/tmp/project"),
        );
        state.stage = PipelineStage::Complete;
        state.review_decision = Some(ReviewDecision::Approve);
        state.completed_at = Some(chrono::Utc::now());

        let json = serde_json::to_string_pretty(&state).unwrap();
        let parsed: PipelineState = serde_json::from_str(&json).unwrap();

        assert_eq!(state.task_id, parsed.task_id);
        assert_eq!(state.session_id, parsed.session_id);
        assert_eq!(state.stage, parsed.stage);
        assert_eq!(state.review_decision, parsed.review_decision);
    }

    #[test]
    fn test_pipeline_state_clone() {
        let state = PipelineState::new(
            TaskId("task-001".to_string()),
            SessionId(1),
            PathBuf::from("/tmp"),
        );
        let cloned = state.clone();
        assert_eq!(state.task_id, cloned.task_id);
        assert_eq!(state.stage, cloned.stage);
    }

    #[test]
    fn test_pipeline_state_debug() {
        let state = PipelineState::new(
            TaskId("task-001".to_string()),
            SessionId(1),
            PathBuf::from("/tmp"),
        );
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("PipelineState"));
        assert!(debug_str.contains("task-001"));
    }

    // PipelineEvent tests

    #[test]
    fn test_pipeline_event_started() {
        let event = PipelineEvent::Started {
            task_id: TaskId("task-001".to_string()),
            session_id: SessionId(1),
        };
        assert!(matches!(event, PipelineEvent::Started { .. }));
    }

    #[test]
    fn test_pipeline_event_stage_changed() {
        let event = PipelineEvent::StageChanged {
            task_id: TaskId("task-001".to_string()),
            stage: PipelineStage::AwaitingReview,
        };
        if let PipelineEvent::StageChanged { stage, .. } = event {
            assert_eq!(stage, PipelineStage::AwaitingReview);
        } else {
            panic!("Expected StageChanged");
        }
    }

    #[test]
    fn test_pipeline_event_verification_completed() {
        let event = PipelineEvent::VerificationCompleted {
            task_id: TaskId("task-001".to_string()),
            passed: true,
        };
        if let PipelineEvent::VerificationCompleted { passed, .. } = event {
            assert!(passed);
        } else {
            panic!("Expected VerificationCompleted");
        }
    }

    #[test]
    fn test_pipeline_event_awaiting_review() {
        let event = PipelineEvent::AwaitingReview {
            task_id: TaskId("task-001".to_string()),
        };
        assert!(matches!(event, PipelineEvent::AwaitingReview { .. }));
    }

    #[test]
    fn test_pipeline_event_reviewed() {
        let event = PipelineEvent::Reviewed {
            task_id: TaskId("task-001".to_string()),
            decision: ReviewDecision::Approve,
        };
        if let PipelineEvent::Reviewed { decision, .. } = event {
            assert_eq!(decision, ReviewDecision::Approve);
        } else {
            panic!("Expected Reviewed");
        }
    }

    #[test]
    fn test_pipeline_event_note_generated() {
        let event = PipelineEvent::NoteGenerated {
            task_id: TaskId("task-001".to_string()),
            note_path: PathBuf::from("/notes/task-001.md"),
        };
        if let PipelineEvent::NoteGenerated { note_path, .. } = event {
            assert_eq!(note_path, PathBuf::from("/notes/task-001.md"));
        } else {
            panic!("Expected NoteGenerated");
        }
    }

    #[test]
    fn test_pipeline_event_completed() {
        let event = PipelineEvent::Completed {
            task_id: TaskId("task-001".to_string()),
        };
        assert!(matches!(event, PipelineEvent::Completed { .. }));
    }

    #[test]
    fn test_pipeline_event_failed() {
        let event = PipelineEvent::Failed {
            task_id: TaskId("task-001".to_string()),
            error: "Verification timeout".to_string(),
        };
        if let PipelineEvent::Failed { error, .. } = event {
            assert!(error.contains("timeout"));
        } else {
            panic!("Expected Failed");
        }
    }

    #[test]
    fn test_pipeline_event_sent_to_session() {
        let event = PipelineEvent::SentToSession {
            task_id: TaskId("task-001".to_string()),
            feedback: "Please fix the tests".to_string(),
        };
        if let PipelineEvent::SentToSession { feedback, .. } = event {
            assert!(feedback.contains("fix"));
        } else {
            panic!("Expected SentToSession");
        }
    }

    #[test]
    fn test_pipeline_event_clone() {
        let event = PipelineEvent::Completed {
            task_id: TaskId("task-001".to_string()),
        };
        let cloned = event.clone();
        assert!(matches!(cloned, PipelineEvent::Completed { .. }));
    }

    #[test]
    fn test_pipeline_event_debug() {
        let event = PipelineEvent::Started {
            task_id: TaskId("task-001".to_string()),
            session_id: SessionId(1),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("Started"));
        assert!(debug_str.contains("task-001"));
    }

    // FailureMessageFormatter trait tests

    struct MockFormatter;

    impl FailureMessageFormatter for MockFormatter {
        fn format_verification_failure(&self, status: &VerificationStatus) -> String {
            format!(
                "Verification failed with {} results, retry {}",
                status.results.len(),
                status.retry_count
            )
        }

        fn format_review_feedback(&self, feedback: &str) -> String {
            format!("Reviewer says: {}", feedback)
        }
    }

    #[test]
    fn test_failure_formatter_verification() {
        let formatter = MockFormatter;
        let status = VerificationStatus::new(TaskId("task-001".to_string()), SessionId(1));
        let message = formatter.format_verification_failure(&status);
        assert!(message.contains("0 results"));
        assert!(message.contains("retry 0"));
    }

    #[test]
    fn test_failure_formatter_review() {
        let formatter = MockFormatter;
        let message = formatter.format_review_feedback("Add more tests");
        assert!(message.contains("Reviewer says"));
        assert!(message.contains("Add more tests"));
    }

    #[test]
    fn test_failure_formatter_trait_is_object_safe() {
        // This compiles only if FailureMessageFormatter is object-safe
        fn _takes_formatter(_: &dyn FailureMessageFormatter) {}
    }
}
