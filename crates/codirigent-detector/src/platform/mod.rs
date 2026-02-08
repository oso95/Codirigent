//! Platform-specific process monitoring.
//!
//! This module provides a unified interface for process monitoring across
//! Linux, macOS, and Windows platforms. Each platform has its own implementation
//! that uses native APIs for optimal performance.

use anyhow::Result;

/// Platform-agnostic process state.
///
/// Represents the current execution state of a process in a way that is
/// meaningful across all supported platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessState {
    /// Process is actively executing (on CPU or ready to run).
    Running,
    /// Process is sleeping (interruptible, waiting for an event).
    Sleeping,
    /// Process is stopped (e.g., by a signal or debugger).
    Stopped,
    /// Process has terminated (zombie or exited).
    Terminated,
    /// State could not be determined.
    #[default]
    Unknown,
}

impl std::fmt::Display for ProcessState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "Running"),
            Self::Sleeping => write!(f, "Sleeping"),
            Self::Stopped => write!(f, "Stopped"),
            Self::Terminated => write!(f, "Terminated"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Platform-agnostic process information.
///
/// Contains essential information about a process that can be retrieved
/// on all supported platforms.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Process ID.
    pub pid: u32,
    /// Current process state.
    pub state: ProcessState,
    /// Parent process ID, if available.
    pub ppid: Option<u32>,
    /// Command name or executable name.
    pub command: Option<String>,
}

impl ProcessInfo {
    /// Create a new ProcessInfo with the given values.
    pub fn new(pid: u32, state: ProcessState) -> Self {
        Self {
            pid,
            state,
            ppid: None,
            command: None,
        }
    }

    /// Set the parent process ID.
    pub fn with_ppid(mut self, ppid: u32) -> Self {
        self.ppid = Some(ppid);
        self
    }

    /// Set the command name.
    pub fn with_command(mut self, command: String) -> Self {
        self.command = Some(command);
        self
    }
}

/// Platform-specific process monitor trait.
///
/// Implementations of this trait provide platform-native process monitoring
/// capabilities. The trait is designed to be object-safe and thread-safe.
pub trait PlatformMonitor: Send + Sync {
    /// Get the current state of a process by PID.
    ///
    /// # Arguments
    ///
    /// * `pid` - The process ID to query
    ///
    /// # Returns
    ///
    /// The current process state, or an error if the process cannot be found
    /// or accessed.
    fn get_process_state(&self, pid: u32) -> Result<ProcessState>;

    /// Get detailed information about a process.
    ///
    /// # Arguments
    ///
    /// * `pid` - The process ID to query
    ///
    /// # Returns
    ///
    /// Process information including state, parent PID, and command name.
    fn get_process_info(&self, pid: u32) -> Result<ProcessInfo>;

    /// Get the child processes of a given PID.
    ///
    /// # Arguments
    ///
    /// * `pid` - The parent process ID
    ///
    /// # Returns
    ///
    /// A vector of child process IDs. May be empty if there are no children.
    fn get_child_processes(&self, pid: u32) -> Result<Vec<u32>>;

    /// Get the foreground process group ID for a TTY.
    ///
    /// On Unix systems, this uses `tcgetpgrp()` to get the foreground
    /// process group. On Windows, this returns -1 as the concept doesn't
    /// directly apply.
    ///
    /// # Arguments
    ///
    /// * `tty_fd` - File descriptor for the TTY (Unix) or -1 (Windows)
    ///
    /// # Returns
    ///
    /// The foreground process group ID, or -1 on Windows.
    fn get_foreground_pgid(&self, tty_fd: i32) -> Result<i32>;

    /// Check if a process is likely waiting for user input.
    ///
    /// This uses platform-specific heuristics to determine if a process
    /// is waiting for input from the terminal.
    ///
    /// # Arguments
    ///
    /// * `pid` - The process ID to check
    /// * `tty_fd` - File descriptor for the TTY (Unix) or -1 (Windows)
    ///
    /// # Returns
    ///
    /// `true` if the process appears to be waiting for input.
    fn is_likely_waiting_for_input(&self, pid: u32, tty_fd: i32) -> Result<bool>;
}

// Platform-specific implementations
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

// Re-export the native implementation for the current platform
#[cfg(target_os = "linux")]
pub use linux::LinuxMonitor as NativeMonitor;
#[cfg(target_os = "macos")]
pub use macos::MacOSMonitor as NativeMonitor;
#[cfg(target_os = "windows")]
pub use windows::WindowsMonitor as NativeMonitor;

/// Create a new platform-native monitor instance.
///
/// This is a convenience function that creates the appropriate monitor
/// for the current platform.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn create_native_monitor() -> NativeMonitor {
    NativeMonitor::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_state_display() {
        assert_eq!(format!("{}", ProcessState::Running), "Running");
        assert_eq!(format!("{}", ProcessState::Sleeping), "Sleeping");
        assert_eq!(format!("{}", ProcessState::Stopped), "Stopped");
        assert_eq!(format!("{}", ProcessState::Terminated), "Terminated");
        assert_eq!(format!("{}", ProcessState::Unknown), "Unknown");
    }

    #[test]
    fn test_process_state_default() {
        assert_eq!(ProcessState::default(), ProcessState::Unknown);
    }

    #[test]
    fn test_process_state_equality() {
        assert_eq!(ProcessState::Running, ProcessState::Running);
        assert_ne!(ProcessState::Running, ProcessState::Sleeping);
    }

    #[test]
    fn test_process_state_clone() {
        let state = ProcessState::Running;
        let cloned = state;
        assert_eq!(state, cloned);
    }

    #[test]
    fn test_process_state_debug() {
        let state = ProcessState::Running;
        let debug_str = format!("{:?}", state);
        assert_eq!(debug_str, "Running");
    }

    #[test]
    fn test_process_info_new() {
        let info = ProcessInfo::new(1234, ProcessState::Running);
        assert_eq!(info.pid, 1234);
        assert_eq!(info.state, ProcessState::Running);
        assert!(info.ppid.is_none());
        assert!(info.command.is_none());
    }

    #[test]
    fn test_process_info_with_ppid() {
        let info = ProcessInfo::new(1234, ProcessState::Running).with_ppid(1);
        assert_eq!(info.ppid, Some(1));
    }

    #[test]
    fn test_process_info_with_command() {
        let info = ProcessInfo::new(1234, ProcessState::Running).with_command("bash".to_string());
        assert_eq!(info.command, Some("bash".to_string()));
    }

    #[test]
    fn test_process_info_builder_chain() {
        let info = ProcessInfo::new(1234, ProcessState::Sleeping)
            .with_ppid(1)
            .with_command("node".to_string());
        assert_eq!(info.pid, 1234);
        assert_eq!(info.state, ProcessState::Sleeping);
        assert_eq!(info.ppid, Some(1));
        assert_eq!(info.command, Some("node".to_string()));
    }

    #[test]
    fn test_process_info_clone() {
        let info = ProcessInfo::new(1234, ProcessState::Running)
            .with_ppid(1)
            .with_command("test".to_string());
        let cloned = info.clone();
        assert_eq!(info.pid, cloned.pid);
        assert_eq!(info.state, cloned.state);
        assert_eq!(info.ppid, cloned.ppid);
        assert_eq!(info.command, cloned.command);
    }

    #[test]
    fn test_process_info_debug() {
        let info = ProcessInfo::new(1234, ProcessState::Running);
        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("1234"));
        assert!(debug_str.contains("Running"));
    }
}
