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
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{debug, trace, warn};

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
    /// Cached settings from ~/.claude/settings.json
    settings_cache: Option<ClaudeSettings>,
    /// Last modification time of settings.json for cache invalidation
    settings_mtime: Option<SystemTime>,
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

/// Parsed ~/.claude/settings.json for permission rules.
#[derive(Debug, Deserialize, Clone)]
struct ClaudeSettings {
    /// Permission rules mapping tool patterns to "allow"/"deny".
    #[serde(default, rename = "permissions")]
    permissions: HashMap<String, String>,
}

/// Tools that are always auto-approved by Claude Code (read-only or safe tools).
const ALWAYS_APPROVED_TOOLS: &[&str] = &[
    "Read",
    "Glob",
    "Grep",
    "WebFetch",
    "WebSearch",
    "Task",
    "AskUserQuestion",
    "TodoRead",
    "TodoWrite",
    "TaskCreate",
    "TaskUpdate",
    "TaskGet",
    "TaskList",
    "EnterPlanMode",
    "ExitPlanMode",
];

impl ClaudeSessionReader {
    /// Create a new reader. Returns `None` if `~/.claude` doesn't exist.
    pub fn new() -> Option<Self> {
        let home = dirs::home_dir()?;
        let claude_home = home.join(".claude");
        if !claude_home.is_dir() {
            debug!("~/.claude not found, Claude session reader disabled");
            return None;
        }
        Some(Self {
            claude_home,
            settings_cache: None,
            settings_mtime: None,
        })
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

    /// Core status determination algorithm.
    ///
    /// 1. Find the last meaningful assistant message
    /// 2. If it contains tool_use with no corresponding tool_result → check permission
    /// 3. If stop_reason is "end_turn" → WaitingForInput
    /// 4. Otherwise → Unknown (fall through)
    fn determine_status(&mut self, entries: &[JsonlEntry]) -> ClaudeSessionStatus {
        // Walk entries in reverse to find the last meaningful entry
        let mut last_assistant: Option<&JsonlEntry> = None;
        let mut tool_results: Vec<String> = Vec::new();

        for entry in entries.iter().rev() {
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
                    // This tool_use has no result — check if it needs permission
                    let tool_name = content.name.as_deref().unwrap_or("unknown");

                    if self.is_tool_auto_approved(tool_name) {
                        // Auto-approved tool still running
                        return ClaudeSessionStatus::Working;
                    } else {
                        // Needs manual permission
                        return ClaudeSessionStatus::NeedsPermission {
                            tool_name: Some(tool_name.to_string()),
                        };
                    }
                }
            }
        }

        // Check stop_reason
        if let Some(ref stop_reason) = msg.stop_reason {
            if stop_reason == "end_turn" {
                return ClaudeSessionStatus::WaitingForInput;
            }
        }

        ClaudeSessionStatus::Unknown
    }

    /// Check if a tool is auto-approved (never needs manual permission).
    fn is_tool_auto_approved(&mut self, tool_name: &str) -> bool {
        // Check always-approved list first
        if ALWAYS_APPROVED_TOOLS.contains(&tool_name) {
            return true;
        }

        // Check user settings
        if let Some(settings) = self.load_settings() {
            return Self::matches_settings_rules(tool_name, &settings);
        }

        false
    }

    /// Load and cache ~/.claude/settings.json.
    fn load_settings(&mut self) -> Option<ClaudeSettings> {
        let settings_path = self.claude_home.join("settings.json");

        // Check if file has been modified since last read
        let mtime = fs::metadata(&settings_path)
            .ok()
            .and_then(|m| m.modified().ok());

        if self.settings_cache.is_some() && mtime == self.settings_mtime {
            return self.settings_cache.clone();
        }

        // Read and parse settings
        match fs::read_to_string(&settings_path) {
            Ok(content) => match serde_json::from_str::<ClaudeSettings>(&content) {
                Ok(settings) => {
                    self.settings_mtime = mtime;
                    self.settings_cache = Some(settings.clone());
                    Some(settings)
                }
                Err(e) => {
                    warn!(?e, "Failed to parse ~/.claude/settings.json");
                    None
                }
            },
            Err(_) => None,
        }
    }

    /// Check if a tool matches any allow rules in settings.json.
    ///
    /// Settings permissions use patterns like:
    /// - `"Bash(git add:*)"` → allows Bash with commands starting with "git add:"
    /// - `"Write(src/**)"` → allows Write to paths under src/
    /// - `"Bash"` → allows all Bash commands
    fn matches_settings_rules(tool_name: &str, settings: &ClaudeSettings) -> bool {
        for (pattern, action) in &settings.permissions {
            if action != "allow" {
                continue;
            }

            // Exact match: "Bash" matches tool_name "Bash"
            if pattern == tool_name {
                return true;
            }

            // Pattern match: "Bash(git:*)" — tool_name "Bash" matches prefix
            if let Some(paren_pos) = pattern.find('(') {
                let pattern_tool = &pattern[..paren_pos];
                if pattern_tool == tool_name {
                    return true;
                }
            }
        }

        false
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
        }
    }

    #[test]
    fn test_end_turn_returns_waiting() {
        let entries = vec![make_assistant_entry(vec![], Some("end_turn"))];
        let mut reader = ClaudeSessionReader {
            claude_home: PathBuf::from("/nonexistent"),
            settings_cache: None,
            settings_mtime: None,
        };
        assert_eq!(
            reader.determine_status(&entries),
            ClaudeSessionStatus::WaitingForInput
        );
    }

    #[test]
    fn test_pending_tool_use_auto_approved() {
        let entries = vec![make_assistant_entry(
            vec![make_tool_use("Read", "tu_1")],
            None,
        )];
        let mut reader = ClaudeSessionReader {
            claude_home: PathBuf::from("/nonexistent"),
            settings_cache: None,
            settings_mtime: None,
        };
        assert_eq!(
            reader.determine_status(&entries),
            ClaudeSessionStatus::Working
        );
    }

    #[test]
    fn test_pending_tool_use_needs_permission() {
        let entries = vec![make_assistant_entry(
            vec![make_tool_use("Bash", "tu_2")],
            None,
        )];
        let mut reader = ClaudeSessionReader {
            claude_home: PathBuf::from("/nonexistent"),
            settings_cache: None,
            settings_mtime: None,
        };
        assert_eq!(
            reader.determine_status(&entries),
            ClaudeSessionStatus::NeedsPermission {
                tool_name: Some("Bash".to_string()),
            }
        );
    }

    #[test]
    fn test_tool_use_with_result_is_not_pending() {
        let entries = vec![
            make_assistant_entry(vec![make_tool_use("Bash", "tu_3")], None),
            make_human_entry(vec![make_tool_result("tu_3")]),
        ];
        let mut reader = ClaudeSessionReader {
            claude_home: PathBuf::from("/nonexistent"),
            settings_cache: None,
            settings_mtime: None,
        };
        // The tool_use has a corresponding result, so it's not pending.
        // No stop_reason → Unknown
        assert_eq!(
            reader.determine_status(&entries),
            ClaudeSessionStatus::Unknown
        );
    }

    #[test]
    fn test_empty_entries_returns_unknown() {
        let mut reader = ClaudeSessionReader {
            claude_home: PathBuf::from("/nonexistent"),
            settings_cache: None,
            settings_mtime: None,
        };
        assert_eq!(
            reader.determine_status(&[]),
            ClaudeSessionStatus::Unknown
        );
    }

    #[test]
    fn test_always_approved_tools() {
        let mut reader = ClaudeSessionReader {
            claude_home: PathBuf::from("/nonexistent"),
            settings_cache: None,
            settings_mtime: None,
        };

        for tool in ALWAYS_APPROVED_TOOLS {
            assert!(
                reader.is_tool_auto_approved(tool),
                "{tool} should be auto-approved"
            );
        }
        assert!(!reader.is_tool_auto_approved("Bash"));
        assert!(!reader.is_tool_auto_approved("Write"));
        assert!(!reader.is_tool_auto_approved("Edit"));
    }

    #[test]
    fn test_settings_rules_exact_match() {
        let mut perms = HashMap::new();
        perms.insert("Bash".to_string(), "allow".to_string());
        let settings = ClaudeSettings {
            permissions: perms,
        };
        assert!(ClaudeSessionReader::matches_settings_rules(
            "Bash", &settings
        ));
        assert!(!ClaudeSessionReader::matches_settings_rules(
            "Write", &settings
        ));
    }

    #[test]
    fn test_settings_rules_pattern_match() {
        let mut perms = HashMap::new();
        perms.insert("Bash(git:*)".to_string(), "allow".to_string());
        let settings = ClaudeSettings {
            permissions: perms,
        };
        // Pattern "Bash(git:*)" matches tool_name "Bash" at the prefix level
        assert!(ClaudeSessionReader::matches_settings_rules(
            "Bash", &settings
        ));
        assert!(!ClaudeSessionReader::matches_settings_rules(
            "Write", &settings
        ));
    }

    #[test]
    fn test_settings_rules_deny_not_matched() {
        let mut perms = HashMap::new();
        perms.insert("Bash".to_string(), "deny".to_string());
        let settings = ClaudeSettings {
            permissions: perms,
        };
        assert!(!ClaudeSessionReader::matches_settings_rules(
            "Bash", &settings
        ));
    }

    #[test]
    fn test_find_session_dir() {
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join("projects");
        fs::create_dir_all(&projects_dir).unwrap();

        let reader = ClaudeSessionReader {
            claude_home: tmp.path().to_path_buf(),
            settings_cache: None,
            settings_mtime: None,
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

        let reader = ClaudeSessionReader {
            claude_home: PathBuf::from("/nonexistent"),
            settings_cache: None,
            settings_mtime: None,
        };

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
    fn test_settings_cache_invalidation() {
        let tmp = TempDir::new().unwrap();
        let settings_path = tmp.path().join("settings.json");
        fs::write(
            &settings_path,
            r#"{"permissions":{"Bash":"allow"}}"#,
        )
        .unwrap();

        let mut reader = ClaudeSessionReader {
            claude_home: tmp.path().to_path_buf(),
            settings_cache: None,
            settings_mtime: None,
        };

        // First load
        let settings = reader.load_settings().unwrap();
        assert!(settings.permissions.contains_key("Bash"));

        // Modify settings
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(
            &settings_path,
            r#"{"permissions":{"Write":"allow"}}"#,
        )
        .unwrap();

        // Should reload
        let settings = reader.load_settings().unwrap();
        assert!(settings.permissions.contains_key("Write"));
        assert!(!settings.permissions.contains_key("Bash"));
    }
}
