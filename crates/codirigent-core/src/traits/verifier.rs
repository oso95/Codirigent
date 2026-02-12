//! Verifier traits for the Codirigent verification system.
//!
//! This module defines the trait contracts for verification implementations,
//! including the main Verifier trait, detection traits, and formatting traits.
//!
//! ## Traits
//!
//! - [`Verifier`] - Main trait for running verification checks
//! - [`VerificationDetector`] - Auto-detect verification commands for a project
//! - [`FailureFormatter`] - Format verification failures for display
//!
//! ## Project Types
//!
//! - [`ProjectType`] - Known project types for auto-detection

use crate::types::TaskId;
use crate::verification::*;
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

/// Trait for verification gate implementations.
///
/// The verifier is responsible for running all configured verification checks
/// on a task's working directory and managing the retry workflow when checks fail.
///
/// # Async
///
/// This trait uses `async_trait` because verification commands are I/O bound
/// and may take significant time to complete.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to allow sharing across threads.
///
/// # Example Implementation
///
/// ```ignore
/// use codirigent_core::traits::Verifier;
/// use codirigent_core::verification::*;
/// use codirigent_core::types::TaskId;
/// use async_trait::async_trait;
///
/// struct MyVerifier { /* ... */ }
///
/// #[async_trait]
/// impl Verifier for MyVerifier {
///     async fn verify(&self, task_id: &TaskId, working_dir: &Path) -> Result<VerificationStatus> {
///         // Run verification checks
///         todo!()
///     }
///     // ... other methods
/// }
/// ```
#[async_trait]
pub trait Verifier: Send + Sync {
    /// Run all configured verification checks for a task.
    ///
    /// This method executes all verification commands in sequence and
    /// returns the complete verification status.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task being verified
    /// * `working_dir` - Directory to run verification commands in
    ///
    /// # Returns
    ///
    /// The complete verification status with all results.
    async fn verify(&self, task_id: &TaskId, working_dir: &Path) -> Result<VerificationStatus>;

    /// Run a single verification check.
    ///
    /// # Arguments
    ///
    /// * `check_type` - Type of verification to perform
    /// * `command` - Shell command to execute
    /// * `working_dir` - Directory to run the command in
    ///
    /// # Returns
    ///
    /// The result of the verification check.
    async fn run_check(
        &self,
        check_type: VerificationCheckType,
        command: &str,
        working_dir: &Path,
    ) -> Result<VerificationResult>;

    /// Get the current verification status for a task.
    ///
    /// # Returns
    ///
    /// The verification status if verification has been started for this task.
    fn get_status(&self, task_id: &TaskId) -> Option<VerificationStatus>;

    /// Mark verification as skipped for a task.
    ///
    /// This is used when verification is manually bypassed.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task to skip verification for
    fn skip(&mut self, task_id: &TaskId) -> Result<()>;

    /// Retry verification after session fix.
    ///
    /// This is called when a session has attempted to fix verification failures
    /// and verification should be run again.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task to retry verification for
    ///
    /// # Returns
    ///
    /// The new verification status after retry.
    async fn retry(&mut self, task_id: &TaskId) -> Result<VerificationStatus>;
}

/// Trait for auto-detecting verification commands.
///
/// Implementations analyze a project directory to determine the appropriate
/// verification commands based on project configuration files.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to allow sharing across threads.
pub trait VerificationDetector: Send + Sync {
    /// Detect verification commands for a project.
    ///
    /// Analyzes the project directory and returns appropriate commands
    /// for running verification checks.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Root directory of the project
    ///
    /// # Returns
    ///
    /// Detected verification commands for this project.
    fn detect(&self, project_dir: &Path) -> VerificationCommands;

    /// Check if a specific project type is detected.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Root directory of the project
    ///
    /// # Returns
    ///
    /// The detected project type, if recognized.
    fn detect_project_type(&self, project_dir: &Path) -> Option<ProjectType>;
}

/// Known project types for auto-detection.
///
/// These represent common project types that have well-known verification
/// commands and configuration files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectType {
    /// Node.js project (package.json).
    NodeJs,
    /// Rust project (Cargo.toml).
    Rust,
    /// Python project (pyproject.toml or setup.py).
    Python,
    /// Go project (go.mod).
    Go,
    /// Java/Kotlin project (pom.xml or build.gradle).
    Jvm,
    /// Generic project with Makefile.
    Make,
}

impl std::fmt::Display for ProjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectType::NodeJs => write!(f, "Node.js"),
            ProjectType::Rust => write!(f, "Rust"),
            ProjectType::Python => write!(f, "Python"),
            ProjectType::Go => write!(f, "Go"),
            ProjectType::Jvm => write!(f, "JVM"),
            ProjectType::Make => write!(f, "Make"),
        }
    }
}

impl ProjectType {
    /// Get the primary configuration file for this project type.
    pub fn config_file(&self) -> &'static str {
        match self {
            ProjectType::NodeJs => "package.json",
            ProjectType::Rust => "Cargo.toml",
            ProjectType::Python => "pyproject.toml",
            ProjectType::Go => "go.mod",
            ProjectType::Jvm => "pom.xml",
            ProjectType::Make => "Makefile",
        }
    }

    /// Get alternative configuration files for this project type.
    pub fn alt_config_files(&self) -> &'static [&'static str] {
        match self {
            ProjectType::NodeJs => &["package-lock.json", "yarn.lock", "pnpm-lock.yaml"],
            ProjectType::Rust => &["Cargo.lock"],
            ProjectType::Python => &["setup.py", "setup.cfg", "requirements.txt"],
            ProjectType::Go => &["go.sum"],
            ProjectType::Jvm => &["build.gradle", "build.gradle.kts", "settings.gradle"],
            ProjectType::Make => &["GNUmakefile", "makefile"],
        }
    }
}

/// Trait for formatting verification failure messages.
///
/// Implementations convert verification failures into human-readable
/// messages that can be sent back to a session for fixing.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to allow sharing across threads.
pub trait FailureFormatter: Send + Sync {
    /// Format failures into a message to send back to the session.
    ///
    /// The formatted message should be clear and actionable, helping
    /// the AI session understand and fix the failures.
    ///
    /// # Arguments
    ///
    /// * `status` - The verification status containing failures
    ///
    /// # Returns
    ///
    /// A formatted string describing the failures.
    fn format(&self, status: &VerificationStatus) -> String;

    /// Format a single failure for display.
    ///
    /// # Arguments
    ///
    /// * `failure` - The failure to format
    ///
    /// # Returns
    ///
    /// A formatted string describing the failure.
    fn format_failure(&self, failure: &VerificationFailure) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_type_variants() {
        let types = [
            ProjectType::NodeJs,
            ProjectType::Rust,
            ProjectType::Python,
            ProjectType::Go,
            ProjectType::Jvm,
            ProjectType::Make,
        ];
        assert_eq!(types.len(), 6);
    }

    #[test]
    fn test_project_type_display() {
        assert_eq!(format!("{}", ProjectType::NodeJs), "Node.js");
        assert_eq!(format!("{}", ProjectType::Rust), "Rust");
        assert_eq!(format!("{}", ProjectType::Python), "Python");
        assert_eq!(format!("{}", ProjectType::Go), "Go");
        assert_eq!(format!("{}", ProjectType::Jvm), "JVM");
        assert_eq!(format!("{}", ProjectType::Make), "Make");
    }

    #[test]
    fn test_project_type_config_file() {
        assert_eq!(ProjectType::NodeJs.config_file(), "package.json");
        assert_eq!(ProjectType::Rust.config_file(), "Cargo.toml");
        assert_eq!(ProjectType::Python.config_file(), "pyproject.toml");
        assert_eq!(ProjectType::Go.config_file(), "go.mod");
        assert_eq!(ProjectType::Jvm.config_file(), "pom.xml");
        assert_eq!(ProjectType::Make.config_file(), "Makefile");
    }

    #[test]
    fn test_project_type_alt_config_files() {
        let nodejs_alts = ProjectType::NodeJs.alt_config_files();
        assert!(nodejs_alts.contains(&"package-lock.json"));
        assert!(nodejs_alts.contains(&"yarn.lock"));

        let python_alts = ProjectType::Python.alt_config_files();
        assert!(python_alts.contains(&"setup.py"));
        assert!(python_alts.contains(&"requirements.txt"));

        let jvm_alts = ProjectType::Jvm.alt_config_files();
        assert!(jvm_alts.contains(&"build.gradle"));
    }

    #[test]
    fn test_project_type_equality() {
        assert_eq!(ProjectType::Rust, ProjectType::Rust);
        assert_ne!(ProjectType::Rust, ProjectType::Python);
    }

    // Trait object safety tests

    #[test]
    fn test_verification_detector_trait_is_object_safe() {
        // This compiles only if VerificationDetector is object-safe
        fn _takes_detector(_: &dyn VerificationDetector) {}
    }

    #[test]
    fn test_failure_formatter_trait_is_object_safe() {
        // This compiles only if FailureFormatter is object-safe
        fn _takes_formatter(_: &dyn FailureFormatter) {}
    }

    // Note: Verifier trait is NOT object-safe due to async_trait
    // This is expected and acceptable for our use case

    // Mock implementations for testing trait contracts

    struct MockVerificationDetector;

    impl VerificationDetector for MockVerificationDetector {
        fn detect(&self, _project_dir: &Path) -> VerificationCommands {
            VerificationCommands {
                unit: Some("npm test".to_string()),
                integration: None,
                type_check: Some("tsc --noEmit".to_string()),
                lint: Some("npm run lint".to_string()),
                format: None,
                custom: vec![],
            }
        }

        fn detect_project_type(&self, _project_dir: &Path) -> Option<ProjectType> {
            Some(ProjectType::NodeJs)
        }
    }

    #[test]
    fn test_mock_verification_detector() {
        let detector = MockVerificationDetector;
        let commands = detector.detect(Path::new("/test"));
        assert!(commands.has_any());
        assert_eq!(commands.unit, Some("npm test".to_string()));

        let project_type = detector.detect_project_type(Path::new("/test"));
        assert_eq!(project_type, Some(ProjectType::NodeJs));
    }

    struct MockFailureFormatter;

    impl FailureFormatter for MockFailureFormatter {
        fn format(&self, status: &VerificationStatus) -> String {
            let failures = status.all_failures();
            if failures.is_empty() {
                return "No failures".to_string();
            }
            format!("{} failures found", failures.len())
        }

        fn format_failure(&self, failure: &VerificationFailure) -> String {
            format!("{}: {}", failure.name, failure.message)
        }
    }

    #[test]
    fn test_mock_failure_formatter() {
        let formatter = MockFailureFormatter;

        let status =
            VerificationStatus::new(TaskId::from("task-001"), crate::types::SessionId(1));
        assert_eq!(formatter.format(&status), "No failures");

        let failure = VerificationFailure::new("test_auth", "assertion failed");
        assert_eq!(
            formatter.format_failure(&failure),
            "test_auth: assertion failed"
        );
    }

    #[test]
    fn test_mock_failure_formatter_with_failures() {
        let formatter = MockFailureFormatter;

        let mut status =
            VerificationStatus::new(TaskId::from("task-001"), crate::types::SessionId(1));
        let failures = vec![
            VerificationFailure::new("test1", "failed"),
            VerificationFailure::new("test2", "failed"),
        ];
        status.results.push(VerificationResult::failed(
            VerificationCheckType::UnitTest,
            failures,
            100,
        ));

        assert_eq!(formatter.format(&status), "2 failures found");
    }
}
