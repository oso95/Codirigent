//! Codirigent hook handler for Claude, Gemini, and Codex CLI.
//!
//! Claude Code passes hook JSON on stdin via `~/.claude/settings.json`.
//! Gemini CLI passes hook JSON on stdin via `~/.gemini/settings.json`.
//! Codex executes the command from `~/.codex/config.toml` with JSON as the first
//! CLI argument.
//!
//! Both sources are normalized to the same signal file format:
//! `<codirigent-signals-dir>/<session_id>.json`.

use serde::{Deserialize, Serialize};
use std::env;
use std::io::Read;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CLI_TYPE_CLAUDE: &str = "claude";
const CLI_TYPE_CODEX: &str = "codex";
const CLI_TYPE_GEMINI: &str = "gemini";

/// Payload from Claude Code hook stdin, Codex notify argument, or Gemini CLI hook.
#[derive(Deserialize)]
struct HookPayload {
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    hook_event_name: Option<String>,
    #[serde(default)]
    notification_type: Option<String>,
    #[serde(rename = "type", alias = "event", alias = "event_type", default)]
    event_type: Option<String>,
    #[serde(default)]
    cli_type: Option<String>,
    #[serde(default)]
    approval_policy: Option<String>,
    #[serde(default)]
    sandbox_policy: Option<serde_json::Value>,
}

/// Signal file format consumed by `check_hook_signals`.
#[derive(Serialize)]
struct SignalFile {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    cli_type: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cli_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sandbox_policy_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    codirigent_session_uuid: Option<String>,
    // Legacy field kept for transition compatibility with older Codirigent builds.
    codirigent_session_id: Option<String>,
    ts: u64,
}

fn main() {
    let payload = match read_payload() {
        Some(p) => p,
        None => return,
    };

    handle_payload(payload);
}

fn handle_payload(payload: HookPayload) {
    // Only process signals for sessions launched by Codirigent.
    let codirigent_session_id = env::var("CODIRIGENT_SESSION_ID").ok();
    let codirigent_session_uuid = env::var("CODIRIGENT_SESSION_UUID").ok();
    if codirigent_session_id.is_none() && codirigent_session_uuid.is_none() {
        return;
    }

    let cli_session_id = payload
        .session_id
        .as_deref()
        .filter(|id| is_safe_filename(id))
        .map(str::to_owned);

    // Prefer the CLI's own session ID (e.g. Claude's UUID), then
    // CODIRIGENT_SESSION_UUID (stable across restarts), then the legacy
    // integer CODIRIGENT_SESSION_ID as a last resort.
    let filename_session_id = payload
        .session_id
        .as_deref()
        .filter(|id| is_safe_filename(id))
        .or_else(|| {
            codirigent_session_uuid
                .as_deref()
                .filter(|id| is_safe_filename(id))
        })
        .or_else(|| {
            codirigent_session_id
                .as_deref()
                .filter(|id| is_safe_filename(id))
        })
        .unwrap_or_default()
        .to_owned();
    if filename_session_id.is_empty() {
        return;
    }

    let cli_type = infer_cli_type(&payload);
    let status = map_status(
        payload.hook_event_name.as_deref(),
        payload.notification_type.as_deref(),
        payload.event_type.as_deref(),
        cli_type,
    );

    let signal = SignalFile {
        status,
        cli_type: Some(cli_type),
        cli_session_id,
        approval_policy: payload.approval_policy,
        sandbox_policy_type: sandbox_policy_type(payload.sandbox_policy.as_ref()),
        codirigent_session_uuid,
        codirigent_session_id,
        ts: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };

    if let Some(signals_dir) = signals_dir() {
        let _ = std::fs::create_dir_all(&signals_dir);
        let path = signals_dir.join(format!("{}.json", filename_session_id));
        if let Ok(json) = serde_json::to_string(&signal) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn read_payload() -> Option<HookPayload> {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_ok() {
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            if let Ok(payload) = serde_json::from_str::<HookPayload>(trimmed) {
                return Some(payload);
            }
        }
    }

    let args: Vec<String> = env::args().collect();
    args.get(1..)
        .and_then(|parts| parts.last())
        .and_then(|last| serde_json::from_str(last).ok())
}

fn sandbox_policy_type(policy: Option<&serde_json::Value>) -> Option<String> {
    match policy? {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Object(map) => map
            .get("type")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        _ => None,
    }
}

fn map_status(
    hook_event: Option<&str>,
    notification_type: Option<&str>,
    event_type: Option<&str>,
    cli_type: &'static str,
) -> &'static str {
    match cli_type {
        CLI_TYPE_CLAUDE => map_claude_status(hook_event, notification_type),
        CLI_TYPE_GEMINI => map_gemini_status(hook_event, notification_type, event_type),
        _ => map_codex_status(event_type),
    }
}

fn infer_cli_type(payload: &HookPayload) -> &'static str {
    infer_cli_type_with_gemini_env(payload, env::var_os("GEMINI_SESSION_ID").is_some())
}

fn infer_cli_type_with_gemini_env(payload: &HookPayload, gemini_env_present: bool) -> &'static str {
    match payload.cli_type.as_deref() {
        Some(CLI_TYPE_CLAUDE) => return CLI_TYPE_CLAUDE,
        Some(CLI_TYPE_CODEX) => return CLI_TYPE_CODEX,
        Some(CLI_TYPE_GEMINI) => return CLI_TYPE_GEMINI,
        _ => {}
    }

    if is_gemini_hook_event(payload.hook_event_name.as_deref()) || gemini_env_present {
        return CLI_TYPE_GEMINI;
    }

    if payload.hook_event_name.is_some() {
        return CLI_TYPE_CLAUDE;
    }

    if payload.event_type.is_some() {
        return CLI_TYPE_CODEX;
    }

    CLI_TYPE_GEMINI
}

fn is_gemini_hook_event(hook_event: Option<&str>) -> bool {
    matches!(
        hook_event,
        Some(
            "BeforeAgent"
                | "AfterAgent"
                | "BeforeTool"
                | "AfterTool"
                | "BeforeModel"
                | "AfterModel"
                | "SessionStart"
                | "SessionEnd"
        )
    )
}

fn map_claude_status(hook_event: Option<&str>, notification_type: Option<&str>) -> &'static str {
    match hook_event {
        Some("UserPromptSubmit") => "working",
        Some("Stop") => "response_ready",
        Some("Notification") => match notification_type {
            Some("permission_prompt") => "needs_attention",
            Some("idle_prompt") => "response_ready",
            _ => "idle",
        },
        _ => "idle",
    }
}

fn map_gemini_status(
    hook_event: Option<&str>,
    notification_type: Option<&str>,
    event_type: Option<&str>,
) -> &'static str {
    match hook_event {
        Some("BeforeAgent" | "BeforeTool" | "BeforeModel") => return "working",
        Some("AfterAgent") => return "response_ready",
        Some("AfterTool" | "AfterModel") => return "working",
        Some("SessionEnd") => return "idle",
        Some("Notification") => {
            let notification_type = notification_type.unwrap_or("").to_ascii_lowercase();
            if notification_type.contains("permission")
                || notification_type.contains("approval")
                || notification_type.contains("prompt")
                || notification_type.contains("input")
            {
                return "needs_attention";
            }
            return "idle";
        }
        _ => {}
    }

    let event_type = event_type.unwrap_or("").to_ascii_lowercase();

    if event_type.contains("working") || event_type.contains("started") {
        return "working";
    }

    if event_type.contains("attention")
        || event_type.contains("prompt")
        || event_type.contains("input")
        || event_type.contains("ask")
    {
        return "needs_attention";
    }

    if event_type.contains("ready")
        || event_type.contains("stopped")
        || event_type.contains("finished")
        || event_type.contains("complete")
    {
        return "response_ready";
    }

    "idle"
}

fn map_codex_status(event_type: Option<&str>) -> &'static str {
    let event_type = event_type.unwrap_or("").to_ascii_lowercase();

    if event_type == "agent-turn-complete"
        || event_type == "task_complete"
        || event_type == "response.completed"
        || event_type == "response.done"
        || event_type == "turn_complete"
    {
        return "response_ready";
    }

    if event_type == "task_started" {
        return "working";
    }

    if event_type.contains("permission")
        || event_type.contains("approval")
        || event_type.contains("question")
        || event_type.contains("ask")
    {
        return "needs_attention";
    }

    if event_type.contains("start")
        || event_type.contains("begin")
        || event_type.contains("turn-start")
        || event_type.contains("working")
    {
        return "working";
    }

    "idle"
}

/// Return true if a session id is safe as a filename stem.
fn is_safe_filename(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Returns the directory where this binary writes signal files.
///
/// Mirrors `codirigent_core::hook_signals_dir()` to avoid a heavy dependency
/// on `codirigent-core` from the hook binary crate.
fn signals_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("codirigent").join("signals"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let config_home = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".config"))
            })?;
        Some(config_home.join("codirigent").join("signals"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_status_user_prompt_submit_is_working() {
        assert_eq!(
            map_status(Some("UserPromptSubmit"), None, None, CLI_TYPE_CLAUDE),
            "working"
        );
    }

    #[test]
    fn map_status_stop_is_response_ready() {
        assert_eq!(
            map_status(Some("Stop"), None, None, CLI_TYPE_CLAUDE),
            "response_ready"
        );
    }

    #[test]
    fn map_status_notification_permission_prompt_is_needs_attention() {
        assert_eq!(
            map_status(
                Some("Notification"),
                Some("permission_prompt"),
                None,
                CLI_TYPE_CLAUDE
            ),
            "needs_attention"
        );
    }

    #[test]
    fn map_status_notification_idle_prompt_is_response_ready() {
        assert_eq!(
            map_status(
                Some("Notification"),
                Some("idle_prompt"),
                None,
                CLI_TYPE_CLAUDE
            ),
            "response_ready"
        );
    }

    #[test]
    fn map_status_notification_without_type_is_idle() {
        assert_eq!(
            map_status(Some("Notification"), None, None, CLI_TYPE_CLAUDE),
            "idle"
        );
    }

    #[test]
    fn map_status_unknown_event_is_idle() {
        assert_eq!(
            map_status(Some("UnknownEvent"), None, None, CLI_TYPE_CLAUDE),
            "idle"
        );
    }

    #[test]
    fn infer_cli_type_uses_gemini_hook_events() {
        let payload = HookPayload {
            session_id: None,
            hook_event_name: Some("BeforeAgent".to_string()),
            notification_type: None,
            event_type: None,
            cli_type: None,
            approval_policy: None,
            sandbox_policy: None,
        };
        assert_eq!(
            infer_cli_type_with_gemini_env(&payload, false),
            CLI_TYPE_GEMINI
        );
    }

    #[test]
    fn map_status_gemini_before_agent_is_working() {
        assert_eq!(
            map_status(Some("BeforeAgent"), None, None, CLI_TYPE_GEMINI),
            "working"
        );
    }

    #[test]
    fn map_status_gemini_after_agent_is_response_ready() {
        assert_eq!(
            map_status(Some("AfterAgent"), None, None, CLI_TYPE_GEMINI),
            "response_ready"
        );
    }

    #[test]
    fn map_status_gemini_notification_permission_is_needs_attention() {
        assert_eq!(
            map_status(
                Some("Notification"),
                Some("tool_permission_required"),
                None,
                CLI_TYPE_GEMINI
            ),
            "needs_attention"
        );
    }

    #[test]
    fn map_codex_status_turn_complete_is_response_ready() {
        assert_eq!(
            map_codex_status(Some("agent-turn-complete")),
            "response_ready"
        );
        assert_eq!(map_codex_status(Some("task_complete")), "response_ready");
    }

    #[test]
    fn map_codex_status_start_events_are_working() {
        assert_eq!(map_codex_status(Some("agent-turn-start")), "working");
        assert_eq!(map_codex_status(Some("turn_start")), "working");
        assert_eq!(map_codex_status(Some("task_started")), "working");
    }

    #[test]
    fn map_codex_status_permission_events_need_attention() {
        assert_eq!(
            map_codex_status(Some("permission_prompt")),
            "needs_attention"
        );
    }

    #[test]
    fn sandbox_policy_type_reads_named_object_policy() {
        let policy = serde_json::json!({ "type": "danger-full-access" });
        assert_eq!(
            super::sandbox_policy_type(Some(&policy)),
            Some("danger-full-access".to_string())
        );
    }

    #[test]
    fn sandbox_policy_type_reads_string_policy() {
        let policy = serde_json::json!("workspace-write");
        assert_eq!(
            super::sandbox_policy_type(Some(&policy)),
            Some("workspace-write".to_string())
        );
    }

    #[test]
    fn is_safe_filename_valid() {
        assert!(is_safe_filename("abc-123_DEF"));
        assert!(is_safe_filename("a"));
        assert!(is_safe_filename("some-uuid-1234"));
    }

    #[test]
    fn is_safe_filename_empty_rejected() {
        assert!(!is_safe_filename(""));
    }

    #[test]
    fn is_safe_filename_path_separators_rejected() {
        assert!(!is_safe_filename("../etc/passwd"));
        assert!(!is_safe_filename("foo/bar"));
        assert!(!is_safe_filename("foo\\bar"));
    }

    #[test]
    fn is_safe_filename_dot_rejected() {
        assert!(!is_safe_filename("foo.json"));
        assert!(!is_safe_filename(".hidden"));
    }
}
