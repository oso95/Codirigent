//! Advanced session features.
//!
//! This module provides types for advanced session management including:
//!
//! - **Context Handoff**: Transfer work-in-progress between sessions when
//!   context window is getting full
//! - **Session Templates**: Quick session creation with predefined settings
//! - **Session Groups**: Group related sessions for shared context
//! - **Overnight Mode**: Automated batch processing during off-hours
//!
//! # Example
//!
//! ```
//! use codirigent_core::session_advanced::{
//!     SessionTemplate, ContextHandoff, OvernightConfig,
//! };
//! use codirigent_core::SessionId;
//!
//! // Create a development template
//! let template = SessionTemplate::development();
//! assert_eq!(template.name, "development");
//!
//! // Create a handoff between sessions
//! let handoff = ContextHandoff::new(SessionId(1), SessionId(2));
//! assert_eq!(handoff.source_session, SessionId(1));
//!
//! // Use default overnight config
//! let config = OvernightConfig::default();
//! assert!(!config.enabled);
//! ```

use chrono::{Datelike, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::types::SessionId;

// ============================================================================
// Context Handoff Types
// ============================================================================

/// Context handoff request between sessions.
///
/// Used when a session's context window is getting full and work needs
/// to continue in a fresh session. The handoff captures the current state
/// of work and generates a prompt for the target session.
///
/// # Example
///
/// ```
/// use codirigent_core::session_advanced::ContextHandoff;
/// use codirigent_core::SessionId;
/// use std::path::PathBuf;
///
/// let mut handoff = ContextHandoff::new(SessionId(1), SessionId(2));
/// handoff.work_summary = "Implementing auth module".to_string();
/// handoff.relevant_files = vec![PathBuf::from("src/auth.rs")];
/// handoff.pending_task = Some("Fix remaining test failures".to_string());
///
/// let prompt = handoff.generate_prompt();
/// assert!(prompt.contains("auth module"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextHandoff {
    /// Source session (high context usage).
    pub source_session: SessionId,
    /// Target session (new or low context).
    pub target_session: SessionId,
    /// Summary of work in progress.
    pub work_summary: String,
    /// Files being worked on.
    pub relevant_files: Vec<PathBuf>,
    /// Pending task description.
    pub pending_task: Option<String>,
    /// When the handoff was initiated.
    pub initiated_at: chrono::DateTime<chrono::Utc>,
    /// Status of the handoff.
    pub status: HandoffStatus,
}

impl ContextHandoff {
    /// Create a new handoff request.
    ///
    /// # Arguments
    ///
    /// * `source` - The session with high context usage
    /// * `target` - The session to continue work in
    pub fn new(source: SessionId, target: SessionId) -> Self {
        Self {
            source_session: source,
            target_session: target,
            work_summary: String::new(),
            relevant_files: Vec::new(),
            pending_task: None,
            initiated_at: chrono::Utc::now(),
            status: HandoffStatus::Preparing,
        }
    }

    /// Set the work summary.
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.work_summary = summary.into();
        self
    }

    /// Add relevant files.
    pub fn with_files(mut self, files: Vec<PathBuf>) -> Self {
        self.relevant_files = files;
        self
    }

    /// Set the pending task.
    pub fn with_pending_task(mut self, task: impl Into<String>) -> Self {
        self.pending_task = Some(task.into());
        self
    }

    /// Generate the handoff prompt for the target session.
    ///
    /// Creates a structured prompt that provides context to the new session
    /// about the work being handed off.
    pub fn generate_prompt(&self) -> String {
        let mut prompt = String::new();

        prompt.push_str("## Context Handoff\n\n");
        prompt.push_str("You are continuing work from another session. Here's the context:\n\n");

        if !self.work_summary.is_empty() {
            prompt.push_str("### Work Summary\n");
            prompt.push_str(&self.work_summary);
            prompt.push_str("\n\n");
        }

        if !self.relevant_files.is_empty() {
            prompt.push_str("### Relevant Files\n");
            for file in &self.relevant_files {
                prompt.push_str(&format!("- {}\n", file.display()));
            }
            prompt.push('\n');
        }

        if let Some(ref task) = self.pending_task {
            prompt.push_str("### Pending Task\n");
            prompt.push_str(task);
            prompt.push_str("\n\n");
        }

        prompt.push_str("Please continue where the previous session left off.\n");

        prompt
    }

    /// Check if handoff is ready to be sent.
    pub fn is_ready(&self) -> bool {
        self.status == HandoffStatus::Ready
    }

    /// Check if handoff has completed.
    pub fn is_completed(&self) -> bool {
        self.status == HandoffStatus::Completed
    }

    /// Check if handoff has failed.
    pub fn is_failed(&self) -> bool {
        self.status == HandoffStatus::Failed
    }

    /// Mark handoff as ready.
    pub fn mark_ready(&mut self) {
        self.status = HandoffStatus::Ready;
    }

    /// Mark handoff as in progress.
    pub fn mark_in_progress(&mut self) {
        self.status = HandoffStatus::InProgress;
    }

    /// Mark handoff as completed.
    pub fn mark_completed(&mut self) {
        self.status = HandoffStatus::Completed;
    }

    /// Mark handoff as failed.
    pub fn mark_failed(&mut self) {
        self.status = HandoffStatus::Failed;
    }
}

/// Status of a context handoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HandoffStatus {
    /// Handoff initiated, generating summary.
    #[default]
    Preparing,
    /// Summary ready, waiting for target session.
    Ready,
    /// Handoff in progress (sending to target).
    InProgress,
    /// Handoff completed successfully.
    Completed,
    /// Handoff failed.
    Failed,
}

// ============================================================================
// Session Template Types
// ============================================================================

/// Session template for quick session creation.
///
/// Templates store common configurations for creating sessions
/// with predefined settings like environment variables, skills,
/// and group assignments.
///
/// # Example
///
/// ```
/// use codirigent_core::session_advanced::SessionTemplate;
/// use std::path::PathBuf;
///
/// // Use a predefined template
/// let dev = SessionTemplate::development();
/// assert!(dev.enabled_skills.contains(&"commit".to_string()));
///
/// // Create a custom template
/// let custom = SessionTemplate::new("custom".to_string(), "My custom setup".to_string())
///     .with_env("DEBUG", "1")
///     .with_skill("review-pr")
///     .with_working_directory(PathBuf::from("/projects/myapp"));
///
/// // Generate session names
/// assert_eq!(custom.generate_name(1), "Session 1");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionTemplate {
    /// Template name (unique identifier).
    pub name: String,
    /// Description of the template.
    pub description: String,
    /// Default working directory (relative or absolute).
    pub working_directory: Option<PathBuf>,
    /// Default session name pattern (supports {n} for number).
    pub name_pattern: String,
    /// Initial command to run (e.g., "claude").
    pub initial_command: Option<String>,
    /// Environment variables to set.
    pub environment: HashMap<String, String>,
    /// Skills to enable.
    pub enabled_skills: Vec<String>,
    /// Associated group name.
    pub group: Option<String>,
    /// Group color.
    pub color: Option<String>,
}

impl SessionTemplate {
    /// Create a new template with minimal configuration.
    ///
    /// # Arguments
    ///
    /// * `name` - Template name (unique identifier)
    /// * `description` - Description of when to use this template
    pub fn new(name: String, description: String) -> Self {
        Self {
            name,
            description,
            working_directory: None,
            name_pattern: "Session {n}".to_string(),
            initial_command: Some("claude".to_string()),
            environment: HashMap::new(),
            enabled_skills: Vec::new(),
            group: None,
            color: None,
        }
    }

    /// Create a default development template.
    ///
    /// Pre-configured for general development work with commit and edit skills.
    pub fn development() -> Self {
        Self {
            name: "development".to_string(),
            description: "Standard development session".to_string(),
            working_directory: None,
            name_pattern: "Dev {n}".to_string(),
            initial_command: Some("claude".to_string()),
            environment: HashMap::new(),
            enabled_skills: vec!["commit".to_string(), "edit".to_string()],
            group: Some("development".to_string()),
            color: Some("#4CAF50".to_string()), // Green
        }
    }

    /// Create a code review template.
    ///
    /// Pre-configured for reviewing code (read-only skills).
    pub fn review() -> Self {
        Self {
            name: "review".to_string(),
            description: "Code review session (read-only skills)".to_string(),
            working_directory: None,
            name_pattern: "Review {n}".to_string(),
            initial_command: Some("claude".to_string()),
            environment: HashMap::new(),
            enabled_skills: vec!["review-pr".to_string()],
            group: Some("review".to_string()),
            color: Some("#2196F3".to_string()), // Blue
        }
    }

    /// Create a testing template.
    ///
    /// Pre-configured for writing and running tests.
    pub fn testing() -> Self {
        Self {
            name: "testing".to_string(),
            description: "Testing session for writing and running tests".to_string(),
            working_directory: None,
            name_pattern: "Test {n}".to_string(),
            initial_command: Some("claude".to_string()),
            environment: HashMap::new(),
            enabled_skills: vec!["test".to_string()],
            group: Some("testing".to_string()),
            color: Some("#FF9800".to_string()), // Orange
        }
    }

    /// Generate a session name from the pattern.
    ///
    /// Replaces `{n}` in the pattern with the provided number.
    ///
    /// # Arguments
    ///
    /// * `number` - The session number to substitute
    pub fn generate_name(&self, number: u32) -> String {
        self.name_pattern.replace("{n}", &number.to_string())
    }

    /// Set the working directory.
    pub fn with_working_directory(mut self, dir: PathBuf) -> Self {
        self.working_directory = Some(dir);
        self
    }

    /// Set the name pattern.
    pub fn with_name_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.name_pattern = pattern.into();
        self
    }

    /// Set the initial command.
    pub fn with_initial_command(mut self, command: impl Into<String>) -> Self {
        self.initial_command = Some(command.into());
        self
    }

    /// Add an environment variable.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }

    /// Add a skill.
    pub fn with_skill(mut self, skill: impl Into<String>) -> Self {
        let skill = skill.into();
        if !self.enabled_skills.contains(&skill) {
            self.enabled_skills.push(skill);
        }
        self
    }

    /// Set the group.
    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Set the color.
    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Check if template has a specific skill enabled.
    pub fn has_skill(&self, skill: &str) -> bool {
        self.enabled_skills.iter().any(|s| s == skill)
    }
}

impl Default for SessionTemplate {
    fn default() -> Self {
        Self::development()
    }
}

// ============================================================================
// Session Group Types
// ============================================================================

/// Session group for organizing related sessions.
///
/// Groups allow sessions to share context, such as working on the same
/// feature or project. Sessions in a group can coordinate work and
/// share relevant information.
///
/// # Example
///
/// ```
/// use codirigent_core::session_advanced::SessionGroup;
/// use codirigent_core::SessionId;
///
/// let mut group = SessionGroup::new("backend".to_string())
///     .with_description("Backend API development")
///     .with_color("#FF5733");
///
/// group.add_session(SessionId(1));
/// group.add_session(SessionId(2));
///
/// assert!(group.contains(SessionId(1)));
/// assert_eq!(group.session_count(), 2);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionGroup {
    /// Group name (unique identifier).
    pub name: String,
    /// Group description.
    pub description: Option<String>,
    /// Group color for visual identification.
    pub color: Option<String>,
    /// Sessions in this group.
    pub sessions: Vec<SessionId>,
    /// Shared context notes for the group.
    pub shared_context: Option<String>,
    /// When the group was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl SessionGroup {
    /// Create a new session group.
    ///
    /// # Arguments
    ///
    /// * `name` - Group name (unique identifier)
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: None,
            color: None,
            sessions: Vec::new(),
            shared_context: None,
            created_at: chrono::Utc::now(),
        }
    }

    /// Set the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the color.
    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Set shared context.
    pub fn with_shared_context(mut self, context: impl Into<String>) -> Self {
        self.shared_context = Some(context.into());
        self
    }

    /// Add a session to the group.
    pub fn add_session(&mut self, session_id: SessionId) {
        if !self.sessions.contains(&session_id) {
            self.sessions.push(session_id);
        }
    }

    /// Remove a session from the group.
    pub fn remove_session(&mut self, session_id: SessionId) {
        self.sessions.retain(|&id| id != session_id);
    }

    /// Check if group contains a session.
    pub fn contains(&self, session_id: SessionId) -> bool {
        self.sessions.contains(&session_id)
    }

    /// Get the number of sessions in the group.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Check if the group is empty.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Update shared context.
    pub fn set_shared_context(&mut self, context: Option<String>) {
        self.shared_context = context;
    }
}

// ============================================================================
// Overnight Mode Types
// ============================================================================

/// Overnight mode configuration.
///
/// Allows scheduling batch work during off-hours with automatic
/// task processing and error handling. Overnight mode processes
/// queued tasks while you're away.
///
/// # Example
///
/// ```
/// use codirigent_core::session_advanced::OvernightConfig;
///
/// let mut config = OvernightConfig::default();
/// assert!(!config.enabled);
/// assert_eq!(config.start_hour, 22); // 10 PM
/// assert_eq!(config.stop_hour, 6);   // 6 AM
///
/// // Enable overnight mode
/// config.enabled = true;
/// config.max_tasks = 20;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OvernightConfig {
    /// Whether overnight mode is enabled.
    pub enabled: bool,
    /// When to start (hour in 24h format, 0-23).
    pub start_hour: u8,
    /// When to stop (hour in 24h format, 0-23).
    pub stop_hour: u8,
    /// Maximum tasks to process overnight.
    pub max_tasks: u32,
    /// Whether to pause on any error.
    pub pause_on_error: bool,
    /// Whether to send summary notification on completion.
    pub send_summary: bool,
    /// Email for summary (optional).
    pub notification_email: Option<String>,
    /// Days of week to run (0=Sunday, 6=Saturday).
    pub days_of_week: Vec<u8>,
}

impl Default for OvernightConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            start_hour: 22, // 10 PM
            stop_hour: 6,   // 6 AM
            max_tasks: 10,
            pause_on_error: true,
            send_summary: true,
            notification_email: None,
            days_of_week: vec![0, 1, 2, 3, 4], // Sun-Thu (night before workday)
        }
    }
}

impl OvernightConfig {
    /// Create a new overnight config with custom hours.
    ///
    /// # Arguments
    ///
    /// * `start_hour` - Hour to start (0-23)
    /// * `stop_hour` - Hour to stop (0-23)
    pub fn new(start_hour: u8, stop_hour: u8) -> Self {
        Self {
            start_hour: start_hour.min(23),
            stop_hour: stop_hour.min(23),
            ..Default::default()
        }
    }

    /// Check if current time is within overnight window.
    ///
    /// Returns true if overnight mode is enabled and the current time
    /// falls within the configured window.
    pub fn is_overnight_time(&self) -> bool {
        if !self.enabled {
            return false;
        }

        let now = chrono::Local::now();
        let hour = now.hour() as u8;
        let weekday = now.weekday().num_days_from_sunday() as u8;

        if !self.days_of_week.contains(&weekday) {
            return false;
        }

        // Handle overnight spanning midnight
        if self.start_hour > self.stop_hour {
            // e.g., 22:00 to 06:00
            hour >= self.start_hour || hour < self.stop_hour
        } else {
            // e.g., 01:00 to 05:00
            hour >= self.start_hour && hour < self.stop_hour
        }
    }

    /// Check if given hour and weekday are within overnight window.
    ///
    /// Useful for testing without depending on current time.
    pub fn is_within_window(&self, hour: u8, weekday: u8) -> bool {
        if !self.enabled {
            return false;
        }

        if !self.days_of_week.contains(&weekday) {
            return false;
        }

        if self.start_hour > self.stop_hour {
            hour >= self.start_hour || hour < self.stop_hour
        } else {
            hour >= self.start_hour && hour < self.stop_hour
        }
    }

    /// Get remaining time in overnight window (in minutes).
    ///
    /// Returns `None` if not currently in overnight window.
    pub fn remaining_minutes(&self) -> Option<u32> {
        if !self.is_overnight_time() {
            return None;
        }

        let now = chrono::Local::now();
        let hour = now.hour() as u8;
        let minute = now.minute();

        let end_minutes = (self.stop_hour as u32) * 60;
        let current_minutes = (hour as u32) * 60 + minute;

        if self.start_hour > self.stop_hour && hour >= self.start_hour {
            // Before midnight
            Some((24 * 60 - current_minutes) + end_minutes)
        } else {
            // After midnight
            Some(end_minutes.saturating_sub(current_minutes))
        }
    }

    /// Enable overnight mode.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable overnight mode.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Set the notification email.
    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.notification_email = Some(email.into());
        self
    }

    /// Add a day to the schedule.
    pub fn add_day(&mut self, day: u8) {
        let day = day.min(6);
        if !self.days_of_week.contains(&day) {
            self.days_of_week.push(day);
            self.days_of_week.sort_unstable();
        }
    }

    /// Remove a day from the schedule.
    pub fn remove_day(&mut self, day: u8) {
        self.days_of_week.retain(|&d| d != day);
    }

    /// Set all weekdays (Monday-Friday nights).
    pub fn set_weekdays(&mut self) {
        self.days_of_week = vec![0, 1, 2, 3, 4]; // Sun-Thu nights before workdays
    }

    /// Set all days (every night).
    pub fn set_all_days(&mut self) {
        self.days_of_week = vec![0, 1, 2, 3, 4, 5, 6];
    }
}

/// Overnight session summary.
///
/// Generated when overnight mode completes or is stopped.
/// Provides a report of work completed during the overnight period.
///
/// # Example
///
/// ```
/// use codirigent_core::session_advanced::OvernightSummary;
///
/// let started = chrono::Utc::now() - chrono::Duration::hours(8);
/// let mut summary = OvernightSummary::new(started);
/// summary.tasks_completed = 5;
/// summary.tasks_failed = 1;
///
/// let report = summary.format_report();
/// assert!(report.contains("5 completed"));
/// assert!(report.contains("1 failed"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvernightSummary {
    /// When overnight mode started.
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// When overnight mode ended.
    pub ended_at: chrono::DateTime<chrono::Utc>,
    /// Tasks completed successfully.
    pub tasks_completed: u32,
    /// Tasks that failed.
    pub tasks_failed: u32,
    /// Total iterations across all Ralph Loops.
    pub total_iterations: u32,
    /// Sessions that were used.
    pub sessions_used: Vec<SessionId>,
    /// Any errors encountered.
    pub errors: Vec<String>,
    /// Estimated tokens used.
    pub estimated_tokens: u64,
}

impl OvernightSummary {
    /// Create a new summary starting from the given time.
    ///
    /// # Arguments
    ///
    /// * `started_at` - When overnight mode started
    pub fn new(started_at: chrono::DateTime<chrono::Utc>) -> Self {
        Self {
            started_at,
            ended_at: chrono::Utc::now(),
            tasks_completed: 0,
            tasks_failed: 0,
            total_iterations: 0,
            sessions_used: Vec::new(),
            estimated_tokens: 0,
            errors: Vec::new(),
        }
    }

    /// Calculate the duration of overnight mode.
    pub fn duration(&self) -> chrono::Duration {
        self.ended_at - self.started_at
    }

    /// Get total tasks processed (completed + failed).
    pub fn total_tasks(&self) -> u32 {
        self.tasks_completed + self.tasks_failed
    }

    /// Get success rate as a percentage.
    pub fn success_rate(&self) -> f32 {
        let total = self.total_tasks();
        if total == 0 {
            100.0
        } else {
            (self.tasks_completed as f32 / total as f32) * 100.0
        }
    }

    /// Record a completed task.
    pub fn record_completed(&mut self) {
        self.tasks_completed += 1;
    }

    /// Record a failed task with error message.
    pub fn record_failed(&mut self, error: impl Into<String>) {
        self.tasks_failed += 1;
        self.errors.push(error.into());
    }

    /// Record session usage.
    pub fn record_session(&mut self, session_id: SessionId) {
        if !self.sessions_used.contains(&session_id) {
            self.sessions_used.push(session_id);
        }
    }

    /// Record iterations.
    pub fn record_iterations(&mut self, count: u32) {
        self.total_iterations += count;
    }

    /// Record token usage.
    pub fn record_tokens(&mut self, tokens: u64) {
        self.estimated_tokens += tokens;
    }

    /// Finalize the summary by setting the end time.
    pub fn finalize(&mut self) {
        self.ended_at = chrono::Utc::now();
    }

    /// Format as a human-readable report.
    pub fn format_report(&self) -> String {
        let duration = self.duration();
        let hours = duration.num_hours();
        let minutes = duration.num_minutes() % 60;

        format!(
            r#"## Overnight Mode Summary

**Duration:** {} hours {} minutes
**Tasks:** {} completed, {} failed ({:.1}% success rate)
**Iterations:** {}
**Sessions Used:** {}
**Estimated Tokens:** {}

{}
"#,
            hours,
            minutes,
            self.tasks_completed,
            self.tasks_failed,
            self.success_rate(),
            self.total_iterations,
            self.sessions_used.len(),
            self.estimated_tokens,
            if self.errors.is_empty() {
                "No errors encountered.".to_string()
            } else {
                format!("### Errors\n{}", self.errors.join("\n"))
            }
        )
    }
}

impl Default for OvernightSummary {
    fn default() -> Self {
        Self::new(chrono::Utc::now())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // === ContextHandoff Tests ===

    #[test]
    fn test_handoff_new() {
        let handoff = ContextHandoff::new(SessionId(1), SessionId(2));
        assert_eq!(handoff.source_session, SessionId(1));
        assert_eq!(handoff.target_session, SessionId(2));
        assert_eq!(handoff.status, HandoffStatus::Preparing);
        assert!(handoff.work_summary.is_empty());
        assert!(handoff.relevant_files.is_empty());
        assert!(handoff.pending_task.is_none());
    }

    #[test]
    fn test_handoff_builder() {
        let handoff = ContextHandoff::new(SessionId(1), SessionId(2))
            .with_summary("Refactoring auth")
            .with_files(vec![PathBuf::from("src/auth.rs")])
            .with_pending_task("Fix tests");

        assert_eq!(handoff.work_summary, "Refactoring auth");
        assert_eq!(handoff.relevant_files.len(), 1);
        assert_eq!(handoff.pending_task, Some("Fix tests".to_string()));
    }

    #[test]
    fn test_generate_prompt_empty() {
        let handoff = ContextHandoff::new(SessionId(1), SessionId(2));
        let prompt = handoff.generate_prompt();
        assert!(prompt.contains("Context Handoff"));
        assert!(prompt.contains("continuing work"));
    }

    #[test]
    fn test_generate_prompt_full() {
        let handoff = ContextHandoff::new(SessionId(1), SessionId(2))
            .with_summary("Implementing user authentication")
            .with_files(vec![
                PathBuf::from("src/auth.rs"),
                PathBuf::from("src/user.rs"),
            ])
            .with_pending_task("Fix the remaining test failures");

        let prompt = handoff.generate_prompt();
        assert!(prompt.contains("Context Handoff"));
        assert!(prompt.contains("Implementing user authentication"));
        assert!(prompt.contains("src/auth.rs"));
        assert!(prompt.contains("src/user.rs"));
        assert!(prompt.contains("Fix the remaining test failures"));
    }

    #[test]
    fn test_handoff_status_methods() {
        let mut handoff = ContextHandoff::new(SessionId(1), SessionId(2));
        assert!(!handoff.is_ready());
        assert!(!handoff.is_completed());
        assert!(!handoff.is_failed());

        handoff.mark_ready();
        assert!(handoff.is_ready());
        assert_eq!(handoff.status, HandoffStatus::Ready);

        handoff.mark_in_progress();
        assert_eq!(handoff.status, HandoffStatus::InProgress);

        handoff.mark_completed();
        assert!(handoff.is_completed());
        assert_eq!(handoff.status, HandoffStatus::Completed);
    }

    #[test]
    fn test_handoff_mark_failed() {
        let mut handoff = ContextHandoff::new(SessionId(1), SessionId(2));
        handoff.mark_failed();
        assert!(handoff.is_failed());
        assert_eq!(handoff.status, HandoffStatus::Failed);
    }

    #[test]
    fn test_handoff_serialization() {
        let handoff = ContextHandoff::new(SessionId(1), SessionId(2))
            .with_summary("Test summary")
            .with_files(vec![PathBuf::from("test.rs")]);

        let json = serde_json::to_string(&handoff).unwrap();
        let parsed: ContextHandoff = serde_json::from_str(&json).unwrap();

        assert_eq!(handoff.source_session, parsed.source_session);
        assert_eq!(handoff.target_session, parsed.target_session);
        assert_eq!(handoff.work_summary, parsed.work_summary);
        assert_eq!(handoff.relevant_files, parsed.relevant_files);
    }

    #[test]
    fn test_handoff_status_default() {
        assert_eq!(HandoffStatus::default(), HandoffStatus::Preparing);
    }

    #[test]
    fn test_handoff_status_serialization() {
        let statuses = [
            HandoffStatus::Preparing,
            HandoffStatus::Ready,
            HandoffStatus::InProgress,
            HandoffStatus::Completed,
            HandoffStatus::Failed,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: HandoffStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, parsed);
        }
    }

    // === SessionTemplate Tests ===

    #[test]
    fn test_template_new() {
        let template = SessionTemplate::new("test".to_string(), "Test template".to_string());
        assert_eq!(template.name, "test");
        assert_eq!(template.description, "Test template");
        assert!(template.initial_command.is_some());
        assert!(template.working_directory.is_none());
        assert!(template.enabled_skills.is_empty());
    }

    #[test]
    fn test_template_development() {
        let template = SessionTemplate::development();
        assert_eq!(template.name, "development");
        assert!(template.has_skill("commit"));
        assert!(template.has_skill("edit"));
        assert_eq!(template.group, Some("development".to_string()));
    }

    #[test]
    fn test_template_review() {
        let template = SessionTemplate::review();
        assert_eq!(template.name, "review");
        assert!(template.has_skill("review-pr"));
        assert_eq!(template.group, Some("review".to_string()));
    }

    #[test]
    fn test_template_testing() {
        let template = SessionTemplate::testing();
        assert_eq!(template.name, "testing");
        assert!(template.has_skill("test"));
        assert_eq!(template.group, Some("testing".to_string()));
    }

    #[test]
    fn test_template_generate_name() {
        let template = SessionTemplate::development();
        assert_eq!(template.generate_name(1), "Dev 1");
        assert_eq!(template.generate_name(42), "Dev 42");
        assert_eq!(template.generate_name(100), "Dev 100");
    }

    #[test]
    fn test_template_builder() {
        let template = SessionTemplate::new("custom".to_string(), "Custom".to_string())
            .with_working_directory(PathBuf::from("/projects"))
            .with_name_pattern("Worker {n}")
            .with_initial_command("codex")
            .with_env("DEBUG", "1")
            .with_env("LOG_LEVEL", "debug")
            .with_skill("commit")
            .with_skill("test")
            .with_group("backend")
            .with_color("#FF0000");

        assert_eq!(template.working_directory, Some(PathBuf::from("/projects")));
        assert_eq!(template.name_pattern, "Worker {n}");
        assert_eq!(template.initial_command, Some("codex".to_string()));
        assert_eq!(template.environment.get("DEBUG"), Some(&"1".to_string()));
        assert_eq!(
            template.environment.get("LOG_LEVEL"),
            Some(&"debug".to_string())
        );
        assert!(template.has_skill("commit"));
        assert!(template.has_skill("test"));
        assert_eq!(template.group, Some("backend".to_string()));
        assert_eq!(template.color, Some("#FF0000".to_string()));
    }

    #[test]
    fn test_template_with_skill_no_duplicate() {
        let template = SessionTemplate::new("test".to_string(), "Test".to_string())
            .with_skill("commit")
            .with_skill("commit"); // Duplicate

        assert_eq!(template.enabled_skills.len(), 1);
    }

    #[test]
    fn test_template_has_skill() {
        let template = SessionTemplate::development();
        assert!(template.has_skill("commit"));
        assert!(!template.has_skill("nonexistent"));
    }

    #[test]
    fn test_template_default() {
        let template = SessionTemplate::default();
        assert_eq!(template.name, "development");
    }

    #[test]
    fn test_template_serialization() {
        let template = SessionTemplate::development();
        let json = serde_json::to_string(&template).unwrap();
        let parsed: SessionTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(template, parsed);
    }

    // === SessionGroup Tests ===

    #[test]
    fn test_group_new() {
        let group = SessionGroup::new("backend".to_string());
        assert_eq!(group.name, "backend");
        assert!(group.description.is_none());
        assert!(group.color.is_none());
        assert!(group.sessions.is_empty());
        assert!(group.shared_context.is_none());
    }

    #[test]
    fn test_group_builder() {
        let group = SessionGroup::new("backend".to_string())
            .with_description("Backend API development")
            .with_color("#FF5733")
            .with_shared_context("Working on user authentication");

        assert_eq!(
            group.description,
            Some("Backend API development".to_string())
        );
        assert_eq!(group.color, Some("#FF5733".to_string()));
        assert_eq!(
            group.shared_context,
            Some("Working on user authentication".to_string())
        );
    }

    #[test]
    fn test_group_add_remove_session() {
        let mut group = SessionGroup::new("test".to_string());

        group.add_session(SessionId(1));
        group.add_session(SessionId(2));
        assert_eq!(group.session_count(), 2);
        assert!(group.contains(SessionId(1)));
        assert!(group.contains(SessionId(2)));

        // Adding duplicate should not increase count
        group.add_session(SessionId(1));
        assert_eq!(group.session_count(), 2);

        group.remove_session(SessionId(1));
        assert_eq!(group.session_count(), 1);
        assert!(!group.contains(SessionId(1)));
        assert!(group.contains(SessionId(2)));
    }

    #[test]
    fn test_group_is_empty() {
        let mut group = SessionGroup::new("test".to_string());
        assert!(group.is_empty());

        group.add_session(SessionId(1));
        assert!(!group.is_empty());
    }

    #[test]
    fn test_group_set_shared_context() {
        let mut group = SessionGroup::new("test".to_string());
        assert!(group.shared_context.is_none());

        group.set_shared_context(Some("New context".to_string()));
        assert_eq!(group.shared_context, Some("New context".to_string()));

        group.set_shared_context(None);
        assert!(group.shared_context.is_none());
    }

    #[test]
    fn test_group_serialization() {
        let mut group = SessionGroup::new("backend".to_string())
            .with_description("Backend development")
            .with_color("#FF0000");

        group.add_session(SessionId(1));
        group.add_session(SessionId(2));

        let json = serde_json::to_string(&group).unwrap();
        let parsed: SessionGroup = serde_json::from_str(&json).unwrap();

        assert_eq!(group.name, parsed.name);
        assert_eq!(group.description, parsed.description);
        assert_eq!(group.sessions.len(), parsed.sessions.len());
    }

    // === OvernightConfig Tests ===

    #[test]
    fn test_overnight_config_default() {
        let config = OvernightConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.start_hour, 22);
        assert_eq!(config.stop_hour, 6);
        assert_eq!(config.max_tasks, 10);
        assert!(config.pause_on_error);
        assert!(config.send_summary);
        assert!(config.notification_email.is_none());
        assert_eq!(config.days_of_week, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_overnight_config_new() {
        let config = OvernightConfig::new(20, 4);
        assert_eq!(config.start_hour, 20);
        assert_eq!(config.stop_hour, 4);
    }

    #[test]
    fn test_overnight_config_new_clamps_hours() {
        let config = OvernightConfig::new(30, 50); // Invalid hours
        assert_eq!(config.start_hour, 23);
        assert_eq!(config.stop_hour, 23);
    }

    #[test]
    fn test_overnight_config_enable_disable() {
        let mut config = OvernightConfig::default();
        assert!(!config.enabled);

        config.enable();
        assert!(config.enabled);

        config.disable();
        assert!(!config.enabled);
    }

    #[test]
    fn test_overnight_is_within_window_disabled() {
        let config = OvernightConfig::default(); // disabled
        assert!(!config.is_within_window(23, 1)); // Tuesday night
    }

    #[test]
    fn test_overnight_is_within_window_wrong_day() {
        let config = OvernightConfig {
            enabled: true,
            ..Default::default()
        };
        // Friday (5) is not in default days [0,1,2,3,4]
        assert!(!config.is_within_window(23, 5));
    }

    #[test]
    fn test_overnight_is_within_window_spanning_midnight() {
        let mut config = OvernightConfig::new(22, 6);
        config.enabled = true;

        // Before midnight
        assert!(config.is_within_window(22, 1)); // 10 PM Tuesday
        assert!(config.is_within_window(23, 1)); // 11 PM Tuesday

        // After midnight
        assert!(config.is_within_window(0, 1)); // 12 AM Tuesday
        assert!(config.is_within_window(5, 1)); // 5 AM Tuesday

        // Outside window
        assert!(!config.is_within_window(7, 1)); // 7 AM Tuesday
        assert!(!config.is_within_window(12, 1)); // 12 PM Tuesday
        assert!(!config.is_within_window(21, 1)); // 9 PM Tuesday
    }

    #[test]
    fn test_overnight_is_within_window_same_day() {
        let mut config = OvernightConfig::new(1, 5);
        config.enabled = true;

        assert!(config.is_within_window(2, 1)); // 2 AM Tuesday
        assert!(config.is_within_window(4, 1)); // 4 AM Tuesday
        assert!(!config.is_within_window(0, 1)); // 12 AM Tuesday
        assert!(!config.is_within_window(6, 1)); // 6 AM Tuesday
    }

    #[test]
    fn test_overnight_config_with_email() {
        let config = OvernightConfig::default().with_email("user@example.com");
        assert_eq!(
            config.notification_email,
            Some("user@example.com".to_string())
        );
    }

    #[test]
    fn test_overnight_config_add_remove_day() {
        let mut config = OvernightConfig::default();
        assert!(!config.days_of_week.contains(&5));

        config.add_day(5);
        assert!(config.days_of_week.contains(&5));

        config.add_day(5); // Duplicate
        assert_eq!(config.days_of_week.iter().filter(|&&d| d == 5).count(), 1);

        config.remove_day(5);
        assert!(!config.days_of_week.contains(&5));
    }

    #[test]
    fn test_overnight_config_add_day_clamps() {
        let mut config = OvernightConfig::default();
        config.add_day(10); // Invalid, should clamp to 6
        assert!(config.days_of_week.contains(&6));
    }

    #[test]
    fn test_overnight_config_set_weekdays() {
        let mut config = OvernightConfig {
            days_of_week: vec![6], // Only Saturday
            ..Default::default()
        };

        config.set_weekdays();
        assert_eq!(config.days_of_week, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_overnight_config_set_all_days() {
        let mut config = OvernightConfig::default();
        config.set_all_days();
        assert_eq!(config.days_of_week, vec![0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_overnight_config_serialization() {
        let config = OvernightConfig::default().with_email("test@example.com");
        let json = serde_json::to_string(&config).unwrap();
        let parsed: OvernightConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }

    // === OvernightSummary Tests ===

    #[test]
    fn test_overnight_summary_new() {
        let started = chrono::Utc::now() - chrono::Duration::hours(1);
        let summary = OvernightSummary::new(started);

        assert_eq!(summary.tasks_completed, 0);
        assert_eq!(summary.tasks_failed, 0);
        assert_eq!(summary.total_iterations, 0);
        assert!(summary.sessions_used.is_empty());
        assert!(summary.errors.is_empty());
        assert_eq!(summary.estimated_tokens, 0);
    }

    #[test]
    fn test_overnight_summary_record_methods() {
        let mut summary = OvernightSummary::default();

        summary.record_completed();
        summary.record_completed();
        assert_eq!(summary.tasks_completed, 2);

        summary.record_failed("Task timed out");
        assert_eq!(summary.tasks_failed, 1);
        assert_eq!(summary.errors.len(), 1);

        summary.record_session(SessionId(1));
        summary.record_session(SessionId(2));
        summary.record_session(SessionId(1)); // Duplicate
        assert_eq!(summary.sessions_used.len(), 2);

        summary.record_iterations(5);
        summary.record_iterations(3);
        assert_eq!(summary.total_iterations, 8);

        summary.record_tokens(1000);
        summary.record_tokens(500);
        assert_eq!(summary.estimated_tokens, 1500);
    }

    #[test]
    fn test_overnight_summary_total_tasks() {
        let summary = OvernightSummary {
            tasks_completed: 5,
            tasks_failed: 2,
            ..Default::default()
        };
        assert_eq!(summary.total_tasks(), 7);
    }

    #[test]
    fn test_overnight_summary_success_rate() {
        let mut summary = OvernightSummary::default();

        // No tasks = 100% success
        assert!((summary.success_rate() - 100.0).abs() < f32::EPSILON);

        summary.tasks_completed = 8;
        summary.tasks_failed = 2;
        assert!((summary.success_rate() - 80.0).abs() < f32::EPSILON);

        summary.tasks_completed = 0;
        summary.tasks_failed = 5;
        assert!((summary.success_rate() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_overnight_summary_duration() {
        let started = chrono::Utc::now() - chrono::Duration::hours(8);
        let summary = OvernightSummary::new(started);

        let duration = summary.duration();
        // Allow some tolerance for test execution time
        assert!(duration.num_hours() >= 7 && duration.num_hours() <= 8);
    }

    #[test]
    fn test_overnight_summary_finalize() {
        let started = chrono::Utc::now() - chrono::Duration::hours(1);
        let mut summary = OvernightSummary::new(started);

        std::thread::sleep(std::time::Duration::from_millis(10));
        let before_finalize = summary.ended_at;

        summary.finalize();
        assert!(summary.ended_at > before_finalize);
    }

    #[test]
    fn test_overnight_summary_format_report() {
        let started = chrono::Utc::now() - chrono::Duration::hours(8);
        let mut summary = OvernightSummary::new(started);
        summary.tasks_completed = 5;
        summary.tasks_failed = 1;
        summary.total_iterations = 15;
        summary.estimated_tokens = 10000;
        summary.sessions_used = vec![SessionId(1), SessionId(2)];

        let report = summary.format_report();
        assert!(report.contains("Overnight Mode Summary"));
        assert!(report.contains("8 hours"));
        assert!(report.contains("5 completed"));
        assert!(report.contains("1 failed"));
        assert!(report.contains("83.3%")); // 5/6 success rate
        assert!(report.contains("15"));
        assert!(report.contains("2")); // sessions
        assert!(report.contains("10000"));
        assert!(report.contains("No errors encountered"));
    }

    #[test]
    fn test_overnight_summary_format_report_with_errors() {
        let mut summary = OvernightSummary::default();
        summary.errors.push("Error 1".to_string());
        summary.errors.push("Error 2".to_string());

        let report = summary.format_report();
        assert!(report.contains("### Errors"));
        assert!(report.contains("Error 1"));
        assert!(report.contains("Error 2"));
    }

    #[test]
    fn test_overnight_summary_serialization() {
        let summary = OvernightSummary {
            tasks_completed: 3,
            tasks_failed: 1,
            sessions_used: vec![SessionId(1)],
            errors: vec!["Test error".to_string()],
            ..Default::default()
        };

        let json = serde_json::to_string(&summary).unwrap();
        let parsed: OvernightSummary = serde_json::from_str(&json).unwrap();

        assert_eq!(summary.tasks_completed, parsed.tasks_completed);
        assert_eq!(summary.tasks_failed, parsed.tasks_failed);
        assert_eq!(summary.sessions_used.len(), parsed.sessions_used.len());
        assert_eq!(summary.errors.len(), parsed.errors.len());
    }

    #[test]
    fn test_overnight_summary_default() {
        let summary = OvernightSummary::default();
        assert_eq!(summary.tasks_completed, 0);
        assert_eq!(summary.tasks_failed, 0);
    }
}
