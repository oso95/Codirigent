//! Ralph Loop controller implementation.
//!
//! This module provides the [`DefaultRalphLoopController`] which implements
//! the [`RalphLoopController`] trait for managing autonomous execution loops.
//!
//! # Example
//!
//! ```no_run
//! use codirigent_session::DefaultRalphLoopController;
//! use codirigent_core::{
//!     DefaultEventBus, RalphLoopConfig, RalphLoopController, SessionId,
//! };
//! use std::sync::Arc;
//!
//! // Create controller with event bus
//! let event_bus = Arc::new(DefaultEventBus::new(16));
//! let mut controller = DefaultRalphLoopController::with_event_bus(event_bus);
//!
//! // Start a loop for a session
//! controller.start(SessionId(1), RalphLoopConfig::for_rust()).unwrap();
//!
//! // Check if active
//! assert!(controller.is_active(SessionId(1)));
//!
//! // Pause and resume
//! controller.pause(SessionId(1)).unwrap();
//! controller.resume(SessionId(1)).unwrap();
//!
//! // Stop when done
//! controller.stop(SessionId(1)).unwrap();
//! ```

use anyhow::{bail, Context, Result};
use codirigent_core::{
    traits::RalphLoopController, CodirigentEvent, EventBus, IterationResult, RalphLoopConfig,
    RalphLoopState, RalphLoopStatus, SessionId,
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Default implementation of the Ralph Loop controller.
///
/// Manages autonomous execution loops for sessions, tracking their state
/// and emitting events for lifecycle changes.
///
/// # Event Emission
///
/// When configured with an event bus, the controller emits events for:
/// - Loop started/stopped/paused/resumed
/// - Iteration completion
/// - Stuck state detection
/// - Context compaction triggers
pub struct DefaultRalphLoopController {
    /// Active loops by session ID.
    loops: HashMap<SessionId, (RalphLoopConfig, RalphLoopState)>,
    /// Event bus for emitting events.
    event_bus: Option<Arc<dyn EventBus>>,
}

impl Default for DefaultRalphLoopController {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultRalphLoopController {
    /// Create a new controller without event bus.
    ///
    /// Events will not be emitted when using this constructor.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_session::DefaultRalphLoopController;
    ///
    /// let controller = DefaultRalphLoopController::new();
    /// ```
    pub fn new() -> Self {
        Self {
            loops: HashMap::new(),
            event_bus: None,
        }
    }

    /// Create a controller with an event bus for emitting events.
    ///
    /// # Arguments
    ///
    /// * `event_bus` - The event bus to emit events to
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_session::DefaultRalphLoopController;
    /// use codirigent_core::DefaultEventBus;
    /// use std::sync::Arc;
    ///
    /// let bus = Arc::new(DefaultEventBus::new(16));
    /// let controller = DefaultRalphLoopController::with_event_bus(bus);
    /// ```
    pub fn with_event_bus(event_bus: Arc<dyn EventBus>) -> Self {
        Self {
            loops: HashMap::new(),
            event_bus: Some(event_bus),
        }
    }

    /// Emit an event if an event bus is configured.
    fn emit(&self, event: CodirigentEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.publish(event);
        }
    }

    /// Check if context compaction should be triggered.
    ///
    /// Returns true if auto_compact is enabled and the current
    /// context usage exceeds the compact threshold.
    ///
    /// Note: This is a placeholder for actual context tracking integration.
    pub fn should_compact(&self, session_id: SessionId, current_context_usage: f32) -> bool {
        if let Some((config, _)) = self.loops.get(&session_id) {
            config.auto_compact && current_context_usage >= config.compact_threshold
        } else {
            false
        }
    }

    /// Trigger context compaction for a session.
    ///
    /// Emits a RalphLoopCompacted event if a loop is active.
    pub fn trigger_compaction(&self, session_id: SessionId) {
        if let Some((_, state)) = self.loops.get(&session_id) {
            if !state.status.is_finished() {
                info!(
                    ?session_id,
                    iteration = state.current_iteration,
                    "Triggering context compaction"
                );
                self.emit(CodirigentEvent::RalphLoopCompacted {
                    session_id,
                    iteration: state.current_iteration,
                });
            }
        }
    }

    /// Get iteration delay for a session's loop.
    ///
    /// Returns the configured delay between iterations, or None if no loop exists.
    pub fn iteration_delay_ms(&self, session_id: SessionId) -> Option<u64> {
        self.loops
            .get(&session_id)
            .map(|(c, _)| c.iteration_delay_ms)
    }

    /// Get statistics for all loops.
    ///
    /// Returns a summary of all managed loops including active count,
    /// total iterations, and success/failure counts.
    pub fn stats(&self) -> LoopStats {
        let mut stats = LoopStats {
            total_loops: self.loops.len(),
            ..Default::default()
        };

        for (_, state) in self.loops.values() {
            match state.status {
                RalphLoopStatus::Running | RalphLoopStatus::Paused => {
                    stats.active_loops += 1;
                }
                RalphLoopStatus::Completed => {
                    stats.completed_loops += 1;
                }
                RalphLoopStatus::Failed | RalphLoopStatus::Cancelled => {
                    stats.failed_loops += 1;
                }
            }
            stats.total_iterations += state.current_iteration as usize;
            stats.successful_iterations += state.successful_iterations() as usize;
            stats.failed_iterations += state.failed_iterations() as usize;
        }

        stats
    }

    /// Remove finished loops from tracking.
    ///
    /// This cleans up loops that have completed, failed, or been cancelled.
    /// Returns the number of loops removed.
    pub fn cleanup_finished(&mut self) -> usize {
        let before = self.loops.len();
        self.loops
            .retain(|_, (_, state)| !state.status.is_finished());
        before - self.loops.len()
    }

    /// Get all sessions that have loops (active or finished).
    pub fn all_sessions(&self) -> Vec<SessionId> {
        self.loops.keys().copied().collect()
    }

    /// Check for stuck state and emit event if detected.
    fn check_and_emit_stuck(&self, session_id: SessionId) {
        if let Some((config, state)) = self.loops.get(&session_id) {
            if state.iterations_without_progress >= config.stuck_threshold
                && !state.status.is_finished()
            {
                warn!(
                    ?session_id,
                    iterations = state.iterations_without_progress,
                    "Ralph Loop detected stuck state"
                );
                self.emit(CodirigentEvent::RalphLoopStuck {
                    session_id,
                    iterations_without_progress: state.iterations_without_progress,
                });
            }
        }
    }
}

impl RalphLoopController for DefaultRalphLoopController {
    fn start(&mut self, session_id: SessionId, config: RalphLoopConfig) -> Result<()> {
        if self.is_active(session_id) {
            bail!("Session {} already has an active loop", session_id);
        }

        if !config.is_valid() {
            bail!("Invalid configuration");
        }

        info!(
            ?session_id,
            max_iter = config.max_iterations,
            command = %config.verification_command,
            "Starting Ralph Loop"
        );

        let state = RalphLoopState::new();
        self.emit(CodirigentEvent::RalphLoopStarted {
            session_id,
            config: config.clone(),
        });

        self.loops.insert(session_id, (config, state));
        Ok(())
    }

    fn pause(&mut self, session_id: SessionId) -> Result<()> {
        let (_, state) = self
            .loops
            .get_mut(&session_id)
            .context("No loop found for session")?;

        if !state.status.can_pause() {
            bail!("Loop cannot be paused (status: {})", state.status);
        }

        state.status = RalphLoopStatus::Paused;
        info!(?session_id, "Ralph Loop paused");

        self.emit(CodirigentEvent::RalphLoopPaused { session_id });
        Ok(())
    }

    fn resume(&mut self, session_id: SessionId) -> Result<()> {
        let (_, state) = self
            .loops
            .get_mut(&session_id)
            .context("No loop found for session")?;

        if !state.status.can_resume() {
            bail!("Loop cannot be resumed (status: {})", state.status);
        }

        state.status = RalphLoopStatus::Running;
        info!(?session_id, "Ralph Loop resumed");

        self.emit(CodirigentEvent::RalphLoopResumed { session_id });
        Ok(())
    }

    fn stop(&mut self, session_id: SessionId) -> Result<()> {
        let (_, state) = self
            .loops
            .get_mut(&session_id)
            .context("No loop found for session")?;

        if state.status.is_finished() {
            bail!("Loop is already finished (status: {})", state.status);
        }

        state.mark_cancelled();
        info!(?session_id, "Ralph Loop cancelled");

        self.emit(CodirigentEvent::RalphLoopCancelled { session_id });
        Ok(())
    }

    fn get_state(&self, session_id: SessionId) -> Option<&RalphLoopState> {
        self.loops.get(&session_id).map(|(_, s)| s)
    }

    fn get_state_mut(&mut self, session_id: SessionId) -> Option<&mut RalphLoopState> {
        self.loops.get_mut(&session_id).map(|(_, s)| s)
    }

    fn get_config(&self, session_id: SessionId) -> Option<&RalphLoopConfig> {
        self.loops.get(&session_id).map(|(c, _)| c)
    }

    fn active_sessions(&self) -> Vec<SessionId> {
        self.loops
            .iter()
            .filter(|(_, (_, s))| !s.status.is_finished())
            .map(|(id, _)| *id)
            .collect()
    }

    fn record_iteration(&mut self, session_id: SessionId, result: IterationResult) -> Result<()> {
        let passed = result.verification_passed;
        let iteration = result.iteration;
        let test_failures = result.test_failures;

        // Get config values before mutable borrow
        let (max_iterations, should_pause) = {
            let (config, _state) = self
                .loops
                .get(&session_id)
                .context("No loop found for session")?;

            // Calculate if we should pause (for after we update state)
            let should_pause = config.pause_on_error && !passed;
            (config.max_iterations, should_pause)
        };

        // Now get mutable reference to update state
        let (_, state) = self
            .loops
            .get_mut(&session_id)
            .context("No loop found for session")?;

        // Track progress
        if passed {
            state.iterations_without_progress = 0;
        } else {
            state.iterations_without_progress += 1;
        }

        state.current_iteration = iteration;
        state.iteration_history.push(result);

        debug!(?session_id, iteration, passed, "Recorded iteration");

        // Emit iteration event
        self.emit(CodirigentEvent::RalphLoopIteration {
            session_id,
            iteration,
            passed,
            test_failures,
        });

        // Check for stuck state
        self.check_and_emit_stuck(session_id);

        // Now get state again to update final status
        let (_, state) = self
            .loops
            .get_mut(&session_id)
            .context("No loop found for session")?;

        // Check for completion
        if passed {
            state.mark_completed();
            info!(?session_id, iteration, "Ralph Loop completed successfully");
            self.emit(CodirigentEvent::RalphLoopCompleted {
                session_id,
                total_iterations: iteration,
            });
        } else if iteration >= max_iterations {
            state.mark_failed("Max iterations reached".to_string());
            warn!(?session_id, "Ralph Loop failed: max iterations reached");
            self.emit(CodirigentEvent::RalphLoopFailed {
                session_id,
                reason: "Max iterations reached".to_string(),
                iterations: iteration,
            });
        } else if should_pause && state.status == RalphLoopStatus::Running {
            // Pause on error if configured
            state.status = RalphLoopStatus::Paused;
            info!(?session_id, "Ralph Loop paused due to error");
            self.emit(CodirigentEvent::RalphLoopPaused { session_id });
        }

        Ok(())
    }
}

/// Statistics about managed loops.
///
/// Provides aggregate information about all loops managed by a controller.
#[derive(Debug, Clone, Default)]
pub struct LoopStats {
    /// Total number of loops (active + finished).
    pub total_loops: usize,
    /// Number of currently active loops (running or paused).
    pub active_loops: usize,
    /// Number of completed loops.
    pub completed_loops: usize,
    /// Number of failed/cancelled loops.
    pub failed_loops: usize,
    /// Total iterations across all loops.
    pub total_iterations: usize,
    /// Total successful iterations.
    pub successful_iterations: usize,
    /// Total failed iterations.
    pub failed_iterations: usize,
}

impl LoopStats {
    /// Get success rate as a percentage.
    ///
    /// Returns 0.0 if no iterations have been recorded.
    pub fn success_rate(&self) -> f32 {
        let total = self.successful_iterations + self.failed_iterations;
        if total == 0 {
            0.0
        } else {
            self.successful_iterations as f32 / total as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codirigent_core::DefaultEventBus;

    #[test]
    fn test_new_controller() {
        let controller = DefaultRalphLoopController::new();
        assert!(controller.active_sessions().is_empty());
    }

    #[test]
    fn test_default_controller() {
        let controller = DefaultRalphLoopController::default();
        assert!(controller.active_sessions().is_empty());
    }

    #[test]
    fn test_with_event_bus() {
        let bus = Arc::new(DefaultEventBus::new(16));
        let controller = DefaultRalphLoopController::with_event_bus(bus);
        assert!(controller.active_sessions().is_empty());
    }

    #[test]
    fn test_start_loop() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        controller
            .start(session_id, RalphLoopConfig::default())
            .unwrap();
        assert!(controller.is_active(session_id));

        let state = controller.get_state(session_id).unwrap();
        assert_eq!(state.status, RalphLoopStatus::Running);
        assert_eq!(state.current_iteration, 0);
    }

    #[test]
    fn test_start_with_custom_config() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        let config = RalphLoopConfig::for_rust();
        controller.start(session_id, config).unwrap();

        let cfg = controller.get_config(session_id).unwrap();
        assert_eq!(cfg.verification_command, "cargo test");
    }

    #[test]
    fn test_start_duplicate_error() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        controller
            .start(session_id, RalphLoopConfig::default())
            .unwrap();
        assert!(controller
            .start(session_id, RalphLoopConfig::default())
            .is_err());
    }

    #[test]
    fn test_start_invalid_config() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        let config = RalphLoopConfig {
            verification_command: String::new(), // Invalid
            ..Default::default()
        };

        assert!(controller.start(session_id, config).is_err());
    }

    #[test]
    fn test_pause_resume() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        controller
            .start(session_id, RalphLoopConfig::default())
            .unwrap();

        controller.pause(session_id).unwrap();
        assert_eq!(
            controller.get_state(session_id).unwrap().status,
            RalphLoopStatus::Paused
        );

        controller.resume(session_id).unwrap();
        assert_eq!(
            controller.get_state(session_id).unwrap().status,
            RalphLoopStatus::Running
        );
    }

    #[test]
    fn test_pause_not_running_error() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        controller
            .start(session_id, RalphLoopConfig::default())
            .unwrap();
        controller.pause(session_id).unwrap();

        // Already paused
        assert!(controller.pause(session_id).is_err());
    }

    #[test]
    fn test_resume_not_paused_error() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        controller
            .start(session_id, RalphLoopConfig::default())
            .unwrap();

        // Running, cannot resume
        assert!(controller.resume(session_id).is_err());
    }

    #[test]
    fn test_stop() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        controller
            .start(session_id, RalphLoopConfig::default())
            .unwrap();
        controller.stop(session_id).unwrap();

        assert!(!controller.is_active(session_id));
        assert_eq!(
            controller.get_state(session_id).unwrap().status,
            RalphLoopStatus::Cancelled
        );
    }

    #[test]
    fn test_stop_already_finished_error() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        controller
            .start(session_id, RalphLoopConfig::default())
            .unwrap();
        controller.stop(session_id).unwrap();

        // Already cancelled
        assert!(controller.stop(session_id).is_err());
    }

    #[test]
    fn test_record_iteration_pass() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        controller
            .start(session_id, RalphLoopConfig::default())
            .unwrap();

        let result = IterationResult::new(1, true, "Fixed".to_string(), 1000);
        controller.record_iteration(session_id, result).unwrap();

        let state = controller.get_state(session_id).unwrap();
        assert_eq!(state.status, RalphLoopStatus::Completed);
        assert_eq!(state.current_iteration, 1);
        assert_eq!(state.iteration_history.len(), 1);
    }

    #[test]
    fn test_record_iteration_fail() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        let config = RalphLoopConfig {
            pause_on_error: false,
            ..Default::default()
        };
        controller.start(session_id, config).unwrap();

        let result = IterationResult::new(1, false, "Still failing".to_string(), 1000);
        controller.record_iteration(session_id, result).unwrap();

        let state = controller.get_state(session_id).unwrap();
        assert_eq!(state.status, RalphLoopStatus::Running); // Still running
        assert_eq!(state.iterations_without_progress, 1);
    }

    #[test]
    fn test_record_iteration_max_reached() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        let config = RalphLoopConfig {
            max_iterations: 2,
            pause_on_error: false,
            ..Default::default()
        };
        controller.start(session_id, config).unwrap();

        for i in 1..=2 {
            let result = IterationResult::new(i, false, "Failing".to_string(), 1000);
            controller.record_iteration(session_id, result).unwrap();
        }

        let state = controller.get_state(session_id).unwrap();
        assert_eq!(state.status, RalphLoopStatus::Failed);
    }

    #[test]
    fn test_record_iteration_with_test_failures() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        controller
            .start(session_id, RalphLoopConfig::default())
            .unwrap();

        let result = IterationResult::with_failures(1, true, "Fixed".to_string(), 1000, 0);
        controller.record_iteration(session_id, result).unwrap();

        let state = controller.get_state(session_id).unwrap();
        assert_eq!(state.iteration_history[0].test_failures, Some(0));
    }

    #[test]
    fn test_active_sessions() {
        let mut controller = DefaultRalphLoopController::new();

        controller
            .start(SessionId(1), RalphLoopConfig::default())
            .unwrap();
        controller
            .start(SessionId(2), RalphLoopConfig::default())
            .unwrap();

        let active = controller.active_sessions();
        assert_eq!(active.len(), 2);

        controller.stop(SessionId(1)).unwrap();
        let active = controller.active_sessions();
        assert_eq!(active.len(), 1);
        assert!(active.contains(&SessionId(2)));
    }

    #[test]
    fn test_all_sessions() {
        let mut controller = DefaultRalphLoopController::new();

        controller
            .start(SessionId(1), RalphLoopConfig::default())
            .unwrap();
        controller
            .start(SessionId(2), RalphLoopConfig::default())
            .unwrap();
        controller.stop(SessionId(1)).unwrap();

        let all = controller.all_sessions();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_cleanup_finished() {
        let mut controller = DefaultRalphLoopController::new();

        controller
            .start(SessionId(1), RalphLoopConfig::default())
            .unwrap();
        controller
            .start(SessionId(2), RalphLoopConfig::default())
            .unwrap();
        controller.stop(SessionId(1)).unwrap();

        let removed = controller.cleanup_finished();
        assert_eq!(removed, 1);
        assert_eq!(controller.all_sessions().len(), 1);
    }

    #[test]
    fn test_iteration_delay_ms() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        let config = RalphLoopConfig {
            iteration_delay_ms: 500,
            ..Default::default()
        };
        controller.start(session_id, config).unwrap();

        assert_eq!(controller.iteration_delay_ms(session_id), Some(500));
        assert_eq!(controller.iteration_delay_ms(SessionId(999)), None);
    }

    #[test]
    fn test_should_compact() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        let config = RalphLoopConfig {
            auto_compact: true,
            compact_threshold: 0.8,
            ..Default::default()
        };
        controller.start(session_id, config).unwrap();

        assert!(!controller.should_compact(session_id, 0.5));
        assert!(controller.should_compact(session_id, 0.8));
        assert!(controller.should_compact(session_id, 0.9));
    }

    #[test]
    fn test_should_compact_disabled() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        let config = RalphLoopConfig {
            auto_compact: false,
            compact_threshold: 0.8,
            ..Default::default()
        };
        controller.start(session_id, config).unwrap();

        assert!(!controller.should_compact(session_id, 0.9));
    }

    #[test]
    fn test_stats_empty() {
        let controller = DefaultRalphLoopController::new();
        let stats = controller.stats();

        assert_eq!(stats.total_loops, 0);
        assert_eq!(stats.active_loops, 0);
        assert!((stats.success_rate() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_stats_with_loops() {
        let mut controller = DefaultRalphLoopController::new();

        // Create and complete a loop
        controller
            .start(SessionId(1), RalphLoopConfig::default())
            .unwrap();
        let result = IterationResult::new(1, true, "Done".to_string(), 1000);
        controller.record_iteration(SessionId(1), result).unwrap();

        // Create and fail a loop
        let config = RalphLoopConfig {
            max_iterations: 1,
            pause_on_error: false,
            ..Default::default()
        };
        controller.start(SessionId(2), config).unwrap();
        let result = IterationResult::new(1, false, "Failed".to_string(), 1000);
        controller.record_iteration(SessionId(2), result).unwrap();

        // Create an active loop
        controller
            .start(SessionId(3), RalphLoopConfig::default())
            .unwrap();

        let stats = controller.stats();
        assert_eq!(stats.total_loops, 3);
        assert_eq!(stats.active_loops, 1);
        assert_eq!(stats.completed_loops, 1);
        assert_eq!(stats.failed_loops, 1);
        assert_eq!(stats.total_iterations, 2);
        assert_eq!(stats.successful_iterations, 1);
        assert_eq!(stats.failed_iterations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_get_state_mut() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        controller
            .start(session_id, RalphLoopConfig::default())
            .unwrap();

        if let Some(state) = controller.get_state_mut(session_id) {
            state.last_error = Some("Custom error".to_string());
        }

        assert_eq!(
            controller.get_state(session_id).unwrap().last_error,
            Some("Custom error".to_string())
        );
    }

    #[test]
    fn test_no_loop_errors() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(999);

        assert!(controller.pause(session_id).is_err());
        assert!(controller.resume(session_id).is_err());
        assert!(controller.stop(session_id).is_err());
        assert!(controller
            .record_iteration(
                session_id,
                IterationResult::new(1, true, "Done".to_string(), 1000)
            )
            .is_err());
    }

    #[test]
    fn test_stuck_sessions() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        let config = RalphLoopConfig {
            stuck_threshold: 3,
            max_iterations: 20,
            pause_on_error: false,
            ..Default::default()
        };
        controller.start(session_id, config).unwrap();

        // Record 3 failed iterations
        for i in 1..=3 {
            let result = IterationResult::new(i, false, "Failed".to_string(), 1000);
            controller.record_iteration(session_id, result).unwrap();
        }

        let stuck = controller.stuck_sessions();
        assert_eq!(stuck.len(), 1);
        assert_eq!(stuck[0], session_id);
    }

    #[test]
    fn test_pause_on_error() {
        let mut controller = DefaultRalphLoopController::new();
        let session_id = SessionId(1);

        let config = RalphLoopConfig {
            pause_on_error: true,
            ..Default::default()
        };
        controller.start(session_id, config).unwrap();

        // First failure should pause
        let result = IterationResult::new(1, false, "Failed".to_string(), 1000);
        controller.record_iteration(session_id, result).unwrap();

        let state = controller.get_state(session_id).unwrap();
        assert_eq!(state.status, RalphLoopStatus::Paused);
    }

    #[test]
    fn test_event_emission() {
        let bus = Arc::new(DefaultEventBus::new(16));
        let mut rx = bus.subscribe();
        let mut controller = DefaultRalphLoopController::with_event_bus(bus);
        let session_id = SessionId(1);

        controller
            .start(session_id, RalphLoopConfig::default())
            .unwrap();

        // Should have received RalphLoopStarted event
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, CodirigentEvent::RalphLoopStarted { .. }));
    }

    #[test]
    fn test_loop_count() {
        let mut controller = DefaultRalphLoopController::new();

        assert_eq!(controller.loop_count(), 0);

        controller
            .start(SessionId(1), RalphLoopConfig::default())
            .unwrap();
        assert_eq!(controller.loop_count(), 1);

        controller
            .start(SessionId(2), RalphLoopConfig::default())
            .unwrap();
        assert_eq!(controller.loop_count(), 2);

        controller.stop(SessionId(1)).unwrap();
        assert_eq!(controller.loop_count(), 1); // Only counts active
    }
}
