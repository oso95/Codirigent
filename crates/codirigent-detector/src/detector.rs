//! Input detector for session status monitoring.
//!
//! This module provides the main input detection functionality for Codirigent.
//! It monitors session output and process state to detect when a CLI tool
//! is waiting for user input.
//!
//! # Architecture
//!
//! The [`InputDetector`] combines two detection strategies:
//!
//! 1. **Pattern matching**: Scans session output for known input prompts
//!    like `[y/n]`, `?`, password prompts, etc.
//!
//! 2. **Process state monitoring**: Uses platform-specific APIs to check
//!    if the process is sleeping/idle, which may indicate input waiting.
//!
//! # Example
//!
//! ```no_run
//! use codirigent_detector::detector::{DetectorConfig, InputDetector};
//! use codirigent_core::{DefaultEventBus, SessionId, ProcessMonitor};
//! use std::sync::Arc;
//!
//! let event_bus = Arc::new(DefaultEventBus::new(16));
//! let mut detector = InputDetector::new(DetectorConfig::default(), event_bus);
//!
//! // Start monitoring a session
//! detector.start_monitoring(SessionId(1), 12345).unwrap();
//!
//! // Process output from the session
//! detector.process_output(SessionId(1), b"Continue? [y/n] ");
//!
//! // Check the detected status
//! let status = detector.get_status(SessionId(1));
//! ```

use crate::patterns::{compile_patterns, find_matching_pattern_with_limit, get_default_patterns};
use crate::platform::{NativeMonitor, PlatformMonitor, ProcessState};
use anyhow::Result;
use codirigent_core::{CodirigentEvent, EventBus, ProcessMonitor, SessionId, SessionStatus, ShellState};
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// Configuration for the input detector.
///
/// Controls detection behavior including polling intervals, idle thresholds,
/// custom patterns, and notification settings.
#[derive(Debug, Clone)]
pub struct DetectorConfig {
    /// Polling interval for process state checks.
    ///
    /// Lower values provide more responsive detection but use more CPU.
    /// Default: 250ms
    pub poll_interval: Duration,

    /// Time threshold for considering a process idle.
    ///
    /// If a process is sleeping and no output has been received for this
    /// duration, it may be considered waiting for input.
    /// Default: 2 seconds
    pub idle_threshold: Duration,

    /// Custom patterns to detect in addition to defaults.
    ///
    /// These are regex patterns that will be compiled and added to
    /// the default pattern set.
    pub custom_patterns: Vec<String>,

    /// Whether to send desktop notifications when input is required.
    ///
    /// Default: true
    pub notifications_enabled: bool,

    /// Maximum output buffer size in bytes.
    ///
    /// Older output is discarded to prevent unbounded memory growth.
    /// Default: 4096 bytes
    pub max_buffer_size: usize,

    /// Number of recent lines to check for patterns.
    ///
    /// Only the most recent lines are checked for efficiency.
    /// Default: 5
    pub recent_lines_to_check: usize,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(250),
            idle_threshold: Duration::from_secs(2),
            custom_patterns: Vec::new(),
            notifications_enabled: true,
            max_buffer_size: 4096,
            recent_lines_to_check: 5,
        }
    }
}

impl DetectorConfig {
    /// Create a new configuration with custom patterns.
    ///
    /// # Arguments
    ///
    /// * `custom_patterns` - Additional regex patterns to detect
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_detector::detector::DetectorConfig;
    ///
    /// let config = DetectorConfig::with_patterns(vec![
    ///     r"custom-prompt:".to_string(),
    /// ]);
    /// ```
    pub fn with_patterns(custom_patterns: Vec<String>) -> Self {
        Self {
            custom_patterns,
            ..Default::default()
        }
    }

    /// Create a configuration with notifications disabled.
    ///
    /// Useful for testing or headless environments.
    pub fn without_notifications() -> Self {
        Self {
            notifications_enabled: false,
            ..Default::default()
        }
    }
}

/// State for a monitored session.
///
/// Tracks the session's output buffer, status, and timing information.
struct MonitoredSession {
    /// The session ID (stored for debugging and logging purposes).
    #[allow(dead_code)]
    session_id: SessionId,
    /// The PTY process ID.
    pty_pid: u32,
    /// When output was last received.
    last_output_time: Instant,
    /// Buffer of recent output.
    output_buffer: String,
    /// Current detected status.
    current_status: SessionStatus,
    /// Pattern that matched, if any.
    pattern_matched: Option<String>,
    /// Last known shell state from OSC 133 markers.
    shell_state: Option<ShellState>,
}

impl MonitoredSession {
    /// Create a new monitored session.
    fn new(session_id: SessionId, pty_pid: u32) -> Self {
        Self {
            session_id,
            pty_pid,
            last_output_time: Instant::now(),
            output_buffer: String::new(),
            current_status: SessionStatus::Idle,
            pattern_matched: None,
            shell_state: None,
        }
    }

    /// Clear the output buffer and pattern match.
    fn clear_buffer(&mut self) {
        self.output_buffer.clear();
        self.pattern_matched = None;
    }
}

/// Input detector for monitoring sessions.
///
/// Implements the [`ProcessMonitor`] trait and provides input detection
/// by combining pattern matching with process state monitoring.
pub struct InputDetector {
    /// Configuration.
    config: DetectorConfig,
    /// Platform-specific process monitor.
    platform_monitor: NativeMonitor,
    /// Monitored sessions by ID.
    sessions: HashMap<SessionId, MonitoredSession>,
    /// Compiled regex patterns.
    compiled_patterns: Vec<Regex>,
    /// Event bus for publishing status changes.
    event_bus: Arc<dyn EventBus>,
}

impl InputDetector {
    /// Create a new input detector.
    ///
    /// # Arguments
    ///
    /// * `config` - Detection configuration
    /// * `event_bus` - Event bus for publishing events
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_detector::detector::{DetectorConfig, InputDetector};
    /// use codirigent_core::DefaultEventBus;
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let detector = InputDetector::new(DetectorConfig::default(), event_bus);
    /// ```
    pub fn new(config: DetectorConfig, event_bus: Arc<dyn EventBus>) -> Self {
        // Compile all patterns (defaults + custom)
        let mut all_patterns = get_default_patterns();
        all_patterns.extend(config.custom_patterns.clone());

        let compiled_patterns = compile_patterns(&all_patterns);

        debug!(
            pattern_count = compiled_patterns.len(),
            "Input detector initialized"
        );

        Self {
            config,
            platform_monitor: NativeMonitor::new(),
            sessions: HashMap::new(),
            compiled_patterns,
            event_bus,
        }
    }

    /// Process output from a session.
    ///
    /// Appends the output to the session's buffer and checks for patterns.
    /// If a pattern is matched, the session status is updated.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session that produced the output
    /// * `data` - Raw output bytes
    pub fn process_output(&mut self, session_id: SessionId, data: &[u8]) {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.last_output_time = Instant::now();

            // Append to buffer (keep limited size)
            let text = String::from_utf8_lossy(data);
            session.output_buffer.push_str(&text);

            // Trim buffer if too large
            if session.output_buffer.len() > self.config.max_buffer_size {
                let target_start = session.output_buffer.len() - self.config.max_buffer_size;
                // Find the next valid char boundary to avoid panic on multi-byte UTF-8
                let start = session
                    .output_buffer
                    .char_indices()
                    .find(|(i, _)| *i >= target_start)
                    .map(|(i, _)| i)
                    .unwrap_or(session.output_buffer.len());
                session.output_buffer = session.output_buffer[start..].to_string();
            }

            // Check for patterns
            session.pattern_matched = find_matching_pattern_with_limit(
                &self.compiled_patterns,
                &session.output_buffer,
                self.config.recent_lines_to_check,
            );

            // Update status
            self.update_session_status(session_id);
        }
    }

    /// Update status for a session.
    ///
    /// Determines the new status based on process state and patterns,
    /// then publishes an event if the status changed.
    fn update_session_status(&mut self, session_id: SessionId) {
        // Get the new status
        let (new_status, pattern_matched) = {
            let session = match self.sessions.get(&session_id) {
                Some(s) => s,
                None => return,
            };
            (self.determine_status(session), session.pattern_matched.clone())
        };

        // Update and publish if changed
        if let Some(session) = self.sessions.get_mut(&session_id) {
            if new_status != session.current_status {
                let old = session.current_status;
                session.current_status = new_status;

                debug!(%session_id, ?old, ?new_status, "Session status changed");

                self.event_bus.publish(CodirigentEvent::SessionStatusChanged {
                    id: session_id,
                    old,
                    new: new_status,
                });

                // Send InputRequired event if waiting for input
                if new_status == SessionStatus::WaitingForInput {
                    self.event_bus.publish(CodirigentEvent::InputRequired {
                        session_id,
                        pattern: pattern_matched,
                    });
                }

                // Clear pattern match when status changes away from WaitingForInput
                if old == SessionStatus::WaitingForInput
                    && new_status != SessionStatus::WaitingForInput
                {
                    session.pattern_matched = None;
                    self.event_bus.publish(CodirigentEvent::InputProvided {
                        session_id,
                    });
                }
            }
        }
    }

    /// Determine status based on pattern matching, OSC 133, and process state.
    ///
    /// Priority order:
    /// 1. Pattern match (y/n prompts from CLIs) — highest priority
    /// 2. OSC 133 shell state — reliable, from the shell itself
    /// 3. Process state heuristic — fallback for shells without OSC 133
    fn determine_status(&self, session: &MonitoredSession) -> SessionStatus {
        // 1. Pattern match takes highest priority
        if session.pattern_matched.is_some() {
            return SessionStatus::WaitingForInput;
        }

        // 2. OSC 133 shell state — reliable, if available
        if let Some(ref shell_state) = session.shell_state {
            return match shell_state {
                ShellState::PromptStart | ShellState::CommandInputStart => SessionStatus::Idle,
                ShellState::CommandExecuted => SessionStatus::Working,
                ShellState::CommandFinished { .. } => SessionStatus::Idle,
            };
        }

        // 3. Fallback: process state heuristic (legacy/unsupported shells)
        let process_state = self
            .platform_monitor
            .get_process_state(session.pty_pid)
            .unwrap_or(ProcessState::Unknown);

        let idle_time = session.last_output_time.elapsed();

        match process_state {
            ProcessState::Terminated => SessionStatus::Done,
            ProcessState::Running | ProcessState::Sleeping => {
                if idle_time > self.config.idle_threshold {
                    SessionStatus::Idle
                } else {
                    SessionStatus::Working
                }
            }
            ProcessState::Stopped | ProcessState::Unknown => SessionStatus::Idle,
        }
    }

    /// Update shell state from an OSC 133 marker.
    ///
    /// OSC 133 provides reliable idle detection directly from the shell,
    /// bypassing platform-specific process state heuristics.
    pub fn set_shell_state(&mut self, session_id: SessionId, state: ShellState) {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.shell_state = Some(state);
            self.update_session_status(session_id);
        }
    }

    /// Tick the detector (called periodically).
    ///
    /// Updates the status of all monitored sessions based on current
    /// process state and timing.
    pub fn tick(&mut self) {
        let session_ids: Vec<SessionId> = self.sessions.keys().copied().collect();
        for session_id in session_ids {
            self.update_session_status(session_id);
        }
    }

    /// Get the configuration.
    pub fn config(&self) -> &DetectorConfig {
        &self.config
    }

    /// Get the number of monitored sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Get the number of compiled patterns.
    pub fn pattern_count(&self) -> usize {
        self.compiled_patterns.len()
    }

    /// Check if a session is being monitored.
    pub fn is_monitoring(&self, session_id: SessionId) -> bool {
        self.sessions.contains_key(&session_id)
    }

    /// Get the pattern that matched for a session, if any.
    pub fn get_matched_pattern(&self, session_id: SessionId) -> Option<String> {
        self.sessions
            .get(&session_id)
            .and_then(|s| s.pattern_matched.clone())
    }

    /// Clear the output buffer for a session.
    ///
    /// Useful when the user provides input and the buffer should be reset.
    pub fn clear_buffer(&mut self, session_id: SessionId) {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.clear_buffer();
        }
    }

    /// Get the idle time for a session.
    ///
    /// Returns the duration since the last output was received.
    pub fn get_idle_time(&self, session_id: SessionId) -> Option<Duration> {
        self.sessions
            .get(&session_id)
            .map(|s| s.last_output_time.elapsed())
    }
}

impl ProcessMonitor for InputDetector {
    fn start_monitoring(&mut self, session_id: SessionId, pty_pid: u32) -> Result<()> {
        info!(%session_id, pty_pid, "Starting monitoring");

        let session = MonitoredSession::new(session_id, pty_pid);
        self.sessions.insert(session_id, session);

        Ok(())
    }

    fn stop_monitoring(&mut self, session_id: SessionId) {
        info!(%session_id, "Stopping monitoring");
        self.sessions.remove(&session_id);
    }

    fn get_status(&self, session_id: SessionId) -> Option<SessionStatus> {
        self.sessions.get(&session_id).map(|s| s.current_status)
    }

    fn add_pattern(&mut self, pattern: String) {
        if let Ok(re) = Regex::new(&pattern) {
            self.compiled_patterns.push(re);
            info!(%pattern, "Added custom pattern");
        } else {
            debug!(%pattern, "Failed to compile pattern");
        }
    }

    fn remove_pattern(&mut self, pattern: &str) {
        let initial_len = self.compiled_patterns.len();
        self.compiled_patterns.retain(|re| re.as_str() != pattern);
        if self.compiled_patterns.len() < initial_len {
            info!(%pattern, "Removed custom pattern");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codirigent_core::DefaultEventBus;

    fn create_test_detector() -> InputDetector {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        InputDetector::new(DetectorConfig::default(), event_bus)
    }

    fn create_detector_with_config(config: DetectorConfig) -> InputDetector {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        InputDetector::new(config, event_bus)
    }

    // DetectorConfig tests
    #[test]
    fn test_detector_config_default() {
        let config = DetectorConfig::default();
        assert_eq!(config.poll_interval, Duration::from_millis(250));
        assert_eq!(config.idle_threshold, Duration::from_secs(2));
        assert!(config.custom_patterns.is_empty());
        assert!(config.notifications_enabled);
        assert_eq!(config.max_buffer_size, 4096);
        assert_eq!(config.recent_lines_to_check, 5);
    }

    #[test]
    fn test_detector_config_with_patterns() {
        let config = DetectorConfig::with_patterns(vec!["custom".to_string()]);
        assert_eq!(config.custom_patterns.len(), 1);
        assert_eq!(config.custom_patterns[0], "custom");
    }

    #[test]
    fn test_detector_config_without_notifications() {
        let config = DetectorConfig::without_notifications();
        assert!(!config.notifications_enabled);
    }

    #[test]
    fn test_detector_config_clone() {
        let config = DetectorConfig::default();
        let cloned = config.clone();
        assert_eq!(config.poll_interval, cloned.poll_interval);
    }

    #[test]
    fn test_detector_config_debug() {
        let config = DetectorConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("DetectorConfig"));
    }

    // InputDetector creation tests
    #[test]
    fn test_detector_creation() {
        let detector = create_test_detector();
        assert!(detector.sessions.is_empty());
        assert!(!detector.compiled_patterns.is_empty());
    }

    #[test]
    fn test_detector_with_custom_patterns() {
        let config = DetectorConfig::with_patterns(vec![r"custom-prompt:".to_string()]);
        let detector = create_detector_with_config(config);

        // Should have default patterns + custom pattern
        assert!(detector.compiled_patterns.len() > 1);
    }

    #[test]
    fn test_detector_config_accessor() {
        let detector = create_test_detector();
        let config = detector.config();
        assert_eq!(config.poll_interval, Duration::from_millis(250));
    }

    #[test]
    fn test_detector_pattern_count() {
        let detector = create_test_detector();
        assert!(detector.pattern_count() > 0);
    }

    #[test]
    fn test_detector_session_count_empty() {
        let detector = create_test_detector();
        assert_eq!(detector.session_count(), 0);
    }

    // ProcessMonitor trait tests
    #[test]
    fn test_start_monitoring() {
        let mut detector = create_test_detector();
        let result = detector.start_monitoring(SessionId(1), 1234);
        assert!(result.is_ok());
        assert!(detector.is_monitoring(SessionId(1)));
        assert_eq!(detector.session_count(), 1);
    }

    #[test]
    fn test_start_monitoring_multiple_sessions() {
        let mut detector = create_test_detector();
        detector.start_monitoring(SessionId(1), 1234).unwrap();
        detector.start_monitoring(SessionId(2), 5678).unwrap();
        assert_eq!(detector.session_count(), 2);
    }

    #[test]
    fn test_stop_monitoring() {
        let mut detector = create_test_detector();
        detector.start_monitoring(SessionId(1), 1234).unwrap();
        assert!(detector.is_monitoring(SessionId(1)));

        detector.stop_monitoring(SessionId(1));
        assert!(!detector.is_monitoring(SessionId(1)));
        assert_eq!(detector.session_count(), 0);
    }

    #[test]
    fn test_stop_monitoring_nonexistent() {
        let mut detector = create_test_detector();
        // Should not panic
        detector.stop_monitoring(SessionId(999));
    }

    #[test]
    fn test_get_status_monitored() {
        let mut detector = create_test_detector();
        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();
        let status = detector.get_status(SessionId(1));
        assert!(status.is_some());
    }

    #[test]
    fn test_get_status_not_monitored() {
        let detector = create_test_detector();
        let status = detector.get_status(SessionId(999));
        assert!(status.is_none());
    }

    #[test]
    fn test_get_status_initial() {
        let mut detector = create_test_detector();
        detector.start_monitoring(SessionId(1), 1234).unwrap();
        let status = detector.get_status(SessionId(1));
        assert_eq!(status, Some(SessionStatus::Idle));
    }

    // Pattern management tests
    #[test]
    fn test_add_pattern() {
        let mut detector = create_test_detector();
        let initial_count = detector.pattern_count();
        detector.add_pattern(r"new-pattern".to_string());
        assert_eq!(detector.pattern_count(), initial_count + 1);
    }

    #[test]
    fn test_add_invalid_pattern() {
        let mut detector = create_test_detector();
        let initial_count = detector.pattern_count();
        detector.add_pattern(r"[invalid".to_string());
        // Invalid pattern should not be added
        assert_eq!(detector.pattern_count(), initial_count);
    }

    #[test]
    fn test_remove_pattern() {
        let mut detector = create_test_detector();
        detector.add_pattern(r"removable".to_string());
        let count_after_add = detector.pattern_count();

        detector.remove_pattern("removable");
        assert_eq!(detector.pattern_count(), count_after_add - 1);
    }

    #[test]
    fn test_remove_nonexistent_pattern() {
        let mut detector = create_test_detector();
        let initial_count = detector.pattern_count();
        detector.remove_pattern("nonexistent");
        assert_eq!(detector.pattern_count(), initial_count);
    }

    // Output processing tests
    #[test]
    fn test_process_output_pattern_detection() {
        let mut detector = create_test_detector();
        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();

        // Process output with pattern
        detector.process_output(SessionId(1), b"Continue? [y/n] ");

        let status = detector.get_status(SessionId(1));
        assert_eq!(status, Some(SessionStatus::WaitingForInput));
    }

    #[test]
    fn test_process_output_no_pattern() {
        let mut detector = create_test_detector();
        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();

        // Process output without pattern
        detector.process_output(SessionId(1), b"Hello world");

        // Status should not be WaitingForInput (pattern-based)
        let status = detector.get_status(SessionId(1));
        // The actual status depends on process state, but not pattern-triggered
        assert!(status.is_some());
    }

    #[test]
    fn test_process_output_nonexistent_session() {
        let mut detector = create_test_detector();
        // Should not panic
        detector.process_output(SessionId(999), b"test");
    }

    #[test]
    fn test_get_matched_pattern() {
        let mut detector = create_test_detector();
        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();

        detector.process_output(SessionId(1), b"Install? [y/n]");

        let pattern = detector.get_matched_pattern(SessionId(1));
        assert!(pattern.is_some());
    }

    #[test]
    fn test_get_matched_pattern_none() {
        let mut detector = create_test_detector();
        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();

        let pattern = detector.get_matched_pattern(SessionId(1));
        assert!(pattern.is_none());
    }

    #[test]
    fn test_get_matched_pattern_nonexistent() {
        let detector = create_test_detector();
        let pattern = detector.get_matched_pattern(SessionId(999));
        assert!(pattern.is_none());
    }

    // Buffer management tests
    #[test]
    fn test_clear_buffer() {
        let mut detector = create_test_detector();
        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();

        detector.process_output(SessionId(1), b"Continue? [y/n]");
        assert!(detector.get_matched_pattern(SessionId(1)).is_some());

        detector.clear_buffer(SessionId(1));
        // After clearing, pattern should be gone
        assert!(detector.get_matched_pattern(SessionId(1)).is_none());
    }

    #[test]
    fn test_clear_buffer_nonexistent() {
        let mut detector = create_test_detector();
        // Should not panic
        detector.clear_buffer(SessionId(999));
    }

    #[test]
    fn test_buffer_size_limit() {
        let config = DetectorConfig {
            max_buffer_size: 100,
            ..Default::default()
        };
        let mut detector = create_detector_with_config(config);
        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();

        // Send more data than buffer size
        let large_data = "x".repeat(200);
        detector.process_output(SessionId(1), large_data.as_bytes());

        // The buffer should be trimmed
        // (we can't easily inspect the buffer size, but it shouldn't panic)
    }

    #[test]
    fn test_buffer_size_limit_with_multibyte_utf8() {
        let config = DetectorConfig {
            max_buffer_size: 50,
            ..Default::default()
        };
        let mut detector = create_detector_with_config(config);
        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();

        // Use multi-byte UTF-8 characters (Japanese hiragana, 3 bytes each)
        // This ensures the trim doesn't panic on UTF-8 boundaries
        let multibyte_data = "あいうえおかきくけこさしすせそたちつてと";
        detector.process_output(SessionId(1), multibyte_data.as_bytes());

        // Should not panic, and the detector should still work
        let status = detector.get_status(SessionId(1));
        assert!(status.is_some());
    }

    #[test]
    fn test_buffer_size_limit_with_emoji() {
        let config = DetectorConfig {
            max_buffer_size: 20,
            ..Default::default()
        };
        let mut detector = create_detector_with_config(config);
        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();

        // Emoji are 4 bytes each in UTF-8
        let emoji_data = "🎉🎊🎁🎂🎄🎃🎆🎇🎈🎀";
        detector.process_output(SessionId(1), emoji_data.as_bytes());

        // Should not panic
        let status = detector.get_status(SessionId(1));
        assert!(status.is_some());
    }

    // Idle time tests
    #[test]
    fn test_get_idle_time() {
        let mut detector = create_test_detector();
        detector.start_monitoring(SessionId(1), 1234).unwrap();

        let idle_time = detector.get_idle_time(SessionId(1));
        assert!(idle_time.is_some());
        // Should be a small duration since we just started
        assert!(idle_time.unwrap() < Duration::from_secs(1));
    }

    #[test]
    fn test_get_idle_time_nonexistent() {
        let detector = create_test_detector();
        let idle_time = detector.get_idle_time(SessionId(999));
        assert!(idle_time.is_none());
    }

    #[test]
    fn test_idle_time_resets_on_output() {
        let mut detector = create_test_detector();
        detector.start_monitoring(SessionId(1), 1234).unwrap();

        // Wait a tiny bit
        std::thread::sleep(Duration::from_millis(10));

        let idle_before = detector.get_idle_time(SessionId(1)).unwrap();

        // Process output resets the timer
        detector.process_output(SessionId(1), b"output");

        let idle_after = detector.get_idle_time(SessionId(1)).unwrap();

        // After output, idle time should be less than before
        assert!(idle_after <= idle_before || idle_after < Duration::from_millis(10));
    }

    // Tick tests
    #[test]
    fn test_tick_empty() {
        let mut detector = create_test_detector();
        // Should not panic with no sessions
        detector.tick();
    }

    #[test]
    fn test_tick_with_sessions() {
        let mut detector = create_test_detector();
        detector.start_monitoring(SessionId(1), 1234).unwrap();
        detector.start_monitoring(SessionId(2), 5678).unwrap();

        // Should not panic
        detector.tick();
    }

    // MonitoredSession tests
    #[test]
    fn test_monitored_session_new() {
        let session = MonitoredSession::new(SessionId(1), 1234);
        assert_eq!(session.session_id, SessionId(1));
        assert_eq!(session.pty_pid, 1234);
        assert!(session.output_buffer.is_empty());
        assert_eq!(session.current_status, SessionStatus::Idle);
        assert!(session.pattern_matched.is_none());
    }

    #[test]
    fn test_monitored_session_clear_buffer() {
        let mut session = MonitoredSession::new(SessionId(1), 1234);
        session.output_buffer = "some output".to_string();
        session.pattern_matched = Some("pattern".to_string());

        session.clear_buffer();

        assert!(session.output_buffer.is_empty());
        assert!(session.pattern_matched.is_none());
    }

    // Pattern detection edge cases
    #[test]
    fn test_password_prompt_detection() {
        let mut detector = create_test_detector();
        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();

        detector.process_output(SessionId(1), b"Password:");

        let status = detector.get_status(SessionId(1));
        assert_eq!(status, Some(SessionStatus::WaitingForInput));
    }

    #[test]
    fn test_press_enter_detection() {
        let mut detector = create_test_detector();
        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();

        detector.process_output(SessionId(1), b"Press Enter to continue");

        let status = detector.get_status(SessionId(1));
        assert_eq!(status, Some(SessionStatus::WaitingForInput));
    }

    #[test]
    fn test_question_prompt_detection() {
        let mut detector = create_test_detector();
        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();

        detector.process_output(SessionId(1), b"What is your name? ");

        let status = detector.get_status(SessionId(1));
        assert_eq!(status, Some(SessionStatus::WaitingForInput));
    }

    // Integration-style tests
    #[test]
    fn test_full_workflow() {
        let mut detector = create_test_detector();

        // Start monitoring
        detector.start_monitoring(SessionId(1), 1234).unwrap();
        assert_eq!(detector.get_status(SessionId(1)), Some(SessionStatus::Idle));

        // Process some output
        detector.process_output(SessionId(1), b"Starting process...\n");

        // Process pattern
        detector.process_output(SessionId(1), b"Continue? [y/n] ");
        assert_eq!(
            detector.get_status(SessionId(1)),
            Some(SessionStatus::WaitingForInput)
        );

        // Clear buffer (simulating user input)
        detector.clear_buffer(SessionId(1));

        // Stop monitoring
        detector.stop_monitoring(SessionId(1));
        assert!(!detector.is_monitoring(SessionId(1)));
    }

    #[test]
    fn test_is_monitoring_false() {
        let detector = create_test_detector();
        assert!(!detector.is_monitoring(SessionId(1)));
    }

    #[test]
    fn test_is_monitoring_true() {
        let mut detector = create_test_detector();
        detector.start_monitoring(SessionId(1), 1234).unwrap();
        assert!(detector.is_monitoring(SessionId(1)));
    }

    // OSC 133 shell state tests
    #[test]
    fn test_shell_state_prompt_start_sets_idle() {
        let mut detector = create_test_detector();
        detector.start_monitoring(SessionId(1), 1234).unwrap();

        detector.set_shell_state(SessionId(1), ShellState::PromptStart);
        assert_eq!(detector.get_status(SessionId(1)), Some(SessionStatus::Idle));
    }

    #[test]
    fn test_shell_state_command_input_start_sets_idle() {
        let mut detector = create_test_detector();
        detector.start_monitoring(SessionId(1), 1234).unwrap();

        detector.set_shell_state(SessionId(1), ShellState::CommandInputStart);
        assert_eq!(detector.get_status(SessionId(1)), Some(SessionStatus::Idle));
    }

    #[test]
    fn test_shell_state_command_executed_sets_working() {
        let mut detector = create_test_detector();
        detector.start_monitoring(SessionId(1), 1234).unwrap();

        detector.set_shell_state(SessionId(1), ShellState::CommandExecuted);
        assert_eq!(
            detector.get_status(SessionId(1)),
            Some(SessionStatus::Working)
        );
    }

    #[test]
    fn test_shell_state_command_finished_sets_idle() {
        let mut detector = create_test_detector();
        detector.start_monitoring(SessionId(1), 1234).unwrap();

        detector.set_shell_state(
            SessionId(1),
            ShellState::CommandFinished { exit_code: Some(0) },
        );
        assert_eq!(detector.get_status(SessionId(1)), Some(SessionStatus::Idle));
    }

    #[test]
    fn test_shell_state_cycle() {
        let mut detector = create_test_detector();
        detector.start_monitoring(SessionId(1), 1234).unwrap();

        // Shell shows prompt → idle
        detector.set_shell_state(SessionId(1), ShellState::PromptStart);
        detector.set_shell_state(SessionId(1), ShellState::CommandInputStart);
        assert_eq!(detector.get_status(SessionId(1)), Some(SessionStatus::Idle));

        // User runs command → working
        detector.set_shell_state(SessionId(1), ShellState::CommandExecuted);
        assert_eq!(
            detector.get_status(SessionId(1)),
            Some(SessionStatus::Working)
        );

        // Command finishes, new prompt → idle
        detector.set_shell_state(
            SessionId(1),
            ShellState::CommandFinished { exit_code: Some(0) },
        );
        assert_eq!(detector.get_status(SessionId(1)), Some(SessionStatus::Idle));

        detector.set_shell_state(SessionId(1), ShellState::PromptStart);
        assert_eq!(detector.get_status(SessionId(1)), Some(SessionStatus::Idle));
    }

    #[test]
    fn test_pattern_match_overrides_shell_state() {
        let mut detector = create_test_detector();
        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();

        // Shell says idle via OSC 133
        detector.set_shell_state(SessionId(1), ShellState::CommandInputStart);

        // But pattern match detects y/n prompt — takes priority
        detector.process_output(SessionId(1), b"Continue? [y/n] ");
        assert_eq!(
            detector.get_status(SessionId(1)),
            Some(SessionStatus::WaitingForInput)
        );
    }

    #[test]
    fn test_set_shell_state_nonexistent_session() {
        let mut detector = create_test_detector();
        // Should not panic
        detector.set_shell_state(SessionId(999), ShellState::PromptStart);
    }
}
