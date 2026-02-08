//! OSC 133 escape sequence parser for shell command lifecycle detection.
//!
//! Modern terminals use OSC 133 (FinalTerm protocol) to track the shell's
//! command lifecycle. The shell emits markers at each phase:
//!
//! - **133;A** — Prompt start (shell is idle, showing prompt)
//! - **133;B** — Command input start (end of prompt, user can type)
//! - **133;C** — Command execution started
//! - **133;D** — Command finished (with optional exit code)
//!
//! This is the same protocol used by iTerm2, VS Code Terminal, and
//! Windows Terminal for shell integration.
//!
//! # OSC 133 Format
//!
//! ```text
//! ESC ] 133 ; A BEL
//! ESC ] 133 ; B BEL
//! ESC ] 133 ; C BEL
//! ESC ] 133 ; D [; exitcode] BEL
//! ```
//!
//! Where:
//! - ESC = 0x1B
//! - BEL = 0x07 (or ST = ESC \)

// Re-export ShellState from core (shared across crates).
pub use codirigent_core::ShellState;

/// Prefix bytes for OSC 133 sequences: ESC ] 133 ;
const OSC133_PREFIX: &[u8] = b"\x1b]133;";

/// Extract all OSC 133 shell state markers from a byte slice.
///
/// Returns them in order of appearance. Incomplete sequences at the
/// end of the buffer are silently ignored (they'll appear in the next
/// chunk).
///
/// # Example
///
/// ```
/// use codirigent_session::osc133::{extract_osc133_events, ShellState};
///
/// let data = b"\x1b]133;A\x07prompt text\x1b]133;B\x07";
/// let events = extract_osc133_events(data);
/// assert_eq!(events, vec![ShellState::PromptStart, ShellState::CommandInputStart]);
/// ```
pub fn extract_osc133_events(data: &[u8]) -> Vec<ShellState> {
    let mut events = Vec::new();
    let mut search_from = 0;

    while search_from < data.len() {
        let prefix_pos = match find_subsequence(&data[search_from..], OSC133_PREFIX) {
            Some(pos) => search_from + pos,
            None => break,
        };

        let payload_start = prefix_pos + OSC133_PREFIX.len();
        if payload_start >= data.len() {
            break;
        }

        // Find the terminator: BEL (0x07) or ST (ESC \)
        let terminator_pos = match find_osc_terminator(&data[payload_start..]) {
            Some(pos) => payload_start + pos,
            None => break, // Incomplete sequence
        };

        let payload = &data[payload_start..terminator_pos];
        if let Some(state) = parse_osc133_payload(payload) {
            events.push(state);
        }

        search_from = terminator_pos + 1;
    }

    events
}

/// Parse the payload after `ESC ] 133 ;` and before the terminator.
///
/// Expected payloads: `A`, `B`, `C`, `D`, or `D;exitcode`.
fn parse_osc133_payload(payload: &[u8]) -> Option<ShellState> {
    if payload.is_empty() {
        return None;
    }

    match payload[0] {
        b'A' => Some(ShellState::PromptStart),
        b'B' => Some(ShellState::CommandInputStart),
        b'C' => Some(ShellState::CommandExecuted),
        b'D' => {
            let exit_code = if payload.len() > 2 && payload[1] == b';' {
                std::str::from_utf8(&payload[2..])
                    .ok()
                    .and_then(|s| s.trim().parse::<i32>().ok())
            } else {
                None
            };
            Some(ShellState::CommandFinished { exit_code })
        }
        _ => None,
    }
}

/// Find the position of an OSC terminator (BEL or ST).
fn find_osc_terminator(data: &[u8]) -> Option<usize> {
    for (i, &byte) in data.iter().enumerate() {
        if byte == 0x07 {
            return Some(i);
        }
        if byte == 0x1b && i + 1 < data.len() && data[i + 1] == b'\\' {
            return Some(i);
        }
    }
    None
}

/// Find the position of a subsequence within a slice.
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_start() {
        let data = b"\x1b]133;A\x07";
        let events = extract_osc133_events(data);
        assert_eq!(events, vec![ShellState::PromptStart]);
    }

    #[test]
    fn test_command_input_start() {
        let data = b"\x1b]133;B\x07";
        let events = extract_osc133_events(data);
        assert_eq!(events, vec![ShellState::CommandInputStart]);
    }

    #[test]
    fn test_command_executed() {
        let data = b"\x1b]133;C\x07";
        let events = extract_osc133_events(data);
        assert_eq!(events, vec![ShellState::CommandExecuted]);
    }

    #[test]
    fn test_command_finished_no_exit_code() {
        let data = b"\x1b]133;D\x07";
        let events = extract_osc133_events(data);
        assert_eq!(
            events,
            vec![ShellState::CommandFinished { exit_code: None }]
        );
    }

    #[test]
    fn test_command_finished_with_exit_code_zero() {
        let data = b"\x1b]133;D;0\x07";
        let events = extract_osc133_events(data);
        assert_eq!(
            events,
            vec![ShellState::CommandFinished { exit_code: Some(0) }]
        );
    }

    #[test]
    fn test_command_finished_with_nonzero_exit_code() {
        let data = b"\x1b]133;D;1\x07";
        let events = extract_osc133_events(data);
        assert_eq!(
            events,
            vec![ShellState::CommandFinished { exit_code: Some(1) }]
        );
    }

    #[test]
    fn test_command_finished_with_large_exit_code() {
        let data = b"\x1b]133;D;127\x07";
        let events = extract_osc133_events(data);
        assert_eq!(
            events,
            vec![ShellState::CommandFinished {
                exit_code: Some(127)
            }]
        );
    }

    #[test]
    fn test_multiple_events_in_sequence() {
        // Simulates: prompt shown, user typed, command ran, command finished
        let data = b"\x1b]133;A\x07PS> \x1b]133;B\x07ls\r\n\x1b]133;C\x07output\x1b]133;D;0\x07";
        let events = extract_osc133_events(data);
        assert_eq!(
            events,
            vec![
                ShellState::PromptStart,
                ShellState::CommandInputStart,
                ShellState::CommandExecuted,
                ShellState::CommandFinished { exit_code: Some(0) },
            ]
        );
    }

    #[test]
    fn test_prompt_cycle() {
        // D from previous command, then A+B for new prompt
        let data = b"\x1b]133;D;0\x07\x1b]133;A\x07PS> \x1b]133;B\x07";
        let events = extract_osc133_events(data);
        assert_eq!(
            events,
            vec![
                ShellState::CommandFinished { exit_code: Some(0) },
                ShellState::PromptStart,
                ShellState::CommandInputStart,
            ]
        );
    }

    #[test]
    fn test_st_terminator() {
        let data = b"\x1b]133;A\x1b\\";
        let events = extract_osc133_events(data);
        assert_eq!(events, vec![ShellState::PromptStart]);
    }

    #[test]
    fn test_mixed_terminators() {
        let data = b"\x1b]133;A\x07\x1b]133;B\x1b\\";
        let events = extract_osc133_events(data);
        assert_eq!(
            events,
            vec![ShellState::PromptStart, ShellState::CommandInputStart]
        );
    }

    #[test]
    fn test_embedded_in_output() {
        let data = b"some output\x1b]133;D;0\x07\x1b]133;A\x07PS> \x1b]133;B\x07more output";
        let events = extract_osc133_events(data);
        assert_eq!(
            events,
            vec![
                ShellState::CommandFinished { exit_code: Some(0) },
                ShellState::PromptStart,
                ShellState::CommandInputStart,
            ]
        );
    }

    #[test]
    fn test_no_events() {
        let data = b"normal terminal output with no escape sequences";
        let events = extract_osc133_events(data);
        assert!(events.is_empty());
    }

    #[test]
    fn test_empty_data() {
        let events = extract_osc133_events(b"");
        assert!(events.is_empty());
    }

    #[test]
    fn test_incomplete_sequence() {
        // Prefix found but no terminator
        let data = b"\x1b]133;A";
        let events = extract_osc133_events(data);
        assert!(events.is_empty());
    }

    #[test]
    fn test_unknown_marker_ignored() {
        let data = b"\x1b]133;Z\x07";
        let events = extract_osc133_events(data);
        assert!(events.is_empty());
    }

    #[test]
    fn test_invalid_exit_code_treated_as_none() {
        let data = b"\x1b]133;D;abc\x07";
        let events = extract_osc133_events(data);
        assert_eq!(
            events,
            vec![ShellState::CommandFinished { exit_code: None }]
        );
    }

    #[test]
    fn test_negative_exit_code() {
        let data = b"\x1b]133;D;-1\x07";
        let events = extract_osc133_events(data);
        assert_eq!(
            events,
            vec![ShellState::CommandFinished {
                exit_code: Some(-1)
            }]
        );
    }

    #[test]
    fn test_osc7_and_osc133_interleaved() {
        // Real-world scenario: shell emits both OSC 7 and OSC 133
        let data = b"\x1b]133;D;0\x07\x1b]133;A\x07\x1b]7;file:///home/user\x07PS> \x1b]133;B\x07";
        let events = extract_osc133_events(data);
        assert_eq!(
            events,
            vec![
                ShellState::CommandFinished { exit_code: Some(0) },
                ShellState::PromptStart,
                ShellState::CommandInputStart,
            ]
        );
    }
}
