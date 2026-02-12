//! Codirigent Detector
//!
//! Process monitoring, input detection, and status tracking for sessions.
//!
//! This crate provides the core detection functionality for Codirigent,
//! determining when AI CLI sessions (Claude Code, Codex CLI, Gemini CLI)
//! are waiting for user input.
//!
//! # Overview
//!
//! The detector uses a combination of strategies:
//!
//! - **Platform-specific process monitoring** via `/proc` on Linux,
//!   `libproc` on macOS, and Win32 APIs on Windows
//! - **Output pattern matching** to detect common input prompts
//! - **Timing heuristics** to identify idle processes
//!
//! # Modules
//!
//! - [`platform`] - Platform-specific process monitoring
//! - [`patterns`] - Input prompt pattern matching
//! - [`detector`] - Main input detector implementation
//! - [`notification`] - Desktop notification support
//!
//! # Quick Start
//!
//! ```no_run
//! use codirigent_detector::{InputDetector, DetectorConfig};
//! use codirigent_core::{DefaultEventBus, SessionId, ProcessMonitor};
//! use std::sync::Arc;
//!
//! // Create an event bus
//! let event_bus = Arc::new(DefaultEventBus::new(16));
//!
//! // Create the detector
//! let mut detector = InputDetector::new(DetectorConfig::default(), event_bus);
//!
//! // Start monitoring a session
//! detector.start_monitoring(SessionId(1), 12345).unwrap();
//!
//! // Process output from the session
//! detector.process_output(SessionId(1), b"Continue? [y/n] ");
//!
//! // Check the detected status
//! use codirigent_core::SessionStatus;
//! assert_eq!(detector.get_status(SessionId(1)), Some(SessionStatus::NeedsAttention));
//! ```
//!
//! # Platform Support
//!
//! The crate supports three platforms:
//!
//! - **Linux**: Uses the `/proc` filesystem via the `procfs` crate
//! - **macOS**: Uses `libproc` for BSD process information
//! - **Windows**: Uses Win32 APIs (ToolHelp32, process status)
//!
//! # Pattern Detection
//!
//! The detector recognizes common input prompts:
//!
//! - Yes/No confirmations: `[y/n]`, `[Y/n]`, `[yes/no]`, `(y/N)`
//! - Question prompts: `? ` at end of line
//! - Shell/REPL prompts: `> ` at end of line
//! - Press Enter prompts
//! - Continue prompts
//! - Password prompts
//!
//! Custom patterns can be added via [`InputDetector::add_pattern`] or
//! through [`DetectorConfig::custom_patterns`].
//!
//! # Notifications
//!
//! The crate can send desktop notifications when sessions require input:
//!
//! ```no_run
//! use codirigent_detector::notification::notify_input_required;
//! use codirigent_core::SessionId;
//!
//! notify_input_required(SessionId(1), "Claude Code");
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod detector;
pub mod notification;
pub mod patterns;
pub mod platform;

// Re-export main types for convenience
pub use detector::{DetectorConfig, InputDetector};
pub use notification::{
    notify_error, notify_input_required, notify_task_completed, send_notification,
};
pub use patterns::{DEFAULT_PATTERNS, DEFAULT_RECENT_LINES_TO_CHECK};
pub use platform::{NativeMonitor, PlatformMonitor, ProcessInfo, ProcessState};

// Re-export the factory function
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub use platform::create_native_monitor;

use std::path::PathBuf;

/// Find which file from `candidates` the given PID currently has open.
///
/// Uses platform-specific APIs:
/// - **Linux**: reads `/proc/<pid>/fd/` symlinks
/// - **macOS**: uses `lsof -p <PID> -Fn`
/// - **Windows**: uses the Restart Manager API
///
/// Returns the original (non-canonicalized) candidate path on match,
/// or `None` if no match is found or on any error.
pub fn find_file_opened_by_pid(candidates: &[PathBuf], pid: u32) -> Option<PathBuf> {
    find_file_opened_by_pid_impl(candidates, pid)
}

/// Build a lookup from canonical path → original candidate path.
/// Entries that fail to canonicalize are skipped.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn canonical_candidate_map(candidates: &[PathBuf]) -> Vec<(PathBuf, PathBuf)> {
    candidates
        .iter()
        .filter_map(|c| {
            std::fs::canonicalize(c)
                .ok()
                .map(|canon| (canon, c.clone()))
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn find_file_opened_by_pid_impl(candidates: &[PathBuf], pid: u32) -> Option<PathBuf> {
    let canon_map = canonical_candidate_map(candidates);
    let fd_dir = format!("/proc/{}/fd", pid);
    let entries = std::fs::read_dir(&fd_dir).ok()?;
    for entry in entries.flatten() {
        if let Ok(target) = std::fs::read_link(entry.path()) {
            // /proc/pid/fd symlinks already resolve to canonical paths
            if let Some((_, original)) = canon_map.iter().find(|(canon, _)| *canon == target) {
                return Some(original.clone());
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn find_file_opened_by_pid_impl(candidates: &[PathBuf], pid: u32) -> Option<PathBuf> {
    let canon_map = canonical_candidate_map(candidates);
    let output = std::process::Command::new("lsof")
        .args(["-p", &pid.to_string(), "-Fn"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(path_str) = line.strip_prefix('n') {
            let path = PathBuf::from(path_str);
            // lsof returns canonical paths (e.g., /private/var/... not /var/...)
            if let Some((_, original)) = canon_map.iter().find(|(canon, _)| *canon == path) {
                return Some(original.clone());
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn find_file_opened_by_pid_impl(candidates: &[PathBuf], pid: u32) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::System::RestartManager::*;

    for candidate in candidates {
        let wide_path: Vec<u16> = candidate
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut session: u32 = 0;
        let mut session_key = [0u16; CCH_RM_SESSION_KEY as usize + 1];

        unsafe {
            if RmStartSession(&mut session, Some(0), PWSTR(session_key.as_mut_ptr())).is_err() {
                continue;
            }
            let file_ptr = PCWSTR(wide_path.as_ptr());
            let files = [file_ptr];
            if RmRegisterResources(session, Some(&files), None, None).is_ok() {
                let mut needed: u32 = 0;
                let mut info_count: u32 = 16;
                let mut infos = vec![RM_PROCESS_INFO::default(); 16];
                let mut reason: u32 = 0;
                let _ = RmGetList(
                    session,
                    &mut needed,
                    &mut info_count,
                    Some(infos.as_mut_ptr()),
                    &mut reason,
                );
                for info in infos.iter().take(info_count as usize) {
                    if info.Process.dwProcessId == pid {
                        let _ = RmEndSession(session);
                        return Some(candidate.clone());
                    }
                }
            }
            let _ = RmEndSession(session);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use codirigent_core::{DefaultEventBus, ProcessMonitor, SessionId, SessionStatus};
    use std::sync::Arc;

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn test_find_file_opened_by_pid_with_current_process() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file_path = tmp.path().join("test.jsonl");

        // Open a file and keep the handle alive
        let _file = std::fs::File::create(&file_path).unwrap();

        let candidates = vec![file_path.clone()];
        let pid = std::process::id();

        let result = find_file_opened_by_pid(&candidates, pid);
        assert_eq!(result, Some(file_path));
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn test_find_file_opened_by_pid_no_match() {
        let tmp = tempfile::TempDir::new().unwrap();
        // This file doesn't exist and isn't opened by anyone
        let fake_path = tmp.path().join("not_opened.jsonl");
        let candidates = vec![fake_path];
        let pid = std::process::id();

        let result = find_file_opened_by_pid(&candidates, pid);
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_file_opened_by_pid_empty_candidates() {
        let result = find_file_opened_by_pid(&[], std::process::id());
        assert_eq!(result, None);
    }

    #[test]
    fn test_process_state_reexport() {
        // Verify that ProcessState is accessible from the crate root
        let state = ProcessState::Running;
        assert_eq!(format!("{}", state), "Running");
    }

    #[test]
    fn test_process_info_reexport() {
        // Verify that ProcessInfo is accessible from the crate root
        let info = ProcessInfo::new(1234, ProcessState::Running);
        assert_eq!(info.pid, 1234);
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn test_native_monitor_reexport() {
        // Verify that NativeMonitor is accessible from the crate root
        let monitor = NativeMonitor::new();
        let pid = std::process::id();
        let result = monitor.get_process_state(pid);
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn test_create_native_monitor_reexport() {
        // Verify that create_native_monitor is accessible from the crate root
        let monitor = create_native_monitor();
        let pid = std::process::id();
        let result = monitor.get_process_state(pid);
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn test_platform_monitor_trait_reexport() {
        // Verify that PlatformMonitor trait is usable with NativeMonitor
        fn use_monitor(monitor: &dyn PlatformMonitor) -> bool {
            let pid = std::process::id();
            monitor.get_process_state(pid).is_ok()
        }

        let monitor = NativeMonitor::new();
        assert!(use_monitor(&monitor));
    }

    #[test]
    fn test_default_patterns_reexport() {
        // Verify DEFAULT_PATTERNS is accessible
        assert!(!DEFAULT_PATTERNS.is_empty());
    }

    #[test]
    fn test_detector_config_reexport() {
        // Verify DetectorConfig is accessible
        let config = DetectorConfig::default();
        assert!(config.notifications_enabled);
    }

    #[test]
    fn test_input_detector_reexport() {
        // Verify InputDetector is accessible and usable
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let mut detector = InputDetector::new(DetectorConfig::default(), event_bus);

        detector.start_monitoring(SessionId(1), 1234).unwrap();
        assert!(detector.get_status(SessionId(1)).is_some());
    }

    #[test]
    fn test_notification_functions_reexport() {
        // Verify notification functions are accessible
        send_notification("Test", "Test");
        notify_input_required(SessionId(1), "Test");
        notify_task_completed(SessionId(1), "Test", true);
        notify_error(SessionId(1), "Test", "Error");
    }

    #[test]
    fn test_full_detection_workflow() {
        // Integration test for the full detection workflow
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let mut detector = InputDetector::new(DetectorConfig::default(), event_bus);

        // Start monitoring
        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();

        // Initial status should be Idle or Working depending on process state
        let initial_status = detector.get_status(SessionId(1));
        assert!(initial_status.is_some());

        // Process output with pattern
        detector.process_output(SessionId(1), b"Continue? [y/n]");

        // Should detect NeedsAttention
        assert_eq!(
            detector.get_status(SessionId(1)),
            Some(SessionStatus::NeedsAttention)
        );

        // Stop monitoring
        detector.stop_monitoring(SessionId(1));
        assert!(detector.get_status(SessionId(1)).is_none());
    }

    #[test]
    fn test_custom_pattern_workflow() {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let mut detector = InputDetector::new(DetectorConfig::default(), event_bus);

        // Add custom pattern
        detector.add_pattern(r"custom-prompt>".to_string());

        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();

        // Process output with custom pattern
        detector.process_output(SessionId(1), b"custom-prompt>");

        // Should detect NeedsAttention
        assert_eq!(
            detector.get_status(SessionId(1)),
            Some(SessionStatus::NeedsAttention)
        );
    }

    #[test]
    fn test_detector_with_custom_config() {
        let config = DetectorConfig::with_patterns(vec![r"my-prompt:".to_string()]);
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let mut detector = InputDetector::new(config, event_bus);

        detector
            .start_monitoring(SessionId(1), std::process::id())
            .unwrap();
        detector.process_output(SessionId(1), b"my-prompt:");

        assert_eq!(
            detector.get_status(SessionId(1)),
            Some(SessionStatus::NeedsAttention)
        );
    }

    #[test]
    fn test_multiple_sessions() {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let mut detector = InputDetector::new(DetectorConfig::default(), event_bus);

        // Start multiple sessions
        detector.start_monitoring(SessionId(1), 1234).unwrap();
        detector.start_monitoring(SessionId(2), 5678).unwrap();
        detector.start_monitoring(SessionId(3), 9012).unwrap();

        assert_eq!(detector.session_count(), 3);

        // Process different outputs
        detector.process_output(SessionId(1), b"Continue? [y/n]");
        detector.process_output(SessionId(2), b"Working...");

        assert_eq!(
            detector.get_status(SessionId(1)),
            Some(SessionStatus::NeedsAttention)
        );

        // Stop one session
        detector.stop_monitoring(SessionId(2));
        assert_eq!(detector.session_count(), 2);
    }
}
