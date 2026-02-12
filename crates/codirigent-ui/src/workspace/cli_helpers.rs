//! CLI detection and formatting helpers.
//!
//! This module provides utilities for:
//! - Detecting CLI type from terminal output
//! - Formatting task prompts for different CLI types
//! - Getting CLI-specific clear/reset commands

use tracing::warn;

/// Detect CLI type from terminal output by scanning for CLI-specific banners.
///
/// Delegates to `codirigent_session::detect_cli_from_output` which performs
/// byte pattern matching for known CLI identification strings.
pub(super) fn detect_cli_from_output(data: &[u8]) -> Option<codirigent_core::CliType> {
    codirigent_session::detect_cli_from_output(data)
}

/// Format a task prompt for sending to a session's PTY.
///
/// Collapses multi-line prompts into a single line so newlines aren't
/// interpreted as individual Enter presses by the CLI. The caller is
/// responsible for scheduling a deferred Enter keypress.
///
/// # Arguments
/// * `prompt` - The multi-line task prompt
/// * `cli_type` - The CLI type running in the session
///
/// # Returns
/// Flattened single-line prompt (no trailing newline)
pub(super) fn format_task_input(prompt: &str, cli_type: codirigent_core::CliType) -> String {
    // Collapse the multi-line prompt into a single line so newlines
    // aren't interpreted as individual Enter presses by the CLI.
    let flat: String = prompt
        .lines()
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    match cli_type {
        codirigent_core::CliType::ClaudeCode
        | codirigent_core::CliType::GeminiCli
        | codirigent_core::CliType::CodexCli => {
            // No trailing newline — caller schedules a deferred Enter
            flat
        }
        codirigent_core::CliType::GenericShell => {
            warn!("format_task_input called with GenericShell — this should not happen");
            flat
        }
    }
}

/// Return the CLI-specific command to clear/reset context between tasks.
///
/// Different CLI tools use different commands to start fresh:
/// - Claude Code: `/clear`
/// - Codex CLI: `/new`
/// - Gemini CLI: `/clear`
/// - GenericShell: empty string (no clear command)
pub(super) fn clear_command(cli_type: codirigent_core::CliType) -> String {
    match cli_type {
        codirigent_core::CliType::ClaudeCode => "/clear".to_string(),
        codirigent_core::CliType::CodexCli => "/new".to_string(),
        codirigent_core::CliType::GeminiCli => "/clear".to_string(),
        codirigent_core::CliType::GenericShell => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_cli_delegates_to_session() {
        // Detection logic is now in codirigent_session::cli_output_detection
        // This test verifies the delegation still works
        let output = b"Welcome to claude code v1.0\n";
        assert_eq!(
            detect_cli_from_output(output),
            Some(codirigent_core::CliType::ClaudeCode)
        );
        assert_eq!(detect_cli_from_output(b"bash-5.0$ \n"), None);
    }

    #[test]
    fn test_format_task_input_multiline() {
        let prompt = "Line 1\nLine 2\n\nLine 3";
        let result = format_task_input(prompt, codirigent_core::CliType::ClaudeCode);
        assert_eq!(result, "Line 1 Line 2 Line 3");
    }

    #[test]
    fn test_clear_command() {
        assert_eq!(
            clear_command(codirigent_core::CliType::ClaudeCode),
            "/clear"
        );
        assert_eq!(clear_command(codirigent_core::CliType::CodexCli), "/new");
        assert_eq!(
            clear_command(codirigent_core::CliType::GeminiCli),
            "/clear"
        );
        assert_eq!(clear_command(codirigent_core::CliType::GenericShell), "");
    }
}
