//! Ralph Loop types for autonomous task execution.
//!
//! Named after Geoffrey Huntley's deterministic loop technique, the Ralph Loop
//! enables sessions to run autonomously: execute task, run tests, fix errors, repeat.
//!
//! The loop follows a Read-Act-Learn-Persist-Handoff cycle:
//! 1. **Read**: Parse output and understand current state
//! 2. **Act**: Execute the next action (code change, test run, etc.)
//! 3. **Learn**: Track patterns of success/failure
//! 4. **Persist**: Save learnings for future sessions
//! 5. **Handoff**: Transfer to another session if needed
//!
//! # Example
//!
//! ```
//! use codirigent_core::ralph::{RalphLoopConfig, RalphLoopState, RalphLoopStatus};
//!
//! // Create a config for a Rust project
//! let config = RalphLoopConfig::for_rust();
//! assert_eq!(config.verification_command, "cargo test");
//!
//! // Create initial loop state
//! let state = RalphLoopState::new();
//! assert_eq!(state.status, RalphLoopStatus::Running);
//! assert_eq!(state.current_iteration, 0);
//! ```

use serde::{Deserialize, Serialize};

/// Configuration for a Ralph Loop.
///
/// Controls how the autonomous execution loop behaves, including
/// iteration limits, verification commands, and context management.
///
/// # Example
///
/// ```
/// use codirigent_core::ralph::RalphLoopConfig;
///
/// let config = RalphLoopConfig::default();
/// assert_eq!(config.max_iterations, 20);
/// assert!(config.auto_compact);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RalphLoopConfig {
    /// Maximum number of iterations before stopping.
    pub max_iterations: u32,
    /// Command to verify task completion (e.g., "npm test", "cargo test").
    pub verification_command: String,
    /// Whether to auto-compact context when threshold is reached.
    pub auto_compact: bool,
    /// Context usage threshold to trigger compaction (0.0 - 1.0).
    pub compact_threshold: f32,
    /// Delay between iterations in milliseconds.
    pub iteration_delay_ms: u64,
    /// Whether to pause automatically on error detection.
    pub pause_on_error: bool,
    /// Number of iterations without progress before detecting stuck state.
    pub stuck_threshold: u32,
}

impl Default for RalphLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            verification_command: "npm test".to_string(),
            auto_compact: true,
            compact_threshold: 0.9,
            iteration_delay_ms: 1000,
            pause_on_error: true,
            stuck_threshold: 5,
        }
    }
}

impl RalphLoopConfig {
    /// Create a configuration for Rust projects.
    ///
    /// Uses `cargo test` as the verification command.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::RalphLoopConfig;
    ///
    /// let config = RalphLoopConfig::for_rust();
    /// assert_eq!(config.verification_command, "cargo test");
    /// ```
    pub fn for_rust() -> Self {
        Self {
            verification_command: "cargo test".to_string(),
            ..Self::default()
        }
    }

    /// Create a configuration for Python projects.
    ///
    /// Uses `pytest` as the verification command.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::RalphLoopConfig;
    ///
    /// let config = RalphLoopConfig::for_python();
    /// assert_eq!(config.verification_command, "pytest");
    /// ```
    pub fn for_python() -> Self {
        Self {
            verification_command: "pytest".to_string(),
            ..Self::default()
        }
    }

    /// Create a configuration for JavaScript/TypeScript projects.
    ///
    /// Uses `npm test` as the verification command.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::RalphLoopConfig;
    ///
    /// let config = RalphLoopConfig::for_javascript();
    /// assert_eq!(config.verification_command, "npm test");
    /// ```
    pub fn for_javascript() -> Self {
        Self {
            verification_command: "npm test".to_string(),
            ..Self::default()
        }
    }

    /// Create a configuration for overnight runs (more iterations, no pause).
    ///
    /// Suitable for unattended operation with 100 max iterations and
    /// disabled pause-on-error.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::RalphLoopConfig;
    ///
    /// let config = RalphLoopConfig::overnight();
    /// assert_eq!(config.max_iterations, 100);
    /// assert!(!config.pause_on_error);
    /// ```
    pub fn overnight() -> Self {
        Self {
            max_iterations: 100,
            pause_on_error: false,
            iteration_delay_ms: 2000,
            ..Self::default()
        }
    }

    /// Create a configuration with a custom verification command.
    ///
    /// # Arguments
    ///
    /// * `command` - The verification command to run
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::RalphLoopConfig;
    ///
    /// let config = RalphLoopConfig::with_command("make test".to_string());
    /// assert_eq!(config.verification_command, "make test");
    /// ```
    pub fn with_command(command: String) -> Self {
        Self {
            verification_command: command,
            ..Self::default()
        }
    }

    /// Set the maximum number of iterations.
    ///
    /// # Arguments
    ///
    /// * `max` - Maximum iteration count
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    /// Set whether to auto-compact context.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether auto-compact is enabled
    pub fn with_auto_compact(mut self, enabled: bool) -> Self {
        self.auto_compact = enabled;
        self
    }

    /// Validate the configuration.
    ///
    /// Returns true if all values are within valid ranges.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::RalphLoopConfig;
    ///
    /// let config = RalphLoopConfig::default();
    /// assert!(config.is_valid());
    ///
    /// let mut invalid = RalphLoopConfig::default();
    /// invalid.compact_threshold = 1.5; // Invalid: > 1.0
    /// assert!(!invalid.is_valid());
    /// ```
    pub fn is_valid(&self) -> bool {
        self.max_iterations > 0
            && !self.verification_command.is_empty()
            && (0.0..=1.0).contains(&self.compact_threshold)
            && self.stuck_threshold > 0
    }
}

/// Current state of a Ralph Loop.
///
/// Tracks the execution progress, iteration history, and timing information
/// for an active or completed Ralph Loop.
///
/// # Example
///
/// ```
/// use codirigent_core::ralph::{RalphLoopState, RalphLoopStatus};
///
/// let state = RalphLoopState::new();
/// assert_eq!(state.current_iteration, 0);
/// assert_eq!(state.status, RalphLoopStatus::Running);
/// assert!(state.iteration_history.is_empty());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RalphLoopState {
    /// Current iteration number (0-indexed, 0 means not yet started).
    pub current_iteration: u32,
    /// Status of the loop.
    pub status: RalphLoopStatus,
    /// Results from each iteration.
    pub iteration_history: Vec<IterationResult>,
    /// When the loop started.
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// When the loop ended, if finished.
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Last error message, if any.
    pub last_error: Option<String>,
    /// Consecutive iterations without progress.
    pub iterations_without_progress: u32,
}

impl RalphLoopState {
    /// Create a new loop state.
    ///
    /// Initializes with status `Running` and iteration 0.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::{RalphLoopState, RalphLoopStatus};
    ///
    /// let state = RalphLoopState::new();
    /// assert_eq!(state.status, RalphLoopStatus::Running);
    /// ```
    pub fn new() -> Self {
        Self {
            current_iteration: 0,
            status: RalphLoopStatus::Running,
            iteration_history: Vec::new(),
            started_at: chrono::Utc::now(),
            ended_at: None,
            last_error: None,
            iterations_without_progress: 0,
        }
    }

    /// Get completion percentage based on max iterations.
    ///
    /// # Arguments
    ///
    /// * `max_iterations` - The maximum iteration count from config
    ///
    /// # Returns
    ///
    /// A value between 0.0 and 1.0 representing progress.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::RalphLoopState;
    ///
    /// let mut state = RalphLoopState::new();
    /// state.current_iteration = 5;
    /// assert!((state.progress_percent(20) - 0.25).abs() < f32::EPSILON);
    /// ```
    pub fn progress_percent(&self, max_iterations: u32) -> f32 {
        if max_iterations == 0 {
            return 0.0;
        }
        (self.current_iteration as f32 / max_iterations as f32).min(1.0)
    }

    /// Get elapsed time since the loop started.
    ///
    /// If the loop has ended, returns the total duration.
    /// If still running, returns the time since start.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::RalphLoopState;
    ///
    /// let state = RalphLoopState::new();
    /// let elapsed = state.elapsed();
    /// // elapsed should be very small since we just created it
    /// assert!(elapsed.num_seconds() < 1);
    /// ```
    pub fn elapsed(&self) -> chrono::Duration {
        let end = self.ended_at.unwrap_or_else(chrono::Utc::now);
        end - self.started_at
    }

    /// Check if the last iteration passed verification.
    ///
    /// Returns false if no iterations have been recorded yet.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::{RalphLoopState, IterationResult};
    ///
    /// let mut state = RalphLoopState::new();
    /// assert!(!state.last_iteration_passed());
    ///
    /// state.iteration_history.push(IterationResult::new(1, true, "Passed".to_string(), 1000));
    /// assert!(state.last_iteration_passed());
    /// ```
    pub fn last_iteration_passed(&self) -> bool {
        self.iteration_history
            .last()
            .map(|r| r.verification_passed)
            .unwrap_or(false)
    }

    /// Get total test failures across all iterations.
    ///
    /// Only counts iterations where test_failures was reported.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::{RalphLoopState, IterationResult};
    ///
    /// let mut state = RalphLoopState::new();
    /// let mut result = IterationResult::new(1, false, "Failed".to_string(), 1000);
    /// result.test_failures = Some(3);
    /// state.iteration_history.push(result);
    ///
    /// assert_eq!(state.total_test_failures(), 3);
    /// ```
    pub fn total_test_failures(&self) -> u32 {
        self.iteration_history
            .iter()
            .filter_map(|r| r.test_failures)
            .sum()
    }

    /// Get the count of successful iterations.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::{RalphLoopState, IterationResult};
    ///
    /// let mut state = RalphLoopState::new();
    /// state.iteration_history.push(IterationResult::new(1, true, "Passed".to_string(), 1000));
    /// state.iteration_history.push(IterationResult::new(2, false, "Failed".to_string(), 1000));
    /// state.iteration_history.push(IterationResult::new(3, true, "Passed".to_string(), 1000));
    ///
    /// assert_eq!(state.successful_iterations(), 2);
    /// ```
    pub fn successful_iterations(&self) -> u32 {
        self.iteration_history
            .iter()
            .filter(|r| r.verification_passed)
            .count() as u32
    }

    /// Get the count of failed iterations.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::{RalphLoopState, IterationResult};
    ///
    /// let mut state = RalphLoopState::new();
    /// state.iteration_history.push(IterationResult::new(1, true, "Passed".to_string(), 1000));
    /// state.iteration_history.push(IterationResult::new(2, false, "Failed".to_string(), 1000));
    ///
    /// assert_eq!(state.failed_iterations(), 1);
    /// ```
    pub fn failed_iterations(&self) -> u32 {
        self.iteration_history
            .iter()
            .filter(|r| !r.verification_passed)
            .count() as u32
    }

    /// Get total duration of all iterations in milliseconds.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::{RalphLoopState, IterationResult};
    ///
    /// let mut state = RalphLoopState::new();
    /// state.iteration_history.push(IterationResult::new(1, true, "Done".to_string(), 1000));
    /// state.iteration_history.push(IterationResult::new(2, true, "Done".to_string(), 2000));
    ///
    /// assert_eq!(state.total_duration_ms(), 3000);
    /// ```
    pub fn total_duration_ms(&self) -> u64 {
        self.iteration_history.iter().map(|r| r.duration_ms).sum()
    }

    /// Mark the loop as completed successfully.
    pub fn mark_completed(&mut self) {
        self.status = RalphLoopStatus::Completed;
        self.ended_at = Some(chrono::Utc::now());
    }

    /// Mark the loop as failed.
    ///
    /// # Arguments
    ///
    /// * `reason` - The reason for failure
    pub fn mark_failed(&mut self, reason: String) {
        self.status = RalphLoopStatus::Failed;
        self.ended_at = Some(chrono::Utc::now());
        self.last_error = Some(reason);
    }

    /// Mark the loop as cancelled.
    pub fn mark_cancelled(&mut self) {
        self.status = RalphLoopStatus::Cancelled;
        self.ended_at = Some(chrono::Utc::now());
    }
}

impl Default for RalphLoopState {
    fn default() -> Self {
        Self::new()
    }
}

/// Status of a Ralph Loop.
///
/// Represents the current execution state of the autonomous loop.
///
/// # State Transitions
///
/// ```text
/// Running -> Paused (pause called)
/// Running -> Completed (verification passed)
/// Running -> Failed (max iterations or error)
/// Running -> Cancelled (user cancelled)
/// Paused -> Running (resume called)
/// Paused -> Cancelled (user cancelled)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RalphLoopStatus {
    /// Loop is actively running.
    #[default]
    Running,
    /// Loop is paused (can be resumed).
    Paused,
    /// Loop completed successfully (verification passed).
    Completed,
    /// Loop failed (max iterations reached or unrecoverable error).
    Failed,
    /// Loop was cancelled by user.
    Cancelled,
}

impl RalphLoopStatus {
    /// Check if the loop is finished (cannot continue).
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::RalphLoopStatus;
    ///
    /// assert!(!RalphLoopStatus::Running.is_finished());
    /// assert!(!RalphLoopStatus::Paused.is_finished());
    /// assert!(RalphLoopStatus::Completed.is_finished());
    /// assert!(RalphLoopStatus::Failed.is_finished());
    /// assert!(RalphLoopStatus::Cancelled.is_finished());
    /// ```
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Check if the loop can be resumed.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::RalphLoopStatus;
    ///
    /// assert!(!RalphLoopStatus::Running.can_resume());
    /// assert!(RalphLoopStatus::Paused.can_resume());
    /// assert!(!RalphLoopStatus::Completed.can_resume());
    /// ```
    pub fn can_resume(&self) -> bool {
        matches!(self, Self::Paused)
    }

    /// Check if the loop can be paused.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::RalphLoopStatus;
    ///
    /// assert!(RalphLoopStatus::Running.can_pause());
    /// assert!(!RalphLoopStatus::Paused.can_pause());
    /// assert!(!RalphLoopStatus::Completed.can_pause());
    /// ```
    pub fn can_pause(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Get a human-readable description of the status.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::RalphLoopStatus;
    ///
    /// assert_eq!(RalphLoopStatus::Running.description(), "Running");
    /// assert_eq!(RalphLoopStatus::Completed.description(), "Completed");
    /// ```
    pub fn description(&self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Paused => "Paused",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl std::fmt::Display for RalphLoopStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Result of a single iteration in a Ralph Loop.
///
/// Captures the outcome, timing, and summary of one execution cycle.
///
/// # Example
///
/// ```
/// use codirigent_core::ralph::IterationResult;
///
/// let result = IterationResult::new(1, true, "Fixed the bug".to_string(), 5000);
/// assert!(result.verification_passed);
/// assert_eq!(result.iteration, 1);
/// assert_eq!(result.duration_ms, 5000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationResult {
    /// Iteration number (1-indexed).
    pub iteration: u32,
    /// Whether verification passed.
    pub verification_passed: bool,
    /// Number of test failures, if available.
    pub test_failures: Option<u32>,
    /// Brief summary of actions taken.
    pub summary: String,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Timestamp when iteration completed.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl IterationResult {
    /// Create a new iteration result.
    ///
    /// # Arguments
    ///
    /// * `iteration` - The iteration number (1-indexed)
    /// * `passed` - Whether verification passed
    /// * `summary` - Brief description of what was done
    /// * `duration_ms` - Duration of the iteration in milliseconds
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::IterationResult;
    ///
    /// let result = IterationResult::new(1, true, "All tests pass".to_string(), 3500);
    /// assert_eq!(result.iteration, 1);
    /// assert!(result.verification_passed);
    /// ```
    pub fn new(iteration: u32, passed: bool, summary: String, duration_ms: u64) -> Self {
        Self {
            iteration,
            verification_passed: passed,
            test_failures: None,
            summary,
            duration_ms,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a new iteration result with test failure count.
    ///
    /// # Arguments
    ///
    /// * `iteration` - The iteration number (1-indexed)
    /// * `passed` - Whether verification passed
    /// * `summary` - Brief description of what was done
    /// * `duration_ms` - Duration of the iteration in milliseconds
    /// * `test_failures` - Number of test failures
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::ralph::IterationResult;
    ///
    /// let result = IterationResult::with_failures(2, false, "3 tests failing".to_string(), 4500, 3);
    /// assert_eq!(result.test_failures, Some(3));
    /// ```
    pub fn with_failures(
        iteration: u32,
        passed: bool,
        summary: String,
        duration_ms: u64,
        test_failures: u32,
    ) -> Self {
        Self {
            iteration,
            verification_passed: passed,
            test_failures: Some(test_failures),
            summary,
            duration_ms,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Get a short status indicator.
    ///
    /// Returns a checkmark or X based on verification status.
    pub fn status_indicator(&self) -> &'static str {
        if self.verification_passed {
            "pass"
        } else {
            "fail"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // RalphLoopConfig tests
    #[test]
    fn test_default_config() {
        let config = RalphLoopConfig::default();
        assert_eq!(config.max_iterations, 20);
        assert_eq!(config.verification_command, "npm test");
        assert!(config.auto_compact);
        assert!((config.compact_threshold - 0.9).abs() < f32::EPSILON);
        assert_eq!(config.iteration_delay_ms, 1000);
        assert!(config.pause_on_error);
        assert_eq!(config.stuck_threshold, 5);
    }

    #[test]
    fn test_rust_config() {
        let config = RalphLoopConfig::for_rust();
        assert_eq!(config.verification_command, "cargo test");
        assert_eq!(config.max_iterations, 20); // Other defaults preserved
    }

    #[test]
    fn test_python_config() {
        let config = RalphLoopConfig::for_python();
        assert_eq!(config.verification_command, "pytest");
    }

    #[test]
    fn test_javascript_config() {
        let config = RalphLoopConfig::for_javascript();
        assert_eq!(config.verification_command, "npm test");
    }

    #[test]
    fn test_overnight_config() {
        let config = RalphLoopConfig::overnight();
        assert_eq!(config.max_iterations, 100);
        assert!(!config.pause_on_error);
        assert_eq!(config.iteration_delay_ms, 2000);
    }

    #[test]
    fn test_with_command() {
        let config = RalphLoopConfig::with_command("make test".to_string());
        assert_eq!(config.verification_command, "make test");
    }

    #[test]
    fn test_builder_methods() {
        let config = RalphLoopConfig::default()
            .with_max_iterations(50)
            .with_auto_compact(false);
        assert_eq!(config.max_iterations, 50);
        assert!(!config.auto_compact);
    }

    #[test]
    fn test_config_is_valid() {
        let config = RalphLoopConfig::default();
        assert!(config.is_valid());
    }

    #[test]
    fn test_config_invalid_empty_command() {
        let config = RalphLoopConfig {
            verification_command: String::new(),
            ..Default::default()
        };
        assert!(!config.is_valid());
    }

    #[test]
    fn test_config_invalid_threshold() {
        let config = RalphLoopConfig {
            compact_threshold: 1.5,
            ..Default::default()
        };
        assert!(!config.is_valid());
    }

    #[test]
    fn test_config_invalid_zero_iterations() {
        let config = RalphLoopConfig {
            max_iterations: 0,
            ..Default::default()
        };
        assert!(!config.is_valid());
    }

    #[test]
    fn test_config_invalid_zero_stuck_threshold() {
        let config = RalphLoopConfig {
            stuck_threshold: 0,
            ..Default::default()
        };
        assert!(!config.is_valid());
    }

    #[test]
    fn test_config_serialization() {
        let config = RalphLoopConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: RalphLoopConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn test_config_equality() {
        let config1 = RalphLoopConfig::default();
        let config2 = RalphLoopConfig::default();
        assert_eq!(config1, config2);

        let config3 = RalphLoopConfig::for_rust();
        assert_ne!(config1, config3);
    }

    // RalphLoopState tests
    #[test]
    fn test_state_new() {
        let state = RalphLoopState::new();
        assert_eq!(state.current_iteration, 0);
        assert_eq!(state.status, RalphLoopStatus::Running);
        assert!(state.iteration_history.is_empty());
        assert!(state.ended_at.is_none());
        assert!(state.last_error.is_none());
        assert_eq!(state.iterations_without_progress, 0);
    }

    #[test]
    fn test_state_default() {
        let state = RalphLoopState::default();
        assert_eq!(state.status, RalphLoopStatus::Running);
    }

    #[test]
    fn test_progress_percent() {
        let mut state = RalphLoopState::new();
        state.current_iteration = 5;
        assert!((state.progress_percent(20) - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn test_progress_percent_zero_max() {
        let state = RalphLoopState::new();
        assert!((state.progress_percent(0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_progress_percent_exceeds_max() {
        let mut state = RalphLoopState::new();
        state.current_iteration = 30;
        assert!((state.progress_percent(20) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_elapsed() {
        let state = RalphLoopState::new();
        let elapsed = state.elapsed();
        assert!(elapsed.num_seconds() >= 0);
    }

    #[test]
    fn test_last_iteration_passed_empty() {
        let state = RalphLoopState::new();
        assert!(!state.last_iteration_passed());
    }

    #[test]
    fn test_last_iteration_passed_true() {
        let mut state = RalphLoopState::new();
        state
            .iteration_history
            .push(IterationResult::new(1, true, "Done".to_string(), 1000));
        assert!(state.last_iteration_passed());
    }

    #[test]
    fn test_last_iteration_passed_false() {
        let mut state = RalphLoopState::new();
        state
            .iteration_history
            .push(IterationResult::new(1, false, "Failed".to_string(), 1000));
        assert!(!state.last_iteration_passed());
    }

    #[test]
    fn test_total_test_failures() {
        let mut state = RalphLoopState::new();
        let mut r1 = IterationResult::new(1, false, "Failed".to_string(), 1000);
        r1.test_failures = Some(3);
        let mut r2 = IterationResult::new(2, false, "Failed".to_string(), 1000);
        r2.test_failures = Some(2);
        state.iteration_history.push(r1);
        state.iteration_history.push(r2);
        assert_eq!(state.total_test_failures(), 5);
    }

    #[test]
    fn test_successful_iterations() {
        let mut state = RalphLoopState::new();
        state
            .iteration_history
            .push(IterationResult::new(1, true, "Done".to_string(), 1000));
        state
            .iteration_history
            .push(IterationResult::new(2, false, "Failed".to_string(), 1000));
        state
            .iteration_history
            .push(IterationResult::new(3, true, "Done".to_string(), 1000));
        assert_eq!(state.successful_iterations(), 2);
    }

    #[test]
    fn test_failed_iterations() {
        let mut state = RalphLoopState::new();
        state
            .iteration_history
            .push(IterationResult::new(1, true, "Done".to_string(), 1000));
        state
            .iteration_history
            .push(IterationResult::new(2, false, "Failed".to_string(), 1000));
        assert_eq!(state.failed_iterations(), 1);
    }

    #[test]
    fn test_total_duration_ms() {
        let mut state = RalphLoopState::new();
        state
            .iteration_history
            .push(IterationResult::new(1, true, "Done".to_string(), 1000));
        state
            .iteration_history
            .push(IterationResult::new(2, true, "Done".to_string(), 2500));
        assert_eq!(state.total_duration_ms(), 3500);
    }

    #[test]
    fn test_mark_completed() {
        let mut state = RalphLoopState::new();
        state.mark_completed();
        assert_eq!(state.status, RalphLoopStatus::Completed);
        assert!(state.ended_at.is_some());
    }

    #[test]
    fn test_mark_failed() {
        let mut state = RalphLoopState::new();
        state.mark_failed("Max iterations".to_string());
        assert_eq!(state.status, RalphLoopStatus::Failed);
        assert!(state.ended_at.is_some());
        assert_eq!(state.last_error, Some("Max iterations".to_string()));
    }

    #[test]
    fn test_mark_cancelled() {
        let mut state = RalphLoopState::new();
        state.mark_cancelled();
        assert_eq!(state.status, RalphLoopStatus::Cancelled);
        assert!(state.ended_at.is_some());
    }

    #[test]
    fn test_state_serialization() {
        let mut state = RalphLoopState::new();
        state
            .iteration_history
            .push(IterationResult::new(1, true, "Done".to_string(), 1000));
        let json = serde_json::to_string(&state).unwrap();
        let parsed: RalphLoopState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.current_iteration, state.current_iteration);
        assert_eq!(parsed.status, state.status);
        assert_eq!(parsed.iteration_history.len(), 1);
    }

    // RalphLoopStatus tests
    #[test]
    fn test_status_default() {
        assert_eq!(RalphLoopStatus::default(), RalphLoopStatus::Running);
    }

    #[test]
    fn test_status_is_finished() {
        assert!(!RalphLoopStatus::Running.is_finished());
        assert!(!RalphLoopStatus::Paused.is_finished());
        assert!(RalphLoopStatus::Completed.is_finished());
        assert!(RalphLoopStatus::Failed.is_finished());
        assert!(RalphLoopStatus::Cancelled.is_finished());
    }

    #[test]
    fn test_status_can_resume() {
        assert!(!RalphLoopStatus::Running.can_resume());
        assert!(RalphLoopStatus::Paused.can_resume());
        assert!(!RalphLoopStatus::Completed.can_resume());
        assert!(!RalphLoopStatus::Failed.can_resume());
        assert!(!RalphLoopStatus::Cancelled.can_resume());
    }

    #[test]
    fn test_status_can_pause() {
        assert!(RalphLoopStatus::Running.can_pause());
        assert!(!RalphLoopStatus::Paused.can_pause());
        assert!(!RalphLoopStatus::Completed.can_pause());
        assert!(!RalphLoopStatus::Failed.can_pause());
        assert!(!RalphLoopStatus::Cancelled.can_pause());
    }

    #[test]
    fn test_status_description() {
        assert_eq!(RalphLoopStatus::Running.description(), "Running");
        assert_eq!(RalphLoopStatus::Paused.description(), "Paused");
        assert_eq!(RalphLoopStatus::Completed.description(), "Completed");
        assert_eq!(RalphLoopStatus::Failed.description(), "Failed");
        assert_eq!(RalphLoopStatus::Cancelled.description(), "Cancelled");
    }

    #[test]
    fn test_status_display() {
        assert_eq!(format!("{}", RalphLoopStatus::Running), "Running");
        assert_eq!(format!("{}", RalphLoopStatus::Completed), "Completed");
    }

    #[test]
    fn test_status_serialization() {
        let statuses = [
            RalphLoopStatus::Running,
            RalphLoopStatus::Paused,
            RalphLoopStatus::Completed,
            RalphLoopStatus::Failed,
            RalphLoopStatus::Cancelled,
        ];
        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: RalphLoopStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, parsed);
        }
    }

    // IterationResult tests
    #[test]
    fn test_iteration_result_new() {
        let result = IterationResult::new(1, true, "Fixed bug".to_string(), 5000);
        assert_eq!(result.iteration, 1);
        assert!(result.verification_passed);
        assert!(result.test_failures.is_none());
        assert_eq!(result.summary, "Fixed bug");
        assert_eq!(result.duration_ms, 5000);
    }

    #[test]
    fn test_iteration_result_with_failures() {
        let result =
            IterationResult::with_failures(2, false, "3 tests failing".to_string(), 4500, 3);
        assert_eq!(result.iteration, 2);
        assert!(!result.verification_passed);
        assert_eq!(result.test_failures, Some(3));
    }

    #[test]
    fn test_iteration_result_status_indicator() {
        let passed = IterationResult::new(1, true, "Done".to_string(), 1000);
        assert_eq!(passed.status_indicator(), "pass");

        let failed = IterationResult::new(2, false, "Failed".to_string(), 1000);
        assert_eq!(failed.status_indicator(), "fail");
    }

    #[test]
    fn test_iteration_result_serialization() {
        let result = IterationResult::with_failures(1, true, "Done".to_string(), 1000, 0);
        let json = serde_json::to_string(&result).unwrap();
        let parsed: IterationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.iteration, result.iteration);
        assert_eq!(parsed.verification_passed, result.verification_passed);
        assert_eq!(parsed.test_failures, result.test_failures);
    }

    #[test]
    fn test_iteration_result_clone() {
        let result = IterationResult::new(1, true, "Done".to_string(), 1000);
        let cloned = result.clone();
        assert_eq!(cloned.iteration, result.iteration);
        assert_eq!(cloned.summary, result.summary);
    }

    #[test]
    fn test_iteration_result_debug() {
        let result = IterationResult::new(1, true, "Done".to_string(), 1000);
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("IterationResult"));
        assert!(debug_str.contains("Done"));
    }
}
