//! Linux-specific process monitoring using the /proc filesystem.
//!
//! This module uses the `procfs` crate to read process information from
//! the Linux `/proc` filesystem. It provides efficient access to process
//! state, parent-child relationships, and foreground process detection.

use super::{PlatformMonitor, ProcessInfo, ProcessState};
use anyhow::{Context, Result};
use procfs::process::Process;
use tracing::debug;

/// Linux process monitor using the /proc filesystem.
///
/// This monitor uses the `procfs` crate for safe access to `/proc` entries,
/// with fallback mechanisms for systems where certain features may be
/// unavailable (e.g., containers without full /proc access).
#[derive(Debug, Default)]
pub struct LinuxMonitor;

impl LinuxMonitor {
    /// Create a new Linux process monitor.
    pub fn new() -> Self {
        Self
    }

    /// Convert a Linux process state character to ProcessState.
    ///
    /// Linux process states from /proc/[pid]/stat:
    /// - R: Running
    /// - S: Sleeping (interruptible)
    /// - D: Disk sleep (uninterruptible)
    /// - T: Stopped (on a signal)
    /// - t: Tracing stop
    /// - Z: Zombie
    /// - X: Dead
    /// - I: Idle (kernel thread)
    fn state_from_char(state_char: char) -> ProcessState {
        match state_char {
            'R' => ProcessState::Running,
            'S' | 'D' | 'I' => ProcessState::Sleeping,
            'T' | 't' => ProcessState::Stopped,
            'Z' | 'X' => ProcessState::Terminated,
            _ => ProcessState::Unknown,
        }
    }

    /// Get the process group ID for a process.
    fn get_process_pgid(&self, pid: u32) -> Result<i32> {
        let process =
            Process::new(pid as i32).with_context(|| format!("Failed to open process {}", pid))?;
        let stat = process
            .stat()
            .with_context(|| format!("Failed to read stat for process {}", pid))?;
        Ok(stat.pgrp)
    }
}

impl PlatformMonitor for LinuxMonitor {
    fn get_process_state(&self, pid: u32) -> Result<ProcessState> {
        let process =
            Process::new(pid as i32).with_context(|| format!("Failed to open process {}", pid))?;

        let stat = process
            .stat()
            .with_context(|| format!("Failed to read stat for process {}", pid))?;

        let state = Self::state_from_char(stat.state);
        debug!(pid, ?state, state_char = %stat.state, "Got Linux process state");
        Ok(state)
    }

    fn get_process_info(&self, pid: u32) -> Result<ProcessInfo> {
        let process =
            Process::new(pid as i32).with_context(|| format!("Failed to open process {}", pid))?;

        let stat = process
            .stat()
            .with_context(|| format!("Failed to read stat for process {}", pid))?;

        let state = Self::state_from_char(stat.state);

        Ok(ProcessInfo {
            pid,
            state,
            ppid: Some(stat.ppid as u32),
            command: Some(stat.comm),
        })
    }

    fn get_child_processes(&self, pid: u32) -> Result<Vec<u32>> {
        let mut children = Vec::new();

        // First, try to read from /proc/<pid>/task/<pid>/children
        // This is more efficient but may not be available in all environments
        let children_path = format!("/proc/{}/task/{}/children", pid, pid);
        if let Ok(content) = std::fs::read_to_string(&children_path) {
            for child_str in content.split_whitespace() {
                if let Ok(child_pid) = child_str.parse::<u32>() {
                    children.push(child_pid);
                }
            }
            debug!(
                pid,
                child_count = children.len(),
                "Got children from /proc children file"
            );
            return Ok(children);
        }

        // Fallback: scan all processes and find those with this pid as parent
        debug!(pid, "Falling back to scanning all processes for children");
        for entry in
            procfs::process::all_processes().with_context(|| "Failed to enumerate processes")?
        {
            if let Ok(proc) = entry {
                if let Ok(stat) = proc.stat() {
                    if stat.ppid == pid as i32 {
                        children.push(proc.pid as u32);
                    }
                }
            }
        }

        debug!(
            pid,
            child_count = children.len(),
            "Got children via process scan"
        );
        Ok(children)
    }

    fn get_foreground_pgid(&self, tty_fd: i32) -> Result<i32> {
        // SAFETY: tcgetpgrp is safe to call with any file descriptor.
        // It will return -1 with errno if the fd is not a valid terminal.
        let pgid = unsafe { libc::tcgetpgrp(tty_fd) };
        if pgid < 0 {
            let err = std::io::Error::last_os_error();
            anyhow::bail!("tcgetpgrp failed for fd {}: {}", tty_fd, err);
        }
        debug!(tty_fd, pgid, "Got foreground process group");
        Ok(pgid)
    }

    fn is_likely_waiting_for_input(&self, pid: u32, tty_fd: i32) -> Result<bool> {
        // First check if the process is sleeping
        let state = self.get_process_state(pid)?;
        if state != ProcessState::Sleeping {
            debug!(pid, ?state, "Process not sleeping, not waiting for input");
            return Ok(false);
        }

        // Check if it's the foreground process group
        let fg_pgid = match self.get_foreground_pgid(tty_fd) {
            Ok(pgid) => pgid,
            Err(e) => {
                debug!(pid, error = %e, "Could not get foreground pgid");
                return Ok(false);
            }
        };

        let process_pgid = self.get_process_pgid(pid)?;
        let is_foreground = process_pgid == fg_pgid;

        debug!(
            pid,
            process_pgid, fg_pgid, is_foreground, "Checking if process is waiting for input"
        );

        Ok(is_foreground)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_monitor_new() {
        let monitor = LinuxMonitor::new();
        // Just verify it can be created
        let _ = format!("{:?}", monitor);
    }

    #[test]
    fn test_linux_monitor_default() {
        let monitor = LinuxMonitor::default();
        let _ = format!("{:?}", monitor);
    }

    #[test]
    fn test_state_from_char_running() {
        assert_eq!(LinuxMonitor::state_from_char('R'), ProcessState::Running);
    }

    #[test]
    fn test_state_from_char_sleeping() {
        assert_eq!(LinuxMonitor::state_from_char('S'), ProcessState::Sleeping);
        assert_eq!(LinuxMonitor::state_from_char('D'), ProcessState::Sleeping);
        assert_eq!(LinuxMonitor::state_from_char('I'), ProcessState::Sleeping);
    }

    #[test]
    fn test_state_from_char_stopped() {
        assert_eq!(LinuxMonitor::state_from_char('T'), ProcessState::Stopped);
        assert_eq!(LinuxMonitor::state_from_char('t'), ProcessState::Stopped);
    }

    #[test]
    fn test_state_from_char_terminated() {
        assert_eq!(LinuxMonitor::state_from_char('Z'), ProcessState::Terminated);
        assert_eq!(LinuxMonitor::state_from_char('X'), ProcessState::Terminated);
    }

    #[test]
    fn test_state_from_char_unknown() {
        assert_eq!(LinuxMonitor::state_from_char('?'), ProcessState::Unknown);
        assert_eq!(LinuxMonitor::state_from_char(' '), ProcessState::Unknown);
    }

    #[test]
    fn test_get_current_process_state() {
        let monitor = LinuxMonitor::new();
        let pid = std::process::id();
        let result = monitor.get_process_state(pid);
        assert!(result.is_ok(), "Failed to get state: {:?}", result.err());
        // Current process should be Running (since it's executing this test)
        let state = result.unwrap();
        assert!(
            matches!(state, ProcessState::Running | ProcessState::Sleeping),
            "Unexpected state: {:?}",
            state
        );
    }

    #[test]
    fn test_get_process_state_nonexistent() {
        let monitor = LinuxMonitor::new();
        // Use a very high PID that's unlikely to exist
        let result = monitor.get_process_state(999_999_999);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_current_process_info() {
        let monitor = LinuxMonitor::new();
        let pid = std::process::id();
        let result = monitor.get_process_info(pid);
        assert!(result.is_ok(), "Failed to get info: {:?}", result.err());

        let info = result.unwrap();
        assert_eq!(info.pid, pid);
        assert!(info.ppid.is_some());
        assert!(info.command.is_some());
    }

    #[test]
    fn test_get_process_info_nonexistent() {
        let monitor = LinuxMonitor::new();
        let result = monitor.get_process_info(999_999_999);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_child_processes_current() {
        let monitor = LinuxMonitor::new();
        let pid = std::process::id();
        let result = monitor.get_child_processes(pid);
        assert!(result.is_ok(), "Failed to get children: {:?}", result.err());
        // May or may not have children, but should not error
    }

    #[test]
    fn test_get_child_processes_init() {
        let monitor = LinuxMonitor::new();
        // PID 1 (init/systemd) typically exists and has children
        let result = monitor.get_child_processes(1);
        // This might fail in containers, so just check it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_get_foreground_pgid_invalid_fd() {
        let monitor = LinuxMonitor::new();
        // -1 is not a valid file descriptor
        let result = monitor.get_foreground_pgid(-1);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_likely_waiting_for_input_invalid_fd() {
        let monitor = LinuxMonitor::new();
        let pid = std::process::id();
        // With an invalid fd, should return false (not error)
        let result = monitor.is_likely_waiting_for_input(pid, -1);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_is_likely_waiting_for_input_nonexistent_process() {
        let monitor = LinuxMonitor::new();
        let result = monitor.is_likely_waiting_for_input(999_999_999, -1);
        assert!(result.is_err());
    }

    #[test]
    fn test_monitor_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LinuxMonitor>();
    }
}
