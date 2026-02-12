//! CLI type detection from terminal output.
//!
//! Provides byte-pattern matching to identify which AI coding CLI
//! is running in a terminal session based on its output banners.

use codirigent_core::CliType;

/// Detect CLI type from terminal output by scanning for CLI-specific banners.
///
/// Scans the first 2KB of output for CLI identification strings:
/// - Claude Code: "claude cod", "claude>", or box-drawing banner
/// - Gemini CLI: "gemini cli", "gemini>"
/// - Codex CLI: "codex", "codex>"
///
/// Returns None if no known CLI is detected (GenericShell).
pub fn detect_cli_from_output(data: &[u8]) -> Option<CliType> {
    // Only scan a reasonable prefix (first 2KB) to avoid scanning large outputs
    let scan_len = data.len().min(2048);
    let scan = &data[..scan_len];

    // Convert to lowercase for case-insensitive matching
    let lower: Vec<u8> = scan.iter().map(|b| b.to_ascii_lowercase()).collect();

    if lower.windows(10).any(|w| w == b"claude cod")
        || lower.windows(7).any(|w| w == b"claude>")
        || lower
            .windows(15)
            .any(|w| w == "\u{256d}\u{2500} claude code".as_bytes())
    {
        return Some(CliType::ClaudeCode);
    }
    if lower.windows(10).any(|w| w == b"gemini cli")
        || lower.windows(7).any(|w| w == b"gemini>")
    {
        return Some(CliType::GeminiCli);
    }
    if lower.windows(5).any(|w| w == b"codex") || lower.windows(6).any(|w| w == b"codex>") {
        return Some(CliType::CodexCli);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_claude_code() {
        let output = b"Welcome to claude code v1.0\n";
        assert_eq!(detect_cli_from_output(output), Some(CliType::ClaudeCode));

        let output2 = b"claude> ready\n";
        assert_eq!(detect_cli_from_output(output2), Some(CliType::ClaudeCode));
    }

    #[test]
    fn test_detect_gemini() {
        let output = b"gemini cli initialized\n";
        assert_eq!(detect_cli_from_output(output), Some(CliType::GeminiCli));
    }

    #[test]
    fn test_detect_codex() {
        let output = b"codex ready\n";
        assert_eq!(detect_cli_from_output(output), Some(CliType::CodexCli));
    }

    #[test]
    fn test_detect_none() {
        let output = b"bash-5.0$ \n";
        assert_eq!(detect_cli_from_output(output), None);
    }
}
