//! Pattern matching for input detection.
//!
//! This module provides pattern matching functionality for detecting when
//! a CLI process is waiting for user input. It uses regex patterns to match
//! common prompts like `[y/n]`, `?`, `>`, password prompts, etc.
//!
//! # Example
//!
//! ```
//! use codirigent_detector::patterns::{compile_patterns, find_matching_pattern, DEFAULT_PATTERNS};
//!
//! let patterns: Vec<String> = DEFAULT_PATTERNS.iter().map(|s| s.to_string()).collect();
//! let compiled = compile_patterns(&patterns);
//!
//! // Check if output matches any pattern
//! if let Some(matched) = find_matching_pattern(&compiled, "Continue? [y/n] ") {
//!     println!("Matched pattern: {}", matched);
//! }
//! ```

use regex::Regex;
use tracing::warn;

/// Default patterns for detecting input prompts.
///
/// These patterns cover common interactive prompts from CLI tools:
/// - Yes/No confirmations: `[y/n]`, `[Y/n]`, `[yes/no]`, `(y/N)`
/// - Question prompts: `? ` at end of line
/// - Shell/REPL prompts: `> ` at end of line
/// - Press Enter prompts
/// - Continue prompts
/// - Password prompts
pub const DEFAULT_PATTERNS: &[&str] = &[
    r"\[y/n\]",
    r"\[Y/n\]",
    r"\[yes/no\]",
    r"\? $",
    r"> $",
    r"Press Enter",
    r"Continue\?",
    r"\(y/N\)",
    r"password:",
    r"Password:",
];

/// Compile a list of pattern strings into regex objects.
///
/// Invalid patterns are skipped with a warning log message.
///
/// # Arguments
///
/// * `patterns` - A slice of regex pattern strings
///
/// # Returns
///
/// A vector of compiled Regex objects (invalid patterns are omitted).
///
/// # Example
///
/// ```
/// use codirigent_detector::patterns::compile_patterns;
///
/// let patterns = vec![r"\[y/n\]".to_string(), r"> $".to_string()];
/// let compiled = compile_patterns(&patterns);
/// assert_eq!(compiled.len(), 2);
/// ```
pub fn compile_patterns(patterns: &[String]) -> Vec<Regex> {
    patterns
        .iter()
        .filter_map(|p| match Regex::new(p) {
            Ok(re) => Some(re),
            Err(e) => {
                warn!(pattern = %p, error = %e, "Failed to compile pattern");
                None
            }
        })
        .collect()
}

/// Check if any pattern matches the output and return the matching pattern string.
///
/// For efficiency, only the last few lines of output are checked, as input
/// prompts typically appear at the end of the output.
///
/// # Arguments
///
/// * `patterns` - A slice of compiled Regex patterns
/// * `output` - The output text to check against
///
/// # Returns
///
/// The pattern string that matched, or `None` if no pattern matched.
///
/// # Example
///
/// ```
/// use codirigent_detector::patterns::{compile_patterns, find_matching_pattern};
///
/// let patterns = compile_patterns(&vec![r"\[y/n\]".to_string()]);
/// let result = find_matching_pattern(&patterns, "Install package? [y/n]");
/// assert!(result.is_some());
/// ```
/// Default number of recent lines to check for pattern matching.
///
/// Only the most recent lines are checked for efficiency, as input
/// prompts typically appear at the end of the output.
///
/// # Example
///
/// ```
/// use codirigent_detector::patterns::DEFAULT_RECENT_LINES_TO_CHECK;
///
/// assert_eq!(DEFAULT_RECENT_LINES_TO_CHECK, 5);
/// ```
pub const DEFAULT_RECENT_LINES_TO_CHECK: usize = 5;

/// Check if any pattern matches the output and return the matching pattern string.
///
/// This is a convenience wrapper around [`find_matching_pattern_with_limit`] that
/// uses the default number of recent lines to check ([`DEFAULT_RECENT_LINES_TO_CHECK`]).
///
/// # Arguments
///
/// * `patterns` - A slice of compiled Regex patterns
/// * `output` - The output text to check against
///
/// # Returns
///
/// The pattern string that matched, or `None` if no pattern matched.
pub fn find_matching_pattern(patterns: &[Regex], output: &str) -> Option<String> {
    find_matching_pattern_with_limit(patterns, output, DEFAULT_RECENT_LINES_TO_CHECK)
}

/// Check if any pattern matches the output and return the matching pattern string.
///
/// For efficiency, only the last `recent_lines` lines of output are checked, as input
/// prompts typically appear at the end of the output.
///
/// # Arguments
///
/// * `patterns` - A slice of compiled Regex patterns
/// * `output` - The output text to check against
/// * `recent_lines` - Number of recent lines to check
///
/// # Returns
///
/// The pattern string that matched, or `None` if no pattern matched.
pub fn find_matching_pattern_with_limit(
    patterns: &[Regex],
    output: &str,
    recent_lines: usize,
) -> Option<String> {
    // Only check the last few lines for efficiency
    // Count the lines and find the starting point
    let line_count = output.lines().count();
    let skip_count = line_count.saturating_sub(recent_lines);
    let recent_output: String = output
        .lines()
        .skip(skip_count)
        .collect::<Vec<_>>()
        .join("\n");

    for pattern in patterns {
        if pattern.is_match(&recent_output) {
            return Some(pattern.as_str().to_string());
        }
    }
    None
}

/// Check if any pattern matches the output (boolean version).
///
/// This is a convenience function when you only need to know if there's a match,
/// without needing to know which pattern matched.
///
/// # Arguments
///
/// * `patterns` - A slice of compiled Regex patterns
/// * `output` - The output text to check against
///
/// # Returns
///
/// `true` if any pattern matches, `false` otherwise.
///
/// # Example
///
/// ```
/// use codirigent_detector::patterns::{compile_patterns, matches_any_pattern};
///
/// let patterns = compile_patterns(&vec![r"\[y/n\]".to_string()]);
/// assert!(matches_any_pattern(&patterns, "Continue? [y/n]"));
/// assert!(!matches_any_pattern(&patterns, "Hello world"));
/// ```
pub fn matches_any_pattern(patterns: &[Regex], output: &str) -> bool {
    find_matching_pattern(patterns, output).is_some()
}

/// Get the default patterns as a vector of strings.
///
/// This is useful for initializing pattern configurations.
///
/// # Returns
///
/// A vector containing all default pattern strings.
///
/// # Example
///
/// ```
/// use codirigent_detector::patterns::get_default_patterns;
///
/// let patterns = get_default_patterns();
/// assert!(!patterns.is_empty());
/// ```
pub fn get_default_patterns() -> Vec<String> {
    DEFAULT_PATTERNS.iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_patterns_not_empty() {
        assert!(!DEFAULT_PATTERNS.is_empty());
    }

    #[test]
    fn test_default_patterns_count() {
        assert_eq!(DEFAULT_PATTERNS.len(), 10);
    }

    #[test]
    fn test_compile_patterns_valid() {
        let patterns = vec![r"\[y/n\]".to_string(), r"> $".to_string()];
        let compiled = compile_patterns(&patterns);
        assert_eq!(compiled.len(), 2);
    }

    #[test]
    fn test_compile_patterns_empty() {
        let patterns: Vec<String> = vec![];
        let compiled = compile_patterns(&patterns);
        assert!(compiled.is_empty());
    }

    #[test]
    fn test_compile_patterns_invalid_skipped() {
        let patterns = vec![
            r"[invalid".to_string(), // Invalid regex (unclosed bracket)
            r"\[y/n\]".to_string(),  // Valid
        ];
        let compiled = compile_patterns(&patterns);
        assert_eq!(compiled.len(), 1);
    }

    #[test]
    fn test_compile_patterns_all_invalid() {
        let patterns = vec![r"[invalid".to_string(), r"(unclosed".to_string()];
        let compiled = compile_patterns(&patterns);
        assert!(compiled.is_empty());
    }

    #[test]
    fn test_find_matching_pattern_yn() {
        let patterns = compile_patterns(&[r"\[y/n\]".to_string()]);
        let result = find_matching_pattern(&patterns, "Continue? [y/n]");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), r"\[y/n\]");
    }

    #[test]
    fn test_find_matching_pattern_yn_upper() {
        let patterns = compile_patterns(&[r"\[Y/n\]".to_string()]);
        let result = find_matching_pattern(&patterns, "Install? [Y/n]");
        assert!(result.is_some());
    }

    #[test]
    fn test_find_matching_pattern_yes_no() {
        let patterns = compile_patterns(&[r"\[yes/no\]".to_string()]);
        let result = find_matching_pattern(&patterns, "Proceed? [yes/no]");
        assert!(result.is_some());
    }

    #[test]
    fn test_find_matching_pattern_question_mark() {
        let patterns = compile_patterns(&[r"\? $".to_string()]);
        let result = find_matching_pattern(&patterns, "What do you want? ");
        assert!(result.is_some());
    }

    #[test]
    fn test_find_matching_pattern_prompt_angle() {
        let patterns = compile_patterns(&[r"> $".to_string()]);
        let result = find_matching_pattern(&patterns, "Enter command> ");
        assert!(result.is_some());
    }

    #[test]
    fn test_find_matching_pattern_press_enter() {
        let patterns = compile_patterns(&[r"Press Enter".to_string()]);
        let result = find_matching_pattern(&patterns, "Press Enter to continue...");
        assert!(result.is_some());
    }

    #[test]
    fn test_find_matching_pattern_continue() {
        let patterns = compile_patterns(&[r"Continue\?".to_string()]);
        let result = find_matching_pattern(&patterns, "Continue?");
        assert!(result.is_some());
    }

    #[test]
    fn test_find_matching_pattern_y_n_paren() {
        let patterns = compile_patterns(&[r"\(y/N\)".to_string()]);
        let result = find_matching_pattern(&patterns, "Delete file? (y/N)");
        assert!(result.is_some());
    }

    #[test]
    fn test_find_matching_pattern_password_lower() {
        let patterns = compile_patterns(&[r"password:".to_string()]);
        let result = find_matching_pattern(&patterns, "Enter password:");
        assert!(result.is_some());
    }

    #[test]
    fn test_find_matching_pattern_password_upper() {
        let patterns = compile_patterns(&[r"Password:".to_string()]);
        let result = find_matching_pattern(&patterns, "Password:");
        assert!(result.is_some());
    }

    #[test]
    fn test_find_matching_pattern_no_match() {
        let patterns = compile_patterns(&[r"\[y/n\]".to_string(), r"\? $".to_string()]);
        let result = find_matching_pattern(&patterns, "Hello world");
        assert!(result.is_none());
    }

    #[test]
    fn test_find_matching_pattern_empty_patterns() {
        let patterns: Vec<Regex> = vec![];
        let result = find_matching_pattern(&patterns, "Continue? [y/n]");
        assert!(result.is_none());
    }

    #[test]
    fn test_find_matching_pattern_empty_output() {
        let patterns = compile_patterns(&[r"\[y/n\]".to_string()]);
        let result = find_matching_pattern(&patterns, "");
        assert!(result.is_none());
    }

    #[test]
    fn test_find_matching_pattern_multiline() {
        let patterns = compile_patterns(&[r"\[y/n\]".to_string()]);
        let output = "Some output\nMore output\nConfirm? [y/n]";
        let result = find_matching_pattern(&patterns, output);
        assert!(result.is_some());
    }

    #[test]
    fn test_find_matching_pattern_returns_first_match() {
        let patterns = compile_patterns(&[r"\[y/n\]".to_string(), r"password:".to_string()]);
        let result = find_matching_pattern(&patterns, "password: [y/n]");
        // Should return the first matching pattern
        assert!(result.is_some());
        assert_eq!(result.unwrap(), r"\[y/n\]");
    }

    #[test]
    fn test_find_matching_pattern_only_recent_lines() {
        let patterns = compile_patterns(&[r"\[y/n\]".to_string()]);
        // Pattern appears in old lines but not in recent 5 lines
        let output = "[y/n]\nline1\nline2\nline3\nline4\nline5\nline6";
        let result = find_matching_pattern(&patterns, output);
        // Should not match because [y/n] is not in the last 5 lines
        assert!(result.is_none());
    }

    #[test]
    fn test_matches_any_pattern_true() {
        let patterns = compile_patterns(&[r"\[y/n\]".to_string()]);
        assert!(matches_any_pattern(&patterns, "Continue? [y/n]"));
    }

    #[test]
    fn test_matches_any_pattern_false() {
        let patterns = compile_patterns(&[r"\[y/n\]".to_string()]);
        assert!(!matches_any_pattern(&patterns, "Hello world"));
    }

    #[test]
    fn test_get_default_patterns() {
        let patterns = get_default_patterns();
        assert_eq!(patterns.len(), DEFAULT_PATTERNS.len());
    }

    #[test]
    fn test_get_default_patterns_compilable() {
        let patterns = get_default_patterns();
        let compiled = compile_patterns(&patterns);
        // All default patterns should be valid regex
        assert_eq!(compiled.len(), patterns.len());
    }

    #[test]
    fn test_default_patterns_all_valid_regex() {
        for pattern in DEFAULT_PATTERNS {
            let result = Regex::new(pattern);
            assert!(result.is_ok(), "Pattern '{}' is not valid regex", pattern);
        }
    }

    #[test]
    fn test_compile_patterns_preserves_order() {
        let patterns = vec![
            r"first".to_string(),
            r"second".to_string(),
            r"third".to_string(),
        ];
        let compiled = compile_patterns(&patterns);
        assert_eq!(compiled[0].as_str(), "first");
        assert_eq!(compiled[1].as_str(), "second");
        assert_eq!(compiled[2].as_str(), "third");
    }

    #[test]
    fn test_default_recent_lines_to_check() {
        assert_eq!(DEFAULT_RECENT_LINES_TO_CHECK, 5);
    }

    #[test]
    fn test_find_matching_pattern_with_limit_custom() {
        let patterns = compile_patterns(&[r"\[y/n\]".to_string()]);
        // Pattern appears in third line from end
        let output = "line1\nline2\n[y/n]\nline4\nline5";

        // With limit 3, should find it
        let result = find_matching_pattern_with_limit(&patterns, output, 3);
        assert!(result.is_some());

        // With limit 2, should not find it (only checks last 2 lines)
        let result = find_matching_pattern_with_limit(&patterns, output, 2);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_matching_pattern_with_limit_zero() {
        let patterns = compile_patterns(&[r"\[y/n\]".to_string()]);
        let output = "[y/n]";
        // With limit 0, checks 0 lines, so no match
        let result = find_matching_pattern_with_limit(&patterns, output, 0);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_matching_pattern_with_limit_large() {
        let patterns = compile_patterns(&[r"\[y/n\]".to_string()]);
        let output = "[y/n]\nline2";
        // With very large limit, should still work (checks all lines)
        let result = find_matching_pattern_with_limit(&patterns, output, 100);
        assert!(result.is_some());
    }
}
