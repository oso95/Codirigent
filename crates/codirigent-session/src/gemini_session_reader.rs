//! Gemini CLI session status reader via JSON conversation files.
//!
//! Reads Gemini CLI's per-session JSON files from `~/.gemini/tmp/<project-slug>/chats/`
//! to determine session status with higher fidelity than OSC 133 alone.
//!
//! Gemini CLI rewrites the entire JSON file on each update (not append-only like JSONL).
//! Session files follow the pattern: `session-<timestamp>-<uuid>.json`
//!
//! The JSON schema contains `sessionId`, `lastUpdated`, and a `messages` array
//! where each message has a `type` and optional `toolCalls`.

use crate::CliSessionStatus;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{debug, trace, warn};

/// Status derived from Gemini CLI's JSON session files.
pub type GeminiSessionStatus = CliSessionStatus;

/// Reads Gemini CLI session data to determine status.
pub struct GeminiSessionReader {
    /// Path to ~/.gemini
    gemini_home: PathBuf,
    /// Cache: project slug mapping from projects.json (abs_path → slug).
    projects_cache: Option<HashMap<String, String>>,
    /// Last modification time of projects.json for cache invalidation.
    projects_mtime: Option<SystemTime>,
    /// Cache: last parsed session data per CWD (to skip re-parsing on unchanged mtime).
    session_cache: HashMap<PathBuf, (SystemTime, GeminiSessionStatus)>,
}

/// A Gemini CLI session JSON file.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiSession {
    #[serde(default)]
    last_updated: Option<String>,
    #[serde(default)]
    messages: Vec<GeminiMessage>,
}

/// A message in a Gemini session.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiMessage {
    /// Message type: "user", "gemini", "info", "error".
    #[serde(rename = "type", default)]
    message_type: String,
    /// Tool calls within this message.
    #[serde(default)]
    tool_calls: Vec<GeminiToolCall>,
}

/// A tool call within a Gemini message.
#[derive(Debug, Deserialize)]
struct GeminiToolCall {
    /// Tool name.
    #[serde(default)]
    name: Option<String>,
    /// Tool result (present if tool has completed).
    #[serde(default)]
    result: Option<serde_json::Value>,
    /// Tool status: "success", "error", or absent if pending.
    #[serde(default)]
    status: Option<String>,
}

/// Projects.json entry mapping absolute paths to slugs.
#[derive(Debug, Deserialize)]
struct ProjectEntry {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    slug: Option<String>,
}

impl GeminiSessionReader {
    /// Create a new reader. Returns `None` if `~/.gemini` doesn't exist.
    pub fn new() -> Option<Self> {
        let gemini_home = Self::resolve_gemini_home()?;
        if !gemini_home.is_dir() {
            debug!("Gemini home not found, Gemini session reader disabled");
            return None;
        }
        Some(Self {
            gemini_home,
            projects_cache: None,
            projects_mtime: None,
            session_cache: HashMap::new(),
        })
    }

    /// Resolve the Gemini home directory.
    /// Uses `GEMINI_CLI_HOME` env var if set, otherwise `~/.gemini`.
    fn resolve_gemini_home() -> Option<PathBuf> {
        if let Ok(home) = std::env::var("GEMINI_CLI_HOME") {
            return Some(PathBuf::from(home));
        }
        let home = dirs::home_dir()?;
        Some(home.join(".gemini"))
    }

    /// Get the status of a Gemini CLI session by reading its JSON session file.
    ///
    /// `working_dir` is the session's working directory, used to locate the
    /// project-specific session file.
    /// `_pid` is accepted for API consistency but not used.
    pub fn get_status(&mut self, working_dir: &Path, _pid: Option<u32>) -> GeminiSessionStatus {
        let Some(session_path) = self.find_session_file(working_dir) else {
            trace!(?working_dir, "No Gemini session file found");
            return GeminiSessionStatus::Unknown;
        };

        // Check mtime to skip re-parsing if unchanged
        let mtime = fs::metadata(&session_path)
            .ok()
            .and_then(|m| m.modified().ok());

        if let Some(mtime) = mtime {
            if let Some((cached_mtime, ref cached_status)) = self.session_cache.get(working_dir) {
                if *cached_mtime == mtime {
                    return cached_status.clone();
                }
            }
        }

        // Read and parse the full JSON file
        let content = match fs::read_to_string(&session_path) {
            Ok(c) => c,
            Err(e) => {
                trace!(?e, "Failed to read Gemini session file");
                return GeminiSessionStatus::Unknown;
            }
        };

        let session: GeminiSession = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                trace!(?e, "Failed to parse Gemini session JSON");
                return GeminiSessionStatus::Unknown;
            }
        };

        let status = Self::determine_status(&session);

        // Cache the result
        if let Some(mtime) = mtime {
            self.session_cache
                .insert(working_dir.to_path_buf(), (mtime, status.clone()));
        }

        status
    }

    /// Find the most recent session file for a given working directory.
    fn find_session_file(&mut self, working_dir: &Path) -> Option<PathBuf> {
        let slug = self.find_project_slug(working_dir)?;

        let chats_dir = self.gemini_home.join("tmp").join(&slug).join("chats");
        if !chats_dir.is_dir() {
            return None;
        }

        Self::find_most_recent_session_json(&chats_dir)
    }

    /// Find the project slug for a working directory using projects.json.
    fn find_project_slug(&mut self, working_dir: &Path) -> Option<String> {
        self.load_projects_cache();

        let working_dir_str = working_dir.to_string_lossy();
        if let Some(ref cache) = self.projects_cache {
            return cache.get(working_dir_str.as_ref()).cloned();
        }

        None
    }

    /// Load and cache projects.json, invalidating on mtime change.
    fn load_projects_cache(&mut self) {
        let projects_path = self.gemini_home.join("projects.json");

        let mtime = fs::metadata(&projects_path)
            .ok()
            .and_then(|m| m.modified().ok());

        // Skip if mtime hasn't changed
        if self.projects_cache.is_some() && mtime == self.projects_mtime {
            return;
        }

        let content = match fs::read_to_string(&projects_path) {
            Ok(c) => c,
            Err(_) => {
                // projects.json doesn't exist — try slug-based fallback
                self.projects_cache = Some(HashMap::new());
                self.projects_mtime = mtime;
                return;
            }
        };

        // projects.json can be an array of entries or an object
        let mut cache = HashMap::new();

        // Try array format first
        if let Ok(entries) = serde_json::from_str::<Vec<ProjectEntry>>(&content) {
            for entry in entries {
                if let (Some(path), Some(slug)) = (entry.path, entry.slug) {
                    cache.insert(path, slug);
                }
            }
        } else if let Ok(obj) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&content)
        {
            // Try object format: { "/abs/path": { "slug": "..." } }
            for (path, value) in obj {
                if let Some(slug) = value.get("slug").and_then(|v| v.as_str()) {
                    cache.insert(path, slug.to_string());
                }
            }
        } else {
            warn!("Failed to parse ~/.gemini/projects.json");
        }

        self.projects_mtime = mtime;
        self.projects_cache = Some(cache);
    }

    /// Find the most recently modified session-*.json file in a directory.
    fn find_most_recent_session_json(dir: &Path) -> Option<PathBuf> {
        let entries = fs::read_dir(dir).ok()?;
        let mut best: Option<(PathBuf, SystemTime)> = None;

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name.starts_with("session-") && file_name.ends_with(".json") {
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

    /// Core status determination algorithm.
    ///
    /// 1. Check `lastUpdated` — if older than 30s, likely idle → Unknown
    /// 2. Find last message:
    ///    - Last message is `user` + recent → Working (Gemini is processing)
    ///    - Last message is `gemini` with pending tool calls → check tool type
    ///    - Last message is `gemini`, all tools resolved → NeedsAttention
    /// 3. Otherwise → Unknown
    fn determine_status(session: &GeminiSession) -> GeminiSessionStatus {
        // Check if session is stale (lastUpdated older than 30s)
        if let Some(ref last_updated) = session.last_updated {
            if Self::is_stale_timestamp(last_updated) {
                return GeminiSessionStatus::Unknown;
            }
        }

        let Some(last_msg) = session.messages.last() else {
            return GeminiSessionStatus::Unknown;
        };

        match last_msg.message_type.as_str() {
            "user" => {
                // User just sent a message — Gemini should be processing
                GeminiSessionStatus::Working
            }
            "gemini" => {
                // Check for pending tool calls (no result yet)
                for tool_call in &last_msg.tool_calls {
                    if tool_call.result.is_none() && tool_call.status.is_none() {
                        let tool_name = tool_call.name.as_deref().unwrap_or("unknown");
                        // Shell commands need confirmation
                        if tool_name == "run_shell_command"
                            || tool_name == "shell"
                            || tool_name == "execute_command"
                        {
                            return GeminiSessionStatus::NeedsAttention {
                                detail: Some(tool_name.to_string()),
                            };
                        }
                        // Other tools are auto-approved
                        return GeminiSessionStatus::Working;
                    }
                }

                // All tool calls resolved (or no tool calls) — waiting for input
                GeminiSessionStatus::NeedsAttention { detail: None }
            }
            _ => GeminiSessionStatus::Unknown,
        }
    }

    /// Check if an ISO-8601 timestamp is older than 30 seconds.
    fn is_stale_timestamp(timestamp: &str) -> bool {
        use chrono::{DateTime, Utc};

        let Ok(parsed) = timestamp.parse::<DateTime<Utc>>() else {
            return false; // Can't parse → don't treat as stale
        };

        let elapsed = Utc::now().signed_duration_since(parsed);
        elapsed.num_seconds() > 30
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_session(messages: Vec<GeminiMessage>, last_updated: Option<&str>) -> GeminiSession {
        GeminiSession {
            last_updated: last_updated.map(|s| s.to_string()),
            messages,
        }
    }

    fn make_gemini_msg(tool_calls: Vec<GeminiToolCall>) -> GeminiMessage {
        GeminiMessage {
            message_type: "gemini".to_string(),
            tool_calls,
        }
    }

    fn make_user_msg() -> GeminiMessage {
        GeminiMessage {
            message_type: "user".to_string(),
            tool_calls: vec![],
        }
    }

    fn recent_timestamp() -> String {
        chrono::Utc::now().to_rfc3339()
    }

    fn stale_timestamp() -> String {
        use chrono::Duration;
        (chrono::Utc::now() - Duration::seconds(60)).to_rfc3339()
    }

    #[test]
    fn test_unknown_on_empty_messages() {
        let session = make_session(vec![], Some(&recent_timestamp()));
        assert_eq!(
            GeminiSessionReader::determine_status(&session),
            GeminiSessionStatus::Unknown
        );
    }

    #[test]
    fn test_stale_session_returns_unknown() {
        let session = make_session(vec![make_user_msg()], Some(&stale_timestamp()));
        assert_eq!(
            GeminiSessionReader::determine_status(&session),
            GeminiSessionStatus::Unknown
        );
    }

    #[test]
    fn test_user_message_last_means_working() {
        let session = make_session(vec![make_user_msg()], Some(&recent_timestamp()));
        assert_eq!(
            GeminiSessionReader::determine_status(&session),
            GeminiSessionStatus::Working
        );
    }

    #[test]
    fn test_gemini_message_no_tools_means_waiting() {
        let session = make_session(vec![make_gemini_msg(vec![])], Some(&recent_timestamp()));
        assert_eq!(
            GeminiSessionReader::determine_status(&session),
            GeminiSessionStatus::NeedsAttention { detail: None }
        );
    }

    #[test]
    fn test_gemini_message_all_tools_resolved_means_waiting() {
        let session = make_session(
            vec![make_gemini_msg(vec![GeminiToolCall {
                name: Some("read_file".to_string()),
                result: Some(serde_json::json!({"content": "hello"})),
                status: Some("success".to_string()),
            }])],
            Some(&recent_timestamp()),
        );
        assert_eq!(
            GeminiSessionReader::determine_status(&session),
            GeminiSessionStatus::NeedsAttention { detail: None }
        );
    }

    #[test]
    fn test_pending_shell_command_needs_permission() {
        let session = make_session(
            vec![make_gemini_msg(vec![GeminiToolCall {
                name: Some("run_shell_command".to_string()),
                result: None,
                status: None,
            }])],
            Some(&recent_timestamp()),
        );
        assert_eq!(
            GeminiSessionReader::determine_status(&session),
            GeminiSessionStatus::NeedsAttention {
                detail: Some("run_shell_command".to_string()),
            }
        );
    }

    #[test]
    fn test_pending_non_shell_tool_means_working() {
        let session = make_session(
            vec![make_gemini_msg(vec![GeminiToolCall {
                name: Some("read_file".to_string()),
                result: None,
                status: None,
            }])],
            Some(&recent_timestamp()),
        );
        assert_eq!(
            GeminiSessionReader::determine_status(&session),
            GeminiSessionStatus::Working
        );
    }

    #[test]
    fn test_find_most_recent_session_json() {
        let tmp = TempDir::new().unwrap();

        // Create two session files
        let path1 = tmp.path().join("session-old-uuid1.json");
        fs::write(&path1, "{}").unwrap();

        std::thread::sleep(std::time::Duration::from_millis(50));

        let path2 = tmp.path().join("session-new-uuid2.json");
        fs::write(&path2, "{}").unwrap();

        let found = GeminiSessionReader::find_most_recent_session_json(tmp.path()).unwrap();
        assert_eq!(found, path2);
    }

    #[test]
    fn test_projects_cache_loading() {
        let tmp = TempDir::new().unwrap();

        // Create projects.json as array format
        let projects_path = tmp.path().join("projects.json");
        fs::write(
            &projects_path,
            r#"[{"path":"/Users/test/project","slug":"test-project"}]"#,
        )
        .unwrap();

        let mut reader = GeminiSessionReader {
            gemini_home: tmp.path().to_path_buf(),
            projects_cache: None,
            projects_mtime: None,
            session_cache: HashMap::new(),
        };

        reader.load_projects_cache();
        assert!(reader.projects_cache.is_some());
        let cache = reader.projects_cache.as_ref().unwrap();
        assert_eq!(
            cache.get("/Users/test/project"),
            Some(&"test-project".to_string())
        );
    }

    #[test]
    fn test_projects_cache_object_format() {
        let tmp = TempDir::new().unwrap();

        let projects_path = tmp.path().join("projects.json");
        fs::write(
            &projects_path,
            r#"{"/Users/test/project":{"slug":"test-project"}}"#,
        )
        .unwrap();

        let mut reader = GeminiSessionReader {
            gemini_home: tmp.path().to_path_buf(),
            projects_cache: None,
            projects_mtime: None,
            session_cache: HashMap::new(),
        };

        reader.load_projects_cache();
        let cache = reader.projects_cache.as_ref().unwrap();
        assert_eq!(
            cache.get("/Users/test/project"),
            Some(&"test-project".to_string())
        );
    }

    #[test]
    fn test_projects_cache_invalidation() {
        let tmp = TempDir::new().unwrap();

        let projects_path = tmp.path().join("projects.json");
        fs::write(
            &projects_path,
            r#"[{"path":"/Users/test/project","slug":"slug-v1"}]"#,
        )
        .unwrap();

        let mut reader = GeminiSessionReader {
            gemini_home: tmp.path().to_path_buf(),
            projects_cache: None,
            projects_mtime: None,
            session_cache: HashMap::new(),
        };

        reader.load_projects_cache();
        let slug = reader
            .projects_cache
            .as_ref()
            .unwrap()
            .get("/Users/test/project")
            .cloned();
        assert_eq!(slug, Some("slug-v1".to_string()));

        // Update file
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(
            &projects_path,
            r#"[{"path":"/Users/test/project","slug":"slug-v2"}]"#,
        )
        .unwrap();

        reader.load_projects_cache();
        let slug = reader
            .projects_cache
            .as_ref()
            .unwrap()
            .get("/Users/test/project")
            .cloned();
        assert_eq!(slug, Some("slug-v2".to_string()));
    }

    #[test]
    fn test_full_session_json_parsing() {
        let tmp = TempDir::new().unwrap();

        // Create a proper gemini directory structure
        let gemini_home = tmp.path();
        let projects_path = gemini_home.join("projects.json");
        fs::write(
            &projects_path,
            r#"[{"path":"/Users/test/project","slug":"test-project"}]"#,
        )
        .unwrap();

        let chats_dir = gemini_home.join("tmp").join("test-project").join("chats");
        fs::create_dir_all(&chats_dir).unwrap();

        let now = chrono::Utc::now().to_rfc3339();
        let session_json = format!(
            r#"{{
                "sessionId": "test-uuid",
                "lastUpdated": "{}",
                "messages": [
                    {{"type": "user", "toolCalls": []}},
                    {{"type": "gemini", "toolCalls": [
                        {{"name": "read_file", "result": {{"content": "hello"}}, "status": "success"}}
                    ]}}
                ]
            }}"#,
            now
        );
        let session_path = chats_dir.join("session-12345-uuid.json");
        fs::write(&session_path, session_json).unwrap();

        let mut reader = GeminiSessionReader {
            gemini_home: gemini_home.to_path_buf(),
            projects_cache: None,
            projects_mtime: None,
            session_cache: HashMap::new(),
        };

        let status = reader.get_status(Path::new("/Users/test/project"), None);
        assert_eq!(status, GeminiSessionStatus::NeedsAttention { detail: None });
    }

    #[test]
    fn test_is_stale_timestamp() {
        // Recent timestamp should not be stale
        let recent = chrono::Utc::now().to_rfc3339();
        assert!(!GeminiSessionReader::is_stale_timestamp(&recent));

        // Old timestamp should be stale
        use chrono::Duration;
        let old = (chrono::Utc::now() - Duration::seconds(60)).to_rfc3339();
        assert!(GeminiSessionReader::is_stale_timestamp(&old));

        // Unparseable should not be treated as stale
        assert!(!GeminiSessionReader::is_stale_timestamp("not-a-date"));
    }

    #[test]
    fn test_missing_projects_json() {
        let tmp = TempDir::new().unwrap();

        let mut reader = GeminiSessionReader {
            gemini_home: tmp.path().to_path_buf(),
            projects_cache: None,
            projects_mtime: None,
            session_cache: HashMap::new(),
        };

        reader.load_projects_cache();
        // Should have an empty cache, not None
        assert!(reader.projects_cache.is_some());
        assert!(reader.projects_cache.as_ref().unwrap().is_empty());
    }
}
