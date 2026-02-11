//! Codex CLI session status reader via JSONL rollout logs.
//!
//! Reads Codex CLI's per-session JSONL files from `~/.codex/sessions/` to determine
//! session status with higher fidelity than OSC 133 alone.
//!
//! Codex CLI writes append-only JSONL rollout files partitioned by date:
//! `~/.codex/sessions/YYYY/MM/DD/rollout-<timestamp>-<uuid>.jsonl`
//!
//! Each line has `{ timestamp, type, payload }` with types:
//! - `session_meta` — first line with `cwd`, `approval_mode`, `sandbox_policy`
//! - `response_item` — messages and function calls
//! - `event_msg` — events like turn completion

use crate::CliSessionStatus;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{debug, trace};

/// Status derived from Codex CLI's JSONL rollout logs.
pub type CodexSessionStatus = CliSessionStatus;

/// Reads Codex CLI session data to determine status.
pub struct CodexSessionReader {
    /// Path to ~/.codex
    codex_home: PathBuf,
    /// Cache: CWD → rollout file path (avoid re-scanning on every poll).
    session_file_cache: HashMap<PathBuf, (PathBuf, SystemTime)>,
}

/// Approval mode from Codex CLI session metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ApprovalMode {
    /// All tool calls require user approval.
    Suggest,
    /// File edits are auto-approved, other tools require approval.
    AutoEdit,
    /// All tool calls are auto-approved.
    FullAuto,
}

/// Parsed session metadata from the first line of a rollout file.
#[derive(Debug, Deserialize)]
struct SessionMeta {
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    approval_mode: Option<String>,
}

/// A JSONL entry from a Codex rollout file.
#[derive(Debug, Deserialize)]
struct RolloutEntry {
    /// Entry type: "session_meta", "response_item", "event_msg".
    #[serde(rename = "type", default)]
    entry_type: String,
    /// Payload varies by type.
    #[serde(default)]
    payload: Option<serde_json::Value>,
}

impl CodexSessionReader {
    /// Create a new reader. Returns `None` if `~/.codex` doesn't exist.
    pub fn new() -> Option<Self> {
        let codex_home = Self::resolve_codex_home()?;
        if !codex_home.is_dir() {
            debug!("Codex home not found, Codex session reader disabled");
            return None;
        }
        Some(Self {
            codex_home,
            session_file_cache: HashMap::new(),
        })
    }

    /// Resolve the Codex home directory.
    /// Uses `CODEX_HOME` env var if set, otherwise `~/.codex`.
    fn resolve_codex_home() -> Option<PathBuf> {
        if let Ok(home) = std::env::var("CODEX_HOME") {
            return Some(PathBuf::from(home));
        }
        let home = dirs::home_dir()?;
        Some(home.join(".codex"))
    }

    /// Get the status of a Codex CLI session by reading its rollout JSONL log.
    ///
    /// `working_dir` is the session's working directory, used to locate the
    /// project-specific rollout file under `~/.codex/sessions/`.
    /// `_pid` is accepted for API consistency but not used (Codex uses different file naming).
    pub fn get_status(&mut self, working_dir: &Path, _pid: Option<u32>) -> CodexSessionStatus {
        let Some(rollout_path) = self.find_rollout_file(working_dir) else {
            trace!(?working_dir, "No Codex rollout file found");
            return CodexSessionStatus::Unknown;
        };

        // Read session metadata (first line) for approval mode
        let approval_mode = self.read_approval_mode(&rollout_path);

        // Read tail of file for recent entries
        let Some(tail) = Self::read_file_tail(&rollout_path, 131_072) else {
            return CodexSessionStatus::Unknown;
        };

        self.determine_status(&tail, &approval_mode)
    }

    /// Find the rollout file for a given working directory.
    ///
    /// Codex stores rollout files under `~/.codex/sessions/YYYY/MM/DD/`.
    /// We scan in reverse date order (newest first) and check the first line
    /// of each file for the `cwd` field.
    fn find_rollout_file(&mut self, working_dir: &Path) -> Option<PathBuf> {
        // Check cache first
        if let Some((cached_path, cached_mtime)) = self.session_file_cache.get(working_dir) {
            if let Ok(meta) = fs::metadata(cached_path) {
                if let Ok(mtime) = meta.modified() {
                    if mtime == *cached_mtime {
                        return Some(cached_path.clone());
                    }
                }
            }
        }

        let sessions_dir = self.codex_home.join("sessions");
        if !sessions_dir.is_dir() {
            return None;
        }

        // Collect date directories and sort in reverse order (newest first)
        let mut date_dirs = Vec::new();
        Self::collect_date_dirs(&sessions_dir, &mut date_dirs);
        date_dirs.sort_unstable_by(|a, b| b.cmp(a));

        let working_dir_str = working_dir.to_string_lossy();

        for date_dir in date_dirs {
            // List rollout files in this date directory, sorted by mtime (newest first)
            let mut rollout_files = Vec::new();
            if let Ok(entries) = fs::read_dir(&date_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("rollout-") && n.ends_with(".jsonl"))
                    {
                        if let Ok(meta) = path.metadata() {
                            if let Ok(mtime) = meta.modified() {
                                rollout_files.push((path, mtime));
                            }
                        }
                    }
                }
            }
            rollout_files.sort_unstable_by(|a, b| b.1.cmp(&a.1));

            for (path, mtime) in rollout_files {
                // Read first line for session_meta
                if let Some(cwd) = Self::read_cwd_from_first_line(&path) {
                    if cwd == working_dir_str.as_ref() {
                        self.session_file_cache
                            .insert(working_dir.to_path_buf(), (path.clone(), mtime));
                        return Some(path);
                    }
                }
            }
        }

        None
    }

    /// Recursively collect date directories (YYYY/MM/DD structure).
    fn collect_date_dirs(base: &Path, out: &mut Vec<PathBuf>) {
        // Walk YYYY dirs
        let Ok(year_entries) = fs::read_dir(base) else {
            return;
        };
        for year_entry in year_entries.flatten() {
            let year_path = year_entry.path();
            if !year_path.is_dir() {
                continue;
            }
            // Walk MM dirs
            let Ok(month_entries) = fs::read_dir(&year_path) else {
                continue;
            };
            for month_entry in month_entries.flatten() {
                let month_path = month_entry.path();
                if !month_path.is_dir() {
                    continue;
                }
                // Walk DD dirs
                let Ok(day_entries) = fs::read_dir(&month_path) else {
                    continue;
                };
                for day_entry in day_entries.flatten() {
                    let day_path = day_entry.path();
                    if day_path.is_dir() {
                        out.push(day_path);
                    }
                }
            }
        }
    }

    /// Read the CWD from the first line (session_meta) of a rollout file.
    fn read_cwd_from_first_line(path: &Path) -> Option<String> {
        let file = fs::File::open(path).ok()?;
        let mut reader = std::io::BufReader::new(file);
        let mut first_line = String::new();
        std::io::BufRead::read_line(&mut reader, &mut first_line).ok()?;

        let entry: RolloutEntry = serde_json::from_str(first_line.trim()).ok()?;
        if entry.entry_type != "session_meta" {
            return None;
        }

        let payload = entry.payload?;
        let meta: SessionMeta = serde_json::from_value(payload).ok()?;
        meta.cwd
    }

    /// Read the approval mode from the first line of a rollout file.
    fn read_approval_mode(&self, path: &Path) -> ApprovalMode {
        let Some(first_line) = Self::read_first_line(path) else {
            return ApprovalMode::Suggest;
        };

        let Ok(entry) = serde_json::from_str::<RolloutEntry>(first_line.trim()) else {
            return ApprovalMode::Suggest;
        };

        if entry.entry_type != "session_meta" {
            return ApprovalMode::Suggest;
        }

        let Some(payload) = entry.payload else {
            return ApprovalMode::Suggest;
        };

        let Ok(meta) = serde_json::from_value::<SessionMeta>(payload) else {
            return ApprovalMode::Suggest;
        };

        match meta.approval_mode.as_deref() {
            Some("full-auto") => ApprovalMode::FullAuto,
            Some("auto-edit") => ApprovalMode::AutoEdit,
            _ => ApprovalMode::Suggest,
        }
    }

    /// Read the first line of a file.
    fn read_first_line(path: &Path) -> Option<String> {
        let file = fs::File::open(path).ok()?;
        let mut reader = std::io::BufReader::new(file);
        let mut line = String::new();
        std::io::BufRead::read_line(&mut reader, &mut line).ok()?;
        Some(line)
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
    /// 1. Find last `response_item` with `function_call` that has no matching
    ///    `function_call_output` → pending tool
    /// 2. If pending tool exists, check approval mode
    /// 3. If no pending tools and last event indicates turn complete → NeedsAttention
    /// 4. Otherwise → Unknown
    fn determine_status(&self, tail: &str, approval_mode: &ApprovalMode) -> CodexSessionStatus {
        let mut pending_calls: HashMap<String, String> = HashMap::new(); // call_id → tool name
        let mut last_entry_type = String::new();
        let mut has_turn_complete = false;

        for line in tail.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let Ok(entry) = serde_json::from_str::<RolloutEntry>(line) else {
                continue;
            };

            last_entry_type = entry.entry_type.clone();

            if entry.entry_type == "response_item" {
                if let Some(ref payload) = entry.payload {
                    // Check for function_call
                    if let Some(item_type) = payload.get("type").and_then(|v| v.as_str()) {
                        if item_type == "function_call" {
                            if let Some(call_id) = payload.get("call_id").and_then(|v| v.as_str())
                            {
                                let name = payload
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                pending_calls.insert(call_id.to_string(), name);
                            }
                        } else if item_type == "function_call_output" {
                            if let Some(call_id) = payload.get("call_id").and_then(|v| v.as_str())
                            {
                                pending_calls.remove(call_id);
                            }
                        }
                    }
                }
            } else if entry.entry_type == "event_msg" {
                // Check for turn completion events
                if let Some(ref payload) = entry.payload {
                    if let Some(event_type) = payload.get("type").and_then(|v| v.as_str()) {
                        if event_type == "turn_complete"
                            || event_type == "response.completed"
                            || event_type == "response.done"
                        {
                            has_turn_complete = true;
                        }
                    }
                }
            }
        }

        // Check for pending tool calls
        if let Some((_call_id, tool_name)) = pending_calls.iter().next() {
            return match approval_mode {
                ApprovalMode::FullAuto => CodexSessionStatus::Working,
                ApprovalMode::AutoEdit => {
                    if Self::is_file_edit_tool(tool_name) {
                        CodexSessionStatus::Working
                    } else {
                        CodexSessionStatus::NeedsAttention {
                            detail: Some(tool_name.clone()),
                        }
                    }
                }
                ApprovalMode::Suggest => CodexSessionStatus::NeedsAttention {
                    detail: Some(tool_name.clone()),
                },
            };
        }

        // No pending tools — check if turn is complete
        if has_turn_complete || last_entry_type == "event_msg" {
            return CodexSessionStatus::NeedsAttention { detail: None };
        }

        CodexSessionStatus::Unknown
    }

    /// Check if a tool name represents a file edit operation.
    fn is_file_edit_tool(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("write")
            || lower.contains("edit")
            || lower.contains("patch")
            || lower.contains("apply")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_unknown_on_empty() {
        let reader = CodexSessionReader {
            codex_home: PathBuf::from("/nonexistent"),
            session_file_cache: HashMap::new(),
        };
        assert_eq!(
            reader.determine_status("", &ApprovalMode::Suggest),
            CodexSessionStatus::Unknown
        );
    }

    #[test]
    fn test_waiting_for_input_on_turn_complete() {
        let tail = r#"{"type":"response_item","payload":{"type":"function_call","call_id":"c1","name":"shell"}}
{"type":"response_item","payload":{"type":"function_call_output","call_id":"c1"}}
{"type":"event_msg","payload":{"type":"turn_complete"}}"#;

        let reader = CodexSessionReader {
            codex_home: PathBuf::from("/nonexistent"),
            session_file_cache: HashMap::new(),
        };
        assert_eq!(
            reader.determine_status(tail, &ApprovalMode::Suggest),
            CodexSessionStatus::NeedsAttention { detail: None }
        );
    }

    #[test]
    fn test_pending_tool_suggest_mode() {
        let tail = r#"{"type":"response_item","payload":{"type":"function_call","call_id":"c1","name":"shell"}}"#;

        let reader = CodexSessionReader {
            codex_home: PathBuf::from("/nonexistent"),
            session_file_cache: HashMap::new(),
        };
        assert_eq!(
            reader.determine_status(tail, &ApprovalMode::Suggest),
            CodexSessionStatus::NeedsAttention {
                detail: Some("shell".to_string()),
            }
        );
    }

    #[test]
    fn test_pending_tool_full_auto() {
        let tail = r#"{"type":"response_item","payload":{"type":"function_call","call_id":"c1","name":"shell"}}"#;

        let reader = CodexSessionReader {
            codex_home: PathBuf::from("/nonexistent"),
            session_file_cache: HashMap::new(),
        };
        assert_eq!(
            reader.determine_status(tail, &ApprovalMode::FullAuto),
            CodexSessionStatus::Working
        );
    }

    #[test]
    fn test_pending_file_edit_auto_edit_mode() {
        let tail = r#"{"type":"response_item","payload":{"type":"function_call","call_id":"c1","name":"file_write"}}"#;

        let reader = CodexSessionReader {
            codex_home: PathBuf::from("/nonexistent"),
            session_file_cache: HashMap::new(),
        };
        assert_eq!(
            reader.determine_status(tail, &ApprovalMode::AutoEdit),
            CodexSessionStatus::Working
        );
    }

    #[test]
    fn test_pending_shell_auto_edit_mode() {
        let tail = r#"{"type":"response_item","payload":{"type":"function_call","call_id":"c1","name":"shell"}}"#;

        let reader = CodexSessionReader {
            codex_home: PathBuf::from("/nonexistent"),
            session_file_cache: HashMap::new(),
        };
        assert_eq!(
            reader.determine_status(tail, &ApprovalMode::AutoEdit),
            CodexSessionStatus::NeedsAttention {
                detail: Some("shell".to_string()),
            }
        );
    }

    #[test]
    fn test_resolved_tool_no_turn_complete() {
        let tail = r#"{"type":"response_item","payload":{"type":"function_call","call_id":"c1","name":"shell"}}
{"type":"response_item","payload":{"type":"function_call_output","call_id":"c1"}}"#;

        let reader = CodexSessionReader {
            codex_home: PathBuf::from("/nonexistent"),
            session_file_cache: HashMap::new(),
        };
        // All tools resolved but no turn_complete event → Unknown
        assert_eq!(
            reader.determine_status(tail, &ApprovalMode::Suggest),
            CodexSessionStatus::Unknown
        );
    }

    #[test]
    fn test_is_file_edit_tool() {
        assert!(CodexSessionReader::is_file_edit_tool("file_write"));
        assert!(CodexSessionReader::is_file_edit_tool("file_edit"));
        assert!(CodexSessionReader::is_file_edit_tool("apply_patch"));
        assert!(!CodexSessionReader::is_file_edit_tool("shell"));
        assert!(!CodexSessionReader::is_file_edit_tool("read_file"));
    }

    #[test]
    fn test_read_cwd_from_first_line() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("rollout.jsonl");
        let mut file = fs::File::create(&path).unwrap();
        writeln!(
            file,
            r#"{{"type":"session_meta","payload":{{"cwd":"/Users/test/project","approval_mode":"suggest"}}}}"#
        )
        .unwrap();

        let cwd = CodexSessionReader::read_cwd_from_first_line(&path);
        assert_eq!(cwd, Some("/Users/test/project".to_string()));
    }

    #[test]
    fn test_read_approval_mode() {
        let tmp = TempDir::new().unwrap();
        let reader = CodexSessionReader {
            codex_home: tmp.path().to_path_buf(),
            session_file_cache: HashMap::new(),
        };

        // full-auto
        let path = tmp.path().join("full_auto.jsonl");
        fs::write(
            &path,
            r#"{"type":"session_meta","payload":{"cwd":"/tmp","approval_mode":"full-auto"}}"#,
        )
        .unwrap();
        assert_eq!(reader.read_approval_mode(&path), ApprovalMode::FullAuto);

        // auto-edit
        let path = tmp.path().join("auto_edit.jsonl");
        fs::write(
            &path,
            r#"{"type":"session_meta","payload":{"cwd":"/tmp","approval_mode":"auto-edit"}}"#,
        )
        .unwrap();
        assert_eq!(reader.read_approval_mode(&path), ApprovalMode::AutoEdit);

        // suggest (default)
        let path = tmp.path().join("suggest.jsonl");
        fs::write(
            &path,
            r#"{"type":"session_meta","payload":{"cwd":"/tmp","approval_mode":"suggest"}}"#,
        )
        .unwrap();
        assert_eq!(reader.read_approval_mode(&path), ApprovalMode::Suggest);
    }

    #[test]
    fn test_find_rollout_file_with_date_dirs() {
        let tmp = TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("sessions").join("2025").join("01").join("15");
        fs::create_dir_all(&sessions_dir).unwrap();

        let rollout_path = sessions_dir.join("rollout-12345-uuid.jsonl");
        let mut file = fs::File::create(&rollout_path).unwrap();
        writeln!(
            file,
            r#"{{"type":"session_meta","payload":{{"cwd":"/Users/test/project","approval_mode":"suggest"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"event_msg","payload":{{"type":"turn_complete"}}}}"#
        )
        .unwrap();

        let mut reader = CodexSessionReader {
            codex_home: tmp.path().to_path_buf(),
            session_file_cache: HashMap::new(),
        };

        let found = reader.find_rollout_file(Path::new("/Users/test/project"));
        assert_eq!(found, Some(rollout_path));
    }

    #[test]
    fn test_find_rollout_file_no_match() {
        let tmp = TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("sessions").join("2025").join("01").join("15");
        fs::create_dir_all(&sessions_dir).unwrap();

        let rollout_path = sessions_dir.join("rollout-12345-uuid.jsonl");
        let mut file = fs::File::create(&rollout_path).unwrap();
        writeln!(
            file,
            r#"{{"type":"session_meta","payload":{{"cwd":"/Users/other/project","approval_mode":"suggest"}}}}"#
        )
        .unwrap();

        let mut reader = CodexSessionReader {
            codex_home: tmp.path().to_path_buf(),
            session_file_cache: HashMap::new(),
        };

        let found = reader.find_rollout_file(Path::new("/Users/test/project"));
        assert!(found.is_none());
    }

    #[test]
    fn test_session_file_cache_hit() {
        let tmp = TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("sessions").join("2025").join("01").join("15");
        fs::create_dir_all(&sessions_dir).unwrap();

        let rollout_path = sessions_dir.join("rollout-12345-uuid.jsonl");
        let mut file = fs::File::create(&rollout_path).unwrap();
        writeln!(
            file,
            r#"{{"type":"session_meta","payload":{{"cwd":"/Users/test/project","approval_mode":"suggest"}}}}"#
        )
        .unwrap();

        let mtime = fs::metadata(&rollout_path).unwrap().modified().unwrap();

        let mut reader = CodexSessionReader {
            codex_home: tmp.path().to_path_buf(),
            session_file_cache: HashMap::new(),
        };

        // Pre-populate cache
        reader.session_file_cache.insert(
            PathBuf::from("/Users/test/project"),
            (rollout_path.clone(), mtime),
        );

        // Should return cached path without scanning
        let found = reader.find_rollout_file(Path::new("/Users/test/project"));
        assert_eq!(found, Some(rollout_path));
    }

    #[test]
    fn test_read_file_tail() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.jsonl");

        let mut file = fs::File::create(&path).unwrap();
        for i in 0..100 {
            writeln!(file, "line {i}").unwrap();
        }

        let tail = CodexSessionReader::read_file_tail(&path, 50).unwrap();
        assert!(tail.contains("line 99"));
        assert!(!tail.contains("line 0\n"));
    }

    #[test]
    fn test_multiple_pending_calls_only_last_matters() {
        // Two function calls, one resolved, one pending
        let tail = r#"{"type":"response_item","payload":{"type":"function_call","call_id":"c1","name":"read_file"}}
{"type":"response_item","payload":{"type":"function_call_output","call_id":"c1"}}
{"type":"response_item","payload":{"type":"function_call","call_id":"c2","name":"shell"}}"#;

        let reader = CodexSessionReader {
            codex_home: PathBuf::from("/nonexistent"),
            session_file_cache: HashMap::new(),
        };
        assert_eq!(
            reader.determine_status(tail, &ApprovalMode::Suggest),
            CodexSessionStatus::NeedsAttention {
                detail: Some("shell".to_string()),
            }
        );
    }
}
