//! Output parsing for different test frameworks.
//!
//! This module provides parsers for extracting structured test results from
//! command output. Each parser is tailored to a specific test framework's
//! output format.
//!
//! ## Available Parsers
//!
//! - [`GenericParser`] - Works with most test frameworks based on exit code
//! - [`CargoTestParser`] - Parses Rust's `cargo test` output
//! - [`JestParser`] - Parses Jest/npm test output
//! - [`PytestParser`] - Parses pytest output
//!
//! ## Example
//!
//! ```
//! use codirigent_verification::{CargoTestParser, OutputParser};
//! use codirigent_core::verification::VerificationCheckType;
//!
//! let parser = CargoTestParser::new();
//! let result = parser.parse(
//!     VerificationCheckType::UnitTest,
//!     0,
//!     "test result: ok. 23 passed; 0 failed; 0 ignored",
//!     "",
//!     1500,
//! );
//!
//! assert!(result.passed);
//! assert_eq!(result.passed_count, Some(23));
//! ```

use codirigent_core::verification::{
    VerificationCheckType, VerificationFailure, VerificationResult,
};
use regex::Regex;

/// Parse test output into structured results.
///
/// Implementations analyze command output and extract information about
/// test passes, failures, and counts.
pub trait OutputParser: Send + Sync {
    /// Parse command output into a verification result.
    ///
    /// # Arguments
    ///
    /// * `check_type` - The type of verification check performed
    /// * `exit_code` - Exit code of the command
    /// * `stdout` - Standard output from the command
    /// * `stderr` - Standard error from the command
    /// * `duration_ms` - Execution duration in milliseconds
    ///
    /// # Returns
    ///
    /// A structured verification result with pass/fail status and details.
    fn parse(
        &self,
        check_type: VerificationCheckType,
        exit_code: i32,
        stdout: &str,
        stderr: &str,
        duration_ms: u64,
    ) -> VerificationResult;
}

/// Generic parser that works with most test frameworks.
///
/// Determines pass/fail based solely on exit code (0 = pass, non-zero = fail).
/// Use this parser when a more specific parser is not available.
#[derive(Debug, Default, Clone)]
pub struct GenericParser;

impl GenericParser {
    /// Create a new generic parser.
    pub fn new() -> Self {
        Self
    }
}

impl OutputParser for GenericParser {
    fn parse(
        &self,
        check_type: VerificationCheckType,
        exit_code: i32,
        stdout: &str,
        stderr: &str,
        duration_ms: u64,
    ) -> VerificationResult {
        let passed = exit_code == 0;
        let combined = format!("{}\n{}", stdout, stderr);

        VerificationResult {
            check_type,
            passed,
            passed_count: None,
            total_count: None,
            failures: if passed {
                vec![]
            } else {
                vec![VerificationFailure {
                    name: "Command failed".to_string(),
                    file: None,
                    line: None,
                    expected: None,
                    actual: None,
                    message: combined.clone(),
                }]
            },
            duration_ms,
            raw_output: Some(combined),
        }
    }
}

/// Parser for cargo test output.
///
/// Extracts test counts and failure names from Rust's `cargo test` output.
///
/// ## Parsed Information
///
/// - Total tests passed/failed from "test result:" line
/// - Individual test failure names from stdout sections
#[derive(Debug)]
pub struct CargoTestParser {
    /// Pattern to match test result summary line.
    test_result_pattern: Regex,
    /// Pattern to match test failure names.
    failure_pattern: Regex,
}

impl Default for CargoTestParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for CargoTestParser {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl CargoTestParser {
    /// Create a new cargo test parser.
    ///
    /// Compiles regex patterns for parsing cargo test output.
    pub fn new() -> Self {
        Self {
            // Matches: "test result: ok. 23 passed; 0 failed; 0 ignored"
            test_result_pattern: Regex::new(
                r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored",
            )
            .expect("Invalid test result regex"),
            // Matches: "---- tests::test_name stdout ----"
            failure_pattern: Regex::new(r"---- ([\w:]+) stdout ----")
                .expect("Invalid failure regex"),
        }
    }
}

impl OutputParser for CargoTestParser {
    fn parse(
        &self,
        check_type: VerificationCheckType,
        exit_code: i32,
        stdout: &str,
        stderr: &str,
        duration_ms: u64,
    ) -> VerificationResult {
        let combined = format!("{}\n{}", stdout, stderr);
        let mut passed_count = None;
        let mut total_count = None;
        let mut failures = Vec::new();

        // Parse test results from the summary line
        if let Some(caps) = self.test_result_pattern.captures(&combined) {
            let passed: u32 = caps
                .get(2)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            let failed: u32 = caps
                .get(3)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            passed_count = Some(passed);
            total_count = Some(passed + failed);
        }

        // Extract individual failure names from stdout sections
        for caps in self.failure_pattern.captures_iter(&combined) {
            if let Some(name) = caps.get(1) {
                failures.push(VerificationFailure {
                    name: name.as_str().to_string(),
                    file: None,
                    line: None,
                    expected: None,
                    actual: None,
                    message: "See full output for details".to_string(),
                });
            }
        }

        VerificationResult {
            check_type,
            passed: exit_code == 0,
            passed_count,
            total_count,
            failures,
            duration_ms,
            raw_output: Some(combined),
        }
    }
}

/// Parser for Jest/npm test output.
///
/// Extracts test counts from Jest's test summary output.
///
/// ## Parsed Information
///
/// - Tests passed/failed/total from "Tests:" summary line
/// - Failure details including file location, expected/actual values
#[derive(Debug)]
pub struct JestParser {
    /// Pattern to match Jest test summary.
    summary_pattern: Regex,
    /// Pattern to extract file location from stack trace.
    /// Example: "at Object.<anonymous> (src/test.js:15:17)"
    location_pattern: Regex,
    /// Pattern to extract expected value from assertion.
    expected_pattern: Regex,
    /// Pattern to extract received/actual value from assertion.
    received_pattern: Regex,
}

impl Default for JestParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for JestParser {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl JestParser {
    /// Create a new Jest parser.
    ///
    /// Compiles regex patterns for parsing Jest output. All patterns are
    /// compiled once at construction time to avoid repeated compilation
    /// during parsing.
    pub fn new() -> Self {
        Self {
            // Matches: "Tests: 2 failed, 23 passed, 25 total"
            // Also matches: "Tests: 23 passed, 23 total" (no failures)
            summary_pattern: Regex::new(
                r"Tests:\s*(?:(\d+)\s*failed,\s*)?(\d+)\s*passed(?:,\s*\d+\s*skipped)?,\s*(\d+)\s*total",
            )
            .expect("Invalid Jest summary regex"),
            // Pattern to extract file location from stack trace
            location_pattern: Regex::new(
                r"at\s+(?:Object\.<anonymous>|[\w.]+)\s+\(([^:]+):(\d+):\d+\)",
            )
            .expect("Invalid Jest location regex"),
            // Pattern to extract expected value
            expected_pattern: Regex::new(r"Expected:\s*(.+)")
                .expect("Invalid Jest expected regex"),
            // Pattern to extract received/actual value
            received_pattern: Regex::new(r"Received:\s*(.+)")
                .expect("Invalid Jest received regex"),
        }
    }

    /// Parse Jest failure blocks from output.
    ///
    /// Jest failures are marked with "●" and include test name, error message,
    /// expected/actual values, and stack trace with file location. Uses the
    /// pre-compiled regex patterns from the struct for efficient parsing.
    fn parse_failures(&self, output: &str) -> Vec<VerificationFailure> {
        let mut failures = Vec::new();

        // Split output by the failure marker and iterate directly (skip the collect)
        for part in output.split('●').skip(1) {
            // First line is the test name
            let lines: Vec<&str> = part.lines().collect();
            if lines.is_empty() {
                continue;
            }

            let name = lines[0].trim().to_string();
            if name.is_empty() {
                continue;
            }

            let mut file = None;
            let mut line = None;
            let mut expected = None;
            let mut actual = None;

            // Extract file and line from stack trace (use part directly, no to_string)
            if let Some(caps) = self.location_pattern.captures(part) {
                file = caps.get(1).map(|m| std::path::PathBuf::from(m.as_str()));
                line = caps.get(2).and_then(|m| m.as_str().parse().ok());
            }

            // Extract expected value
            if let Some(caps) = self.expected_pattern.captures(part) {
                expected = caps.get(1).map(|m| m.as_str().trim().to_string());
            }

            // Extract actual/received value
            if let Some(caps) = self.received_pattern.captures(part) {
                actual = caps.get(1).map(|m| m.as_str().trim().to_string());
            }

            // Build the message from the first few non-empty lines after the name
            let message = lines
                .iter()
                .skip(1)
                .take(5)
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            let message = if message.is_empty() {
                "Test failed".to_string()
            } else {
                message
            };

            failures.push(VerificationFailure {
                name,
                file,
                line,
                expected,
                actual,
                message,
            });
        }

        failures
    }
}

impl OutputParser for JestParser {
    fn parse(
        &self,
        check_type: VerificationCheckType,
        exit_code: i32,
        stdout: &str,
        stderr: &str,
        duration_ms: u64,
    ) -> VerificationResult {
        let combined = format!("{}\n{}", stdout, stderr);
        let mut passed_count = None;
        let mut total_count = None;

        if let Some(caps) = self.summary_pattern.captures(&combined) {
            let passed: u32 = caps
                .get(2)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            let total: u32 = caps
                .get(3)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            passed_count = Some(passed);
            total_count = Some(total);
        }

        // Parse failure details if tests failed
        let failures = if exit_code != 0 {
            self.parse_failures(&combined)
        } else {
            vec![]
        };

        VerificationResult {
            check_type,
            passed: exit_code == 0,
            passed_count,
            total_count,
            failures,
            duration_ms,
            raw_output: Some(combined),
        }
    }
}

/// Parser for pytest output.
///
/// Currently delegates to the generic parser since pytest's exit code
/// is sufficient for determining pass/fail. Future versions may extract
/// more detailed information.
#[derive(Debug, Default, Clone)]
pub struct PytestParser;

impl PytestParser {
    /// Create a new pytest parser.
    pub fn new() -> Self {
        Self
    }
}

impl OutputParser for PytestParser {
    fn parse(
        &self,
        check_type: VerificationCheckType,
        exit_code: i32,
        stdout: &str,
        stderr: &str,
        duration_ms: u64,
    ) -> VerificationResult {
        // pytest exit codes: 0 = success, 1 = tests failed, 2+ = error
        GenericParser.parse(check_type, exit_code, stdout, stderr, duration_ms)
    }
}

/// Select the appropriate parser for a given project type and check type.
///
/// This is a convenience function for choosing parsers based on context.
///
/// # Arguments
///
/// * `project_type` - The detected project type (optional)
///
/// # Returns
///
/// A boxed parser suitable for the project type.
pub fn parser_for_project(
    project_type: Option<codirigent_core::ProjectType>,
) -> Box<dyn OutputParser> {
    use codirigent_core::ProjectType;

    match project_type {
        Some(ProjectType::Rust) => Box::new(CargoTestParser::new()),
        Some(ProjectType::NodeJs) => Box::new(JestParser::new()),
        Some(ProjectType::Python) => Box::new(PytestParser::new()),
        _ => Box::new(GenericParser),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // GenericParser tests

    #[test]
    fn test_generic_parser_new() {
        let parser = GenericParser::new();
        // Just verify it can be created
        assert!(std::mem::size_of_val(&parser) == 0);
    }

    #[test]
    fn test_generic_parser_default() {
        let parser = GenericParser;
        assert!(std::mem::size_of_val(&parser) == 0);
    }

    #[test]
    fn test_generic_parser_clone() {
        let parser = GenericParser::new();
        let cloned = parser.clone();
        assert!(std::mem::size_of_val(&cloned) == 0);
    }

    #[test]
    fn test_generic_parser_success() {
        let parser = GenericParser;
        let result = parser.parse(
            VerificationCheckType::UnitTest,
            0,
            "All tests passed",
            "",
            1000,
        );
        assert!(result.passed);
        assert!(result.failures.is_empty());
        assert_eq!(result.duration_ms, 1000);
    }

    #[test]
    fn test_generic_parser_failure() {
        let parser = GenericParser;
        let result = parser.parse(
            VerificationCheckType::UnitTest,
            1,
            "",
            "Error: test failed",
            1000,
        );
        assert!(!result.passed);
        assert!(!result.failures.is_empty());
        assert_eq!(result.failures[0].name, "Command failed");
    }

    #[test]
    fn test_generic_parser_combined_output() {
        let parser = GenericParser;
        let result = parser.parse(VerificationCheckType::Lint, 0, "stdout", "stderr", 500);
        let raw = result.raw_output.unwrap();
        assert!(raw.contains("stdout"));
        assert!(raw.contains("stderr"));
    }

    // CargoTestParser tests

    #[test]
    fn test_cargo_parser_new() {
        let parser = CargoTestParser::new();
        // Verify patterns are compiled
        assert!(parser
            .test_result_pattern
            .is_match("test result: ok. 1 passed; 0 failed; 0 ignored"));
    }

    #[test]
    fn test_cargo_parser_default() {
        let parser = CargoTestParser::default();
        assert!(parser
            .test_result_pattern
            .is_match("test result: ok. 1 passed; 0 failed; 0 ignored"));
    }

    #[test]
    fn test_cargo_parser_clone() {
        let parser = CargoTestParser::new();
        let cloned = parser.clone();
        assert!(cloned
            .test_result_pattern
            .is_match("test result: ok. 1 passed; 0 failed; 0 ignored"));
    }

    #[test]
    fn test_cargo_parser_success() {
        let parser = CargoTestParser::new();
        let stdout = "running 23 tests\ntest result: ok. 23 passed; 0 failed; 0 ignored";
        let result = parser.parse(VerificationCheckType::UnitTest, 0, stdout, "", 2000);

        assert!(result.passed);
        assert_eq!(result.passed_count, Some(23));
        assert_eq!(result.total_count, Some(23));
        assert!(result.failures.is_empty());
    }

    #[test]
    fn test_cargo_parser_failure() {
        let parser = CargoTestParser::new();
        let stdout = r#"
            ---- tests::test_auth stdout ----
            assertion failed
            test result: FAILED. 22 passed; 1 failed; 0 ignored
        "#;
        let result = parser.parse(VerificationCheckType::UnitTest, 1, stdout, "", 2000);

        assert!(!result.passed);
        assert_eq!(result.passed_count, Some(22));
        assert_eq!(result.total_count, Some(23));
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].name, "tests::test_auth");
    }

    #[test]
    fn test_cargo_parser_multiple_failures() {
        let parser = CargoTestParser::new();
        let stdout = r#"
            ---- tests::test_one stdout ----
            failed
            ---- tests::test_two stdout ----
            failed
            test result: FAILED. 20 passed; 2 failed; 0 ignored
        "#;
        let result = parser.parse(VerificationCheckType::UnitTest, 1, stdout, "", 2000);

        assert!(!result.passed);
        assert_eq!(result.failures.len(), 2);
        assert_eq!(result.failures[0].name, "tests::test_one");
        assert_eq!(result.failures[1].name, "tests::test_two");
    }

    #[test]
    fn test_cargo_parser_no_summary() {
        let parser = CargoTestParser::new();
        let stdout = "running tests...";
        let result = parser.parse(VerificationCheckType::UnitTest, 0, stdout, "", 1000);

        assert!(result.passed);
        assert!(result.passed_count.is_none());
        assert!(result.total_count.is_none());
    }

    #[test]
    fn test_cargo_parser_raw_output() {
        let parser = CargoTestParser::new();
        let result = parser.parse(
            VerificationCheckType::UnitTest,
            0,
            "stdout content",
            "stderr content",
            1000,
        );
        let raw = result.raw_output.unwrap();
        assert!(raw.contains("stdout content"));
        assert!(raw.contains("stderr content"));
    }

    // JestParser tests

    #[test]
    fn test_jest_parser_new() {
        let parser = JestParser::new();
        // Verify pattern is compiled
        assert!(parser
            .summary_pattern
            .is_match("Tests: 10 passed, 10 total"));
    }

    #[test]
    fn test_jest_parser_default() {
        let parser = JestParser::default();
        assert!(parser
            .summary_pattern
            .is_match("Tests: 10 passed, 10 total"));
    }

    #[test]
    fn test_jest_parser_clone() {
        let parser = JestParser::new();
        let cloned = parser.clone();
        assert!(cloned
            .summary_pattern
            .is_match("Tests: 10 passed, 10 total"));
    }

    #[test]
    fn test_jest_parser_success() {
        let parser = JestParser::new();
        let stdout = "Tests: 23 passed, 23 total";
        let result = parser.parse(VerificationCheckType::UnitTest, 0, stdout, "", 1500);

        assert!(result.passed);
        assert_eq!(result.passed_count, Some(23));
        assert_eq!(result.total_count, Some(23));
    }

    #[test]
    fn test_jest_parser_failure() {
        let parser = JestParser::new();
        let stdout = "Tests: 2 failed, 21 passed, 23 total";
        let result = parser.parse(VerificationCheckType::UnitTest, 1, stdout, "", 1500);

        assert!(!result.passed);
        assert_eq!(result.passed_count, Some(21));
        assert_eq!(result.total_count, Some(23));
    }

    #[test]
    fn test_jest_parser_with_skipped() {
        let parser = JestParser::new();
        let stdout = "Tests: 20 passed, 2 skipped, 22 total";
        let result = parser.parse(VerificationCheckType::UnitTest, 0, stdout, "", 1500);

        assert!(result.passed);
        assert_eq!(result.passed_count, Some(20));
        assert_eq!(result.total_count, Some(22));
    }

    #[test]
    fn test_jest_parser_no_summary() {
        let parser = JestParser::new();
        let stdout = "Running tests...";
        let result = parser.parse(VerificationCheckType::UnitTest, 0, stdout, "", 1000);

        assert!(result.passed);
        assert!(result.passed_count.is_none());
        assert!(result.total_count.is_none());
    }

    // PytestParser tests

    #[test]
    fn test_pytest_parser_new() {
        let parser = PytestParser::new();
        assert!(std::mem::size_of_val(&parser) == 0);
    }

    #[test]
    fn test_pytest_parser_default() {
        let parser = PytestParser;
        assert!(std::mem::size_of_val(&parser) == 0);
    }

    #[test]
    fn test_pytest_parser_clone() {
        let parser = PytestParser::new();
        let cloned = parser.clone();
        assert!(std::mem::size_of_val(&cloned) == 0);
    }

    #[test]
    fn test_pytest_parser_success() {
        let parser = PytestParser::new();
        let result = parser.parse(VerificationCheckType::UnitTest, 0, "5 passed", "", 1000);
        assert!(result.passed);
    }

    #[test]
    fn test_pytest_parser_failure() {
        let parser = PytestParser::new();
        let result = parser.parse(
            VerificationCheckType::UnitTest,
            1,
            "1 failed, 4 passed",
            "",
            1000,
        );
        assert!(!result.passed);
    }

    // parser_for_project tests

    #[test]
    fn test_parser_for_rust() {
        use codirigent_core::ProjectType;
        let parser = parser_for_project(Some(ProjectType::Rust));
        let result = parser.parse(
            VerificationCheckType::UnitTest,
            0,
            "test result: ok. 5 passed; 0 failed; 0 ignored",
            "",
            1000,
        );
        assert_eq!(result.passed_count, Some(5));
    }

    #[test]
    fn test_parser_for_nodejs() {
        use codirigent_core::ProjectType;
        let parser = parser_for_project(Some(ProjectType::NodeJs));
        let result = parser.parse(
            VerificationCheckType::UnitTest,
            0,
            "Tests: 10 passed, 10 total",
            "",
            1000,
        );
        assert_eq!(result.passed_count, Some(10));
    }

    #[test]
    fn test_parser_for_python() {
        use codirigent_core::ProjectType;
        let parser = parser_for_project(Some(ProjectType::Python));
        let result = parser.parse(VerificationCheckType::UnitTest, 0, "5 passed", "", 1000);
        assert!(result.passed);
    }

    #[test]
    fn test_parser_for_unknown() {
        let parser = parser_for_project(None);
        let result = parser.parse(VerificationCheckType::UnitTest, 0, "OK", "", 1000);
        assert!(result.passed);
    }

    #[test]
    fn test_parser_for_go() {
        use codirigent_core::ProjectType;
        let parser = parser_for_project(Some(ProjectType::Go));
        let result = parser.parse(VerificationCheckType::UnitTest, 0, "ok", "", 1000);
        assert!(result.passed);
    }

    // OutputParser trait object safety

    #[test]
    fn test_output_parser_trait_object() {
        let parsers: Vec<Box<dyn OutputParser>> = vec![
            Box::new(GenericParser::new()),
            Box::new(CargoTestParser::new()),
            Box::new(JestParser::new()),
            Box::new(PytestParser::new()),
        ];

        for parser in parsers {
            let result = parser.parse(VerificationCheckType::UnitTest, 0, "ok", "", 100);
            assert!(result.passed);
        }
    }
}
