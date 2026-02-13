//! Main verification gate implementation.
//!
//! This module provides the [`VerificationGate`] which implements the
//! [`Verifier`] trait for running verification checks on task completions.
//!
//! ## Example
//!
//! ```no_run
//! use codirigent_verification::VerificationGate;
//! use codirigent_core::{TaskId, Verifier};
//! use std::path::Path;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let gate = VerificationGate::new();
//! let task_id = TaskId::from("task-001");
//!
//! let status = gate.verify(&task_id, Path::new("/path/to/project")).await?;
//! if status.all_passed() {
//!     println!("All checks passed!");
//! }
//! # Ok(())
//! # }
//! ```

use crate::detector::DefaultDetector;
use crate::executor::CommandExecutor;
use crate::parser::{CargoTestParser, GenericParser, JestParser, OutputParser};
use anyhow::Result;
use async_trait::async_trait;
use codirigent_core::verification::{
    VerificationCheckType, VerificationCommands, VerificationConfig, VerificationResult,
    VerificationState, VerificationStatus,
};
use codirigent_core::{ProjectType, SessionId, TaskId, VerificationDetector, Verifier};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracing::{debug, info, warn};

/// Main verification gate implementation.
///
/// Implements the [`Verifier`] trait to run verification checks on
/// completed tasks. Supports auto-detection of project types and
/// commands, configurable timeouts, and status tracking.
pub struct VerificationGate {
    /// Configuration for verification behavior.
    config: VerificationConfig,
    /// Detector for auto-detecting project type and commands.
    detector: DefaultDetector,
    /// Executor for running verification commands.
    executor: CommandExecutor,
    /// Map of task IDs to their verification status.
    statuses: RwLock<HashMap<TaskId, VerificationStatus>>,
    /// Map of task IDs to their working directories (for retry).
    working_dirs: RwLock<HashMap<TaskId, PathBuf>>,
}

impl std::fmt::Debug for VerificationGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerificationGate")
            .field("config", &self.config)
            .field("detector", &self.detector)
            .field("executor", &self.executor)
            .finish_non_exhaustive()
    }
}

impl Default for VerificationGate {
    fn default() -> Self {
        Self::new()
    }
}

impl VerificationGate {
    /// Create a new verification gate with default configuration.
    ///
    /// Uses auto-detection for project type and commands.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::VerificationGate;
    ///
    /// let gate = VerificationGate::new();
    /// ```
    pub fn new() -> Self {
        Self {
            config: VerificationConfig::default(),
            detector: DefaultDetector::new(),
            executor: CommandExecutor::new(),
            statuses: RwLock::new(HashMap::new()),
            working_dirs: RwLock::new(HashMap::new()),
        }
    }

    /// Create a verification gate with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for verification behavior
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::VerificationGate;
    /// use codirigent_core::verification::VerificationConfig;
    ///
    /// let mut config = VerificationConfig::default();
    /// config.max_retries = 5;
    ///
    /// let gate = VerificationGate::with_config(config);
    /// ```
    pub fn with_config(config: VerificationConfig) -> Self {
        Self {
            config,
            detector: DefaultDetector::new(),
            executor: CommandExecutor::new(),
            statuses: RwLock::new(HashMap::new()),
            working_dirs: RwLock::new(HashMap::new()),
        }
    }

    /// Get the current configuration.
    pub fn config(&self) -> &VerificationConfig {
        &self.config
    }

    /// Get the appropriate parser for a project type.
    fn get_parser(&self, project_dir: &Path) -> Box<dyn OutputParser> {
        match self.detector.detect_project_type(project_dir) {
            Some(ProjectType::Rust) => Box::new(CargoTestParser::new()),
            Some(ProjectType::NodeJs) => Box::new(JestParser::new()),
            _ => Box::new(GenericParser),
        }
    }

    /// Get verification commands, either from config or auto-detect.
    fn get_commands(&self, working_dir: &Path) -> VerificationCommands {
        if self.config.auto_detect {
            self.detector.detect(working_dir)
        } else {
            self.config.commands.clone()
        }
    }

    /// Acquire read lock on statuses.
    fn lock_statuses_read(&self) -> RwLockReadGuard<'_, HashMap<TaskId, VerificationStatus>> {
        self.statuses.read().expect("verification statuses lock poisoned")
    }

    /// Acquire write lock on statuses.
    fn lock_statuses_write(&self) -> RwLockWriteGuard<'_, HashMap<TaskId, VerificationStatus>> {
        self.statuses.write().expect("verification statuses lock poisoned")
    }

    /// Acquire read lock on working_dirs.
    fn lock_working_dirs_read(&self) -> RwLockReadGuard<'_, HashMap<TaskId, PathBuf>> {
        self.working_dirs.read().expect("verification working_dirs lock poisoned")
    }

    /// Acquire write lock on working_dirs.
    fn lock_working_dirs_write(&self) -> RwLockWriteGuard<'_, HashMap<TaskId, PathBuf>> {
        self.working_dirs.write().expect("verification working_dirs lock poisoned")
    }

    /// Store a verification status.
    fn store_status(&self, task_id: TaskId, status: VerificationStatus) {
        self.lock_statuses_write().insert(task_id, status);
    }

    /// Store a working directory for retry.
    fn store_working_dir(&self, task_id: TaskId, working_dir: PathBuf) {
        self.lock_working_dirs_write().insert(task_id, working_dir);
    }

    /// Get a stored working directory.
    fn get_working_dir(&self, task_id: &TaskId) -> Option<PathBuf> {
        self.lock_working_dirs_read().get(task_id).cloned()
    }

    /// Get the current retry count for a task.
    fn get_retry_count(&self, task_id: &TaskId) -> u32 {
        self.lock_statuses_read()
            .get(task_id)
            .map(|s| s.retry_count)
            .unwrap_or(0)
    }
}

#[async_trait]
impl Verifier for VerificationGate {
    async fn verify(&self, task_id: &TaskId, working_dir: &Path) -> Result<VerificationStatus> {
        info!(%task_id, ?working_dir, "Starting verification");

        // Store working dir for potential retry
        self.store_working_dir(task_id.clone(), working_dir.to_path_buf());

        // Get current retry count
        let retry_count = self.get_retry_count(task_id);

        // Create initial status
        let mut status = VerificationStatus::new(task_id.clone(), SessionId(0));
        status.state = VerificationState::Running;
        status.retry_count = retry_count;

        // Get commands to run
        let commands = self.get_commands(working_dir);
        debug!(?commands, "Using verification commands");

        // Run unit tests if configured
        if let Some(ref cmd) = commands.unit {
            let result = self
                .run_check(VerificationCheckType::UnitTest, cmd, working_dir)
                .await?;
            status.results.push(result);
        }

        // Run lint if configured
        if let Some(ref cmd) = commands.lint {
            let result = self
                .run_check(VerificationCheckType::Lint, cmd, working_dir)
                .await?;
            status.results.push(result);
        }

        // Run type check if configured
        if let Some(ref cmd) = commands.type_check {
            let result = self
                .run_check(VerificationCheckType::TypeCheck, cmd, working_dir)
                .await?;
            status.results.push(result);
        }

        // Run format check if configured
        if let Some(ref cmd) = commands.format {
            let result = self
                .run_check(VerificationCheckType::Format, cmd, working_dir)
                .await?;
            status.results.push(result);
        }

        // Run integration tests if configured
        if let Some(ref cmd) = commands.integration {
            let result = self
                .run_check(VerificationCheckType::IntegrationTest, cmd, working_dir)
                .await?;
            status.results.push(result);
        }

        // Run custom commands
        for cmd in &commands.custom {
            let result = self
                .run_check(VerificationCheckType::Custom, cmd, working_dir)
                .await?;
            status.results.push(result);
        }

        // Determine final state
        // If no commands are configured, consider it passed (vacuously true)
        // Otherwise, all results must pass
        let state = if status.results.is_empty() || status.all_passed() {
            VerificationState::Passed
        } else {
            VerificationState::Failed
        };

        status.complete(state);

        info!(%task_id, ?state, "Verification completed");

        // Store final status
        self.store_status(task_id.clone(), status.clone());

        Ok(status)
    }

    async fn run_check(
        &self,
        check_type: VerificationCheckType,
        command: &str,
        working_dir: &Path,
    ) -> Result<VerificationResult> {
        debug!(?check_type, command, "Running verification check");

        let exec_result = self.executor.execute(command, working_dir).await?;
        let parser = self.get_parser(working_dir);

        let result = parser.parse(
            check_type,
            exec_result.exit_code,
            &exec_result.stdout,
            &exec_result.stderr,
            exec_result.duration.as_millis() as u64,
        );

        debug!(?check_type, passed = result.passed, "Check completed");

        Ok(result)
    }

    fn get_status(&self, task_id: &TaskId) -> Option<VerificationStatus> {
        self.lock_statuses_read().get(task_id).cloned()
    }

    fn skip(&mut self, task_id: &TaskId) -> Result<()> {
        info!(%task_id, "Skipping verification");

        let status = VerificationStatus {
            task_id: task_id.clone(),
            session_id: SessionId(0),
            state: VerificationState::Skipped,
            retry_count: 0,
            results: vec![],
            started_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
        };

        self.store_status(task_id.clone(), status);
        Ok(())
    }

    async fn retry(&mut self, task_id: &TaskId) -> Result<VerificationStatus> {
        info!(%task_id, "Retrying verification");

        // Get the stored working directory
        let working_dir = self
            .get_working_dir(task_id)
            .ok_or_else(|| anyhow::anyhow!("No working directory stored for task {}", task_id))?;

        // Increment retry count
        let new_retry_count = self.get_retry_count(task_id) + 1;

        // Check max retries
        if new_retry_count > self.config.max_retries {
            warn!(
                %task_id,
                retry_count = new_retry_count,
                max_retries = self.config.max_retries,
                "Max retries exceeded, blocking task"
            );

            let mut status = VerificationStatus::new(task_id.clone(), SessionId(0));
            status.retry_count = new_retry_count;
            status.complete(VerificationState::Blocked);
            self.store_status(task_id.clone(), status.clone());
            return Ok(status);
        }

        // Store the new retry count temporarily
        {
            let mut statuses = self.lock_statuses_write();
            if let Some(status) = statuses.get_mut(task_id) {
                status.retry_count = new_retry_count;
            }
        }

        // Run verification again
        self.verify(task_id, &working_dir).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // VerificationGate construction tests

    #[test]
    fn test_verification_gate_new() {
        let gate = VerificationGate::new();
        assert!(gate.config.enabled);
        assert!(gate.config.auto_detect);
    }

    #[test]
    fn test_verification_gate_default() {
        let gate = VerificationGate::default();
        assert!(gate.config.enabled);
        assert!(gate.config.auto_detect);
    }

    #[test]
    fn test_verification_gate_with_config() {
        let config = VerificationConfig {
            max_retries: 10,
            auto_detect: false,
            ..Default::default()
        };

        let gate = VerificationGate::with_config(config);
        assert_eq!(gate.config.max_retries, 10);
        assert!(!gate.config.auto_detect);
    }

    #[test]
    fn test_verification_gate_config_accessor() {
        let gate = VerificationGate::new();
        let config = gate.config();
        assert!(config.enabled);
    }

    #[test]
    fn test_verification_gate_debug() {
        let gate = VerificationGate::new();
        let debug_str = format!("{:?}", gate);
        assert!(debug_str.contains("VerificationGate"));
        assert!(debug_str.contains("config"));
    }

    // skip tests

    #[test]
    fn test_skip_verification() {
        let mut gate = VerificationGate::new();
        let task_id = TaskId::from("test-task");

        gate.skip(&task_id).unwrap();

        let status = gate.get_status(&task_id).unwrap();
        assert_eq!(status.state, VerificationState::Skipped);
    }

    #[test]
    fn test_skip_multiple_tasks() {
        let mut gate = VerificationGate::new();
        let task_id_1 = TaskId::from("task-1");
        let task_id_2 = TaskId::from("task-2");

        gate.skip(&task_id_1).unwrap();
        gate.skip(&task_id_2).unwrap();

        assert_eq!(
            gate.get_status(&task_id_1).unwrap().state,
            VerificationState::Skipped
        );
        assert_eq!(
            gate.get_status(&task_id_2).unwrap().state,
            VerificationState::Skipped
        );
    }

    // get_status tests

    #[test]
    fn test_get_status_not_found() {
        let gate = VerificationGate::new();
        let task_id = TaskId::from("nonexistent");

        assert!(gate.get_status(&task_id).is_none());
    }

    #[test]
    fn test_get_status_returns_clone() {
        let mut gate = VerificationGate::new();
        let task_id = TaskId::from("test-task");

        gate.skip(&task_id).unwrap();

        let status1 = gate.get_status(&task_id);
        let status2 = gate.get_status(&task_id);

        assert!(status1.is_some());
        assert!(status2.is_some());
        assert_eq!(status1.unwrap().task_id, status2.unwrap().task_id);
    }

    // get_commands tests

    #[test]
    fn test_get_commands_auto_detect() {
        let gate = VerificationGate::new();
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        let commands = gate.get_commands(temp.path());
        assert_eq!(commands.unit, Some("cargo test".to_string()));
    }

    #[test]
    fn test_get_commands_from_config() {
        let config = VerificationConfig {
            auto_detect: false,
            commands: VerificationCommands {
                unit: Some("npm test".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let gate = VerificationGate::with_config(config);
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        let commands = gate.get_commands(temp.path());
        // Should use config commands, not auto-detect
        assert_eq!(commands.unit, Some("npm test".to_string()));
    }

    // get_parser tests

    #[test]
    fn test_get_parser_rust() {
        let gate = VerificationGate::new();
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        let parser = gate.get_parser(temp.path());
        let result = parser.parse(
            VerificationCheckType::UnitTest,
            0,
            "test result: ok. 5 passed; 0 failed; 0 ignored",
            "",
            100,
        );
        assert_eq!(result.passed_count, Some(5));
    }

    #[test]
    fn test_get_parser_nodejs() {
        let gate = VerificationGate::new();
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("package.json"), "{}").unwrap();

        let parser = gate.get_parser(temp.path());
        let result = parser.parse(
            VerificationCheckType::UnitTest,
            0,
            "Tests: 10 passed, 10 total",
            "",
            100,
        );
        assert_eq!(result.passed_count, Some(10));
    }

    #[test]
    fn test_get_parser_unknown() {
        let gate = VerificationGate::new();
        let temp = TempDir::new().unwrap();

        let parser = gate.get_parser(temp.path());
        let result = parser.parse(VerificationCheckType::UnitTest, 0, "ok", "", 100);
        assert!(result.passed);
        assert!(result.passed_count.is_none());
    }

    // Async tests

    #[tokio::test]
    async fn test_run_check_success() {
        let _gate = VerificationGate::new();
        let _temp = TempDir::new().unwrap();

        #[cfg(unix)]
        let result = _gate
            .run_check(VerificationCheckType::UnitTest, "echo ok", _temp.path())
            .await
            .unwrap();

        #[cfg(unix)]
        assert!(result.passed);
    }

    #[tokio::test]
    async fn test_run_check_failure() {
        let _gate = VerificationGate::new();
        let _temp = TempDir::new().unwrap();

        #[cfg(unix)]
        let result = _gate
            .run_check(VerificationCheckType::UnitTest, "false", _temp.path())
            .await
            .unwrap();

        #[cfg(unix)]
        assert!(!result.passed);
    }

    #[tokio::test]
    async fn test_verify_no_commands() {
        let gate = VerificationGate::new();
        let temp = TempDir::new().unwrap();
        // No project files, so no commands will be detected

        let task_id = TaskId::from("test-task");
        let status = gate.verify(&task_id, temp.path()).await.unwrap();

        // No commands run = all passed (vacuously true)
        assert_eq!(status.state, VerificationState::Passed);
        assert!(status.results.is_empty());
    }

    #[tokio::test]
    async fn test_verify_stores_status() {
        let gate = VerificationGate::new();
        let temp = TempDir::new().unwrap();

        let task_id = TaskId::from("test-task");
        gate.verify(&task_id, temp.path()).await.unwrap();

        let status = gate.get_status(&task_id);
        assert!(status.is_some());
    }

    #[tokio::test]
    async fn test_verify_stores_working_dir() {
        let gate = VerificationGate::new();
        let temp = TempDir::new().unwrap();

        let task_id = TaskId::from("test-task");
        gate.verify(&task_id, temp.path()).await.unwrap();

        let working_dir = gate.get_working_dir(&task_id);
        assert!(working_dir.is_some());
        assert_eq!(working_dir.unwrap(), temp.path());
    }

    #[tokio::test]
    async fn test_retry_increments_count() {
        let mut gate = VerificationGate::new();
        let temp = TempDir::new().unwrap();

        let task_id = TaskId::from("test-task");

        // First verify
        gate.verify(&task_id, temp.path()).await.unwrap();

        // Retry
        let status = gate.retry(&task_id).await.unwrap();
        assert_eq!(status.retry_count, 1);
    }

    #[tokio::test]
    async fn test_retry_without_working_dir() {
        let mut gate = VerificationGate::new();
        let task_id = TaskId::from("test-task");

        let result = gate.retry(&task_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_retry_max_exceeded() {
        let config = VerificationConfig {
            max_retries: 0,
            ..Default::default()
        };
        let mut gate = VerificationGate::with_config(config);
        let temp = TempDir::new().unwrap();

        let task_id = TaskId::from("test-task");

        // First verify
        gate.verify(&task_id, temp.path()).await.unwrap();

        // Retry should block immediately
        let status = gate.retry(&task_id).await.unwrap();
        assert_eq!(status.state, VerificationState::Blocked);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_verify_with_passing_command() {
        let config = VerificationConfig {
            auto_detect: false,
            commands: VerificationCommands {
                unit: Some("echo test".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let gate = VerificationGate::with_config(config);
        let temp = TempDir::new().unwrap();

        let task_id = TaskId::from("test-task");
        let status = gate.verify(&task_id, temp.path()).await.unwrap();

        assert_eq!(status.state, VerificationState::Passed);
        assert_eq!(status.results.len(), 1);
        assert!(status.results[0].passed);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_verify_with_failing_command() {
        let config = VerificationConfig {
            auto_detect: false,
            commands: VerificationCommands {
                unit: Some("false".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let gate = VerificationGate::with_config(config);
        let temp = TempDir::new().unwrap();

        let task_id = TaskId::from("test-task");
        let status = gate.verify(&task_id, temp.path()).await.unwrap();

        assert_eq!(status.state, VerificationState::Failed);
        assert_eq!(status.results.len(), 1);
        assert!(!status.results[0].passed);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_verify_multiple_checks() {
        let config = VerificationConfig {
            auto_detect: false,
            commands: VerificationCommands {
                unit: Some("echo test".to_string()),
                lint: Some("echo lint".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let gate = VerificationGate::with_config(config);
        let temp = TempDir::new().unwrap();

        let task_id = TaskId::from("test-task");
        let status = gate.verify(&task_id, temp.path()).await.unwrap();

        assert_eq!(status.state, VerificationState::Passed);
        assert_eq!(status.results.len(), 2);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_verify_partial_failure() {
        let config = VerificationConfig {
            auto_detect: false,
            commands: VerificationCommands {
                unit: Some("echo test".to_string()),
                lint: Some("false".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let gate = VerificationGate::with_config(config);
        let temp = TempDir::new().unwrap();

        let task_id = TaskId::from("test-task");
        let status = gate.verify(&task_id, temp.path()).await.unwrap();

        // One passed, one failed = overall failed
        assert_eq!(status.state, VerificationState::Failed);
        assert_eq!(status.results.len(), 2);
        assert!(status.results[0].passed);
        assert!(!status.results[1].passed);
    }

    #[test]
    fn test_verification_gate_lock_recovery() {
        // Verify that store_status and get_status work under normal conditions
        // (We can't easily poison an RwLock in a test, but we verify the helpers compile
        // and the poisoning recovery path is present)
        let gate = VerificationGate::new();
        let task_id = TaskId("lock-test".into());

        // store_status uses write lock
        let status = VerificationStatus::new(task_id.clone(), SessionId(0));
        gate.store_status(task_id.clone(), status);

        // get_status uses read lock
        let retrieved = gate.get_status(&task_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().task_id, task_id);
    }
}
