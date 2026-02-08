//! Command execution for verification checks.
//!
//! This module provides asynchronous command execution with timeout support
//! for running verification commands like tests, linters, and type checkers.
//!
//! ## Example
//!
//! ```no_run
//! use codirigent_verification::CommandExecutor;
//! use std::path::Path;
//! use std::time::Duration;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let executor = CommandExecutor::with_timeout(Duration::from_secs(60));
//! let result = executor.execute("cargo test", Path::new("/path/to/project")).await?;
//!
//! if result.success() {
//!     println!("Tests passed in {:?}", result.duration);
//! } else {
//!     eprintln!("Tests failed with exit code {}", result.exit_code);
//! }
//! # Ok(())
//! # }
//! ```

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, warn};

/// Command execution result.
///
/// Contains the outcome of executing a verification command, including
/// the exit code, stdout/stderr output, and execution duration.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Exit code of the command.
    ///
    /// Returns -1 if the exit code could not be determined (e.g., killed by signal).
    pub exit_code: i32,

    /// Stdout output as a string.
    pub stdout: String,

    /// Stderr output as a string.
    pub stderr: String,

    /// Duration of execution.
    pub duration: Duration,
}

impl ExecutionResult {
    /// Check if the command succeeded (exit code 0).
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::ExecutionResult;
    /// use std::time::Duration;
    ///
    /// let result = ExecutionResult {
    ///     exit_code: 0,
    ///     stdout: "OK".to_string(),
    ///     stderr: String::new(),
    ///     duration: Duration::from_secs(1),
    /// };
    /// assert!(result.success());
    /// ```
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }

    /// Get combined output (stdout + stderr).
    ///
    /// Useful for getting all output from a command regardless of
    /// which stream it was written to.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::ExecutionResult;
    /// use std::time::Duration;
    ///
    /// let result = ExecutionResult {
    ///     exit_code: 1,
    ///     stdout: "Tests ran".to_string(),
    ///     stderr: "1 failure".to_string(),
    ///     duration: Duration::from_secs(1),
    /// };
    /// let combined = result.combined_output();
    /// assert!(combined.contains("Tests ran"));
    /// assert!(combined.contains("1 failure"));
    /// ```
    pub fn combined_output(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }

    /// Get the duration in milliseconds.
    ///
    /// Convenience method for getting duration as u64 milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.duration.as_millis() as u64
    }
}

/// Execute verification commands.
///
/// Runs shell commands with configurable timeout and captures their output.
/// Commands are executed in a specified working directory.
#[derive(Debug, Clone)]
pub struct CommandExecutor {
    timeout: Duration,
}

impl Default for CommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandExecutor {
    /// Create executor with default timeout (5 minutes).
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::CommandExecutor;
    ///
    /// let executor = CommandExecutor::new();
    /// ```
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(300),
        }
    }

    /// Create executor with custom timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum duration to wait for command completion
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::CommandExecutor;
    /// use std::time::Duration;
    ///
    /// let executor = CommandExecutor::with_timeout(Duration::from_secs(60));
    /// ```
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Get the configured timeout duration.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Execute a command in the given working directory.
    ///
    /// The command string is split by whitespace into program and arguments.
    /// This is suitable for simple commands but may not work correctly for
    /// commands with quoted arguments containing spaces.
    ///
    /// # Arguments
    ///
    /// * `command` - The shell command to execute
    /// * `working_dir` - Directory to run the command in
    ///
    /// # Returns
    ///
    /// The execution result containing exit code, output, and duration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The command is empty
    /// - The command fails to start
    /// - The command times out
    pub async fn execute(&self, command: &str, working_dir: &Path) -> Result<ExecutionResult> {
        self.execute_with_env(command, working_dir, &[]).await
    }

    /// Execute with additional environment variables.
    ///
    /// # Arguments
    ///
    /// * `command` - The shell command to execute
    /// * `working_dir` - Directory to run the command in
    /// * `env` - Additional environment variables to set
    ///
    /// # Returns
    ///
    /// The execution result containing exit code, output, and duration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The command is empty
    /// - The command fails to start
    /// - The command times out
    pub async fn execute_with_env(
        &self,
        command: &str,
        working_dir: &Path,
        env: &[(&str, &str)],
    ) -> Result<ExecutionResult> {
        debug!(command, ?working_dir, "Executing verification command");

        let start = Instant::now();

        // Parse command into program and arguments
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            anyhow::bail!("Empty command");
        }

        let program = parts[0];
        let args = &parts[1..];

        let mut cmd = Command::new(program);
        cmd.args(args)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set user-provided environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        // Set CI=true to get better output from test runners
        cmd.env("CI", "true");

        let output = timeout(self.timeout, cmd.output())
            .await
            .context("Command timed out")?
            .context("Failed to execute command")?;

        let duration = start.elapsed();
        let exit_code = output.status.code().unwrap_or(-1);

        let result = ExecutionResult {
            exit_code,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            duration,
        };

        debug!(
            exit_code,
            duration_ms = duration.as_millis(),
            "Command completed"
        );

        if !result.success() {
            warn!(exit_code, "Verification command failed");
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ExecutionResult tests

    #[test]
    fn test_execution_result_success() {
        let result = ExecutionResult {
            exit_code: 0,
            stdout: "OK".to_string(),
            stderr: String::new(),
            duration: Duration::from_secs(1),
        };
        assert!(result.success());
    }

    #[test]
    fn test_execution_result_failure() {
        let result = ExecutionResult {
            exit_code: 1,
            stdout: String::new(),
            stderr: "Error".to_string(),
            duration: Duration::from_secs(1),
        };
        assert!(!result.success());
    }

    #[test]
    fn test_execution_result_combined_output() {
        let result = ExecutionResult {
            exit_code: 0,
            stdout: "stdout content".to_string(),
            stderr: "stderr content".to_string(),
            duration: Duration::from_secs(1),
        };
        let combined = result.combined_output();
        assert!(combined.contains("stdout content"));
        assert!(combined.contains("stderr content"));
    }

    #[test]
    fn test_execution_result_duration_ms() {
        let result = ExecutionResult {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            duration: Duration::from_millis(1500),
        };
        assert_eq!(result.duration_ms(), 1500);
    }

    #[test]
    fn test_execution_result_clone() {
        let result = ExecutionResult {
            exit_code: 0,
            stdout: "test".to_string(),
            stderr: String::new(),
            duration: Duration::from_secs(1),
        };
        let cloned = result.clone();
        assert_eq!(result.exit_code, cloned.exit_code);
        assert_eq!(result.stdout, cloned.stdout);
    }

    // CommandExecutor tests

    #[test]
    fn test_executor_new() {
        let executor = CommandExecutor::new();
        assert_eq!(executor.timeout(), Duration::from_secs(300));
    }

    #[test]
    fn test_executor_default() {
        let executor = CommandExecutor::default();
        assert_eq!(executor.timeout(), Duration::from_secs(300));
    }

    #[test]
    fn test_executor_with_timeout() {
        let executor = CommandExecutor::with_timeout(Duration::from_secs(60));
        assert_eq!(executor.timeout(), Duration::from_secs(60));
    }

    #[test]
    fn test_executor_clone() {
        let executor = CommandExecutor::with_timeout(Duration::from_secs(120));
        let cloned = executor.clone();
        assert_eq!(executor.timeout(), cloned.timeout());
    }

    #[tokio::test]
    async fn test_execute_success() {
        let temp = TempDir::new().unwrap();
        let executor = CommandExecutor::new();

        #[cfg(unix)]
        let result = executor.execute("echo hello", temp.path()).await.unwrap();
        #[cfg(windows)]
        let result = executor
            .execute("cmd /c echo hello", temp.path())
            .await
            .unwrap();

        assert!(result.success());
        assert!(result.stdout.contains("hello"));
    }

    #[tokio::test]
    async fn test_execute_failure() {
        let temp = TempDir::new().unwrap();
        let executor = CommandExecutor::new();

        #[cfg(unix)]
        let result = executor.execute("false", temp.path()).await.unwrap();
        #[cfg(windows)]
        let result = executor
            .execute("cmd /c exit 1", temp.path())
            .await
            .unwrap();

        assert!(!result.success());
        assert_ne!(result.exit_code, 0);
    }

    #[tokio::test]
    async fn test_execute_empty_command() {
        let temp = TempDir::new().unwrap();
        let executor = CommandExecutor::new();

        let result = executor.execute("", temp.path()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_with_env() {
        let _temp = TempDir::new().unwrap();
        let _executor = CommandExecutor::new();

        #[cfg(unix)]
        {
            // Create a simple script that echoes an env var
            let script_path = _temp.path().join("test.sh");
            std::fs::write(&script_path, "#!/bin/sh\necho $TEST_VAR").unwrap();
            std::fs::set_permissions(
                &script_path,
                std::os::unix::fs::PermissionsExt::from_mode(0o755),
            )
            .unwrap();

            let result = _executor
                .execute_with_env("./test.sh", _temp.path(), &[("TEST_VAR", "hello_world")])
                .await
                .unwrap();

            assert!(result.stdout.contains("hello_world"));
        }
    }

    #[tokio::test]
    async fn test_execute_sets_ci_env() {
        let _temp = TempDir::new().unwrap();
        let _executor = CommandExecutor::new();

        #[cfg(unix)]
        {
            // Create a simple script that echoes the CI env var
            let script_path = _temp.path().join("test.sh");
            std::fs::write(&script_path, "#!/bin/sh\necho $CI").unwrap();
            std::fs::set_permissions(
                &script_path,
                std::os::unix::fs::PermissionsExt::from_mode(0o755),
            )
            .unwrap();

            let result = _executor.execute("./test.sh", _temp.path()).await.unwrap();

            assert!(result.stdout.contains("true"));
        }
    }

    #[tokio::test]
    async fn test_execute_captures_stderr() {
        let _temp = TempDir::new().unwrap();
        let _executor = CommandExecutor::new();

        #[cfg(unix)]
        {
            let result = _executor
                .execute("sh -c 'echo error >&2'", _temp.path())
                .await
                .unwrap();

            assert!(result.stderr.contains("error"));
        }
    }

    #[tokio::test]
    async fn test_execute_duration_recorded() {
        let _temp = TempDir::new().unwrap();
        let _executor = CommandExecutor::new();

        #[cfg(unix)]
        let result = _executor.execute("echo test", _temp.path()).await.unwrap();
        #[cfg(windows)]
        let result = _executor
            .execute("cmd /c echo test", _temp.path())
            .await
            .unwrap();

        // Duration should be recorded (at least > 0)
        assert!(result.duration.as_nanos() > 0);
    }

    #[tokio::test]
    async fn test_timeout() {
        let _temp = TempDir::new().unwrap();
        let _executor = CommandExecutor::with_timeout(Duration::from_millis(100));

        #[cfg(unix)]
        {
            let result = _executor.execute("sleep 10", _temp.path()).await;
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(err.to_string().contains("timed out"));
        }
    }

    #[tokio::test]
    async fn test_execute_nonexistent_command() {
        let temp = TempDir::new().unwrap();
        let executor = CommandExecutor::new();

        let result = executor
            .execute("nonexistent_command_xyz", temp.path())
            .await;
        assert!(result.is_err());
    }
}
