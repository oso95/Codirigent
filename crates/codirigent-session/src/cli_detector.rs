//! CLI Type Auto-Detection.
//!
//! This module provides automatic detection of CLI types (Claude Code, Gemini CLI,
//! Codex CLI) from process trees associated with PTY sessions.
//!
//! # Overview
//!
//! The CLI detector walks the process tree starting from a PTY's root process,
//! looking for known AI CLI tool processes. It caches detected CLI types per
//! session for efficient lookup.
//!
//! # Example
//!
//! ```no_run
//! use codirigent_session::cli_detector::{CliDetector, DefaultCliDetector};
//! use codirigent_core::{CliType, SessionId};
//!
//! let mut detector = DefaultCliDetector::new();
//!
//! // Detect CLI type from process tree (requires valid PTY PID)
//! let cli_type = detector.detect_cli_type(12345);
//!
//! // Update and cache for a session
//! detector.update_session_cli(SessionId(1), 12345);
//!
//! // Get cached CLI type
//! if let Some(cli) = detector.get_session_cli(SessionId(1)) {
//!     println!("Session 1 is running: {:?}", cli);
//! }
//! ```

use codirigent_core::{CliType, SessionId};
use codirigent_detector::platform::{create_native_monitor, PlatformMonitor, ProcessInfo};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, trace, warn};

/// Maximum depth to walk the process tree.
///
/// This prevents infinite loops and excessive resource usage when
/// walking deeply nested process trees.
const MAX_RECURSION_DEPTH: u32 = 10;

/// Trait for CLI type detection from process trees.
///
/// Implementations of this trait provide the ability to detect which
/// AI CLI tool is running in a session by examining its process tree.
pub trait CliDetector: Send + Sync {
    /// Detect the CLI type from a process tree starting at the given PTY PID.
    ///
    /// Walks the process tree up to [`MAX_RECURSION_DEPTH`] levels deep,
    /// looking for known CLI process patterns.
    ///
    /// # Arguments
    ///
    /// * `pty_pid` - The root process ID of the PTY
    ///
    /// # Returns
    ///
    /// The detected [`CliType`], or [`CliType::GenericShell`] if no known
    /// CLI is detected.
    fn detect_cli_type(&self, pty_pid: u32) -> CliType;

    /// Update the cached CLI type for a session.
    ///
    /// Performs detection and caches the result for the given session.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session to update
    /// * `pty_pid` - The PTY process ID to detect from
    fn update_session_cli(&self, session_id: SessionId, pty_pid: u32);

    /// Get the cached CLI type for a session.
    ///
    /// Returns the previously detected and cached CLI type for the session,
    /// or `None` if no detection has been performed.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session to query
    ///
    /// # Returns
    ///
    /// The cached [`CliType`] if available.
    fn get_session_cli(&self, session_id: SessionId) -> Option<CliType>;
}

/// Default implementation of CLI type detection.
///
/// Uses platform-native process monitoring to walk process trees and
/// detect CLI types. Caches results per session for efficient lookup.
///
/// # Thread Safety
///
/// This implementation is `Send + Sync` and uses internal locking
/// for the cache, making it safe to use from multiple threads.
///
/// # Example
///
/// ```no_run
/// use codirigent_session::cli_detector::{CliDetector, DefaultCliDetector};
/// use codirigent_core::SessionId;
///
/// let detector = DefaultCliDetector::new();
///
/// // Detection works without caching
/// let cli = detector.detect_cli_type(12345);
///
/// // Cache for repeated lookups
/// detector.update_session_cli(SessionId(1), 12345);
/// let cached = detector.get_session_cli(SessionId(1));
/// ```
pub struct DefaultCliDetector {
    /// Cached CLI types per session.
    cache: Arc<RwLock<HashMap<SessionId, CliType>>>,
}

impl DefaultCliDetector {
    /// Create a new CLI detector.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_session::cli_detector::DefaultCliDetector;
    ///
    /// let detector = DefaultCliDetector::new();
    /// ```
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Walk the process tree and detect CLI type.
    ///
    /// Recursively examines child processes up to the maximum depth,
    /// checking each process against known CLI patterns.
    fn walk_process_tree(
        &self,
        monitor: &dyn PlatformMonitor,
        pid: u32,
        depth: u32,
    ) -> Option<CliType> {
        if depth > MAX_RECURSION_DEPTH {
            trace!(pid, depth, "Reached max recursion depth");
            return None;
        }

        // Get process info for current PID
        let info = match monitor.get_process_info(pid) {
            Ok(info) => info,
            Err(e) => {
                trace!(pid, error = %e, "Failed to get process info");
                return None;
            }
        };

        // Check if this process is a known CLI
        if let Some(cli) = self.detect_from_process_info(&info) {
            debug!(pid, ?cli, command = ?info.command, "Detected CLI type");
            return Some(cli);
        }

        // Get child processes and recurse
        let children = match monitor.get_child_processes(pid) {
            Ok(children) => children,
            Err(e) => {
                trace!(pid, error = %e, "Failed to get child processes");
                return None;
            }
        };

        // Check each child process
        for child_pid in children {
            if let Some(cli) = self.walk_process_tree(monitor, child_pid, depth + 1) {
                return Some(cli);
            }
        }

        None
    }

    /// Detect CLI type from process information.
    ///
    /// Uses the process command name to detect known CLI tools.
    fn detect_from_process_info(&self, info: &ProcessInfo) -> Option<CliType> {
        let command = info.command.as_ref()?;

        // Use CliType::detect from dirigent-core
        let cli = CliType::detect(command, None);

        // Only return if we detected a specific CLI (not GenericShell)
        if cli != CliType::GenericShell {
            Some(cli)
        } else {
            None
        }
    }
}

impl Default for DefaultCliDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl CliDetector for DefaultCliDetector {
    fn detect_cli_type(&self, pty_pid: u32) -> CliType {
        let monitor = create_native_monitor();

        match self.walk_process_tree(&monitor, pty_pid, 0) {
            Some(cli) => {
                debug!(pty_pid, ?cli, "Detected CLI type from process tree");
                cli
            }
            None => {
                debug!(pty_pid, "No CLI detected, using GenericShell");
                CliType::GenericShell
            }
        }
    }

    fn update_session_cli(&self, session_id: SessionId, pty_pid: u32) {
        let cli_type = self.detect_cli_type(pty_pid);

        match self.cache.write() {
            Ok(mut cache) => {
                debug!(?session_id, ?cli_type, "Cached CLI type for session");
                cache.insert(session_id, cli_type);
            }
            Err(e) => {
                warn!(?session_id, error = %e, "Failed to acquire cache write lock");
            }
        }
    }

    fn get_session_cli(&self, session_id: SessionId) -> Option<CliType> {
        match self.cache.read() {
            Ok(cache) => cache.get(&session_id).copied(),
            Err(e) => {
                warn!(?session_id, error = %e, "Failed to acquire cache read lock");
                None
            }
        }
    }
}

impl std::fmt::Debug for DefaultCliDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultCliDetector")
            .field(
                "cache_size",
                &self.cache.read().map(|c| c.len()).unwrap_or(0),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_detector_new() {
        let detector = DefaultCliDetector::new();
        assert!(detector.cache.read().unwrap().is_empty());
    }

    #[test]
    fn test_cli_detector_default() {
        let detector = DefaultCliDetector::default();
        assert!(detector.cache.read().unwrap().is_empty());
    }

    #[test]
    fn test_cli_detector_cache_update() {
        let detector = DefaultCliDetector::new();
        let session_id = SessionId(42);

        // Update with current process (will detect as GenericShell most likely)
        let pid = std::process::id();
        detector.update_session_cli(session_id, pid);

        // Should have cached something
        let cached = detector.get_session_cli(session_id);
        assert!(cached.is_some());
    }

    #[test]
    fn test_cli_detector_cache_get() {
        let detector = DefaultCliDetector::new();
        let session_id = SessionId(99);

        // No cache entry yet
        assert!(detector.get_session_cli(session_id).is_none());

        // Manually insert into cache
        {
            let mut cache = detector.cache.write().unwrap();
            cache.insert(session_id, CliType::ClaudeCode);
        }

        // Should retrieve from cache
        let cached = detector.get_session_cli(session_id);
        assert_eq!(cached, Some(CliType::ClaudeCode));
    }

    #[test]
    fn test_cli_detector_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DefaultCliDetector>();
    }

    #[test]
    fn test_cli_detector_debug() {
        let detector = DefaultCliDetector::new();
        let debug_str = format!("{:?}", detector);
        assert!(debug_str.contains("DefaultCliDetector"));
        assert!(debug_str.contains("cache_size"));
    }

    #[test]
    fn test_cli_detector_multiple_sessions() {
        let detector = DefaultCliDetector::new();

        // Manually cache multiple sessions
        {
            let mut cache = detector.cache.write().unwrap();
            cache.insert(SessionId(1), CliType::ClaudeCode);
            cache.insert(SessionId(2), CliType::GeminiCli);
            cache.insert(SessionId(3), CliType::CodexCli);
        }

        assert_eq!(
            detector.get_session_cli(SessionId(1)),
            Some(CliType::ClaudeCode)
        );
        assert_eq!(
            detector.get_session_cli(SessionId(2)),
            Some(CliType::GeminiCli)
        );
        assert_eq!(
            detector.get_session_cli(SessionId(3)),
            Some(CliType::CodexCli)
        );
        assert_eq!(detector.get_session_cli(SessionId(4)), None);
    }

    #[test]
    fn test_cli_detector_update_overwrites() {
        let detector = DefaultCliDetector::new();
        let session_id = SessionId(1);

        // Manually insert initial value
        {
            let mut cache = detector.cache.write().unwrap();
            cache.insert(session_id, CliType::GeminiCli);
        }

        assert_eq!(
            detector.get_session_cli(session_id),
            Some(CliType::GeminiCli)
        );

        // Update (will detect from current process)
        let pid = std::process::id();
        detector.update_session_cli(session_id, pid);

        // Value should be updated (most likely to GenericShell for test process)
        let cached = detector.get_session_cli(session_id);
        assert!(cached.is_some());
    }

    #[test]
    fn test_cli_detector_detect_from_process_info() {
        let detector = DefaultCliDetector::new();

        // Test with Claude process
        let claude_info = ProcessInfo {
            pid: 1234,
            state: codirigent_detector::platform::ProcessState::Running,
            ppid: Some(1),
            command: Some("claude".to_string()),
        };
        assert_eq!(
            detector.detect_from_process_info(&claude_info),
            Some(CliType::ClaudeCode)
        );

        // Test with Gemini process
        let gemini_info = ProcessInfo {
            pid: 1235,
            state: codirigent_detector::platform::ProcessState::Running,
            ppid: Some(1),
            command: Some("gemini-cli".to_string()),
        };
        assert_eq!(
            detector.detect_from_process_info(&gemini_info),
            Some(CliType::GeminiCli)
        );

        // Test with Codex process
        let codex_info = ProcessInfo {
            pid: 1236,
            state: codirigent_detector::platform::ProcessState::Running,
            ppid: Some(1),
            command: Some("codex".to_string()),
        };
        assert_eq!(
            detector.detect_from_process_info(&codex_info),
            Some(CliType::CodexCli)
        );

        // Test with generic process
        let bash_info = ProcessInfo {
            pid: 1237,
            state: codirigent_detector::platform::ProcessState::Running,
            ppid: Some(1),
            command: Some("bash".to_string()),
        };
        assert_eq!(detector.detect_from_process_info(&bash_info), None);

        // Test with no command
        let no_cmd_info = ProcessInfo {
            pid: 1238,
            state: codirigent_detector::platform::ProcessState::Running,
            ppid: Some(1),
            command: None,
        };
        assert_eq!(detector.detect_from_process_info(&no_cmd_info), None);
    }

    #[test]
    fn test_cli_detector_detect_current_process() {
        let detector = DefaultCliDetector::new();
        let pid = std::process::id();

        // Detect from current process - should not panic
        let cli_type = detector.detect_cli_type(pid);

        // Current test process should be GenericShell (not a known CLI)
        assert_eq!(cli_type, CliType::GenericShell);
    }

    #[test]
    fn test_cli_detector_detect_nonexistent_process() {
        let detector = DefaultCliDetector::new();

        // Very high PID that's unlikely to exist
        let cli_type = detector.detect_cli_type(999_999_999);

        // Should return GenericShell for non-existent process
        assert_eq!(cli_type, CliType::GenericShell);
    }

    #[test]
    fn test_max_recursion_depth_constant() {
        // Verify the constant is reasonable
        assert_eq!(MAX_RECURSION_DEPTH, 10);
    }
}
