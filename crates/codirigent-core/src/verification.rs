//! Verification types for the Codirigent application.
//!
//! This module contains all types related to the verification pipeline,
//! including verification results, configurations, status tracking, and events.
//!
//! ## Overview
//!
//! The verification system is responsible for validating task completions
//! by running automated checks (tests, linting, type checking, etc.) and
//! managing the retry workflow when checks fail.
//!
//! ## Key Types
//!
//! - [`VerificationResult`] - Result of a single verification check
//! - [`VerificationCheckType`] - Types of checks (unit test, lint, etc.)
//! - [`VerificationFailure`] - Details of a single failure
//! - [`VerificationConfig`] - Configuration for verification commands
//! - [`VerificationStatus`] - Current status of verification for a task
//! - [`VerificationState`] - State machine for the verification pipeline
//! - [`VerificationEvent`] - Events emitted by the verification system

use crate::types::{SessionId, TaskId};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Result of a single verification check.
///
/// Contains the outcome of running a verification command, including
/// pass/fail status, test counts, failure details, and timing.
///
/// # Example
///
/// ```
/// use codirigent_core::verification::{VerificationResult, VerificationCheckType};
///
/// let result = VerificationResult {
///     check_type: VerificationCheckType::UnitTest,
///     passed: true,
///     passed_count: Some(23),
///     total_count: Some(23),
///     failures: vec![],
///     duration_ms: 1500,
///     raw_output: None,
/// };
/// assert!(result.passed);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Type of verification that was performed.
    pub check_type: VerificationCheckType,
    /// Whether the check passed.
    pub passed: bool,
    /// Number of tests that passed (if applicable).
    pub passed_count: Option<u32>,
    /// Total number of tests (if applicable).
    pub total_count: Option<u32>,
    /// Detailed failure messages.
    pub failures: Vec<VerificationFailure>,
    /// Time taken to run the check in milliseconds.
    pub duration_ms: u64,
    /// Raw output from the verification command.
    pub raw_output: Option<String>,
}

impl VerificationResult {
    /// Create a new passed verification result.
    ///
    /// # Arguments
    ///
    /// * `check_type` - The type of verification check
    /// * `duration_ms` - Time taken in milliseconds
    pub fn passed(check_type: VerificationCheckType, duration_ms: u64) -> Self {
        Self {
            check_type,
            passed: true,
            passed_count: None,
            total_count: None,
            failures: vec![],
            duration_ms,
            raw_output: None,
        }
    }

    /// Create a new failed verification result.
    ///
    /// # Arguments
    ///
    /// * `check_type` - The type of verification check
    /// * `failures` - List of verification failures
    /// * `duration_ms` - Time taken in milliseconds
    pub fn failed(
        check_type: VerificationCheckType,
        failures: Vec<VerificationFailure>,
        duration_ms: u64,
    ) -> Self {
        Self {
            check_type,
            passed: false,
            passed_count: None,
            total_count: None,
            failures,
            duration_ms,
            raw_output: None,
        }
    }

    /// Set the test counts for this result.
    ///
    /// # Arguments
    ///
    /// * `passed` - Number of tests that passed
    /// * `total` - Total number of tests
    pub fn with_counts(mut self, passed: u32, total: u32) -> Self {
        self.passed_count = Some(passed);
        self.total_count = Some(total);
        self
    }

    /// Set the raw output for this result.
    ///
    /// # Arguments
    ///
    /// * `output` - Raw command output
    pub fn with_raw_output(mut self, output: String) -> Self {
        self.raw_output = Some(output);
        self
    }
}

/// Type of verification check.
///
/// Represents the different categories of verification that can be performed
/// on a codebase after task completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationCheckType {
    /// Unit tests (npm test, cargo test, pytest, etc.)
    UnitTest,
    /// Integration tests.
    IntegrationTest,
    /// Type checking (tsc, mypy, etc.)
    TypeCheck,
    /// Linting (eslint, clippy, etc.)
    Lint,
    /// Code formatting check.
    Format,
    /// Custom verification script.
    Custom,
}

impl std::fmt::Display for VerificationCheckType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationCheckType::UnitTest => write!(f, "Unit Test"),
            VerificationCheckType::IntegrationTest => write!(f, "Integration Test"),
            VerificationCheckType::TypeCheck => write!(f, "Type Check"),
            VerificationCheckType::Lint => write!(f, "Lint"),
            VerificationCheckType::Format => write!(f, "Format"),
            VerificationCheckType::Custom => write!(f, "Custom"),
        }
    }
}

/// Details of a single verification failure.
///
/// Provides structured information about what went wrong during verification,
/// including the location of the failure and expected vs actual values.
///
/// # Example
///
/// ```
/// use codirigent_core::verification::VerificationFailure;
/// use std::path::PathBuf;
///
/// let failure = VerificationFailure {
///     name: "test_auth_expired_token".to_string(),
///     file: Some(PathBuf::from("src/auth.test.ts")),
///     line: Some(42),
///     expected: Some("401".to_string()),
///     actual: Some("200".to_string()),
///     message: "Expected 401, received 200".to_string(),
/// };
/// assert_eq!(failure.line, Some(42));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationFailure {
    /// Test or check name that failed.
    pub name: String,
    /// File where the failure occurred (if known).
    pub file: Option<PathBuf>,
    /// Line number (if known).
    pub line: Option<u32>,
    /// Expected value or condition.
    pub expected: Option<String>,
    /// Actual value or condition.
    pub actual: Option<String>,
    /// Full error message.
    pub message: String,
}

impl VerificationFailure {
    /// Create a new verification failure with just a name and message.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the failed test or check
    /// * `message` - Error message
    pub fn new(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            file: None,
            line: None,
            expected: None,
            actual: None,
            message: message.into(),
        }
    }

    /// Set the file location for this failure.
    pub fn with_file(mut self, file: PathBuf) -> Self {
        self.file = Some(file);
        self
    }

    /// Set the line number for this failure.
    pub fn with_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    /// Set the expected and actual values for this failure.
    pub fn with_comparison(mut self, expected: String, actual: String) -> Self {
        self.expected = Some(expected);
        self.actual = Some(actual);
        self
    }
}

/// Configuration for verification commands.
///
/// Defines how verification should be performed for a project,
/// including which commands to run and retry behavior.
///
/// # Example
///
/// ```
/// use codirigent_core::verification::{VerificationConfig, VerificationCommands};
///
/// let config = VerificationConfig::default();
/// assert!(config.enabled);
/// assert!(config.auto_detect);
/// assert_eq!(config.max_retries, 3);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    /// Whether verification is enabled.
    pub enabled: bool,
    /// Whether to auto-detect verification commands.
    pub auto_detect: bool,
    /// Maximum retry attempts before marking as blocked.
    pub max_retries: u32,
    /// Specific commands for each check type.
    pub commands: VerificationCommands,
    /// Whether human review is required after verification passes.
    pub requires_human_review: bool,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_detect: true,
            max_retries: 3,
            commands: VerificationCommands::default(),
            requires_human_review: true,
        }
    }
}

/// Verification commands for different check types.
///
/// Stores the actual shell commands to run for each type of verification.
/// Commands are optional to allow auto-detection or selective verification.
///
/// # Example
///
/// ```
/// use codirigent_core::verification::VerificationCommands;
///
/// let mut commands = VerificationCommands::default();
/// commands.unit = Some("npm test".to_string());
/// commands.lint = Some("npm run lint".to_string());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VerificationCommands {
    /// Unit test command (e.g., "npm test", "cargo test").
    pub unit: Option<String>,
    /// Integration test command.
    pub integration: Option<String>,
    /// Type check command (e.g., "tsc --noEmit").
    pub type_check: Option<String>,
    /// Lint command (e.g., "npm run lint").
    pub lint: Option<String>,
    /// Format check command.
    pub format: Option<String>,
    /// Custom verification scripts.
    pub custom: Vec<String>,
}

impl VerificationCommands {
    /// Check if any commands are configured.
    pub fn has_any(&self) -> bool {
        self.unit.is_some()
            || self.integration.is_some()
            || self.type_check.is_some()
            || self.lint.is_some()
            || self.format.is_some()
            || !self.custom.is_empty()
    }

    /// Get all configured commands as a list of (check_type, command) pairs.
    pub fn to_list(&self) -> Vec<(VerificationCheckType, String)> {
        let mut list = Vec::new();
        if let Some(ref cmd) = self.unit {
            list.push((VerificationCheckType::UnitTest, cmd.clone()));
        }
        if let Some(ref cmd) = self.integration {
            list.push((VerificationCheckType::IntegrationTest, cmd.clone()));
        }
        if let Some(ref cmd) = self.type_check {
            list.push((VerificationCheckType::TypeCheck, cmd.clone()));
        }
        if let Some(ref cmd) = self.lint {
            list.push((VerificationCheckType::Lint, cmd.clone()));
        }
        if let Some(ref cmd) = self.format {
            list.push((VerificationCheckType::Format, cmd.clone()));
        }
        for cmd in &self.custom {
            list.push((VerificationCheckType::Custom, cmd.clone()));
        }
        list
    }
}

/// Status of the verification pipeline for a task.
///
/// Tracks the complete verification state for a task including
/// all results, retry counts, and timing information.
///
/// # Example
///
/// ```
/// use codirigent_core::verification::{VerificationStatus, VerificationState};
/// use codirigent_core::{TaskId, SessionId};
///
/// let status = VerificationStatus {
///     task_id: TaskId::from("task-001"),
///     session_id: SessionId(1),
///     state: VerificationState::Running,
///     retry_count: 0,
///     results: vec![],
///     started_at: chrono::Utc::now(),
///     completed_at: None,
/// };
/// assert_eq!(status.state, VerificationState::Running);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationStatus {
    /// Task being verified.
    pub task_id: TaskId,
    /// Session that completed the task.
    pub session_id: SessionId,
    /// Current state of verification.
    pub state: VerificationState,
    /// Number of retry attempts made.
    pub retry_count: u32,
    /// Results from each verification check.
    pub results: Vec<VerificationResult>,
    /// When verification started.
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// When verification completed (if finished).
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl VerificationStatus {
    /// Create a new verification status in the pending state.
    pub fn new(task_id: TaskId, session_id: SessionId) -> Self {
        Self {
            task_id,
            session_id,
            state: VerificationState::Pending,
            retry_count: 0,
            results: vec![],
            started_at: chrono::Utc::now(),
            completed_at: None,
        }
    }

    /// Check if all verification checks passed.
    pub fn all_passed(&self) -> bool {
        !self.results.is_empty() && self.results.iter().all(|r| r.passed)
    }

    /// Get all failures from all results.
    pub fn all_failures(&self) -> Vec<&VerificationFailure> {
        self.results.iter().flat_map(|r| &r.failures).collect()
    }

    /// Get the total duration of all checks in milliseconds.
    pub fn total_duration_ms(&self) -> u64 {
        self.results.iter().map(|r| r.duration_ms).sum()
    }

    /// Mark the verification as completed.
    pub fn complete(&mut self, state: VerificationState) {
        self.state = state;
        self.completed_at = Some(chrono::Utc::now());
    }
}

/// Current state of the verification pipeline.
///
/// Represents the state machine for verification workflow,
/// from initial pending state through completion or blocking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VerificationState {
    /// Waiting to start verification.
    #[default]
    Pending,
    /// Verification checks are running.
    Running,
    /// All checks passed.
    Passed,
    /// One or more checks failed.
    Failed,
    /// Failure was sent back to session for retry.
    RetryingInSession,
    /// Max retries exceeded, blocked for human intervention.
    Blocked,
    /// Verification was skipped.
    Skipped,
}

impl std::fmt::Display for VerificationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationState::Pending => write!(f, "Pending"),
            VerificationState::Running => write!(f, "Running"),
            VerificationState::Passed => write!(f, "Passed"),
            VerificationState::Failed => write!(f, "Failed"),
            VerificationState::RetryingInSession => write!(f, "Retrying in Session"),
            VerificationState::Blocked => write!(f, "Blocked"),
            VerificationState::Skipped => write!(f, "Skipped"),
        }
    }
}

/// Event emitted by the verification system.
///
/// These events are published to the event bus to notify other
/// modules about verification progress and outcomes.
#[derive(Debug, Clone)]
pub enum VerificationEvent {
    /// Verification started for a task.
    Started {
        /// The task being verified.
        task_id: TaskId,
        /// The session that completed the task.
        session_id: SessionId,
    },
    /// A single check completed.
    CheckCompleted {
        /// The task being verified.
        task_id: TaskId,
        /// The result of the check.
        result: VerificationResult,
    },
    /// All verification passed.
    Passed {
        /// The task that passed verification.
        task_id: TaskId,
    },
    /// Verification failed, retrying.
    FailedRetrying {
        /// The task that failed verification.
        task_id: TaskId,
        /// Current retry count.
        retry_count: u32,
        /// List of failures to address.
        failures: Vec<VerificationFailure>,
    },
    /// Verification failed, max retries exceeded.
    FailedBlocked {
        /// The task that is now blocked.
        task_id: TaskId,
        /// List of failures that could not be resolved.
        failures: Vec<VerificationFailure>,
    },
}

// === Test Command Detection ===

/// A rule for detecting test commands in a project directory.
///
/// Detection rules check for the presence of a marker file (like `Cargo.toml`
/// or `package.json`) and optionally verify specific targets in that file.
///
/// # Example
///
/// ```
/// use codirigent_core::verification::DetectionRule;
///
/// let rule = DetectionRule {
///     marker_file: "Cargo.toml".to_string(),
///     command: "cargo test".to_string(),
///     make_target: None,
/// };
/// assert_eq!(rule.marker_file, "Cargo.toml");
/// ```
#[derive(Debug, Clone)]
pub struct DetectionRule {
    /// File to check for (e.g., "package.json", "Cargo.toml").
    pub marker_file: String,
    /// Command to run if marker exists (e.g., "npm test", "cargo test").
    pub command: String,
    /// Optional Makefile target to verify exists.
    pub make_target: Option<String>,
}

impl DetectionRule {
    /// Create a new detection rule.
    ///
    /// # Arguments
    ///
    /// * `marker_file` - File to check for
    /// * `command` - Command to run if marker exists
    pub fn new(marker_file: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            marker_file: marker_file.into(),
            command: command.into(),
            make_target: None,
        }
    }

    /// Create a rule with a Makefile target check.
    ///
    /// # Arguments
    ///
    /// * `marker_file` - File to check for (typically "Makefile")
    /// * `command` - Command to run (e.g., "make test")
    /// * `target` - Target to verify exists in the Makefile
    pub fn with_target(
        marker_file: impl Into<String>,
        command: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            marker_file: marker_file.into(),
            command: command.into(),
            make_target: Some(target.into()),
        }
    }
}

/// Test command detector for auto-detecting verification commands.
///
/// Analyzes a project directory to determine the appropriate test command
/// based on the presence of configuration files like `package.json`,
/// `Cargo.toml`, `pyproject.toml`, etc.
///
/// # Example
///
/// ```
/// use codirigent_core::verification::TestCommandDetector;
/// use std::path::Path;
///
/// let detector = TestCommandDetector::new();
/// // detector.detect(Path::new("/path/to/project"));
/// ```
#[derive(Debug, Clone)]
pub struct TestCommandDetector {
    /// Detection rules in priority order.
    rules: Vec<DetectionRule>,
}

impl Default for TestCommandDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCommandDetector {
    /// Create a new detector with default rules.
    ///
    /// The default rules detect common project types:
    /// - Node.js (package.json) -> `npm test`
    /// - Rust (Cargo.toml) -> `cargo test`
    /// - Python (pyproject.toml, setup.py) -> `pytest`
    /// - Go (go.mod) -> `go test ./...`
    /// - Make (Makefile with test target) -> `make test`
    pub fn new() -> Self {
        Self {
            rules: vec![
                // Node.js / npm
                DetectionRule::new("package.json", "npm test"),
                // Rust / Cargo
                DetectionRule::new("Cargo.toml", "cargo test"),
                // Python / pytest
                DetectionRule::new("pyproject.toml", "pytest"),
                DetectionRule::new("setup.py", "pytest"),
                // Go
                DetectionRule::new("go.mod", "go test ./..."),
                // Makefile with test target
                DetectionRule::with_target("Makefile", "make test", "test"),
            ],
        }
    }

    /// Add a custom detection rule.
    ///
    /// Custom rules are inserted at the beginning of the rule list,
    /// giving them priority over default rules.
    ///
    /// # Arguments
    ///
    /// * `rule` - The detection rule to add
    pub fn add_rule(&mut self, rule: DetectionRule) {
        self.rules.insert(0, rule); // Custom rules take priority
    }

    /// Get all detection rules.
    pub fn rules(&self) -> &[DetectionRule] {
        &self.rules
    }

    /// Detect test command for a directory.
    ///
    /// Iterates through detection rules in priority order and returns
    /// the first matching command.
    ///
    /// # Arguments
    ///
    /// * `working_dir` - Directory to analyze
    ///
    /// # Returns
    ///
    /// The detected test command, or `None` if no rule matched.
    pub fn detect(&self, working_dir: &std::path::Path) -> Option<String> {
        for rule in &self.rules {
            let marker_path = working_dir.join(&rule.marker_file);
            if marker_path.exists() {
                // Check Makefile target if specified
                if let Some(ref target) = rule.make_target {
                    if self.has_makefile_target(&marker_path, target) {
                        return Some(rule.command.clone());
                    }
                } else {
                    return Some(rule.command.clone());
                }
            }
        }
        None
    }

    /// Check if a Makefile has a specific target.
    ///
    /// Performs a simple check by looking for "target:" at the start of a line.
    fn has_makefile_target(&self, makefile_path: &std::path::Path, target: &str) -> bool {
        if let Ok(content) = std::fs::read_to_string(makefile_path) {
            // Simple check: look for "target:" at start of line
            let pattern = format!("{}:", target);
            content.lines().any(|line| line.starts_with(&pattern))
        } else {
            false
        }
    }
}

// === Output Parser ===

/// Test output parsers for different test frameworks.
///
/// Provides specialized parsing for common test frameworks (Jest, Cargo, pytest)
/// and a generic fallback parser.
///
/// # Example
///
/// ```
/// use codirigent_core::verification::OutputParser;
///
/// let output = "test result: ok. 15 passed; 0 failed; 2 ignored";
/// let results = OutputParser::parse_cargo(output).unwrap();
/// assert_eq!(results.passed, 15);
/// assert_eq!(results.failed, 0);
/// assert_eq!(results.skipped, 2);
/// ```
pub struct OutputParser;

impl OutputParser {
    /// Parse Jest/npm test output.
    ///
    /// Looks for patterns like:
    /// - "Tests: X failed, Y passed, Z total"
    /// - "Test Suites: X failed, Y passed, Z total"
    ///
    /// # Arguments
    ///
    /// * `output` - Combined stdout/stderr from test run
    ///
    /// # Returns
    ///
    /// Parsed test results if a recognized pattern was found.
    pub fn parse_jest(output: &str) -> Option<ParsedTestResults> {
        // Pattern: "Tests: X failed, Y passed, Z total"
        let re = regex::Regex::new(
            r"Tests?:\s*(?:(\d+)\s+failed,\s*)?(\d+)\s+passed(?:,\s*(\d+)\s+total)?",
        )
        .ok()?;

        if let Some(caps) = re.captures(output) {
            let failed = caps
                .get(1)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            let passed: u32 = caps.get(2)?.as_str().parse().ok()?;
            let total = caps
                .get(3)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(passed + failed);

            let failures = Self::extract_jest_failures(output);

            return Some(ParsedTestResults {
                total,
                passed,
                failed,
                skipped: total.saturating_sub(passed + failed),
                failures,
            });
        }

        None
    }

    /// Extract failure names from Jest output.
    fn extract_jest_failures(output: &str) -> Vec<ParsedTestFailure> {
        let mut failures = Vec::new();

        // Pattern: "x test_name" or "✗ test_name"
        let fail_re = regex::Regex::new(r"(?m)^\s*[x\u2717]\s+(.+)$").ok();

        if let Some(re) = fail_re {
            for cap in re.captures_iter(output) {
                if let Some(name) = cap.get(1) {
                    failures.push(ParsedTestFailure {
                        name: name.as_str().trim().to_string(),
                        message: String::new(),
                    });
                }
            }
        }

        failures
    }

    /// Parse Cargo test output.
    ///
    /// Looks for patterns like:
    /// - "test result: ok. X passed; Y failed; Z ignored"
    ///
    /// # Arguments
    ///
    /// * `output` - Combined stdout/stderr from cargo test
    ///
    /// # Returns
    ///
    /// Parsed test results if a recognized pattern was found.
    pub fn parse_cargo(output: &str) -> Option<ParsedTestResults> {
        // Pattern: "test result: ok. X passed; Y failed; Z ignored"
        let re = regex::Regex::new(
            r"test result: \w+\.\s*(\d+)\s+passed;\s*(\d+)\s+failed;\s*(\d+)\s+ignored",
        )
        .ok()?;

        if let Some(caps) = re.captures(output) {
            let passed: u32 = caps.get(1)?.as_str().parse().ok()?;
            let failed: u32 = caps.get(2)?.as_str().parse().ok()?;
            let skipped: u32 = caps.get(3)?.as_str().parse().ok()?;

            let failures = Self::extract_cargo_failures(output);

            return Some(ParsedTestResults {
                total: passed + failed + skipped,
                passed,
                failed,
                skipped,
                failures,
            });
        }

        None
    }

    /// Extract failure names from Cargo output.
    fn extract_cargo_failures(output: &str) -> Vec<ParsedTestFailure> {
        let mut failures = Vec::new();

        // Pattern: "---- test_name stdout ----"
        let fail_re = regex::Regex::new(r"---- (\S+) stdout ----").ok();

        if let Some(re) = fail_re {
            for cap in re.captures_iter(output) {
                if let Some(name) = cap.get(1) {
                    failures.push(ParsedTestFailure {
                        name: name.as_str().to_string(),
                        message: String::new(),
                    });
                }
            }
        }

        failures
    }

    /// Parse pytest output.
    ///
    /// Looks for patterns like:
    /// - "X passed, Y failed, Z skipped"
    /// - "=== X passed in Y.Zs ==="
    ///
    /// # Arguments
    ///
    /// * `output` - Combined stdout/stderr from pytest
    ///
    /// # Returns
    ///
    /// Parsed test results if a recognized pattern was found.
    pub fn parse_pytest(output: &str) -> Option<ParsedTestResults> {
        // Pattern: "X passed, Y failed, Z skipped"
        let re =
            regex::Regex::new(r"(\d+)\s+passed(?:,\s*(\d+)\s+failed)?(?:,\s*(\d+)\s+skipped)?")
                .ok()?;

        if let Some(caps) = re.captures(output) {
            let passed: u32 = caps.get(1)?.as_str().parse().ok()?;
            let failed = caps
                .get(2)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            let skipped = caps
                .get(3)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);

            let failures = Self::extract_pytest_failures(output);

            return Some(ParsedTestResults {
                total: passed + failed + skipped,
                passed,
                failed,
                skipped,
                failures,
            });
        }

        None
    }

    /// Extract failure names from pytest output.
    fn extract_pytest_failures(output: &str) -> Vec<ParsedTestFailure> {
        let mut failures = Vec::new();

        // Pattern: "FAILED test_file.py::test_name"
        let fail_re = regex::Regex::new(r"FAILED\s+(\S+)").ok();

        if let Some(re) = fail_re {
            for cap in re.captures_iter(output) {
                if let Some(name) = cap.get(1) {
                    failures.push(ParsedTestFailure {
                        name: name.as_str().to_string(),
                        message: String::new(),
                    });
                }
            }
        }

        failures
    }

    /// Parse generic test output (fallback).
    ///
    /// Counts occurrences of common pass/fail indicators in the output.
    ///
    /// # Arguments
    ///
    /// * `output` - Test output to analyze
    ///
    /// # Returns
    ///
    /// Best-effort test results based on keyword counting.
    pub fn parse_generic(output: &str) -> ParsedTestResults {
        // Count lines containing common pass/fail indicators
        let passed = output.matches("PASS").count()
            + output.matches("pass").count()
            + output.matches("ok").count();
        let failed = output.matches("FAIL").count()
            + output.matches("fail").count()
            + output.matches("error").count();

        ParsedTestResults {
            total: (passed + failed) as u32,
            passed: passed as u32,
            failed: failed as u32,
            skipped: 0,
            failures: vec![],
        }
    }

    /// Auto-detect parser based on command.
    ///
    /// Tries framework-specific parsers based on the command, then falls back
    /// to generic parsing.
    ///
    /// # Arguments
    ///
    /// * `output` - Test output to parse
    /// * `command` - The command that was run (used to select parser)
    pub fn auto_parse(output: &str, command: &str) -> ParsedTestResults {
        // Try framework-specific parsers based on command
        if command.contains("npm") || command.contains("jest") || command.contains("vitest") {
            if let Some(results) = Self::parse_jest(output) {
                return results;
            }
        }

        if command.contains("cargo") {
            if let Some(results) = Self::parse_cargo(output) {
                return results;
            }
        }

        if command.contains("pytest") {
            if let Some(results) = Self::parse_pytest(output) {
                return results;
            }
        }

        // Try all parsers as fallback
        if let Some(results) = Self::parse_cargo(output) {
            return results;
        }
        if let Some(results) = Self::parse_jest(output) {
            return results;
        }
        if let Some(results) = Self::parse_pytest(output) {
            return results;
        }

        // Generic fallback
        Self::parse_generic(output)
    }
}

/// Parsed test results from output.
///
/// Intermediate representation of test results extracted from output,
/// before conversion to [`VerificationResult`].
#[derive(Debug, Clone, Default)]
pub struct ParsedTestResults {
    /// Total tests run.
    pub total: u32,
    /// Tests passed.
    pub passed: u32,
    /// Tests failed.
    pub failed: u32,
    /// Tests skipped.
    pub skipped: u32,
    /// Extracted failure information.
    pub failures: Vec<ParsedTestFailure>,
}

/// Parsed test failure from output.
#[derive(Debug, Clone)]
pub struct ParsedTestFailure {
    /// Test name or path.
    pub name: String,
    /// Error message if extracted.
    pub message: String,
}

// === Verification Runner ===

/// Configuration for the verification runner.
///
/// Controls timeout, auto-detection, and retry behavior.
///
/// # Example
///
/// ```
/// use codirigent_core::verification::VerificationRunnerConfig;
/// use std::time::Duration;
///
/// let config = VerificationRunnerConfig::default();
/// assert_eq!(config.default_timeout, Duration::from_secs(300));
/// assert!(config.auto_detect);
/// assert_eq!(config.max_retries, 3);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRunnerConfig {
    /// Default timeout for verification commands.
    #[serde(with = "humantime_serde")]
    pub default_timeout: std::time::Duration,

    /// Whether to auto-detect test commands.
    pub auto_detect: bool,

    /// Maximum retries before marking as blocked.
    pub max_retries: u32,
}

impl Default for VerificationRunnerConfig {
    fn default() -> Self {
        Self {
            default_timeout: std::time::Duration::from_secs(300),
            auto_detect: true,
            max_retries: 3,
        }
    }
}

/// Verification runner executes tests and parses results.
///
/// Combines command detection, execution, and output parsing into
/// a single verification workflow.
///
/// # Example
///
/// ```
/// use codirigent_core::verification::{VerificationRunner, VerificationRunnerConfig};
///
/// let runner = VerificationRunner::new(VerificationRunnerConfig::default());
/// ```
pub struct VerificationRunner {
    /// Runner configuration.
    config: VerificationRunnerConfig,
    /// Test command detector.
    detector: TestCommandDetector,
}

impl VerificationRunner {
    /// Create a new verification runner.
    ///
    /// # Arguments
    ///
    /// * `config` - Runner configuration
    pub fn new(config: VerificationRunnerConfig) -> Self {
        Self {
            config,
            detector: TestCommandDetector::new(),
        }
    }

    /// Get the runner configuration.
    pub fn config(&self) -> &VerificationRunnerConfig {
        &self.config
    }

    /// Get the test command detector.
    pub fn detector(&self) -> &TestCommandDetector {
        &self.detector
    }

    /// Get a mutable reference to the detector for adding custom rules.
    pub fn detector_mut(&mut self) -> &mut TestCommandDetector {
        &mut self.detector
    }

    /// Get or detect verification command for a task.
    ///
    /// First checks the task's explicit verification config, then falls back
    /// to auto-detection if enabled.
    ///
    /// # Arguments
    ///
    /// * `commands` - Optional explicit verification commands
    /// * `working_dir` - Directory to detect commands for
    ///
    /// # Returns
    ///
    /// The verification command if found.
    pub fn get_command(
        &self,
        commands: Option<&VerificationCommands>,
        working_dir: &std::path::Path,
    ) -> Option<String> {
        // First check explicit command
        if let Some(cmds) = commands {
            if let Some(ref unit) = cmds.unit {
                return Some(unit.clone());
            }
        }

        // Auto-detect if enabled
        if self.config.auto_detect {
            self.detector.detect(working_dir)
        } else {
            None
        }
    }

    /// Run verification for a task.
    ///
    /// Executes the verification command and parses the output.
    ///
    /// # Arguments
    ///
    /// * `command` - Command to execute
    /// * `working_dir` - Directory to run in
    /// * `timeout` - Optional timeout override
    ///
    /// # Returns
    ///
    /// The verification result.
    pub async fn run(
        &self,
        command: &str,
        working_dir: &std::path::Path,
        timeout: Option<std::time::Duration>,
    ) -> anyhow::Result<VerificationResult> {
        use std::process::Stdio;
        use tokio::process::Command;

        let timeout_duration = timeout.unwrap_or(self.config.default_timeout);
        let start = std::time::Instant::now();

        // Parse command into program and args using shell
        // Use shell to handle complex commands with pipes, redirects, etc.
        #[cfg(unix)]
        let output = tokio::time::timeout(
            timeout_duration,
            Command::new("sh")
                .args(["-c", command])
                .current_dir(working_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Verification timed out after {:?}", timeout_duration))?
        .map_err(|e| anyhow::anyhow!("Failed to execute verification command: {}", e))?;

        #[cfg(windows)]
        let output = tokio::time::timeout(
            timeout_duration,
            Command::new("cmd")
                .args(["/C", command])
                .current_dir(working_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Verification timed out after {:?}", timeout_duration))?
        .map_err(|e| anyhow::anyhow!("Failed to execute verification command: {}", e))?;

        let duration = start.elapsed();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined_output = format!("{}\n{}", stdout, stderr);

        // Parse the output
        let parsed = OutputParser::auto_parse(&combined_output, command);

        // Convert to VerificationResult
        let failures: Vec<VerificationFailure> = parsed
            .failures
            .into_iter()
            .map(|f| VerificationFailure::new(f.name, f.message))
            .collect();

        let mut result = if output.status.success() && parsed.failed == 0 {
            VerificationResult::passed(VerificationCheckType::UnitTest, duration.as_millis() as u64)
        } else {
            VerificationResult::failed(
                VerificationCheckType::UnitTest,
                failures,
                duration.as_millis() as u64,
            )
        };

        // Add counts if available
        if parsed.total > 0 {
            result = result.with_counts(parsed.passed, parsed.total);
        }

        // Add raw output
        result = result.with_raw_output(combined_output);

        Ok(result)
    }

    /// Format verification failure as markdown for sending back to session.
    ///
    /// Creates a clear, actionable message describing what failed and how many
    /// retries remain.
    ///
    /// # Arguments
    ///
    /// * `result` - The verification result containing failures
    /// * `retry_count` - Current retry count
    /// * `max_retries` - Maximum allowed retries
    ///
    /// # Returns
    ///
    /// Formatted markdown string.
    pub fn format_failure_message(
        &self,
        result: &VerificationResult,
        retry_count: u32,
        max_retries: u32,
    ) -> String {
        let mut message = String::new();

        message.push_str("## Verification Failed\n\n");

        // Test results summary
        if let (Some(passed), Some(total)) = (result.passed_count, result.total_count) {
            let failed = total.saturating_sub(passed);
            message.push_str(&format!(
                "**Test Results:** {} passed, {} failed\n\n",
                passed, failed
            ));
        }

        // List failures
        if !result.failures.is_empty() {
            message.push_str("### Failures:\n\n");
            for (i, failure) in result.failures.iter().enumerate() {
                message.push_str(&format!("**{}. {}**\n", i + 1, failure.name));
                if !failure.message.is_empty() {
                    message.push_str(&format!("```\n{}\n```\n", failure.message));
                }
                message.push('\n');
            }
        }

        // Include raw output if no structured failures and output is reasonable size
        if result.failures.is_empty() {
            if let Some(ref output) = result.raw_output {
                if !output.is_empty() && output.len() < 2000 {
                    message.push_str("### Output:\n\n");
                    message.push_str("```\n");
                    message.push_str(output);
                    message.push_str("\n```\n\n");
                }
            }
        }

        // Retry info
        message.push_str("---\n\n");
        message.push_str(&format!(
            "Please fix the above issues and complete the task again.\n\n*Retry: {}/{}*",
            retry_count, max_retries
        ));

        message
    }
}

/// Trait for verification service implementations.
///
/// Provides a high-level interface for verification operations.
#[async_trait]
pub trait VerificationService: Send + Sync {
    /// Run verification for a task.
    ///
    /// # Arguments
    ///
    /// * `commands` - Optional verification commands
    /// * `working_dir` - Directory to run verification in
    async fn verify(
        &self,
        commands: Option<&VerificationCommands>,
        working_dir: &std::path::Path,
    ) -> anyhow::Result<VerificationResult>;

    /// Check if verification should run.
    ///
    /// # Arguments
    ///
    /// * `commands` - Optional verification commands
    /// * `working_dir` - Directory to check
    fn should_verify(
        &self,
        commands: Option<&VerificationCommands>,
        working_dir: &std::path::Path,
    ) -> bool;

    /// Format failure for retry.
    ///
    /// # Arguments
    ///
    /// * `result` - Verification result to format
    /// * `retry_count` - Current retry count
    /// * `max_retries` - Maximum retries allowed
    fn format_failure(
        &self,
        result: &VerificationResult,
        retry_count: u32,
        max_retries: u32,
    ) -> String;
}

#[async_trait]
impl VerificationService for VerificationRunner {
    async fn verify(
        &self,
        commands: Option<&VerificationCommands>,
        working_dir: &std::path::Path,
    ) -> anyhow::Result<VerificationResult> {
        let command = self
            .get_command(commands, working_dir)
            .ok_or_else(|| anyhow::anyhow!("No verification command found"))?;

        self.run(&command, working_dir, None).await
    }

    fn should_verify(
        &self,
        commands: Option<&VerificationCommands>,
        working_dir: &std::path::Path,
    ) -> bool {
        // Verify if commands are provided or if auto-detect finds something
        if let Some(cmds) = commands {
            if cmds.has_any() {
                return true;
            }
        }

        if self.config.auto_detect {
            return self.detector.detect(working_dir).is_some();
        }

        false
    }

    fn format_failure(
        &self,
        result: &VerificationResult,
        retry_count: u32,
        max_retries: u32,
    ) -> String {
        self.format_failure_message(result, retry_count, max_retries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // VerificationResult tests

    #[test]
    fn test_verification_result_passed() {
        let result = VerificationResult {
            check_type: VerificationCheckType::UnitTest,
            passed: true,
            passed_count: Some(23),
            total_count: Some(23),
            failures: vec![],
            duration_ms: 1500,
            raw_output: None,
        };
        assert!(result.passed);
        assert!(result.failures.is_empty());
    }

    #[test]
    fn test_verification_result_failed() {
        let failure = VerificationFailure {
            name: "test_auth_expired_token".to_string(),
            file: Some(PathBuf::from("src/auth.test.ts")),
            line: Some(42),
            expected: Some("401".to_string()),
            actual: Some("200".to_string()),
            message: "Expected 401, received 200".to_string(),
        };
        let result = VerificationResult {
            check_type: VerificationCheckType::UnitTest,
            passed: false,
            passed_count: Some(21),
            total_count: Some(23),
            failures: vec![failure],
            duration_ms: 2000,
            raw_output: None,
        };
        assert!(!result.passed);
        assert_eq!(result.failures.len(), 1);
    }

    #[test]
    fn test_verification_result_passed_constructor() {
        let result = VerificationResult::passed(VerificationCheckType::Lint, 500);
        assert!(result.passed);
        assert!(result.failures.is_empty());
        assert_eq!(result.duration_ms, 500);
    }

    #[test]
    fn test_verification_result_failed_constructor() {
        let failures = vec![VerificationFailure::new("test_foo", "assertion failed")];
        let result = VerificationResult::failed(VerificationCheckType::UnitTest, failures, 1000);
        assert!(!result.passed);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.duration_ms, 1000);
    }

    #[test]
    fn test_verification_result_with_counts() {
        let result =
            VerificationResult::passed(VerificationCheckType::UnitTest, 100).with_counts(10, 10);
        assert_eq!(result.passed_count, Some(10));
        assert_eq!(result.total_count, Some(10));
    }

    #[test]
    fn test_verification_result_with_raw_output() {
        let result = VerificationResult::passed(VerificationCheckType::UnitTest, 100)
            .with_raw_output("test output".to_string());
        assert_eq!(result.raw_output, Some("test output".to_string()));
    }

    #[test]
    fn test_verification_result_serialization() {
        let result = VerificationResult {
            check_type: VerificationCheckType::UnitTest,
            passed: true,
            passed_count: Some(5),
            total_count: Some(5),
            failures: vec![],
            duration_ms: 100,
            raw_output: Some("all tests passed".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: VerificationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.check_type, parsed.check_type);
        assert_eq!(result.passed, parsed.passed);
        assert_eq!(result.passed_count, parsed.passed_count);
    }

    // VerificationCheckType tests

    #[test]
    fn test_verification_check_type_serialization() {
        let check = VerificationCheckType::UnitTest;
        let json = serde_json::to_string(&check).unwrap();
        assert_eq!(json, "\"UnitTest\"");
    }

    #[test]
    fn test_verification_check_type_all_variants() {
        let variants = [
            VerificationCheckType::UnitTest,
            VerificationCheckType::IntegrationTest,
            VerificationCheckType::TypeCheck,
            VerificationCheckType::Lint,
            VerificationCheckType::Format,
            VerificationCheckType::Custom,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: VerificationCheckType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_verification_check_type_display() {
        assert_eq!(format!("{}", VerificationCheckType::UnitTest), "Unit Test");
        assert_eq!(
            format!("{}", VerificationCheckType::IntegrationTest),
            "Integration Test"
        );
        assert_eq!(
            format!("{}", VerificationCheckType::TypeCheck),
            "Type Check"
        );
        assert_eq!(format!("{}", VerificationCheckType::Lint), "Lint");
        assert_eq!(format!("{}", VerificationCheckType::Format), "Format");
        assert_eq!(format!("{}", VerificationCheckType::Custom), "Custom");
    }

    // VerificationFailure tests

    #[test]
    fn test_verification_failure_new() {
        let failure = VerificationFailure::new("test_foo", "assertion failed");
        assert_eq!(failure.name, "test_foo");
        assert_eq!(failure.message, "assertion failed");
        assert!(failure.file.is_none());
        assert!(failure.line.is_none());
    }

    #[test]
    fn test_verification_failure_with_file() {
        let failure =
            VerificationFailure::new("test_foo", "failed").with_file(PathBuf::from("src/test.rs"));
        assert_eq!(failure.file, Some(PathBuf::from("src/test.rs")));
    }

    #[test]
    fn test_verification_failure_with_line() {
        let failure = VerificationFailure::new("test_foo", "failed").with_line(42);
        assert_eq!(failure.line, Some(42));
    }

    #[test]
    fn test_verification_failure_with_comparison() {
        let failure = VerificationFailure::new("test_foo", "mismatch")
            .with_comparison("expected".to_string(), "actual".to_string());
        assert_eq!(failure.expected, Some("expected".to_string()));
        assert_eq!(failure.actual, Some("actual".to_string()));
    }

    #[test]
    fn test_verification_failure_serialization() {
        let failure = VerificationFailure {
            name: "test_auth".to_string(),
            file: Some(PathBuf::from("src/auth.rs")),
            line: Some(10),
            expected: Some("true".to_string()),
            actual: Some("false".to_string()),
            message: "Expected true, got false".to_string(),
        };
        let json = serde_json::to_string(&failure).unwrap();
        let parsed: VerificationFailure = serde_json::from_str(&json).unwrap();
        assert_eq!(failure.name, parsed.name);
        assert_eq!(failure.file, parsed.file);
        assert_eq!(failure.line, parsed.line);
    }

    // VerificationConfig tests

    #[test]
    fn test_verification_config_default() {
        let config = VerificationConfig::default();
        assert!(config.enabled);
        assert!(config.auto_detect);
        assert_eq!(config.max_retries, 3);
        assert!(config.requires_human_review);
    }

    #[test]
    fn test_verification_config_serialization() {
        let config = VerificationConfig {
            enabled: true,
            auto_detect: false,
            max_retries: 5,
            commands: VerificationCommands {
                unit: Some("npm test".to_string()),
                integration: None,
                type_check: Some("tsc --noEmit".to_string()),
                lint: None,
                format: None,
                custom: vec![],
            },
            requires_human_review: false,
        };
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: VerificationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.max_retries, parsed.max_retries);
        assert_eq!(config.enabled, parsed.enabled);
    }

    // VerificationCommands tests

    #[test]
    fn test_verification_commands_default() {
        let commands = VerificationCommands::default();
        assert!(commands.unit.is_none());
        assert!(commands.integration.is_none());
        assert!(commands.custom.is_empty());
    }

    #[test]
    fn test_verification_commands_has_any() {
        let mut commands = VerificationCommands::default();
        assert!(!commands.has_any());

        commands.unit = Some("npm test".to_string());
        assert!(commands.has_any());
    }

    #[test]
    fn test_verification_commands_has_any_custom() {
        let mut commands = VerificationCommands::default();
        commands.custom.push("./scripts/verify.sh".to_string());
        assert!(commands.has_any());
    }

    #[test]
    fn test_verification_commands_to_list() {
        let commands = VerificationCommands {
            unit: Some("npm test".to_string()),
            integration: None,
            type_check: Some("tsc --noEmit".to_string()),
            lint: None,
            format: None,
            custom: vec!["./verify.sh".to_string()],
        };
        let list = commands.to_list();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].0, VerificationCheckType::UnitTest);
        assert_eq!(list[1].0, VerificationCheckType::TypeCheck);
        assert_eq!(list[2].0, VerificationCheckType::Custom);
    }

    #[test]
    fn test_verification_commands_serialization() {
        let commands = VerificationCommands {
            unit: Some("cargo test".to_string()),
            integration: Some("cargo test --test integration".to_string()),
            type_check: None,
            lint: Some("cargo clippy".to_string()),
            format: Some("cargo fmt --check".to_string()),
            custom: vec!["./check.sh".to_string()],
        };
        let json = serde_json::to_string(&commands).unwrap();
        let parsed: VerificationCommands = serde_json::from_str(&json).unwrap();
        assert_eq!(commands.unit, parsed.unit);
        assert_eq!(commands.custom, parsed.custom);
    }

    // VerificationState tests

    #[test]
    fn test_verification_state_default() {
        assert_eq!(VerificationState::default(), VerificationState::Pending);
    }

    #[test]
    fn test_verification_state_display() {
        assert_eq!(format!("{}", VerificationState::Pending), "Pending");
        assert_eq!(format!("{}", VerificationState::Running), "Running");
        assert_eq!(format!("{}", VerificationState::Passed), "Passed");
        assert_eq!(format!("{}", VerificationState::Failed), "Failed");
        assert_eq!(
            format!("{}", VerificationState::RetryingInSession),
            "Retrying in Session"
        );
        assert_eq!(format!("{}", VerificationState::Blocked), "Blocked");
        assert_eq!(format!("{}", VerificationState::Skipped), "Skipped");
    }

    #[test]
    fn test_verification_state_serialization() {
        let states = [
            VerificationState::Pending,
            VerificationState::Running,
            VerificationState::Passed,
            VerificationState::Failed,
            VerificationState::RetryingInSession,
            VerificationState::Blocked,
            VerificationState::Skipped,
        ];
        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            let parsed: VerificationState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, parsed);
        }
    }

    // VerificationStatus tests

    #[test]
    fn test_verification_status_creation() {
        let status = VerificationStatus {
            task_id: TaskId::from("task-001"),
            session_id: SessionId(1),
            state: VerificationState::Running,
            retry_count: 0,
            results: vec![],
            started_at: chrono::Utc::now(),
            completed_at: None,
        };
        assert_eq!(status.state, VerificationState::Running);
        assert!(status.completed_at.is_none());
    }

    #[test]
    fn test_verification_status_new() {
        let status = VerificationStatus::new(TaskId::from("task-001"), SessionId(1));
        assert_eq!(status.state, VerificationState::Pending);
        assert_eq!(status.retry_count, 0);
        assert!(status.results.is_empty());
    }

    #[test]
    fn test_verification_status_all_passed() {
        let mut status = VerificationStatus::new(TaskId::from("task-001"), SessionId(1));
        assert!(!status.all_passed()); // No results

        status.results.push(VerificationResult::passed(
            VerificationCheckType::UnitTest,
            100,
        ));
        status
            .results
            .push(VerificationResult::passed(VerificationCheckType::Lint, 50));
        assert!(status.all_passed());

        let failures = vec![VerificationFailure::new("test", "failed")];
        status.results.push(VerificationResult::failed(
            VerificationCheckType::TypeCheck,
            failures,
            100,
        ));
        assert!(!status.all_passed());
    }

    #[test]
    fn test_verification_status_all_failures() {
        let mut status = VerificationStatus::new(TaskId::from("task-001"), SessionId(1));

        status
            .results
            .push(VerificationResult::passed(VerificationCheckType::Lint, 100));

        let failures1 = vec![
            VerificationFailure::new("test1", "failed1"),
            VerificationFailure::new("test2", "failed2"),
        ];
        status.results.push(VerificationResult::failed(
            VerificationCheckType::UnitTest,
            failures1,
            100,
        ));

        let failures2 = vec![VerificationFailure::new("test3", "failed3")];
        status.results.push(VerificationResult::failed(
            VerificationCheckType::TypeCheck,
            failures2,
            100,
        ));

        let all_failures = status.all_failures();
        assert_eq!(all_failures.len(), 3);
    }

    #[test]
    fn test_verification_status_total_duration() {
        let mut status = VerificationStatus::new(TaskId::from("task-001"), SessionId(1));
        status.results.push(VerificationResult::passed(
            VerificationCheckType::UnitTest,
            100,
        ));
        status
            .results
            .push(VerificationResult::passed(VerificationCheckType::Lint, 50));
        status.results.push(VerificationResult::passed(
            VerificationCheckType::TypeCheck,
            200,
        ));

        assert_eq!(status.total_duration_ms(), 350);
    }

    #[test]
    fn test_verification_status_complete() {
        let mut status = VerificationStatus::new(TaskId::from("task-001"), SessionId(1));
        assert!(status.completed_at.is_none());

        status.complete(VerificationState::Passed);
        assert_eq!(status.state, VerificationState::Passed);
        assert!(status.completed_at.is_some());
    }

    #[test]
    fn test_verification_status_serialization() {
        let status = VerificationStatus {
            task_id: TaskId::from("task-001"),
            session_id: SessionId(1),
            state: VerificationState::Passed,
            retry_count: 1,
            results: vec![],
            started_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
        };
        let json = serde_json::to_string_pretty(&status).unwrap();
        let parsed: VerificationStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status.task_id, parsed.task_id);
        assert_eq!(status.state, parsed.state);
    }

    // VerificationEvent tests

    #[test]
    fn test_verification_event_started() {
        let event = VerificationEvent::Started {
            task_id: TaskId::from("task-001"),
            session_id: SessionId(1),
        };
        assert!(matches!(event, VerificationEvent::Started { .. }));
    }

    #[test]
    fn test_verification_event_check_completed() {
        let result = VerificationResult::passed(VerificationCheckType::UnitTest, 100);
        let event = VerificationEvent::CheckCompleted {
            task_id: TaskId::from("task-001"),
            result,
        };
        assert!(matches!(event, VerificationEvent::CheckCompleted { .. }));
    }

    #[test]
    fn test_verification_event_passed() {
        let event = VerificationEvent::Passed {
            task_id: TaskId::from("task-001"),
        };
        assert!(matches!(event, VerificationEvent::Passed { .. }));
    }

    #[test]
    fn test_verification_event_failed_retrying() {
        let event = VerificationEvent::FailedRetrying {
            task_id: TaskId::from("task-001"),
            retry_count: 2,
            failures: vec![],
        };
        if let VerificationEvent::FailedRetrying { retry_count, .. } = event {
            assert_eq!(retry_count, 2);
        } else {
            panic!("Expected FailedRetrying");
        }
    }

    #[test]
    fn test_verification_event_failed_blocked() {
        let failures = vec![VerificationFailure::new("test", "failed")];
        let event = VerificationEvent::FailedBlocked {
            task_id: TaskId::from("task-001"),
            failures,
        };
        if let VerificationEvent::FailedBlocked { failures, .. } = event {
            assert_eq!(failures.len(), 1);
        } else {
            panic!("Expected FailedBlocked");
        }
    }

    #[test]
    fn test_verification_event_clone() {
        let event = VerificationEvent::Passed {
            task_id: TaskId::from("task-001"),
        };
        let cloned = event.clone();
        assert!(matches!(cloned, VerificationEvent::Passed { .. }));
    }

    // TestCommandDetector tests

    #[test]
    fn test_detection_rule_new() {
        let rule = DetectionRule::new("package.json", "npm test");
        assert_eq!(rule.marker_file, "package.json");
        assert_eq!(rule.command, "npm test");
        assert!(rule.make_target.is_none());
    }

    #[test]
    fn test_detection_rule_with_target() {
        let rule = DetectionRule::with_target("Makefile", "make test", "test");
        assert_eq!(rule.marker_file, "Makefile");
        assert_eq!(rule.command, "make test");
        assert_eq!(rule.make_target, Some("test".to_string()));
    }

    #[test]
    fn test_detection_rule_clone() {
        let rule = DetectionRule::new("Cargo.toml", "cargo test");
        let cloned = rule.clone();
        assert_eq!(cloned.marker_file, "Cargo.toml");
        assert_eq!(cloned.command, "cargo test");
    }

    #[test]
    fn test_detection_rule_debug() {
        let rule = DetectionRule::new("go.mod", "go test ./...");
        let debug_str = format!("{:?}", rule);
        assert!(debug_str.contains("go.mod"));
        assert!(debug_str.contains("go test"));
    }

    #[test]
    fn test_test_command_detector_new() {
        let detector = TestCommandDetector::new();
        assert!(!detector.rules().is_empty());
        // Should have default rules for common project types
        let rules = detector.rules();
        assert!(rules.iter().any(|r| r.marker_file == "package.json"));
        assert!(rules.iter().any(|r| r.marker_file == "Cargo.toml"));
        assert!(rules.iter().any(|r| r.marker_file == "pyproject.toml"));
        assert!(rules.iter().any(|r| r.marker_file == "go.mod"));
    }

    #[test]
    fn test_test_command_detector_default() {
        let detector = TestCommandDetector::default();
        assert!(!detector.rules().is_empty());
    }

    #[test]
    fn test_detect_npm() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("package.json"), "{}").unwrap();

        let detector = TestCommandDetector::new();
        assert_eq!(detector.detect(temp.path()), Some("npm test".to_string()));
    }

    #[test]
    fn test_detect_cargo() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        let detector = TestCommandDetector::new();
        assert_eq!(detector.detect(temp.path()), Some("cargo test".to_string()));
    }

    #[test]
    fn test_detect_pyproject() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("pyproject.toml"), "[project]").unwrap();

        let detector = TestCommandDetector::new();
        assert_eq!(detector.detect(temp.path()), Some("pytest".to_string()));
    }

    #[test]
    fn test_detect_setup_py() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("setup.py"), "from setuptools import setup").unwrap();

        let detector = TestCommandDetector::new();
        assert_eq!(detector.detect(temp.path()), Some("pytest".to_string()));
    }

    #[test]
    fn test_detect_go() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("go.mod"), "module test").unwrap();

        let detector = TestCommandDetector::new();
        assert_eq!(
            detector.detect(temp.path()),
            Some("go test ./...".to_string())
        );
    }

    #[test]
    fn test_detect_makefile_with_test_target() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("Makefile"), "test:\n\techo test").unwrap();

        let detector = TestCommandDetector::new();
        assert_eq!(detector.detect(temp.path()), Some("make test".to_string()));
    }

    #[test]
    fn test_detect_makefile_without_test_target() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("Makefile"), "build:\n\techo build").unwrap();

        let detector = TestCommandDetector::new();
        // Should not match because there's no "test:" target
        assert!(detector.detect(temp.path()).is_none());
    }

    #[test]
    fn test_detect_nothing() {
        let temp = tempfile::TempDir::new().unwrap();

        let detector = TestCommandDetector::new();
        assert!(detector.detect(temp.path()).is_none());
    }

    #[test]
    fn test_detect_priority() {
        let temp = tempfile::TempDir::new().unwrap();
        // Create both package.json and Cargo.toml
        std::fs::write(temp.path().join("package.json"), "{}").unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        let detector = TestCommandDetector::new();
        // package.json comes first in the default rules
        assert_eq!(detector.detect(temp.path()), Some("npm test".to_string()));
    }

    #[test]
    fn test_custom_rule_priority() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("custom.toml"), "").unwrap();
        std::fs::write(temp.path().join("package.json"), "{}").unwrap();

        let mut detector = TestCommandDetector::new();
        detector.add_rule(DetectionRule::new("custom.toml", "custom-test"));

        // Custom rule should match first
        assert_eq!(
            detector.detect(temp.path()),
            Some("custom-test".to_string())
        );
    }

    #[test]
    fn test_add_multiple_rules() {
        let mut detector = TestCommandDetector::new();
        let initial_count = detector.rules().len();

        detector.add_rule(DetectionRule::new("first.toml", "first-test"));
        detector.add_rule(DetectionRule::new("second.toml", "second-test"));

        assert_eq!(detector.rules().len(), initial_count + 2);
        // Most recently added rule should be first
        assert_eq!(detector.rules()[0].marker_file, "second.toml");
        assert_eq!(detector.rules()[1].marker_file, "first.toml");
    }

    #[test]
    fn test_detector_clone() {
        let detector = TestCommandDetector::new();
        let cloned = detector.clone();
        assert_eq!(detector.rules().len(), cloned.rules().len());
    }

    #[test]
    fn test_detector_debug() {
        let detector = TestCommandDetector::new();
        let debug_str = format!("{:?}", detector);
        assert!(debug_str.contains("TestCommandDetector"));
        assert!(debug_str.contains("rules"));
    }

    // OutputParser tests

    #[test]
    fn test_parse_jest_basic() {
        let output = "Tests: 2 failed, 23 passed, 25 total";
        let result = OutputParser::parse_jest(output).unwrap();

        assert_eq!(result.total, 25);
        assert_eq!(result.passed, 23);
        assert_eq!(result.failed, 2);
    }

    #[test]
    fn test_parse_jest_all_passed() {
        let output = "Tests: 10 passed, 10 total";
        let result = OutputParser::parse_jest(output).unwrap();

        assert_eq!(result.total, 10);
        assert_eq!(result.passed, 10);
        assert_eq!(result.failed, 0);
    }

    #[test]
    fn test_parse_jest_no_total() {
        let output = "Tests: 5 passed";
        let result = OutputParser::parse_jest(output).unwrap();

        assert_eq!(result.passed, 5);
        assert_eq!(result.total, 5); // Inferred
    }

    #[test]
    fn test_parse_jest_no_match() {
        let output = "random text without test results";
        let result = OutputParser::parse_jest(output);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_cargo_basic() {
        let output = "test result: ok. 15 passed; 0 failed; 2 ignored";
        let result = OutputParser::parse_cargo(output).unwrap();

        assert_eq!(result.total, 17);
        assert_eq!(result.passed, 15);
        assert_eq!(result.failed, 0);
        assert_eq!(result.skipped, 2);
    }

    #[test]
    fn test_parse_cargo_with_failures() {
        let output = "test result: FAILED. 10 passed; 2 failed; 1 ignored";
        let result = OutputParser::parse_cargo(output).unwrap();

        assert_eq!(result.total, 13);
        assert_eq!(result.passed, 10);
        assert_eq!(result.failed, 2);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    fn test_parse_cargo_no_match() {
        let output = "random text";
        let result = OutputParser::parse_cargo(output);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_cargo_extract_failures() {
        let output = "---- my_test stdout ----\nfailed\n---- another_test stdout ----\nfailed\ntest result: FAILED. 10 passed; 2 failed; 0 ignored";
        let result = OutputParser::parse_cargo(output).unwrap();

        assert_eq!(result.failures.len(), 2);
        assert_eq!(result.failures[0].name, "my_test");
        assert_eq!(result.failures[1].name, "another_test");
    }

    #[test]
    fn test_parse_pytest_basic() {
        let output = "===== 10 passed, 2 failed in 5.2s =====";
        let result = OutputParser::parse_pytest(output).unwrap();

        assert_eq!(result.passed, 10);
        assert_eq!(result.failed, 2);
        assert_eq!(result.total, 12);
    }

    #[test]
    fn test_parse_pytest_with_skipped() {
        let output = "5 passed, 1 failed, 3 skipped";
        let result = OutputParser::parse_pytest(output).unwrap();

        assert_eq!(result.passed, 5);
        assert_eq!(result.failed, 1);
        assert_eq!(result.skipped, 3);
        assert_eq!(result.total, 9);
    }

    #[test]
    fn test_parse_pytest_all_passed() {
        let output = "=== 20 passed in 3.5s ===";
        let result = OutputParser::parse_pytest(output).unwrap();

        assert_eq!(result.passed, 20);
        assert_eq!(result.failed, 0);
    }

    #[test]
    fn test_parse_pytest_no_match() {
        let output = "random text";
        let result = OutputParser::parse_pytest(output);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_pytest_extract_failures() {
        let output =
            "FAILED test_file.py::test_one\nFAILED test_file.py::test_two\n5 passed, 2 failed";
        let result = OutputParser::parse_pytest(output).unwrap();

        assert_eq!(result.failures.len(), 2);
        assert_eq!(result.failures[0].name, "test_file.py::test_one");
        assert_eq!(result.failures[1].name, "test_file.py::test_two");
    }

    #[test]
    fn test_parse_generic_pass_fail() {
        let output = "PASS test1\nPASS test2\nFAIL test3";
        let result = OutputParser::parse_generic(output);

        assert_eq!(result.passed, 2);
        assert_eq!(result.failed, 1);
        assert_eq!(result.total, 3);
    }

    #[test]
    fn test_parse_generic_lowercase() {
        let output = "pass test1\npass test2\nfail test3\nerror test4";
        let result = OutputParser::parse_generic(output);

        assert_eq!(result.passed, 2);
        assert_eq!(result.failed, 2); // fail + error
    }

    #[test]
    fn test_parse_generic_ok() {
        let output = "ok test1\nok test2\nok test3";
        let result = OutputParser::parse_generic(output);

        assert_eq!(result.passed, 3);
        assert_eq!(result.failed, 0);
    }

    #[test]
    fn test_auto_parse_npm() {
        let output = "Tests: 5 passed, 5 total";
        let result = OutputParser::auto_parse(output, "npm test");

        assert_eq!(result.passed, 5);
        assert_eq!(result.total, 5);
    }

    #[test]
    fn test_auto_parse_cargo() {
        let output = "test result: ok. 10 passed; 0 failed; 0 ignored";
        let result = OutputParser::auto_parse(output, "cargo test");

        assert_eq!(result.passed, 10);
        assert_eq!(result.failed, 0);
    }

    #[test]
    fn test_auto_parse_pytest() {
        let output = "5 passed, 1 failed";
        let result = OutputParser::auto_parse(output, "pytest");

        assert_eq!(result.passed, 5);
        assert_eq!(result.failed, 1);
    }

    #[test]
    fn test_auto_parse_fallback_to_cargo() {
        // Cargo output with unknown command
        let output = "test result: ok. 5 passed; 0 failed; 0 ignored";
        let result = OutputParser::auto_parse(output, "unknown command");

        assert_eq!(result.passed, 5);
    }

    #[test]
    fn test_auto_parse_fallback_to_generic() {
        let output = "PASS one\nPASS two\nFAIL three";
        let result = OutputParser::auto_parse(output, "unknown");

        assert_eq!(result.passed, 2);
        assert_eq!(result.failed, 1);
    }

    // ParsedTestResults tests

    #[test]
    fn test_parsed_test_results_default() {
        let results = ParsedTestResults::default();
        assert_eq!(results.total, 0);
        assert_eq!(results.passed, 0);
        assert_eq!(results.failed, 0);
        assert_eq!(results.skipped, 0);
        assert!(results.failures.is_empty());
    }

    #[test]
    fn test_parsed_test_results_clone() {
        let results = ParsedTestResults {
            total: 10,
            passed: 8,
            failed: 2,
            skipped: 0,
            failures: vec![ParsedTestFailure {
                name: "test".to_string(),
                message: "failed".to_string(),
            }],
        };
        let cloned = results.clone();
        assert_eq!(cloned.total, 10);
        assert_eq!(cloned.failures.len(), 1);
    }

    #[test]
    fn test_parsed_test_results_debug() {
        let results = ParsedTestResults::default();
        let debug_str = format!("{:?}", results);
        assert!(debug_str.contains("ParsedTestResults"));
    }

    // VerificationRunnerConfig tests

    #[test]
    fn test_verification_runner_config_default() {
        let config = VerificationRunnerConfig::default();
        assert_eq!(config.default_timeout, std::time::Duration::from_secs(300));
        assert!(config.auto_detect);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_verification_runner_config_serialization() {
        let config = VerificationRunnerConfig {
            default_timeout: std::time::Duration::from_secs(60),
            auto_detect: false,
            max_retries: 5,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: VerificationRunnerConfig = serde_json::from_str(&json).unwrap();
        assert!(!parsed.auto_detect);
        assert_eq!(parsed.max_retries, 5);
    }

    #[test]
    fn test_verification_runner_config_clone() {
        let config = VerificationRunnerConfig::default();
        let cloned = config.clone();
        assert_eq!(config.max_retries, cloned.max_retries);
    }

    // VerificationRunner tests

    #[test]
    fn test_verification_runner_new() {
        let runner = VerificationRunner::new(VerificationRunnerConfig::default());
        assert!(runner.config().auto_detect);
        assert!(!runner.detector().rules().is_empty());
    }

    #[test]
    fn test_verification_runner_get_command_explicit() {
        let runner = VerificationRunner::new(VerificationRunnerConfig::default());
        let temp = tempfile::TempDir::new().unwrap();

        let commands = VerificationCommands {
            unit: Some("custom-test".to_string()),
            ..Default::default()
        };

        assert_eq!(
            runner.get_command(Some(&commands), temp.path()),
            Some("custom-test".to_string())
        );
    }

    #[test]
    fn test_verification_runner_get_command_auto_detect() {
        let runner = VerificationRunner::new(VerificationRunnerConfig::default());
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        assert_eq!(
            runner.get_command(None, temp.path()),
            Some("cargo test".to_string())
        );
    }

    #[test]
    fn test_verification_runner_get_command_no_auto_detect() {
        let config = VerificationRunnerConfig {
            auto_detect: false,
            ..Default::default()
        };
        let runner = VerificationRunner::new(config);
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        // Should not auto-detect when disabled
        assert!(runner.get_command(None, temp.path()).is_none());
    }

    #[test]
    fn test_verification_runner_detector_mut() {
        let mut runner = VerificationRunner::new(VerificationRunnerConfig::default());
        let initial_count = runner.detector().rules().len();

        runner
            .detector_mut()
            .add_rule(DetectionRule::new("custom.json", "custom"));

        assert_eq!(runner.detector().rules().len(), initial_count + 1);
    }

    #[tokio::test]
    async fn test_verification_runner_run_echo() {
        let runner = VerificationRunner::new(VerificationRunnerConfig::default());
        let temp = tempfile::TempDir::new().unwrap();

        let result = runner.run("echo success", temp.path(), None).await.unwrap();
        assert!(result.passed);
        assert!(result.raw_output.as_ref().unwrap().contains("success"));
    }

    #[tokio::test]
    async fn test_verification_runner_run_failing_command() {
        let runner = VerificationRunner::new(VerificationRunnerConfig::default());
        let temp = tempfile::TempDir::new().unwrap();

        // exit 1 causes command to fail
        let result = runner.run("exit 1", temp.path(), None).await.unwrap();
        assert!(!result.passed);
    }

    #[tokio::test]
    async fn test_verification_runner_run_with_timeout() {
        let runner = VerificationRunner::new(VerificationRunnerConfig::default());
        let temp = tempfile::TempDir::new().unwrap();

        let result = runner
            .run(
                "echo fast",
                temp.path(),
                Some(std::time::Duration::from_secs(10)),
            )
            .await
            .unwrap();
        assert!(result.passed);
    }

    // VerificationRunner format_failure_message tests

    #[test]
    fn test_format_failure_message_basic() {
        let runner = VerificationRunner::new(VerificationRunnerConfig::default());

        let result = VerificationResult::failed(
            VerificationCheckType::UnitTest,
            vec![VerificationFailure::new(
                "test_auth",
                "Expected 401, got 200",
            )],
            5000,
        );

        let message = runner.format_failure_message(&result, 1, 3);

        assert!(message.contains("Verification Failed"));
        assert!(message.contains("test_auth"));
        assert!(message.contains("Retry: 1/3"));
    }

    #[test]
    fn test_format_failure_message_with_counts() {
        let runner = VerificationRunner::new(VerificationRunnerConfig::default());

        let result = VerificationResult::failed(
            VerificationCheckType::UnitTest,
            vec![VerificationFailure::new("test", "failed")],
            1000,
        )
        .with_counts(8, 10);

        let message = runner.format_failure_message(&result, 2, 3);

        assert!(message.contains("8 passed, 2 failed"));
    }

    #[test]
    fn test_format_failure_message_with_raw_output() {
        let runner = VerificationRunner::new(VerificationRunnerConfig::default());

        let result = VerificationResult::failed(
            VerificationCheckType::UnitTest,
            vec![], // No structured failures
            1000,
        )
        .with_raw_output("Error: something went wrong".to_string());

        let message = runner.format_failure_message(&result, 1, 3);

        assert!(message.contains("Error: something went wrong"));
    }

    #[test]
    fn test_format_failure_message_multiple_failures() {
        let runner = VerificationRunner::new(VerificationRunnerConfig::default());

        let result = VerificationResult::failed(
            VerificationCheckType::UnitTest,
            vec![
                VerificationFailure::new("test_one", "first failure"),
                VerificationFailure::new("test_two", "second failure"),
                VerificationFailure::new("test_three", "third failure"),
            ],
            1000,
        );

        let message = runner.format_failure_message(&result, 1, 3);

        assert!(message.contains("1. test_one"));
        assert!(message.contains("2. test_two"));
        assert!(message.contains("3. test_three"));
    }

    // VerificationService trait tests

    #[test]
    fn test_verification_service_should_verify_with_commands() {
        let runner = VerificationRunner::new(VerificationRunnerConfig::default());
        let temp = tempfile::TempDir::new().unwrap();

        let commands = VerificationCommands {
            unit: Some("npm test".to_string()),
            ..Default::default()
        };

        assert!(runner.should_verify(Some(&commands), temp.path()));
    }

    #[test]
    fn test_verification_service_should_verify_auto_detect() {
        let runner = VerificationRunner::new(VerificationRunnerConfig::default());
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("package.json"), "{}").unwrap();

        assert!(runner.should_verify(None, temp.path()));
    }

    #[test]
    fn test_verification_service_should_not_verify_empty() {
        let runner = VerificationRunner::new(VerificationRunnerConfig::default());
        let temp = tempfile::TempDir::new().unwrap();

        assert!(!runner.should_verify(None, temp.path()));
    }

    #[test]
    fn test_verification_service_format_failure() {
        let runner = VerificationRunner::new(VerificationRunnerConfig::default());

        let result = VerificationResult::failed(VerificationCheckType::UnitTest, vec![], 1000);

        let message = runner.format_failure(&result, 1, 3);
        assert!(message.contains("Verification Failed"));
        assert!(message.contains("Retry: 1/3"));
    }

    #[tokio::test]
    async fn test_verification_service_verify() {
        let runner = VerificationRunner::new(VerificationRunnerConfig::default());
        let temp = tempfile::TempDir::new().unwrap();

        let commands = VerificationCommands {
            unit: Some("echo all tests passed".to_string()),
            ..Default::default()
        };

        let result = runner.verify(Some(&commands), temp.path()).await.unwrap();
        assert!(result.passed);
    }

    #[tokio::test]
    async fn test_verification_service_verify_no_command_error() {
        let config = VerificationRunnerConfig {
            auto_detect: false,
            ..Default::default()
        };
        let runner = VerificationRunner::new(config);
        let temp = tempfile::TempDir::new().unwrap();

        let result = runner.verify(None, temp.path()).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No verification command"));
    }
}
