//! macOS-specific process monitoring using libproc.
//!
//! This module uses the `libproc` crate to access macOS-specific process
//! information through the `libproc.h` system library. It provides access
//! to BSD process info, process states, and process relationships.

use super::{PlatformMonitor, ProcessInfo, ProcessState};
use anyhow::{anyhow, Result};
use libproc::bsd_info::BSDInfo;
use libproc::proc_pid::pidinfo;
use libproc::processes::{pids_by_type, ProcFilter};
use tracing::debug;

/// macOS process monitor using libproc.
///
/// This monitor uses the `libproc` crate which wraps macOS's `libproc.h`
/// library for accessing process information. Some operations may require
/// appropriate entitlements or permissions.
#[derive(Debug, Default)]
pub struct MacOSMonitor;

impl MacOSMonitor {
    /// Create a new macOS process monitor.
    pub fn new() -> Self {
        Self
    }

    /// Convert a macOS BSD process status to ProcessState.
    ///
    /// BSD process status values from sys/proc.h:
    /// - SIDL (1): Process being created
    /// - SRUN (2): Currently runnable
    /// - SSLEEP (3): Sleeping on an address
    /// - SSTOP (4): Process debugging or suspension
    /// - SZOMB (5): Awaiting collection by parent
    fn state_from_bsd_status(status: u32) -> ProcessState {
        match status {
            1 => ProcessState::Sleeping,   // SIDL - being created, treat as sleeping
            2 => ProcessState::Running,    // SRUN
            3 => ProcessState::Sleeping,   // SSLEEP
            4 => ProcessState::Stopped,    // SSTOP
            5 => ProcessState::Terminated, // SZOMB
            _ => ProcessState::Unknown,
        }
    }

    /// Get the process group ID for a process using BSD info.
    fn get_process_pgid(&self, pid: u32) -> Result<i32> {
        let info: BSDInfo = pidinfo(pid as i32, 0)
            .map_err(|e| anyhow!("Failed to get BSD info for process {}: {}", pid, e))?;
        Ok(info.pbi_pgid as i32)
    }
}

impl PlatformMonitor for MacOSMonitor {
    fn get_process_state(&self, pid: u32) -> Result<ProcessState> {
        let info: BSDInfo = pidinfo(pid as i32, 0)
            .map_err(|e| anyhow!("Failed to get BSD info for process {}: {}", pid, e))?;

        let state = Self::state_from_bsd_status(info.pbi_status);
        debug!(
            pid,
            ?state,
            bsd_status = info.pbi_status,
            "Got macOS process state"
        );
        Ok(state)
    }

    fn get_process_info(&self, pid: u32) -> Result<ProcessInfo> {
        let info: BSDInfo = pidinfo(pid as i32, 0)
            .map_err(|e| anyhow!("Failed to get BSD info for process {}: {}", pid, e))?;

        let state = Self::state_from_bsd_status(info.pbi_status);

        // Convert the command name from the pbi_comm field
        // pbi_comm is a fixed-size array, find the null terminator
        let command = {
            let comm_bytes: Vec<u8> = info
                .pbi_comm
                .iter()
                .take_while(|&&c| c != 0)
                .map(|&c| c as u8)
                .collect();
            String::from_utf8_lossy(&comm_bytes).to_string()
        };

        Ok(ProcessInfo {
            pid,
            state,
            ppid: Some(info.pbi_ppid),
            command: if command.is_empty() {
                None
            } else {
                Some(command)
            },
        })
    }

    fn get_child_processes(&self, pid: u32) -> Result<Vec<u32>> {
        // Get all process IDs
        let all_pids = pids_by_type(ProcFilter::All)
            .map_err(|e| anyhow!("Failed to enumerate processes: {}", e))?;

        let mut children = Vec::new();

        for child_pid in all_pids {
            // Skip invalid PIDs (0 is returned for unused slots)
            if child_pid == 0 {
                continue;
            }

            // Try to get BSD info for each process
            if let Ok(info) = pidinfo::<BSDInfo>(child_pid as i32, 0) {
                if info.pbi_ppid == pid {
                    children.push(child_pid);
                }
            }
        }

        debug!(pid, child_count = children.len(), "Got child processes");
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

        // Check if it's in the foreground process group
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
    fn test_macos_monitor_new() {
        let monitor = MacOSMonitor::new();
        let _ = format!("{:?}", monitor);
    }

    #[test]
    fn test_macos_monitor_default() {
        let monitor = MacOSMonitor;
        let _ = format!("{:?}", monitor);
    }

    #[test]
    fn test_state_from_bsd_status_running() {
        assert_eq!(
            MacOSMonitor::state_from_bsd_status(2),
            ProcessState::Running
        );
    }

    #[test]
    fn test_state_from_bsd_status_sleeping() {
        assert_eq!(
            MacOSMonitor::state_from_bsd_status(1),
            ProcessState::Sleeping
        );
        assert_eq!(
            MacOSMonitor::state_from_bsd_status(3),
            ProcessState::Sleeping
        );
    }

    #[test]
    fn test_state_from_bsd_status_stopped() {
        assert_eq!(
            MacOSMonitor::state_from_bsd_status(4),
            ProcessState::Stopped
        );
    }

    #[test]
    fn test_state_from_bsd_status_terminated() {
        assert_eq!(
            MacOSMonitor::state_from_bsd_status(5),
            ProcessState::Terminated
        );
    }

    #[test]
    fn test_state_from_bsd_status_unknown() {
        assert_eq!(
            MacOSMonitor::state_from_bsd_status(0),
            ProcessState::Unknown
        );
        assert_eq!(
            MacOSMonitor::state_from_bsd_status(99),
            ProcessState::Unknown
        );
    }

    #[test]
    fn test_get_current_process_state() {
        let monitor = MacOSMonitor::new();
        let pid = std::process::id();
        let result = monitor.get_process_state(pid);
        assert!(result.is_ok(), "Failed to get state: {:?}", result.err());
        let state = result.unwrap();
        // Current process should be Running or Sleeping
        assert!(
            matches!(state, ProcessState::Running | ProcessState::Sleeping),
            "Unexpected state: {:?}",
            state
        );
    }

    #[test]
    fn test_get_process_state_nonexistent() {
        let monitor = MacOSMonitor::new();
        // Use a very high PID that's unlikely to exist
        let result = monitor.get_process_state(999_999_999);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_current_process_info() {
        let monitor = MacOSMonitor::new();
        let pid = std::process::id();
        let result = monitor.get_process_info(pid);
        assert!(result.is_ok(), "Failed to get info: {:?}", result.err());

        let info = result.unwrap();
        assert_eq!(info.pid, pid);
        assert!(info.ppid.is_some());
        // Command might be empty in some cases, so don't assert on it
    }

    #[test]
    fn test_get_process_info_nonexistent() {
        let monitor = MacOSMonitor::new();
        let result = monitor.get_process_info(999_999_999);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_child_processes_current() {
        let monitor = MacOSMonitor::new();
        let pid = std::process::id();
        let result = monitor.get_child_processes(pid);
        assert!(result.is_ok(), "Failed to get children: {:?}", result.err());
        // May or may not have children, but should not error
    }

    #[test]
    fn test_get_child_processes_launchd() {
        let monitor = MacOSMonitor::new();
        // PID 1 (launchd) typically exists and has children on macOS
        let result = monitor.get_child_processes(1);
        // This might fail without proper permissions, so just check it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_get_foreground_pgid_invalid_fd() {
        let monitor = MacOSMonitor::new();
        let result = monitor.get_foreground_pgid(-1);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_likely_waiting_for_input_invalid_fd() {
        let monitor = MacOSMonitor::new();
        let pid = std::process::id();
        // With an invalid fd, should return false (not error)
        let result = monitor.is_likely_waiting_for_input(pid, -1);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_is_likely_waiting_for_input_nonexistent_process() {
        let monitor = MacOSMonitor::new();
        let result = monitor.is_likely_waiting_for_input(999_999_999, -1);
        assert!(result.is_err());
    }

    #[test]
    fn test_monitor_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MacOSMonitor>();
    }

    #[test]
    fn test_get_process_pgid_current() {
        let monitor = MacOSMonitor::new();
        let pid = std::process::id();
        let result = monitor.get_process_pgid(pid);
        assert!(result.is_ok());
        // The pgid should be a valid positive number
        assert!(result.unwrap() > 0);
    }
}
