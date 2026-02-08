//! Failure message formatting for session retry.
//!
//! This module provides formatting utilities for converting verification
//! failures and review feedback into human-readable messages that can be
//! sent back to an AI session for fixing.
//!
//! ## Example
//!
//! ```
//! use codirigent_verification::DefaultFailureFormatter;
//! use codirigent_core::pipeline::FailureMessageFormatter;
//! use codirigent_core::verification::{VerificationStatus, VerificationState};
//! use codirigent_core::{SessionId, TaskId};
//!
//! let formatter = DefaultFailureFormatter::new();
//! let feedback = formatter.format_review_feedback("Please add error handling");
//! assert!(feedback.contains("Changes Requested"));
//! ```

use codirigent_core::pipeline::FailureMessageFormatter;
use codirigent_core::verification::{VerificationCheckType, VerificationStatus};

/// Default failure message formatter.
///
/// Produces markdown-formatted messages suitable for display to
/// AI sessions that need to fix verification failures or address
/// review feedback.
///
/// # Example
///
/// ```
/// use codirigent_verification::DefaultFailureFormatter;
/// use codirigent_core::pipeline::FailureMessageFormatter;
///
/// let formatter = DefaultFailureFormatter::new();
/// let message = formatter.format_review_feedback("Add tests for edge cases");
/// assert!(message.contains("Add tests"));
/// ```
#[derive(Debug, Default, Clone)]
pub struct DefaultFailureFormatter;

impl DefaultFailureFormatter {
    /// Create a new default failure formatter.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::DefaultFailureFormatter;
    ///
    /// let formatter = DefaultFailureFormatter::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Get the display name for a check type.
    fn check_type_name(&self, check_type: VerificationCheckType) -> &'static str {
        match check_type {
            VerificationCheckType::UnitTest => "Unit Tests",
            VerificationCheckType::IntegrationTest => "Integration Tests",
            VerificationCheckType::TypeCheck => "Type Check",
            VerificationCheckType::Lint => "Lint",
            VerificationCheckType::Format => "Format Check",
            VerificationCheckType::Custom => "Custom Check",
        }
    }
}

impl FailureMessageFormatter for DefaultFailureFormatter {
    fn format_verification_failure(&self, status: &VerificationStatus) -> String {
        let mut output = String::new();

        output.push_str("## Verification Failed\n\n");

        // Summary of test results
        let total_passed: u32 = status.results.iter().filter_map(|r| r.passed_count).sum();
        let total_failed: u32 = status
            .results
            .iter()
            .filter_map(|r| r.total_count.zip(r.passed_count).map(|(t, p)| t - p))
            .sum();

        if total_passed > 0 || total_failed > 0 {
            output.push_str(&format!(
                "**Test Results:** {} passed, {} failed\n\n",
                total_passed, total_failed
            ));
        }

        // Details per check type
        for result in &status.results {
            if result.passed {
                continue;
            }

            let check_name = self.check_type_name(result.check_type);
            output.push_str(&format!("### {} Failures\n\n", check_name));

            if result.failures.is_empty() {
                // No structured failures, include raw output if available
                if let Some(ref raw) = result.raw_output {
                    if !raw.is_empty() && raw.len() < 2000 {
                        output.push_str("```\n");
                        output.push_str(raw);
                        if !raw.ends_with('\n') {
                            output.push('\n');
                        }
                        output.push_str("```\n\n");
                    }
                }
                continue;
            }

            for (i, failure) in result.failures.iter().enumerate() {
                output.push_str(&format!("**{}. {}**\n", i + 1, failure.name));

                if let Some(ref file) = failure.file {
                    if let Some(line) = failure.line {
                        output.push_str(&format!("File: `{}:{}`\n", file.display(), line));
                    } else {
                        output.push_str(&format!("File: `{}`\n", file.display()));
                    }
                }

                if let (Some(ref expected), Some(ref actual)) = (&failure.expected, &failure.actual)
                {
                    output.push_str("```\n");
                    output.push_str(&format!("Expected: {}\n", expected));
                    output.push_str(&format!("Received: {}\n", actual));
                    output.push_str("```\n");
                }

                if !failure.message.is_empty() && failure.expected.is_none() {
                    output.push_str(&format!("Error: {}\n", failure.message));
                }

                output.push('\n');
            }
        }

        // Retry info
        output.push_str("---\n\n");
        output.push_str("Please fix the above issues and complete the task again.\n\n");
        output.push_str(&format!("*Retry: {}/{}*\n", status.retry_count + 1, 3));

        output
    }

    fn format_review_feedback(&self, feedback: &str) -> String {
        let mut output = String::new();

        output.push_str("## Changes Requested\n\n");
        output.push_str("The reviewer has requested the following changes:\n\n");
        output.push_str(feedback);
        if !feedback.ends_with('\n') {
            output.push('\n');
        }
        output.push_str("\n---\n\n");
        output.push_str("Please address the feedback and complete the task again.\n");

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codirigent_core::verification::{
        VerificationFailure, VerificationResult, VerificationState,
    };
    use codirigent_core::{SessionId, TaskId};
    use std::path::PathBuf;

    fn create_failed_status() -> VerificationStatus {
        VerificationStatus {
            task_id: TaskId("task-001".to_string()),
            session_id: SessionId(1),
            state: VerificationState::Failed,
            retry_count: 1,
            results: vec![VerificationResult {
                check_type: VerificationCheckType::UnitTest,
                passed: false,
                passed_count: Some(21),
                total_count: Some(23),
                failures: vec![VerificationFailure {
                    name: "test_auth_expired_token".to_string(),
                    file: Some(PathBuf::from("src/auth.test.ts")),
                    line: Some(42),
                    expected: Some("401".to_string()),
                    actual: Some("200".to_string()),
                    message: String::new(),
                }],
                duration_ms: 2000,
                raw_output: None,
            }],
            started_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
        }
    }

    fn create_status_with_raw_output() -> VerificationStatus {
        VerificationStatus {
            task_id: TaskId("task-001".to_string()),
            session_id: SessionId(1),
            state: VerificationState::Failed,
            retry_count: 0,
            results: vec![VerificationResult {
                check_type: VerificationCheckType::Lint,
                passed: false,
                passed_count: None,
                total_count: None,
                failures: vec![],
                duration_ms: 500,
                raw_output: Some("error: unused variable `x`".to_string()),
            }],
            started_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
        }
    }

    // Constructor tests

    #[test]
    fn test_new_formatter() {
        let formatter = DefaultFailureFormatter::new();
        // Verify we can use it
        let message = formatter.format_review_feedback("test");
        assert!(!message.is_empty());
    }

    #[test]
    fn test_default_formatter() {
        let formatter = DefaultFailureFormatter;
        let message = formatter.format_review_feedback("test");
        assert!(!message.is_empty());
    }

    #[test]
    fn test_formatter_clone() {
        let formatter = DefaultFailureFormatter::new();
        let cloned = formatter.clone();
        let msg1 = formatter.format_review_feedback("test");
        let msg2 = cloned.format_review_feedback("test");
        assert_eq!(msg1, msg2);
    }

    #[test]
    fn test_formatter_debug() {
        let formatter = DefaultFailureFormatter::new();
        let debug_str = format!("{:?}", formatter);
        assert!(debug_str.contains("DefaultFailureFormatter"));
    }

    // format_verification_failure tests

    #[test]
    fn test_format_verification_failure() {
        let formatter = DefaultFailureFormatter::new();
        let status = create_failed_status();

        let message = formatter.format_verification_failure(&status);

        assert!(message.contains("## Verification Failed"));
        assert!(message.contains("test_auth_expired_token"));
        assert!(message.contains("Expected: 401"));
        assert!(message.contains("Received: 200"));
        assert!(message.contains("Retry: 2/3"));
    }

    #[test]
    fn test_format_verification_failure_with_test_counts() {
        let formatter = DefaultFailureFormatter::new();
        let status = create_failed_status();

        let message = formatter.format_verification_failure(&status);

        assert!(message.contains("21 passed, 2 failed"));
    }

    #[test]
    fn test_format_verification_failure_with_file_location() {
        let formatter = DefaultFailureFormatter::new();
        let status = create_failed_status();

        let message = formatter.format_verification_failure(&status);

        assert!(message.contains("src/auth.test.ts:42"));
    }

    #[test]
    fn test_format_verification_failure_with_raw_output() {
        let formatter = DefaultFailureFormatter::new();
        let status = create_status_with_raw_output();

        let message = formatter.format_verification_failure(&status);

        assert!(message.contains("unused variable"));
    }

    #[test]
    fn test_format_verification_failure_empty_results() {
        let formatter = DefaultFailureFormatter::new();
        let status = VerificationStatus::new(TaskId("task-001".to_string()), SessionId(1));

        let message = formatter.format_verification_failure(&status);

        assert!(message.contains("## Verification Failed"));
        assert!(message.contains("Retry: 1/3"));
    }

    #[test]
    fn test_format_verification_failure_multiple_check_types() {
        let formatter = DefaultFailureFormatter::new();
        let mut status = VerificationStatus::new(TaskId("task-001".to_string()), SessionId(1));
        status.state = VerificationState::Failed;

        // Add unit test failure
        status.results.push(VerificationResult {
            check_type: VerificationCheckType::UnitTest,
            passed: false,
            passed_count: Some(10),
            total_count: Some(11),
            failures: vec![VerificationFailure::new("test_one", "failed")],
            duration_ms: 1000,
            raw_output: None,
        });

        // Add lint failure
        status.results.push(VerificationResult {
            check_type: VerificationCheckType::Lint,
            passed: false,
            passed_count: None,
            total_count: None,
            failures: vec![VerificationFailure::new("unused_var", "unused variable")],
            duration_ms: 100,
            raw_output: None,
        });

        let message = formatter.format_verification_failure(&status);

        assert!(message.contains("Unit Tests Failures"));
        assert!(message.contains("Lint Failures"));
    }

    #[test]
    fn test_format_verification_failure_passed_results_skipped() {
        let formatter = DefaultFailureFormatter::new();
        let mut status = VerificationStatus::new(TaskId("task-001".to_string()), SessionId(1));
        status.state = VerificationState::Failed;

        // Add passed result
        status
            .results
            .push(VerificationResult::passed(VerificationCheckType::Lint, 100));

        // Add failed result
        status.results.push(VerificationResult {
            check_type: VerificationCheckType::UnitTest,
            passed: false,
            passed_count: None,
            total_count: None,
            failures: vec![VerificationFailure::new("test_fail", "assertion failed")],
            duration_ms: 1000,
            raw_output: None,
        });

        let message = formatter.format_verification_failure(&status);

        // Should only contain the failed section
        assert!(!message.contains("Lint Failures"));
        assert!(message.contains("Unit Tests Failures"));
    }

    #[test]
    fn test_format_verification_failure_with_message_only() {
        let formatter = DefaultFailureFormatter::new();
        let mut status = VerificationStatus::new(TaskId("task-001".to_string()), SessionId(1));
        status.state = VerificationState::Failed;

        status.results.push(VerificationResult {
            check_type: VerificationCheckType::UnitTest,
            passed: false,
            passed_count: None,
            total_count: None,
            failures: vec![VerificationFailure::new(
                "test_network",
                "Connection refused",
            )],
            duration_ms: 1000,
            raw_output: None,
        });

        let message = formatter.format_verification_failure(&status);

        assert!(message.contains("Error: Connection refused"));
    }

    // format_review_feedback tests

    #[test]
    fn test_format_review_feedback() {
        let formatter = DefaultFailureFormatter::new();
        let feedback = "Please add error handling for the edge case when user is not found.";

        let message = formatter.format_review_feedback(feedback);

        assert!(message.contains("## Changes Requested"));
        assert!(message.contains("error handling"));
        assert!(message.contains("edge case"));
        assert!(message.contains("Please address the feedback"));
    }

    #[test]
    fn test_format_review_feedback_empty() {
        let formatter = DefaultFailureFormatter::new();
        let message = formatter.format_review_feedback("");

        assert!(message.contains("## Changes Requested"));
        assert!(message.contains("Please address the feedback"));
    }

    #[test]
    fn test_format_review_feedback_multiline() {
        let formatter = DefaultFailureFormatter::new();
        let feedback = "1. Add error handling\n2. Improve performance\n3. Update docs";

        let message = formatter.format_review_feedback(feedback);

        assert!(message.contains("Add error handling"));
        assert!(message.contains("Improve performance"));
        assert!(message.contains("Update docs"));
    }

    #[test]
    fn test_format_review_feedback_with_newline() {
        let formatter = DefaultFailureFormatter::new();
        let feedback = "Fix the bug\n";

        let message = formatter.format_review_feedback(feedback);

        // Should not add extra newline
        assert!(message.contains("Fix the bug\n\n---"));
    }

    // Check type name tests

    #[test]
    fn test_check_type_names() {
        let formatter = DefaultFailureFormatter::new();

        assert_eq!(
            formatter.check_type_name(VerificationCheckType::UnitTest),
            "Unit Tests"
        );
        assert_eq!(
            formatter.check_type_name(VerificationCheckType::IntegrationTest),
            "Integration Tests"
        );
        assert_eq!(
            formatter.check_type_name(VerificationCheckType::TypeCheck),
            "Type Check"
        );
        assert_eq!(
            formatter.check_type_name(VerificationCheckType::Lint),
            "Lint"
        );
        assert_eq!(
            formatter.check_type_name(VerificationCheckType::Format),
            "Format Check"
        );
        assert_eq!(
            formatter.check_type_name(VerificationCheckType::Custom),
            "Custom Check"
        );
    }

    // Trait object tests

    #[test]
    fn test_formatter_as_trait_object() {
        let formatter: Box<dyn FailureMessageFormatter> = Box::new(DefaultFailureFormatter::new());
        let message = formatter.format_review_feedback("test feedback");
        assert!(message.contains("test feedback"));
    }
}
