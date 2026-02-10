//! Claude Code session status reader via JSONL conversation logs.
//!
//! Reads Claude Code's own data files from `~/.claude/projects/` to determine
//! session status with higher fidelity than OSC 133 alone. Inspired by the
//! [c9watch](https://github.com/minchenlee/c9watch) project.
//!
//! This gives direct visibility into Claude's internal state:
//! - Whether it's actively streaming a response
//! - Whether it has a pending tool use awaiting permission
//! - Whether it has finished its turn and is idle

use serde::Deserialize;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{debug, trace};

/// Status derived from Claude Code's JSONL logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeSessionStatus {
    /// Claude is actively working (streaming, tool executing).
    Working,
    /// Claude is waiting for user input (end of turn).
    WaitingForInput,
    /// A tool needs permission approval.
    NeedsPermission {
        /// The tool name that needs approval.
        tool_name: Option<String>,
    },
    /// Could not determine status from JSONL (fall through to other detectors).
    Unknown,
}

/// Reads Claude Code session data to determine status.
pub struct ClaudeSessionReader {
    /// Path to ~/.claude
    claude_home: PathBuf,
}

/// Parsed entry from a Claude Code JSONL conversation log.
///
/// Uses `#[serde(deny_unknown_fields)]` is intentionally NOT set — JSONL entries
/// contain many fields we don't need; lenient deserialization skips them.
#[derive(Debug, Deserialize)]
struct JsonlEntry {
    /// Entry type: "assistant", "human", "system", "progress", etc.
    #[serde(rename = "type", default)]
    entry_type: String,
    /// The message content (for assistant/human entries).
    #[serde(default)]
    message: Option<JsonlMessage>,
    /// ISO 8601 timestamp of this entry.
    #[serde(default)]
    timestamp: Option<String>,
}

/// Message payload within a JSONL entry.
#[derive(Debug, Deserialize)]
struct JsonlMessage {
    /// Role of the message sender.
    #[serde(default)]
    role: String,
    /// Content blocks (text, tool_use, tool_result, etc.)
    #[serde(default)]
    content: Vec<JsonlContent>,
    /// Stop reason: "end_turn", "tool_use", "max_tokens", etc.
    #[serde(default)]
    stop_reason: Option<String>,
}

/// A content block within a message.
#[derive(Debug, Deserialize)]
struct JsonlContent {
    /// Content type: "text", "tool_use", "tool_result".
    #[serde(rename = "type", default)]
    content_type: String,
    /// Tool name (only for tool_use blocks).
    #[serde(default)]
    name: Option<String>,
    /// Tool use ID (for correlating tool_use with tool_result).
    #[serde(default)]
    id: Option<String>,
    /// Tool use ID reference in tool_result blocks.
    #[serde(default)]
    tool_use_id: Option<String>,
}

impl ClaudeSessionReader {
    /// Create a new reader. Returns `None` if `~/.claude` doesn't exist.
    pub fn new() -> Option<Self> {
        let home = dirs::home_dir()?;
        let claude_home = home.join(".claude");
        if !claude_home.is_dir() {
            debug!("~/.claude not found, Claude session reader disabled");
            return None;
        }
        Some(Self { claude_home })
    }

    /// Get the status of a Claude Code session by reading its JSONL log.
    ///
    /// `working_dir` is the session's working directory, used to locate the
    /// project-specific JSONL files under `~/.claude/projects/`.
    pub fn get_status(&mut self, working_dir: &Path) -> ClaudeSessionStatus {
        let Some(session_dir) = self.find_session_dir(working_dir) else {
            trace!(?working_dir, "No Claude session dir found");
            return ClaudeSessionStatus::Unknown;
        };

        let entries = self.read_recent_entries(&session_dir, 20);
        if entries.is_empty() {
            return ClaudeSessionStatus::Unknown;
        }

        self.determine_status(&entries)
    }

    /// Find the Claude Code session directory for a given working directory.
    ///
    /// Claude Code stores project data under `~/.claude/projects/<hashed-path>/`.
    /// The hashed path replaces `/` with `-` and strips the leading separator.
    fn find_session_dir(&self, working_dir: &Path) -> Option<PathBuf> {
        let projects_dir = self.claude_home.join("projects");
        if !projects_dir.is_dir() {
            return None;
        }

        // Claude Code uses the working directory path with slashes replaced by dashes
        // e.g., /Users/foo/project -> -Users-foo-project
        let dir_str = working_dir.to_string_lossy();
        let hashed = dir_str.replace('/', "-");

        let session_dir = projects_dir.join(&hashed);
        session_dir.is_dir().then_some(session_dir)
    }

    /// Read recent entries from the most recent JSONL file in a session directory.
    ///
    /// Uses seek-from-end for efficiency — never reads the entire file.
    fn read_recent_entries(&self, session_dir: &Path, max_entries: usize) -> Vec<JsonlEntry> {
        let Some(jsonl_path) = Self::find_most_recent_jsonl(session_dir) else {
            return Vec::new();
        };
        let Some(tail) = Self::read_file_tail(&jsonl_path, 8192) else {
            return Vec::new();
        };

        // Parse JSONL lines (last N entries)
        let mut entries = Vec::new();
        for line in tail.lines().rev() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<JsonlEntry>(line) {
                Ok(entry) => {
                    entries.push(entry);
                    if entries.len() >= max_entries {
                        break;
                    }
                }
                Err(e) => {
                    trace!(?e, "Skipping unparseable JSONL line");
                }
            }
        }

        // Reverse so entries are in chronological order
        entries.reverse();
        entries
    }

    /// Find the most recently modified .jsonl file in a directory.
    fn find_most_recent_jsonl(dir: &Path) -> Option<PathBuf> {
        let entries = fs::read_dir(dir).ok()?;
        let mut best: Option<(PathBuf, SystemTime)> = None;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                if let Ok(meta) = path.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        if best.as_ref().map_or(true, |(_, t)| mtime > *t) {
                            best = Some((path, mtime));
                        }
                    }
                }
            }
        }

        best.map(|(p, _)| p)
    }

    /// Read the last `max_bytes` of a file as a UTF-8 string.
    fn read_file_tail(path: &Path, max_bytes: u64) -> Option<String> {
        let mut file = fs::File::open(path).ok()?;
        let file_len = file.metadata().ok()?.len();
        let seeked = file_len > max_bytes;

        if seeked {
            file.seek(SeekFrom::End(-(max_bytes as i64))).ok()?;
        }

        let mut buf = String::new();
        file.read_to_string(&mut buf).ok()?;

        // If we seeked into the middle, discard the first partial line
        if seeked {
            if let Some(pos) = buf.find('\n') {
                buf = buf[pos + 1..].to_string();
            }
        }

        Some(buf)
    }

    /// Check whether a timestamp string is within `threshold_secs` of now.
    ///
    /// Returns `None` if the timestamp cannot be parsed.
    fn is_timestamp_recent(timestamp: &str, threshold_secs: i64) -> Option<bool> {
        use chrono::{DateTime, Utc};
        let parsed = timestamp.parse::<DateTime<Utc>>().ok()?;
        let elapsed = Utc::now().signed_duration_since(parsed);
        Some(elapsed.num_seconds() < threshold_secs)
    }

    /// Core status determination algorithm.
    ///
    /// Uses JSONL as the sole source of truth:
    /// 1. Find the last assistant message
    /// 2. If it has a tool_use with no tool_result → NeedsPermission
    ///    (if it's actually auto-approved, the result appears within ~1s
    ///     and the next poll will update the status)
    /// 3. If stop_reason is "end_turn" → WaitingForInput
    /// 4. Otherwise → Unknown (fall through to other detectors)
    fn determine_status(&self, entries: &[JsonlEntry]) -> ClaudeSessionStatus {
        // Walk entries in reverse to find the last meaningful entry
        let mut last_assistant: Option<&JsonlEntry> = None;
        let mut last_assistant_idx: usize = 0;
        let mut tool_results: Vec<String> = Vec::new();

        for (i, entry) in entries.iter().enumerate().rev() {
            // Collect tool_result IDs from human/tool entries
            if let Some(ref msg) = entry.message {
                for content in &msg.content {
                    if content.content_type == "tool_result" {
                        if let Some(ref id) = content.tool_use_id {
                            tool_results.push(id.clone());
                        }
                    }
                }
            }

            // Find the last assistant message
            if last_assistant.is_none()
                && (entry.entry_type == "assistant"
                    || entry
                        .message
                        .as_ref()
                        .is_some_and(|m| m.role == "assistant"))
            {
                last_assistant = Some(entry);
                last_assistant_idx = i;
            }
        }

        let Some(assistant) = last_assistant else {
            return ClaudeSessionStatus::Unknown;
        };
        let Some(msg) = &assistant.message else {
            return ClaudeSessionStatus::Unknown;
        };

        // Check for pending tool_use (no corresponding tool_result)
        for content in &msg.content {
            if content.content_type == "tool_use" {
                let tool_id = content.id.as_deref().unwrap_or("");
                let has_result = tool_results.iter().any(|r| r == tool_id);

                if !has_result {
                    let tool_name = content.name.as_deref().unwrap_or("unknown");
                    return ClaudeSessionStatus::NeedsPermission {
                        tool_name: Some(tool_name.to_string()),
                    };
                }
            }
        }

        // Check stop_reason
        if let Some(ref stop_reason) = msg.stop_reason {
            if stop_reason == "end_turn" {
                return ClaudeSessionStatus::WaitingForInput;
            }
        }

        // Check if there's a human entry after the last assistant (tool results
        // sent back → Claude is processing the next turn).
        let has_human_after_assistant = entries[last_assistant_idx + 1..].iter().any(|e| {
            e.entry_type == "human"
                || e.message.as_ref().is_some_and(|m| m.role == "user")
        });

        if has_human_after_assistant {
            // Tool results were sent back; Claude should be generating.
            // Use the timestamp of the last entry to gauge recency.
            if let Some(ts) = entries.last().and_then(|e| e.timestamp.as_deref()) {
                return match Self::is_timestamp_recent(ts, 15) {
                    Some(true) => ClaudeSessionStatus::Working,
                    Some(false) => ClaudeSessionStatus::WaitingForInput,
                    None => ClaudeSessionStatus::Unknown,
                };
            }
            // No timestamp available — assume working (human just sent input)
            return ClaudeSessionStatus::Working;
        }

        // stop_reason is null, no pending tools, no human after assistant.
        // Claude may still be streaming or may have finished without "end_turn".
        // Use the assistant entry's timestamp to decide.
        if let Some(ts) = assistant.timestamp.as_deref() {
            return match Self::is_timestamp_recent(ts, 10) {
                Some(true) => ClaudeSessionStatus::Working,
                Some(false) => ClaudeSessionStatus::WaitingForInput,
                None => ClaudeSessionStatus::Unknown,
            };
        }

        ClaudeSessionStatus::Unknown
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_assistant_entry(content: Vec<JsonlContent>, stop_reason: Option<&str>) -> JsonlEntry {
        JsonlEntry {
            entry_type: "assistant".to_string(),
            message: Some(JsonlMessage {
                role: "assistant".to_string(),
                content,
                stop_reason: stop_reason.map(|s| s.to_string()),
            }),
            timestamp: None,
        }
    }

    fn make_assistant_entry_ts(
        content: Vec<JsonlContent>,
        stop_reason: Option<&str>,
        timestamp: &str,
    ) -> JsonlEntry {
        JsonlEntry {
            entry_type: "assistant".to_string(),
            message: Some(JsonlMessage {
                role: "assistant".to_string(),
                content,
                stop_reason: stop_reason.map(|s| s.to_string()),
            }),
            timestamp: Some(timestamp.to_string()),
        }
    }

    fn make_tool_use(name: &str, id: &str) -> JsonlContent {
        JsonlContent {
            content_type: "tool_use".to_string(),
            name: Some(name.to_string()),
            id: Some(id.to_string()),
            tool_use_id: None,
        }
    }

    fn make_tool_result(tool_use_id: &str) -> JsonlContent {
        JsonlContent {
            content_type: "tool_result".to_string(),
            name: None,
            id: None,
            tool_use_id: Some(tool_use_id.to_string()),
        }
    }

    fn make_human_entry(content: Vec<JsonlContent>) -> JsonlEntry {
        JsonlEntry {
            entry_type: "human".to_string(),
            message: Some(JsonlMessage {
                role: "user".to_string(),
                content,
                stop_reason: None,
            }),
            timestamp: None,
        }
    }

    fn make_human_entry_ts(content: Vec<JsonlContent>, timestamp: &str) -> JsonlEntry {
        JsonlEntry {
            entry_type: "human".to_string(),
            message: Some(JsonlMessage {
                role: "user".to_string(),
                content,
                stop_reason: None,
            }),
            timestamp: Some(timestamp.to_string()),
        }
    }

    fn test_reader() -> ClaudeSessionReader {
        ClaudeSessionReader {
            claude_home: PathBuf::from("/nonexistent"),
        }
    }

    #[test]
    fn test_end_turn_returns_waiting() {
        let entries = vec![make_assistant_entry(vec![], Some("end_turn"))];
        let reader = test_reader();
        assert_eq!(
            reader.determine_status(&entries),
            ClaudeSessionStatus::WaitingForInput
        );
    }

    #[test]
    fn test_pending_tool_use_needs_permission() {
        // Any pending tool_use without a result = NeedsPermission
        let entries = vec![make_assistant_entry(
            vec![make_tool_use("Bash", "tu_1")],
            None,
        )];
        let reader = test_reader();
        assert_eq!(
            reader.determine_status(&entries),
            ClaudeSessionStatus::NeedsPermission {
                tool_name: Some("Bash".to_string()),
            }
        );
    }

    #[test]
    fn test_pending_read_tool_also_needs_permission() {
        // We don't assume any tool is auto-approved — JSONL is source of truth.
        // If there's no tool_result yet, the tool is pending.
        let entries = vec![make_assistant_entry(
            vec![make_tool_use("Read", "tu_1")],
            None,
        )];
        let reader = test_reader();
        assert_eq!(
            reader.determine_status(&entries),
            ClaudeSessionStatus::NeedsPermission {
                tool_name: Some("Read".to_string()),
            }
        );
    }

    #[test]
    fn test_tool_use_with_result_is_not_pending() {
        let entries = vec![
            make_assistant_entry(vec![make_tool_use("Bash", "tu_3")], None),
            make_human_entry(vec![make_tool_result("tu_3")]),
        ];
        let reader = test_reader();
        // The tool_use has a corresponding result, so it's not pending.
        // Human entry after assistant with no timestamp → Working (assumes processing)
        assert_eq!(
            reader.determine_status(&entries),
            ClaudeSessionStatus::Working
        );
    }

    #[test]
    fn test_empty_entries_returns_unknown() {
        let reader = test_reader();
        assert_eq!(
            reader.determine_status(&[]),
            ClaudeSessionStatus::Unknown
        );
    }

    #[test]
    fn test_find_session_dir() {
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join("projects");
        fs::create_dir_all(&projects_dir).unwrap();

        let reader = ClaudeSessionReader {
            claude_home: tmp.path().to_path_buf(),
        };

        // No matching dir
        assert!(reader
            .find_session_dir(Path::new("/Users/foo/project"))
            .is_none());

        // Create matching dir
        let session_dir = projects_dir.join("-Users-foo-project");
        fs::create_dir_all(&session_dir).unwrap();

        let found = reader.find_session_dir(Path::new("/Users/foo/project"));
        assert_eq!(found, Some(session_dir));
    }

    #[test]
    fn test_read_recent_entries_from_jsonl() {
        let tmp = TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("conversation.jsonl");

        // Write some JSONL entries
        let mut file = fs::File::create(&jsonl_path).unwrap();
        writeln!(
            file,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[],"stop_reason":"end_turn"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"human","message":{{"role":"user","content":[]}}}}"#
        )
        .unwrap();

        let reader = test_reader();

        let entries = reader.read_recent_entries(tmp.path(), 10);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].entry_type, "assistant");
        assert_eq!(entries[1].entry_type, "human");
    }

    #[test]
    fn test_read_file_tail() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.jsonl");

        let mut file = fs::File::create(&path).unwrap();
        for i in 0..100 {
            writeln!(file, "line {i}").unwrap();
        }

        // Read tail should get the last lines
        let tail = ClaudeSessionReader::read_file_tail(&path, 50).unwrap();
        assert!(tail.contains("line 99"));
        // Should not contain very early lines
        assert!(!tail.contains("line 0\n"));
    }

    #[test]
    fn test_find_most_recent_jsonl() {
        let tmp = TempDir::new().unwrap();

        // Create two jsonl files
        let path1 = tmp.path().join("old.jsonl");
        fs::write(&path1, "old").unwrap();

        // Ensure different mtime
        std::thread::sleep(std::time::Duration::from_millis(50));

        let path2 = tmp.path().join("new.jsonl");
        fs::write(&path2, "new").unwrap();

        let found = ClaudeSessionReader::find_most_recent_jsonl(tmp.path()).unwrap();
        assert_eq!(found, path2);
    }

    #[test]
    fn test_null_stop_reason_recent_timestamp_returns_working() {
        // Assistant with no stop_reason but a recent timestamp → Working (still streaming)
        let recent = chrono::Utc::now().to_rfc3339();
        let entries = vec![make_assistant_entry_ts(vec![], None, &recent)];
        let reader = test_reader();
        assert_eq!(
            reader.determine_status(&entries),
            ClaudeSessionStatus::Working
        );
    }

    #[test]
    fn test_null_stop_reason_old_timestamp_returns_waiting() {
        // Assistant with no stop_reason and an old timestamp → WaitingForInput (done)
        let old = (chrono::Utc::now() - chrono::Duration::seconds(30)).to_rfc3339();
        let entries = vec![make_assistant_entry_ts(vec![], None, &old)];
        let reader = test_reader();
        assert_eq!(
            reader.determine_status(&entries),
            ClaudeSessionStatus::WaitingForInput
        );
    }

    #[test]
    fn test_human_after_assistant_recent_returns_working() {
        // Tool result sent back recently → Claude is processing
        let recent = chrono::Utc::now().to_rfc3339();
        let entries = vec![
            make_assistant_entry(vec![make_tool_use("Bash", "tu_5")], None),
            make_human_entry_ts(vec![make_tool_result("tu_5")], &recent),
        ];
        let reader = test_reader();
        assert_eq!(
            reader.determine_status(&entries),
            ClaudeSessionStatus::Working
        );
    }

    #[test]
    fn test_human_after_assistant_old_returns_waiting() {
        // Tool result sent back long ago → Claude is done
        let old = (chrono::Utc::now() - chrono::Duration::seconds(30)).to_rfc3339();
        let entries = vec![
            make_assistant_entry(vec![make_tool_use("Bash", "tu_6")], None),
            make_human_entry_ts(vec![make_tool_result("tu_6")], &old),
        ];
        let reader = test_reader();
        assert_eq!(
            reader.determine_status(&entries),
            ClaudeSessionStatus::WaitingForInput
        );
    }

    #[test]
    fn test_is_timestamp_recent() {
        let now = chrono::Utc::now().to_rfc3339();
        assert_eq!(
            ClaudeSessionReader::is_timestamp_recent(&now, 10),
            Some(true)
        );

        let old = (chrono::Utc::now() - chrono::Duration::seconds(60)).to_rfc3339();
        assert_eq!(
            ClaudeSessionReader::is_timestamp_recent(&old, 10),
            Some(false)
        );

        assert_eq!(
            ClaudeSessionReader::is_timestamp_recent("not-a-timestamp", 10),
            None
        );
    }
}
