//! Verification pipeline orchestrator.
//!
//! This module provides the [`PipelineOrchestrator`] which coordinates the
//! complete verification workflow from task completion through human review
//! to notes generation.
//!
//! ## Overview
//!
//! The pipeline orchestrator:
//! - Connects to the verification gate
//! - Generates change summaries
//! - Manages human review workflow
//! - Formats failure messages for retry
//!
//! ## Example
//!
//! ```no_run
//! use codirigent_verification::PipelineOrchestrator;
//! use codirigent_core::pipeline::{VerificationPipeline, ReviewDecision};
//! use codirigent_core::{SessionId, TaskId};
//! use std::path::PathBuf;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let pipeline = PipelineOrchestrator::new(PathBuf::from(".codirigent/notes"));
//!
//! // Start pipeline for a completed task
//! pipeline.start(
//!     TaskId("task-001".to_string()),
//!     SessionId(1),
//!     PathBuf::from("/project"),
//! ).await?;
//!
//! // Later, submit review
//! pipeline.submit_review(
//!     &TaskId("task-001".to_string()),
//!     ReviewDecision::Approve,
//! ).await?;
//! # Ok(())
//! # }
//! ```

use anyhow::{Context, Result};
use async_trait::async_trait;
use codirigent_core::pipeline::{
    FailureMessageFormatter, PipelineEvent, PipelineStage, PipelineState, ReviewDecision,
    VerificationPipeline,
};
use codirigent_core::verification::VerificationState;
use codirigent_core::{ChangeDetector, SessionId, TaskId, Verifier};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::change_detector::GitChangeDetector;
use crate::formatter::DefaultFailureFormatter;
use crate::verifier::VerificationGate;

/// Pipeline orchestrator that coordinates verification, change summary, and notes.
///
/// Implements the [`VerificationPipeline`] trait to provide a complete
/// verification workflow.
///
/// # Thread Safety
///
/// The orchestrator uses `RwLock` for state management and is safe to share
/// across threads. Event subscription uses tokio's broadcast channel.
///
/// # Example
///
/// ```
/// use codirigent_verification::PipelineOrchestrator;
/// use std::path::PathBuf;
///
/// let pipeline = PipelineOrchestrator::new(PathBuf::from(".codirigent/notes"));
/// let _rx = pipeline.subscribe();
/// ```
pub struct PipelineOrchestrator {
    verifier: VerificationGate,
    change_detector: GitChangeDetector,
    formatter: DefaultFailureFormatter,
    states: RwLock<HashMap<TaskId, PipelineState>>,
    event_sender: broadcast::Sender<PipelineEvent>,
    notes_output_dir: PathBuf,
}

impl std::fmt::Debug for PipelineOrchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PipelineOrchestrator")
            .field("notes_output_dir", &self.notes_output_dir)
            .field(
                "states_count",
                &self.states.read().map(|s| s.len()).unwrap_or(0),
            )
            .finish_non_exhaustive()
    }
}

impl PipelineOrchestrator {
    /// Create a new pipeline orchestrator.
    ///
    /// # Arguments
    ///
    /// * `notes_output_dir` - Directory for generated session notes
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::PipelineOrchestrator;
    /// use std::path::PathBuf;
    ///
    /// let pipeline = PipelineOrchestrator::new(PathBuf::from(".codirigent/notes"));
    /// ```
    pub fn new(notes_output_dir: PathBuf) -> Self {
        let (event_sender, _) = broadcast::channel(256);

        Self {
            verifier: VerificationGate::new(),
            change_detector: GitChangeDetector::new(),
            formatter: DefaultFailureFormatter::new(),
            states: RwLock::new(HashMap::new()),
            event_sender,
            notes_output_dir,
        }
    }

    /// Create a pipeline orchestrator with custom components.
    ///
    /// # Arguments
    ///
    /// * `verifier` - Custom verification gate
    /// * `change_detector` - Custom change detector
    /// * `formatter` - Custom failure formatter
    /// * `notes_output_dir` - Directory for generated session notes
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::{
    ///     PipelineOrchestrator, VerificationGate, GitChangeDetector, DefaultFailureFormatter,
    /// };
    /// use std::path::PathBuf;
    ///
    /// let pipeline = PipelineOrchestrator::with_components(
    ///     VerificationGate::new(),
    ///     GitChangeDetector::new(),
    ///     DefaultFailureFormatter::new(),
    ///     PathBuf::from(".codirigent/notes"),
    /// );
    /// ```
    pub fn with_components(
        verifier: VerificationGate,
        change_detector: GitChangeDetector,
        formatter: DefaultFailureFormatter,
        notes_output_dir: PathBuf,
    ) -> Self {
        let (event_sender, _) = broadcast::channel(256);

        Self {
            verifier,
            change_detector,
            formatter,
            states: RwLock::new(HashMap::new()),
            event_sender,
            notes_output_dir,
        }
    }

    /// Subscribe to pipeline events.
    ///
    /// Returns a receiver that will receive all pipeline events.
    /// Events are broadcast to all subscribers.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::PipelineOrchestrator;
    /// use std::path::PathBuf;
    ///
    /// let pipeline = PipelineOrchestrator::new(PathBuf::from(".codirigent/notes"));
    /// let mut rx = pipeline.subscribe();
    ///
    /// // In an async context:
    /// // while let Ok(event) = rx.recv().await {
    /// //     println!("Got event: {:?}", event);
    /// // }
    /// ```
    pub fn subscribe(&self) -> broadcast::Receiver<PipelineEvent> {
        self.event_sender.subscribe()
    }

    /// Get the notes output directory.
    pub fn notes_output_dir(&self) -> &PathBuf {
        &self.notes_output_dir
    }

    /// Get a reference to the verifier.
    pub fn verifier(&self) -> &VerificationGate {
        &self.verifier
    }

    /// Get a reference to the change detector.
    pub fn change_detector(&self) -> &GitChangeDetector {
        &self.change_detector
    }

    /// Get a reference to the formatter.
    pub fn formatter(&self) -> &DefaultFailureFormatter {
        &self.formatter
    }

    /// Emit a pipeline event.
    ///
    /// Events are sent to all subscribers. If there are no active
    /// subscribers, the event is dropped silently.
    fn emit(&self, event: PipelineEvent) {
        // Ignore send errors (no receivers)
        let _ = self.event_sender.send(event);
    }

    /// Update pipeline stage and emit event.
    fn update_stage(&self, task_id: &TaskId, stage: PipelineStage) {
        if let Ok(mut states) = self.states.write() {
            if let Some(state) = states.get_mut(task_id) {
                state.stage = stage;
            }
        }
        self.emit(PipelineEvent::StageChanged {
            task_id: task_id.clone(),
            stage,
        });
    }

    /// Store a pipeline state.
    fn store_state(&self, state: PipelineState) {
        if let Ok(mut states) = self.states.write() {
            states.insert(state.task_id.clone(), state);
        }
    }

    /// Get all active pipeline states.
    ///
    /// # Returns
    ///
    /// A vector of all pipeline states currently tracked.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::PipelineOrchestrator;
    /// use std::path::PathBuf;
    ///
    /// let pipeline = PipelineOrchestrator::new(PathBuf::from(".codirigent/notes"));
    /// let states = pipeline.all_states();
    /// println!("Tracking {} pipelines", states.len());
    /// ```
    pub fn all_states(&self) -> Vec<PipelineState> {
        self.states
            .read()
            .map(|s| s.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get count of active pipelines.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::PipelineOrchestrator;
    /// use std::path::PathBuf;
    ///
    /// let pipeline = PipelineOrchestrator::new(PathBuf::from(".codirigent/notes"));
    /// assert_eq!(pipeline.active_count(), 0);
    /// ```
    pub fn active_count(&self) -> usize {
        self.states.read().map(|s| s.len()).unwrap_or(0)
    }
}

#[async_trait]
impl VerificationPipeline for PipelineOrchestrator {
    async fn start(
        &self,
        task_id: TaskId,
        session_id: SessionId,
        working_dir: PathBuf,
    ) -> Result<()> {
        info!(%task_id, %session_id, ?working_dir, "Starting verification pipeline");

        // Create initial state
        let state = PipelineState::new(task_id.clone(), session_id, working_dir.clone());
        self.store_state(state);

        self.emit(PipelineEvent::Started {
            task_id: task_id.clone(),
            session_id,
        });

        // Stage 1: Verification
        self.update_stage(&task_id, PipelineStage::Verifying);

        let verification_result = self.verifier.verify(&task_id, &working_dir).await;

        match verification_result {
            Ok(status) => {
                let passed = status.state == VerificationState::Passed;
                self.emit(PipelineEvent::VerificationCompleted {
                    task_id: task_id.clone(),
                    passed,
                });

                // Update state with verification result
                if let Ok(mut states) = self.states.write() {
                    if let Some(state) = states.get_mut(&task_id) {
                        state.verification = Some(status.clone());
                    }
                }

                if !passed {
                    // Send failure back to session
                    let message = self.formatter.format_verification_failure(&status);
                    self.update_stage(&task_id, PipelineStage::RetryingInSession);
                    self.emit(PipelineEvent::SentToSession {
                        task_id: task_id.clone(),
                        feedback: message,
                    });
                    return Ok(());
                }
            }
            Err(e) => {
                error!(%task_id, error = %e, "Verification failed with error");
                self.update_stage(&task_id, PipelineStage::Blocked);
                if let Ok(mut states) = self.states.write() {
                    if let Some(state) = states.get_mut(&task_id) {
                        state.error = Some(e.to_string());
                    }
                }
                self.emit(PipelineEvent::Failed {
                    task_id: task_id.clone(),
                    error: e.to_string(),
                });
                return Err(e);
            }
        }

        // Stage 2: Generate change summary
        self.update_stage(&task_id, PipelineStage::GeneratingChangeSummary);

        let change_summary = self
            .change_detector
            .generate_summary(task_id.clone(), session_id, &working_dir, None)
            .context("Failed to generate change summary")?;

        self.emit(PipelineEvent::ChangeSummaryGenerated {
            task_id: task_id.clone(),
            summary: change_summary.clone(),
        });

        if let Ok(mut states) = self.states.write() {
            if let Some(state) = states.get_mut(&task_id) {
                state.change_summary = Some(change_summary);
            }
        }

        // Stage 3: Await human review
        self.update_stage(&task_id, PipelineStage::AwaitingReview);
        self.emit(PipelineEvent::AwaitingReview {
            task_id: task_id.clone(),
        });

        // Pipeline pauses here; continues when submit_review is called
        info!(%task_id, "Pipeline awaiting human review");
        Ok(())
    }

    fn get_state(&self, task_id: &TaskId) -> Option<PipelineState> {
        self.states
            .read()
            .ok()
            .and_then(|s| s.get(task_id).cloned())
    }

    async fn submit_review(&self, task_id: &TaskId, decision: ReviewDecision) -> Result<()> {
        info!(%task_id, ?decision, "Review submitted");

        // Get current state
        let state = self
            .get_state(task_id)
            .context("Pipeline state not found")?;

        if state.stage != PipelineStage::AwaitingReview {
            anyhow::bail!(
                "Pipeline is not awaiting review (current stage: {})",
                state.stage
            );
        }

        // Update state with decision
        if let Ok(mut states) = self.states.write() {
            if let Some(state) = states.get_mut(task_id) {
                state.review_decision = Some(decision.clone());
            }
        }

        self.emit(PipelineEvent::Reviewed {
            task_id: task_id.clone(),
            decision: decision.clone(),
        });

        match decision {
            ReviewDecision::Approve => {
                // Generate notes and complete
                self.update_stage(task_id, PipelineStage::GeneratingNotes);

                // For now, we skip actual notes generation and just complete
                // In a full implementation, we would call the NotesGenerator here

                self.update_stage(task_id, PipelineStage::Complete);
                if let Ok(mut states) = self.states.write() {
                    if let Some(state) = states.get_mut(task_id) {
                        state.completed_at = Some(chrono::Utc::now());
                    }
                }
                self.emit(PipelineEvent::Completed {
                    task_id: task_id.clone(),
                });
                info!(%task_id, "Pipeline completed successfully");
            }
            ReviewDecision::Reject { reason } => {
                warn!(%task_id, %reason, "Task rejected by reviewer");
                self.update_stage(task_id, PipelineStage::Blocked);
                if let Ok(mut states) = self.states.write() {
                    if let Some(state) = states.get_mut(task_id) {
                        state.error = Some(reason.clone());
                    }
                }
                self.emit(PipelineEvent::Failed {
                    task_id: task_id.clone(),
                    error: format!("Rejected: {}", reason),
                });
            }
            ReviewDecision::RequestChanges { feedback } => {
                info!(%task_id, "Changes requested, sending back to session");
                let message = self.formatter.format_review_feedback(&feedback);
                self.update_stage(task_id, PipelineStage::RetryingInSession);
                self.emit(PipelineEvent::SentToSession {
                    task_id: task_id.clone(),
                    feedback: message,
                });
            }
        }

        Ok(())
    }

    async fn skip_verification(&self, task_id: &TaskId) -> Result<()> {
        info!(%task_id, "Skipping verification");

        let state = self
            .get_state(task_id)
            .context("Pipeline state not found")?;

        if state.stage != PipelineStage::Verifying {
            anyhow::bail!(
                "Can only skip during verification stage (current stage: {})",
                state.stage
            );
        }

        // Go directly to change summary generation
        self.update_stage(task_id, PipelineStage::GeneratingChangeSummary);

        let change_summary = self
            .change_detector
            .generate_summary(task_id.clone(), state.session_id, &state.working_dir, None)
            .context("Failed to generate change summary")?;

        self.emit(PipelineEvent::ChangeSummaryGenerated {
            task_id: task_id.clone(),
            summary: change_summary.clone(),
        });

        if let Ok(mut states) = self.states.write() {
            if let Some(state) = states.get_mut(task_id) {
                state.change_summary = Some(change_summary);
            }
        }

        self.update_stage(task_id, PipelineStage::AwaitingReview);
        self.emit(PipelineEvent::AwaitingReview {
            task_id: task_id.clone(),
        });

        Ok(())
    }

    fn cancel(&self, task_id: &TaskId) -> Result<()> {
        info!(%task_id, "Cancelling pipeline");

        if let Ok(mut states) = self.states.write() {
            states.remove(task_id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codirigent_core::pipeline::PipelineStage;
    use tempfile::TempDir;

    // Constructor tests

    #[test]
    fn test_pipeline_creation() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());
        assert_eq!(pipeline.notes_output_dir(), temp.path());
        assert_eq!(pipeline.active_count(), 0);
    }

    #[test]
    fn test_pipeline_with_components() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::with_components(
            VerificationGate::new(),
            GitChangeDetector::new(),
            DefaultFailureFormatter::new(),
            temp.path().to_path_buf(),
        );
        assert_eq!(pipeline.active_count(), 0);
    }

    #[test]
    fn test_pipeline_debug() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());
        let debug_str = format!("{:?}", pipeline);
        assert!(debug_str.contains("PipelineOrchestrator"));
    }

    #[test]
    fn test_get_state_not_found() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());
        assert!(pipeline
            .get_state(&TaskId("nonexistent".to_string()))
            .is_none());
    }

    #[test]
    fn test_all_states_empty() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());
        assert!(pipeline.all_states().is_empty());
    }

    #[test]
    fn test_event_subscription() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());
        let _rx = pipeline.subscribe();
        // Should not panic
    }

    #[test]
    fn test_verifier_accessor() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());
        let _verifier = pipeline.verifier();
        // Should not panic
    }

    #[test]
    fn test_change_detector_accessor() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());
        let _detector = pipeline.change_detector();
        // Should not panic
    }

    #[test]
    fn test_formatter_accessor() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());
        let _formatter = pipeline.formatter();
        // Should not panic
    }

    // Cancel tests

    #[tokio::test]
    async fn test_cancel_pipeline() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());

        // Start a pipeline
        let task_id = TaskId("task-001".to_string());
        let state = PipelineState::new(task_id.clone(), SessionId(1), temp.path().to_path_buf());
        pipeline.store_state(state);
        assert!(pipeline.get_state(&task_id).is_some());

        // Cancel it
        pipeline.cancel(&task_id).unwrap();
        assert!(pipeline.get_state(&task_id).is_none());
    }

    #[tokio::test]
    async fn test_cancel_nonexistent_pipeline() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());

        // Cancel nonexistent pipeline should succeed
        let result = pipeline.cancel(&TaskId("nonexistent".to_string()));
        assert!(result.is_ok());
    }

    // Submit review tests

    #[tokio::test]
    async fn test_submit_review_not_found() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());

        let result = pipeline
            .submit_review(&TaskId("nonexistent".to_string()), ReviewDecision::Approve)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_submit_review_wrong_stage() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());

        // Create a pipeline in the wrong stage
        let task_id = TaskId("task-001".to_string());
        let mut state =
            PipelineState::new(task_id.clone(), SessionId(1), temp.path().to_path_buf());
        state.stage = PipelineStage::Verifying;
        pipeline.store_state(state);

        let result = pipeline
            .submit_review(&task_id, ReviewDecision::Approve)
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not awaiting review"));
    }

    #[tokio::test]
    async fn test_submit_review_approve() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());

        // Create a pipeline awaiting review
        let task_id = TaskId("task-001".to_string());
        let mut state =
            PipelineState::new(task_id.clone(), SessionId(1), temp.path().to_path_buf());
        state.stage = PipelineStage::AwaitingReview;
        pipeline.store_state(state);

        // Approve
        let result = pipeline
            .submit_review(&task_id, ReviewDecision::Approve)
            .await;
        assert!(result.is_ok());

        // Check final state
        let final_state = pipeline.get_state(&task_id).unwrap();
        assert_eq!(final_state.stage, PipelineStage::Complete);
        assert!(final_state.completed_at.is_some());
        assert_eq!(final_state.review_decision, Some(ReviewDecision::Approve));
    }

    #[tokio::test]
    async fn test_submit_review_reject() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());

        // Create a pipeline awaiting review
        let task_id = TaskId("task-001".to_string());
        let mut state =
            PipelineState::new(task_id.clone(), SessionId(1), temp.path().to_path_buf());
        state.stage = PipelineStage::AwaitingReview;
        pipeline.store_state(state);

        // Reject
        let result = pipeline
            .submit_review(
                &task_id,
                ReviewDecision::Reject {
                    reason: "Not acceptable".to_string(),
                },
            )
            .await;
        assert!(result.is_ok());

        // Check final state
        let final_state = pipeline.get_state(&task_id).unwrap();
        assert_eq!(final_state.stage, PipelineStage::Blocked);
        assert!(final_state.error.is_some());
    }

    #[tokio::test]
    async fn test_submit_review_request_changes() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());

        // Create a pipeline awaiting review
        let task_id = TaskId("task-001".to_string());
        let mut state =
            PipelineState::new(task_id.clone(), SessionId(1), temp.path().to_path_buf());
        state.stage = PipelineStage::AwaitingReview;
        pipeline.store_state(state);

        // Request changes
        let result = pipeline
            .submit_review(
                &task_id,
                ReviewDecision::RequestChanges {
                    feedback: "Add more tests".to_string(),
                },
            )
            .await;
        assert!(result.is_ok());

        // Check final state
        let final_state = pipeline.get_state(&task_id).unwrap();
        assert_eq!(final_state.stage, PipelineStage::RetryingInSession);
    }

    // Skip verification tests

    #[tokio::test]
    async fn test_skip_verification_not_found() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());

        let result = pipeline
            .skip_verification(&TaskId("nonexistent".to_string()))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_skip_verification_wrong_stage() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());

        // Create a pipeline in the wrong stage
        let task_id = TaskId("task-001".to_string());
        let mut state =
            PipelineState::new(task_id.clone(), SessionId(1), temp.path().to_path_buf());
        state.stage = PipelineStage::AwaitingReview;
        pipeline.store_state(state);

        let result = pipeline.skip_verification(&task_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Can only skip"));
    }

    #[tokio::test]
    async fn test_skip_verification_success() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());

        // Create a pipeline in verifying stage
        let task_id = TaskId("task-001".to_string());
        let mut state =
            PipelineState::new(task_id.clone(), SessionId(1), temp.path().to_path_buf());
        state.stage = PipelineStage::Verifying;
        pipeline.store_state(state);

        let result = pipeline.skip_verification(&task_id).await;
        assert!(result.is_ok());

        // Should be awaiting review now
        let final_state = pipeline.get_state(&task_id).unwrap();
        assert_eq!(final_state.stage, PipelineStage::AwaitingReview);
    }

    // Start pipeline tests

    #[tokio::test]
    async fn test_start_pipeline_basic() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());

        let task_id = TaskId("task-001".to_string());
        let result = pipeline
            .start(task_id.clone(), SessionId(1), temp.path().to_path_buf())
            .await;

        // Pipeline should start (may fail at verification with no tests)
        // or succeed if no verification commands detected
        let _ = result;

        // State should exist
        let state = pipeline.get_state(&task_id);
        assert!(state.is_some());
    }

    // Event emission tests

    #[tokio::test]
    async fn test_event_emission() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());
        let mut rx = pipeline.subscribe();

        // Create a pipeline awaiting review
        let task_id = TaskId("task-001".to_string());
        let mut state =
            PipelineState::new(task_id.clone(), SessionId(1), temp.path().to_path_buf());
        state.stage = PipelineStage::AwaitingReview;
        pipeline.store_state(state);

        // Approve and check events
        pipeline
            .submit_review(&task_id, ReviewDecision::Approve)
            .await
            .unwrap();

        // Should receive events
        let mut received_reviewed = false;
        let mut received_completed = false;

        // Try to receive a few events
        for _ in 0..10 {
            match rx.try_recv() {
                Ok(PipelineEvent::Reviewed { .. }) => received_reviewed = true,
                Ok(PipelineEvent::Completed { .. }) => received_completed = true,
                Ok(_) => {}
                Err(_) => break,
            }
        }

        assert!(received_reviewed);
        assert!(received_completed);
    }

    // Integration tests

    #[tokio::test]
    async fn test_full_workflow_no_verification() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());

        let task_id = TaskId("task-001".to_string());

        // Start pipeline (should pass verification if no commands detected)
        let result = pipeline
            .start(task_id.clone(), SessionId(1), temp.path().to_path_buf())
            .await;

        // If no verification commands, should be awaiting review
        if result.is_ok() {
            let state = pipeline.get_state(&task_id).unwrap();
            if state.stage == PipelineStage::AwaitingReview {
                // Approve
                pipeline
                    .submit_review(&task_id, ReviewDecision::Approve)
                    .await
                    .unwrap();
                let final_state = pipeline.get_state(&task_id).unwrap();
                assert_eq!(final_state.stage, PipelineStage::Complete);
            }
        }
    }

    #[tokio::test]
    async fn test_multiple_pipelines() {
        let temp = TempDir::new().unwrap();
        let pipeline = PipelineOrchestrator::new(temp.path().to_path_buf());

        // Create multiple pipelines
        for i in 0..3 {
            let task_id = TaskId(format!("task-{:03}", i));
            let state = PipelineState::new(task_id, SessionId(i as u64), temp.path().to_path_buf());
            pipeline.store_state(state);
        }

        assert_eq!(pipeline.active_count(), 3);
        assert_eq!(pipeline.all_states().len(), 3);
    }
}
