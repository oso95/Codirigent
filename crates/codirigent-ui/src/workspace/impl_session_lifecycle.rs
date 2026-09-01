//! Session lifecycle management for WorkspaceView.
//!
//! This module contains all methods related to:
//! - Session creation (create, create_at, create_in_slot)
//! - Session restoration from disk
//! - Session closure (close, close_focused)
//! - State persistence to disk

use super::cli_helpers::is_safe_cli_session_id;
use super::gpui::{
    cli_type_badge_name, pending_git_file_counts, session_project_name, WorkspaceView,
};
use super::types::{RestoreShellFallback, SESSION_NAME_PREFIX, SESSION_SHELL_AUTO_LABEL};
use crate::terminal::Terminal;
use crate::terminal_header::TerminalHeader;
use crate::terminal_view::TerminalView;
use codirigent_core::{
    CliType, CodexExecutionMode, CodirigentEvent, EventBus, GridPosition, LayoutMode, PaneId,
    PaneStackState, PaneTabGroup, ProcessMonitor, Session, SessionId, SessionManager,
    SessionStatus, SlotId,
};
use codirigent_session::clipboard_service::ClipboardService;
use codirigent_session::DefaultSessionManager;
use gpui::Context;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tracing::{info, warn};

#[derive(Debug, Clone)]
struct RestoreSessionPlan {
    original_session_id: SessionId,
    session_name: String,
    working_dir: PathBuf,
    shell: Option<String>,
    group: Option<String>,
    color: Option<String>,
    claude_resume: Option<String>,
    codex_resume: Option<String>,
    codex_execution_mode: Option<CodexExecutionMode>,
    codex_started_at: Option<chrono::DateTime<chrono::Utc>>,
    gemini_resume: Option<String>,
}

#[derive(Debug, Clone)]
struct RestorePlan {
    layout: LayoutMode,
    pane_stacks: Vec<PaneStackState>,
    sessions: Vec<RestoreSessionPlan>,
}

#[derive(Debug)]
struct SessionBootstrapRequest {
    session_name: String,
    working_dir: PathBuf,
    requested_shell: Option<String>,
    launch_shell: Option<String>,
    shell_warning: Option<String>,
}

#[derive(Debug)]
struct SessionBootstrapResult {
    request: SessionBootstrapRequest,
    session_id: SessionId,
    session: Session,
    child_pid: Option<u32>,
}

#[derive(Debug)]
struct CompletedRestoreBootstrap {
    plan: RestoreSessionPlan,
    result: Result<SessionBootstrapResult, String>,
}

/// Safety-net timeout for prompt-aware resume dispatch. If a session's shell
/// has not produced any PTY output within this window, the resume command is
/// sent anyway (the shell may not emit OSC 133 or may be very slow to start).
const RESUME_COMMAND_FALLBACK_TIMEOUT: Duration = Duration::from_secs(3);

fn resume_command_is_ready(shell_output_seen: bool, pty_size_synced: bool) -> bool {
    shell_output_seen && pty_size_synced
}

fn timed_out_resume_can_dispatch(visible: bool, pty_size_synced: bool) -> bool {
    pty_size_synced || !visible
}

fn legacy_pane_stacks_from_groups(
    saved_sessions: &[Session],
    pane_tab_groups: &[PaneTabGroup],
) -> Vec<PaneStackState> {
    let mut groups = pane_tab_groups.to_vec();
    groups.sort_by_key(|group| match group.pane {
        PaneId::GridCell { index } => (0u8, index),
        PaneId::SplitSlot { slot } => (1u8, slot.0 as usize),
    });

    let valid_sessions: HashSet<SessionId> =
        saved_sessions.iter().map(|session| session.id).collect();
    let mut assigned = HashSet::new();
    let mut stacks = Vec::new();

    for group in groups {
        let session_ids = group
            .session_ids
            .into_iter()
            .filter(|session_id| {
                valid_sessions.contains(session_id) && assigned.insert(*session_id)
            })
            .collect::<Vec<_>>();
        if session_ids.is_empty() {
            continue;
        }

        let active_session_id = if session_ids.contains(&group.active_session_id) {
            group.active_session_id
        } else {
            session_ids[0]
        };
        stacks.push(PaneStackState {
            session_ids,
            active_session_id,
        });
    }

    stacks.extend(
        saved_sessions
            .iter()
            .map(|session| session.id)
            .filter(|session_id| !assigned.contains(session_id))
            .map(|session_id| PaneStackState {
                session_ids: vec![session_id],
                active_session_id: session_id,
            }),
    );

    stacks
}

fn next_available_session_number(
    existing_sessions: &[Session],
    reserved_numbers: &HashSet<u64>,
) -> u64 {
    let existing_numbers: HashSet<u64> = existing_sessions
        .iter()
        .filter_map(|session| {
            session
                .name
                .strip_prefix(SESSION_NAME_PREFIX)
                .and_then(|number| number.parse::<u64>().ok())
        })
        .collect();

    let mut num = 1u64;
    while existing_numbers.contains(&num) || reserved_numbers.contains(&num) {
        num += 1;
    }
    num
}

fn restore_resume_commands(plan: &RestoreSessionPlan) -> Vec<&str> {
    [
        plan.claude_resume.as_deref(),
        plan.codex_resume.as_deref(),
        plan.gemini_resume.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn restore_plan_cli_type(plan: &RestoreSessionPlan) -> CliType {
    if plan.codex_resume.is_some()
        || plan.codex_execution_mode.is_some()
        || plan.codex_started_at.is_some()
    {
        CliType::CodexCli
    } else if plan.claude_resume.is_some() {
        CliType::ClaudeCode
    } else if plan.gemini_resume.is_some() {
        CliType::GeminiCli
    } else {
        CliType::GenericShell
    }
}

fn bootstrap_session(
    session_manager: Arc<Mutex<DefaultSessionManager>>,
    request: SessionBootstrapRequest,
) -> Result<SessionBootstrapResult, String> {
    let manager = session_manager
        .lock()
        .map_err(|_| "session manager mutex poisoned".to_string())?;

    let session_id = manager
        .create_session(
            request.session_name.clone(),
            request.working_dir.clone(),
            request.launch_shell.clone(),
        )
        .map_err(|error| error.to_string())?;

    let child_pid = manager.get_child_pid(session_id);
    let mut session = manager.get_session(session_id).unwrap_or_else(|| {
        Session::new(
            session_id,
            request.session_name.clone(),
            request.working_dir.clone(),
        )
    });
    session.shell = request.requested_shell.clone();

    Ok(SessionBootstrapResult {
        request,
        session_id,
        session,
        child_pid,
    })
}

fn resolve_restore_shell_choice(
    requested_shell: Option<&str>,
    available_shells: &[String],
    configured_shell: Option<&str>,
) -> (Option<String>, Option<String>) {
    let requested_shell = requested_shell
        .map(str::trim)
        .filter(|shell| !shell.is_empty())
        .map(str::to_string);
    let configured_shell = configured_shell
        .map(str::trim)
        .filter(|shell| !shell.is_empty())
        .map(str::to_string);

    match requested_shell {
        Some(shell) if available_shells.iter().any(|candidate| candidate == &shell) => {
            (Some(shell), None)
        }
        Some(shell) => (configured_shell, Some(shell)),
        None => (configured_shell, None),
    }
}

fn restore_shell_fallback_message(fallback: &RestoreShellFallback) -> String {
    format!(
        "Requested shell '{}' was unavailable, so this session was opened with {}.",
        fallback.requested_shell, fallback.effective_shell_label
    )
}

fn layout_profile_for_restore(layout: &LayoutMode) -> Option<crate::layout::LayoutProfile> {
    if let Some(profile) = crate::layout::LayoutProfile::from_mode(layout) {
        return Some(profile);
    }

    match layout {
        LayoutMode::Custom { positions } => custom_positions_layout_profile(positions),
        _ => None,
    }
}

fn custom_positions_layout_profile(
    positions: &[(SessionId, GridPosition)],
) -> Option<crate::layout::LayoutProfile> {
    let max_row = positions.iter().map(|(_, position)| position.row).max()?;
    let max_col = positions.iter().map(|(_, position)| position.col).max()?;
    crate::layout::LayoutProfile::custom(max_row + 1, max_col + 1)
}

fn restore_layout_capacity(layout: &LayoutMode) -> usize {
    match layout {
        LayoutMode::Grid { rows, cols } => (*rows as usize) * (*cols as usize),
        LayoutMode::Single => 1,
        LayoutMode::Custom { positions } => layout_profile_for_restore(layout)
            .map(|profile| profile.max_sessions())
            .unwrap_or_else(|| positions.len()),
        LayoutMode::SplitTree { root } => root.leaf_count(),
    }
}

fn expanded_restore_layout(session_count: usize) -> LayoutMode {
    if session_count <= 1 {
        LayoutMode::Single
    } else if session_count <= 4 {
        LayoutMode::Grid { rows: 2, cols: 2 }
    } else if session_count <= 6 {
        LayoutMode::Grid { rows: 2, cols: 3 }
    } else if session_count <= 9 {
        LayoutMode::Grid { rows: 3, cols: 3 }
    } else {
        let cols = 4u32;
        let rows = (session_count as u32).div_ceil(cols);
        LayoutMode::Grid { rows, cols }
    }
}

fn staging_layout_for_restore(saved_layout: &LayoutMode, session_count: usize) -> LayoutMode {
    if restore_layout_capacity(saved_layout) >= session_count {
        saved_layout.clone()
    } else {
        expanded_restore_layout(session_count)
    }
}

/// Read the `permissionMode` value from the last complete JSON line of a
/// Claude Code JSONL session file.
///
/// Returns `None` if the file cannot be found or does not contain the field.
fn read_claude_permission_mode(claude_session_id: &str) -> Option<String> {
    let home = {
        #[cfg(target_os = "windows")]
        {
            std::env::var("USERPROFILE").ok().map(PathBuf::from)?
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::env::var("HOME").ok().map(PathBuf::from)?
        }
    };
    let projects_dir = home.join(".claude").join("projects");
    // Search all project dirs for <claude_session_id>.jsonl
    let target = format!("{}.jsonl", claude_session_id);
    let entries = std::fs::read_dir(&projects_dir).ok()?;
    for project_entry in entries.flatten() {
        let jsonl_path = project_entry.path().join(&target);
        if !jsonl_path.exists() {
            continue;
        }
        // Scan lines in reverse to find the most recent entry that carries
        // permissionMode — not every line has this field.
        let content = std::fs::read_to_string(&jsonl_path).ok()?;
        for line in content.lines().rev() {
            if line.trim().is_empty() || !line.contains("permissionMode") {
                continue;
            }
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(mode) = obj.get("permissionMode").and_then(|v| v.as_str()) {
                    return Some(mode.to_owned());
                }
            }
        }
    }
    None
}

#[derive(Debug, Deserialize)]
struct CodexRolloutEntry {
    #[serde(rename = "type", default)]
    entry_type: String,
    #[serde(default)]
    payload: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct CodexSessionMeta {
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    approval_mode: Option<String>,
    #[serde(default)]
    sandbox_policy: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct CodexTurnContext {
    #[serde(default)]
    approval_policy: Option<String>,
    #[serde(default)]
    sandbox_policy: Option<serde_json::Value>,
}

fn resolve_codex_home() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("CODEX_HOME") {
        return Some(PathBuf::from(home));
    }
    dirs::home_dir().map(|home| home.join(".codex"))
}

fn read_first_line(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut first = String::new();
    if reader.read_line(&mut first).ok()? == 0 {
        None
    } else {
        Some(first)
    }
}

fn read_codex_session_meta(path: &Path) -> Option<CodexSessionMeta> {
    let first_line = read_first_line(path)?;
    let entry: CodexRolloutEntry = serde_json::from_str(first_line.trim()).ok()?;
    if entry.entry_type != "session_meta" {
        return None;
    }
    let payload = entry.payload?;
    serde_json::from_value::<CodexSessionMeta>(payload).ok()
}

fn read_codex_turn_context(path: &Path) -> Option<CodexTurnContext> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    // Turn context appears near the beginning of each turn, so scanning a
    // small prefix keeps restore cheap even when rollout logs are long.
    for line in reader.lines().take(64).flatten() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Ok(entry) = serde_json::from_str::<CodexRolloutEntry>(trimmed) else {
            continue;
        };
        if entry.entry_type != "turn_context" {
            continue;
        }

        let Some(payload) = entry.payload else {
            continue;
        };
        let Ok(turn_context) = serde_json::from_value::<CodexTurnContext>(payload) else {
            continue;
        };
        if turn_context.approval_policy.is_some() || turn_context.sandbox_policy.is_some() {
            return Some(turn_context);
        }
    }

    None
}

fn collect_codex_rollout_files(base: &Path, out: &mut Vec<(PathBuf, SystemTime)>) {
    let entries = match fs::read_dir(base) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_codex_rollout_files(&path, out);
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !file_name.starts_with("rollout-") || !file_name.ends_with(".jsonl") {
            continue;
        }

        let Ok(metadata) = path.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        out.push((path, modified));
    }
}

fn codex_execution_mode_from_approval_and_sandbox(
    approval_policy: Option<&str>,
    sandbox_policy: Option<&serde_json::Value>,
) -> Option<CodexExecutionMode> {
    if !approval_policy.is_some_and(|value| value.eq_ignore_ascii_case("never")) {
        return None;
    }

    let sandbox_policy_type = match sandbox_policy? {
        serde_json::Value::String(value) => Some(value.as_str()),
        serde_json::Value::Object(map) => map.get("type").and_then(serde_json::Value::as_str),
        _ => None,
    }?;

    if sandbox_policy_type.eq_ignore_ascii_case("danger-full-access") {
        Some(CodexExecutionMode::Bypass)
    } else if sandbox_policy_type.eq_ignore_ascii_case("workspace-write")
        || sandbox_policy_type.eq_ignore_ascii_case("workspace_write")
    {
        Some(CodexExecutionMode::FullAuto)
    } else {
        None
    }
}

fn read_codex_execution_mode(path: &Path) -> Option<CodexExecutionMode> {
    let meta = read_codex_session_meta(path)?;

    meta.approval_mode
        .as_deref()
        .and_then(codex_execution_mode_from_str)
        .or_else(|| {
            codex_execution_mode_from_approval_and_sandbox(None, meta.sandbox_policy.as_ref())
        })
        .or_else(|| {
            let turn_context = read_codex_turn_context(path)?;
            codex_execution_mode_from_approval_and_sandbox(
                turn_context.approval_policy.as_deref(),
                turn_context.sandbox_policy.as_ref(),
            )
        })
}

fn read_saved_codex_execution_mode(
    working_directory: &Path,
    codex_session_id: &str,
) -> Option<CodexExecutionMode> {
    let codex_home = resolve_codex_home()?;
    let sessions_dir = codex_home.join("sessions");
    if !sessions_dir.is_dir() {
        return None;
    }

    let mut rollout_files = Vec::new();
    collect_codex_rollout_files(&sessions_dir, &mut rollout_files);
    if rollout_files.is_empty() {
        return None;
    }

    rollout_files.sort_unstable_by(|a, b| b.1.cmp(&a.1));

    if !codex_session_id.is_empty() {
        let session_suffix = format!("-{}.jsonl", codex_session_id);
        for (path, _) in &rollout_files {
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !file_name.ends_with(&session_suffix) {
                continue;
            }
            if let Some(mode) = read_codex_execution_mode(path) {
                return Some(mode);
            }
        }
    }

    let expected_cwd = working_directory.to_string_lossy();
    for (path, _) in &rollout_files {
        let Some(meta) = read_codex_session_meta(path) else {
            continue;
        };
        if meta.cwd.as_deref() == Some(expected_cwd.as_ref()) {
            return read_codex_execution_mode(path);
        }
    }

    None
}

fn is_codex_full_auto_mode(mode: &str) -> bool {
    mode.eq_ignore_ascii_case("full-auto")
        || mode.eq_ignore_ascii_case("full_auto")
        || mode.eq_ignore_ascii_case("fullauto")
}

fn is_codex_bypass_mode(mode: &str) -> bool {
    mode.eq_ignore_ascii_case("yolo")
        || mode.eq_ignore_ascii_case("bypass")
        || mode.eq_ignore_ascii_case("dangerously-bypass-approvals-and-sandbox")
        || mode.eq_ignore_ascii_case("dangerously_bypass_approvals_and_sandbox")
}

fn codex_execution_mode_from_str(mode: &str) -> Option<CodexExecutionMode> {
    if is_codex_bypass_mode(mode) {
        Some(CodexExecutionMode::Bypass)
    } else if is_codex_full_auto_mode(mode) {
        Some(CodexExecutionMode::FullAuto)
    } else {
        None
    }
}

fn codex_resume_flag(mode: CodexExecutionMode) -> &'static str {
    match mode {
        CodexExecutionMode::FullAuto => "--full-auto",
        CodexExecutionMode::Bypass => "--dangerously-bypass-approvals-and-sandbox",
    }
}

fn resolve_saved_codex_execution_mode(
    saved_mode: Option<CodexExecutionMode>,
    working_dir: &Path,
    codex_session_id: &str,
) -> Option<CodexExecutionMode> {
    saved_mode.or_else(|| read_saved_codex_execution_mode(working_dir, codex_session_id))
}

fn build_codex_resume_command(
    codex_session_id: &str,
    mode: Option<CodexExecutionMode>,
) -> Option<String> {
    if !is_safe_cli_session_id(codex_session_id) {
        warn!(
            session_id = %codex_session_id,
            "Ignoring unsafe persisted Codex session ID during restore"
        );
        return None;
    }

    let mut cmd = format!("codex resume {}", codex_session_id);
    if let Some(mode) = mode {
        cmd.push(' ');
        cmd.push_str(codex_resume_flag(mode));
    }
    cmd.push('\r');
    Some(cmd)
}

fn build_resume_command(program: &str, session_id: &str, extra_args: &[&str]) -> Option<String> {
    if !is_safe_cli_session_id(session_id) {
        warn!(
            program,
            session_id = %session_id,
            "Ignoring unsafe persisted CLI session ID during restore"
        );
        return None;
    }

    let mut cmd = format!("{} --resume {}", program, session_id);
    for arg in extra_args {
        cmd.push(' ');
        cmd.push_str(arg);
    }
    cmd.push('\r');
    Some(cmd)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use codirigent_core::{
        AppState, DefaultEventBus, LayoutNode, Session, SessionId, SessionManager, SplitDirection,
    };
    use codirigent_session::{normalize_path, DefaultSessionManager};
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    fn create_test_session_manager() -> Arc<Mutex<DefaultSessionManager>> {
        Arc::new(Mutex::new(DefaultSessionManager::new(Arc::new(
            DefaultEventBus::new(16),
        ))))
    }

    fn sample_working_dir() -> PathBuf {
        std::env::temp_dir()
    }

    #[test]
    fn next_available_session_number_skips_existing_and_reserved_values() {
        let sessions = vec![
            Session::new(SessionId(1), "Session 1".to_string(), sample_working_dir()),
            Session::new(SessionId(2), "Session 3".to_string(), sample_working_dir()),
        ];
        let reserved = HashSet::from([2u64, 4u64]);

        assert_eq!(next_available_session_number(&sessions, &reserved), 5);
    }

    #[test]
    fn restore_resume_commands_preserve_cli_order() {
        let plan = RestoreSessionPlan {
            original_session_id: SessionId(1),
            session_name: "Session 1".to_string(),
            working_dir: sample_working_dir(),
            shell: None,
            group: None,
            color: None,
            claude_resume: Some("claude --resume abc\r".to_string()),
            codex_resume: Some("codex resume def\r".to_string()),
            codex_execution_mode: None,
            codex_started_at: None,
            gemini_resume: Some("gemini --resume ghi\r".to_string()),
        };

        assert_eq!(
            restore_resume_commands(&plan),
            vec![
                "claude --resume abc\r",
                "codex resume def\r",
                "gemini --resume ghi\r",
            ]
        );
    }

    #[test]
    fn restored_agent_waits_for_shell_output_and_real_pty_size() {
        assert!(!resume_command_is_ready(false, false));
        assert!(!resume_command_is_ready(true, false));
        assert!(!resume_command_is_ready(false, true));
        assert!(resume_command_is_ready(true, true));
    }

    #[test]
    fn visible_timed_out_agent_still_waits_for_real_pty_size() {
        assert!(!timed_out_resume_can_dispatch(true, false));
        assert!(timed_out_resume_can_dispatch(true, true));
        assert!(timed_out_resume_can_dispatch(false, false));
    }

    #[test]
    fn bootstrap_session_returns_session_metadata() {
        let session_manager = create_test_session_manager();
        let temp = TempDir::new().unwrap();
        let request = SessionBootstrapRequest {
            session_name: "Session 1".to_string(),
            working_dir: temp.path().to_path_buf(),
            requested_shell: None,
            launch_shell: None,
            shell_warning: None,
        };

        let result = bootstrap_session(session_manager.clone(), request).unwrap();

        assert_eq!(result.session.name, "Session 1");
        assert_eq!(
            result.session.working_directory,
            normalize_path(temp.path())
        );
        assert_eq!(result.request.session_name, "Session 1");
        assert!(
            result.child_pid.is_some(),
            "bootstrap should capture a PTY child pid"
        );
        let manager = session_manager.lock().unwrap_or_else(|p| p.into_inner());
        assert!(manager.get_session(result.session_id).is_some());
    }

    #[test]
    fn bootstrap_session_preserves_requested_shell() {
        let session_manager = create_test_session_manager();
        let temp = TempDir::new().unwrap();
        let shell = codirigent_session::detect_available_shells()
            .into_iter()
            .find(|shell| !shell.is_empty())
            .expect("at least one shell should be detected in test environments");
        let request = SessionBootstrapRequest {
            session_name: "Session 1".to_string(),
            working_dir: temp.path().to_path_buf(),
            requested_shell: Some(shell.clone()),
            launch_shell: Some(shell.clone()),
            shell_warning: None,
        };

        let result = bootstrap_session(session_manager.clone(), request).unwrap();

        assert_eq!(result.session.shell, Some(shell.clone()));
        let manager = session_manager.lock().unwrap_or_else(|p| p.into_inner());
        assert_eq!(
            manager
                .get_session(result.session_id)
                .and_then(|session| session.shell),
            Some(shell)
        );
    }

    #[test]
    fn bootstrap_session_invalid_working_directory_returns_error_without_creating_session() {
        let session_manager = create_test_session_manager();
        let temp = TempDir::new().unwrap();
        let request = SessionBootstrapRequest {
            session_name: "Session 1".to_string(),
            working_dir: temp.path().join("missing-session-bootstrap"),
            requested_shell: None,
            launch_shell: None,
            shell_warning: None,
        };

        let result = bootstrap_session(session_manager.clone(), request);
        assert!(result.is_err());

        let manager = session_manager.lock().unwrap_or_else(|p| p.into_inner());
        assert_eq!(manager.session_count(), 0);
    }

    #[test]
    fn codex_resume_command_uses_resume_subcommand() {
        assert_eq!(
            build_codex_resume_command("session-123", None),
            Some("codex resume session-123\r".to_string())
        );
    }

    #[test]
    fn codex_resume_command_preserves_bypass_mode() {
        assert_eq!(
            build_codex_resume_command("session-123", Some(CodexExecutionMode::Bypass)),
            Some(
                "codex resume session-123 --dangerously-bypass-approvals-and-sandbox\r".to_string()
            )
        );
    }

    #[test]
    fn codex_resume_command_preserves_full_auto_mode() {
        assert_eq!(
            build_codex_resume_command("session-123", Some(CodexExecutionMode::FullAuto)),
            Some("codex resume session-123 --full-auto\r".to_string())
        );
    }

    #[test]
    fn codex_resume_command_rejects_unsafe_session_id() {
        assert_eq!(build_codex_resume_command("session-123;rm", None), None);
    }

    #[test]
    fn codex_execution_mode_can_be_inferred_from_turn_context() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("rollout.jsonl");
        let cwd = tmp.path().display().to_string();
        let session_meta = serde_json::json!({
            "type": "session_meta",
            "payload": { "cwd": cwd },
        });
        let turn_context = serde_json::json!({
            "type": "turn_context",
            "payload": {
                "approval_policy": "never",
                "sandbox_policy": { "type": "danger-full-access" },
            },
        });
        fs::write(&path, format!("{session_meta}\n{turn_context}")).unwrap();

        assert_eq!(
            read_codex_execution_mode(&path),
            Some(CodexExecutionMode::Bypass)
        );
    }

    #[test]
    fn build_restore_plan_preserves_saved_custom_grid_layout() {
        let fallback_dir = sample_working_dir();
        let mut session = Session::new(SessionId(1), "Session 1".to_string(), fallback_dir.clone());
        session.shell = Some("bash".to_string());
        let state = AppState {
            sessions: vec![session],
            layout: LayoutMode::Grid { rows: 1, cols: 4 },
            pane_tab_groups: Vec::new(),
            pane_stacks: Vec::new(),
            updated_at: None,
            window_bounds: None,
        };

        let plan = WorkspaceView::build_restore_plan(state, fallback_dir).unwrap();
        assert_eq!(plan.layout, LayoutMode::Grid { rows: 1, cols: 4 });
        assert_eq!(plan.sessions[0].shell, Some("bash".to_string()));
    }

    #[test]
    fn build_restore_plan_preserves_saved_split_tree_layout() {
        let fallback_dir = sample_working_dir();
        let split_tree = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf { slot: SlotId(0) }),
            second: Box::new(LayoutNode::Leaf { slot: SlotId(1) }),
        };
        let state = AppState {
            sessions: vec![Session::new(
                SessionId(1),
                "Session 1".to_string(),
                fallback_dir.clone(),
            )],
            layout: LayoutMode::SplitTree {
                root: split_tree.clone(),
            },
            pane_tab_groups: Vec::new(),
            pane_stacks: Vec::new(),
            updated_at: None,
            window_bounds: None,
        };

        let plan = WorkspaceView::build_restore_plan(state, fallback_dir).unwrap();
        assert_eq!(
            plan.layout,
            LayoutMode::SplitTree {
                root: split_tree.clone(),
            }
        );
        assert_eq!(
            restore_layout_capacity(&plan.layout),
            split_tree.leaf_count()
        );
    }

    #[test]
    fn staging_layout_for_restore_expands_when_saved_layout_is_too_small() {
        assert_eq!(
            staging_layout_for_restore(&LayoutMode::Single, 3),
            LayoutMode::Grid { rows: 2, cols: 2 }
        );
    }

    #[test]
    fn layout_profile_for_restore_supports_legacy_custom_positions() {
        let profile = layout_profile_for_restore(&LayoutMode::Custom {
            positions: vec![
                (SessionId(1), GridPosition { row: 0, col: 0 }),
                (SessionId(2), GridPosition { row: 1, col: 2 }),
            ],
        });

        assert_eq!(
            profile,
            Some(crate::layout::LayoutProfile::Custom { rows: 2, cols: 3 })
        );
    }

    #[test]
    fn resolve_restore_shell_choice_uses_requested_shell_when_available() {
        let available_shells = vec!["".to_string(), "bash".to_string(), "zsh".to_string()];
        assert_eq!(
            resolve_restore_shell_choice(Some("zsh"), &available_shells, Some("bash")),
            (Some("zsh".to_string()), None)
        );
    }

    #[test]
    fn resolve_restore_shell_choice_falls_back_to_auto_when_requested_shell_missing() {
        let available_shells = vec!["".to_string(), "bash".to_string(), "zsh".to_string()];
        assert_eq!(
            resolve_restore_shell_choice(Some("pwsh"), &available_shells, Some("bash")),
            (Some("bash".to_string()), Some("pwsh".to_string()))
        );
    }

    #[test]
    fn resolve_restore_shell_choice_preserves_auto_behavior_for_configured_default_shell() {
        let available_shells = vec!["".to_string(), "bash".to_string()];
        assert_eq!(
            resolve_restore_shell_choice(Some("pwsh"), &available_shells, Some("missing")),
            (Some("missing".to_string()), Some("pwsh".to_string()))
        );
        assert_eq!(
            resolve_restore_shell_choice(None, &available_shells, Some("missing")),
            (Some("missing".to_string()), None)
        );
    }

    #[test]
    fn restore_shell_fallback_message_uses_effective_shell_label() {
        let fallback = RestoreShellFallback {
            requested_shell: "pwsh".to_string(),
            effective_shell_label: "bash".to_string(),
        };

        assert_eq!(
            restore_shell_fallback_message(&fallback),
            "Requested shell 'pwsh' was unavailable, so this session was opened with bash."
        );
    }

    #[test]
    fn restore_shell_fallback_message_uses_explicit_fallback_label() {
        let fallback = RestoreShellFallback {
            requested_shell: "pwsh".to_string(),
            effective_shell_label: "zsh".to_string(),
        };

        assert_eq!(
            restore_shell_fallback_message(&fallback),
            "Requested shell 'pwsh' was unavailable, so this session was opened with zsh."
        );
    }

    #[test]
    fn shell_display_label_normalizes_shell_paths() {
        assert_eq!(WorkspaceView::shell_display_label(Some("/bin/zsh")), "zsh");
        assert_eq!(
            WorkspaceView::shell_display_label(Some(r"C:\Windows\System32\cmd.exe")),
            "cmd"
        );
        assert_eq!(
            WorkspaceView::shell_display_label(Some(
                r"C:\Windows\System32\WindowsPowerShell\v1.0\POWERSHELL.EXE"
            )),
            "POWERSHELL"
        );
        assert_eq!(WorkspaceView::shell_display_label(None), "Auto");
    }

    #[test]
    fn restore_plan_cli_type_prefers_known_resume_metadata() {
        let base = RestoreSessionPlan {
            original_session_id: SessionId(1),
            session_name: "Session 1".to_string(),
            working_dir: PathBuf::from("/tmp"),
            shell: None,
            group: None,
            color: None,
            claude_resume: None,
            codex_resume: None,
            codex_execution_mode: None,
            codex_started_at: None,
            gemini_resume: None,
        };

        let mut claude = base.clone();
        claude.claude_resume = Some("claude --resume".to_string());
        assert_eq!(restore_plan_cli_type(&claude), CliType::ClaudeCode);

        let mut gemini = base.clone();
        gemini.gemini_resume = Some("gemini resume".to_string());
        assert_eq!(restore_plan_cli_type(&gemini), CliType::GeminiCli);

        let mut codex = base.clone();
        codex.codex_execution_mode = Some(CodexExecutionMode::FullAuto);
        assert_eq!(restore_plan_cli_type(&codex), CliType::CodexCli);

        assert_eq!(restore_plan_cli_type(&base), CliType::GenericShell);
    }

    #[test]
    fn restore_resume_commands_empty_for_plan_with_no_cli_fields() {
        let plan = RestoreSessionPlan {
            original_session_id: SessionId(1),
            session_name: "Session 1".to_string(),
            working_dir: sample_working_dir(),
            shell: None,
            group: None,
            color: None,
            claude_resume: None,
            codex_resume: None,
            codex_execution_mode: None,
            codex_started_at: None,
            gemini_resume: None,
        };
        assert!(restore_resume_commands(&plan).is_empty());
    }
}

impl WorkspaceView {
    pub(super) fn detected_shell_options(&self) -> Vec<String> {
        let mut shells = self
            .cache
            .detected_shells
            .clone()
            .unwrap_or_else(codirigent_session::detect_available_shells);
        shells.retain(|shell| !shell.trim().is_empty());
        shells.sort();
        shells.dedup();
        shells.insert(0, String::new());
        shells
    }

    fn configured_shell(&self) -> Option<String> {
        let shell = self
            .effective_user_settings()
            .general
            .default_shell
            .trim()
            .to_string();
        (!shell.is_empty()).then_some(shell)
    }

    fn auto_launch_shell(&self) -> Option<String> {
        self.configured_shell()
    }

    fn launch_shell_for_requested_shell(&self, requested_shell: Option<&str>) -> Option<String> {
        requested_shell
            .filter(|shell| !shell.is_empty())
            .map(str::to_string)
            .or_else(|| self.auto_launch_shell())
    }

    fn restore_launch_shell(
        &self,
        requested_shell: Option<&str>,
    ) -> (Option<String>, Option<String>) {
        resolve_restore_shell_choice(
            requested_shell,
            &self.detected_shell_options(),
            self.configured_shell().as_deref(),
        )
    }

    fn shell_name_fragment(shell: &str) -> &str {
        let fragment = shell
            .trim()
            .rsplit(['/', '\\'])
            .find(|fragment| !fragment.is_empty())
            .unwrap_or(shell.trim());
        if fragment.len() > 4 && fragment[fragment.len() - 4..].eq_ignore_ascii_case(".exe") {
            &fragment[..fragment.len() - 4]
        } else {
            fragment
        }
    }

    pub(super) fn shell_display_label(shell: Option<&str>) -> String {
        shell
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(Self::shell_name_fragment)
            .unwrap_or(SESSION_SHELL_AUTO_LABEL)
            .to_string()
    }

    fn detected_auto_shell_label(&self) -> String {
        if let Ok(shell) = std::env::var("CODIRIGENT_SHELL") {
            if !shell.trim().is_empty() {
                return Self::shell_display_label(Some(&shell));
            }
        }

        #[cfg(unix)]
        {
            std::env::var("SHELL")
                .ok()
                .filter(|shell| !shell.trim().is_empty())
                .map(|shell| Self::shell_display_label(Some(&shell)))
                .unwrap_or_else(|| "bash".to_string())
        }

        #[cfg(windows)]
        {
            if self
                .detected_shell_options()
                .iter()
                .any(|shell| shell == "pwsh")
            {
                return "pwsh".to_string();
            }
            if self
                .detected_shell_options()
                .iter()
                .any(|shell| shell == "powershell")
            {
                return "powershell".to_string();
            }

            std::env::var("COMSPEC")
                .ok()
                .filter(|shell| !shell.trim().is_empty())
                .map(|shell| Self::shell_display_label(Some(&shell)))
                .unwrap_or_else(|| "cmd".to_string())
        }

        #[cfg(not(any(unix, windows)))]
        {
            "sh".to_string()
        }
    }

    fn effective_shell_label_for_launch_shell(&self, launch_shell: Option<&str>) -> String {
        launch_shell
            .map(str::trim)
            .filter(|shell| !shell.is_empty())
            .map(|shell| Self::shell_display_label(Some(shell)))
            .unwrap_or_else(|| self.detected_auto_shell_label())
    }

    fn record_effective_session_shell(
        &mut self,
        session_id: SessionId,
        launch_shell: Option<&str>,
    ) {
        let shell_label = self.effective_shell_label_for_launch_shell(launch_shell);
        self.cache
            .effective_shell_labels
            .insert(session_id, shell_label);
    }

    pub(super) fn session_cli_display_name(&self, session_id: SessionId) -> Option<String> {
        let cli_type = self
            .clipboard
            .clipboard_service
            .get_session_cli_type(session_id);
        cli_type_badge_name(cli_type).map(str::to_string)
    }

    pub(super) fn session_shell_warning_message(&self, session_id: SessionId) -> Option<String> {
        self.cache
            .restore_shell_fallbacks
            .get(&session_id)
            .map(restore_shell_fallback_message)
    }

    pub(super) fn session_shell_display(
        &self,
        session_id: SessionId,
        requested_shell: Option<&str>,
    ) -> (String, Option<String>) {
        let shell_label = self
            .cache
            .effective_shell_labels
            .get(&session_id)
            .cloned()
            .unwrap_or_else(|| Self::shell_display_label(requested_shell));

        (shell_label, self.session_shell_warning_message(session_id))
    }

    fn sync_manager_session_shell(
        &mut self,
        session_id: SessionId,
        requested_shell: Option<String>,
    ) {
        if let Ok(manager) = self.session_manager.lock() {
            let requested_shell = requested_shell.clone();
            manager.with_session_state_mut(session_id, move |state| {
                state.session.shell = requested_shell.clone();
            });
        }
    }

    fn record_restored_shell_warning(
        &mut self,
        session_id: SessionId,
        shell_warning: Option<String>,
        effective_shell_label: String,
    ) {
        if let Some(requested_shell) = shell_warning {
            self.cache.restore_shell_fallbacks.insert(
                session_id,
                RestoreShellFallback {
                    requested_shell,
                    effective_shell_label,
                },
            );
        } else {
            self.cache.restore_shell_fallbacks.remove(&session_id);
        }
    }

    fn release_session_create_reservation(
        &mut self,
        reserved_number: u64,
        target_pane: Option<PaneId>,
    ) {
        self.polling
            .pending_session_bootstrap_numbers
            .remove(&reserved_number);
        if let Some(PaneId::SplitSlot { slot }) = target_pane {
            self.polling.pending_session_bootstrap_slots.remove(&slot);
        }
    }

    fn build_terminal_header(
        &self,
        session: &Session,
        session_name: &str,
        group: Option<&String>,
        color: Option<&String>,
    ) -> TerminalHeader {
        let mut header = TerminalHeader::new(session_name, SessionStatus::Idle);
        if let Some(ref git_info) = session.git_info {
            let (additions, deletions) = pending_git_file_counts(git_info);
            header = header.with_git_info(git_info.branch.clone(), additions, deletions);
        }

        if let Some(project_name) = session_project_name(session) {
            header = header.with_project_name(project_name);
        }
        if let Some(cli_name) = self.session_cli_display_name(session.id) {
            header = header.with_cli_name(cli_name);
        }
        let (shell_label, shell_warning) =
            self.session_shell_display(session.id, session.shell.as_deref());
        header = header.with_shell(shell_label, shell_warning);

        if let Some(group) = group {
            header.group_name = Some(group.clone());
        }
        if let Some(color) = color {
            header.session_color = crate::sidebar::Color::from_hex(color);
        }

        header
    }

    fn create_terminal_view_for_session(&mut self, session_id: SessionId) {
        // Session IDs can be reused after closing a pane. A new ConPTY always
        // starts at its default 80x24 size and must be synchronized again.
        self.cache.pty_sizes.remove(&session_id);
        let (pty_tx, pty_rx) = tokio::sync::mpsc::unbounded_channel();
        let terminal = Terminal::new(24, 80, session_id, pty_tx);
        let theme = self.workspace.theme();
        let terminal_view = TerminalView::new(terminal, theme.clone());
        self.terminals.insert(session_id, terminal_view);
        self.pty_write_receivers.insert(session_id, pty_rx);
    }

    fn discard_bootstrapped_session(&mut self, session_id: SessionId) {
        self.terminals.remove(&session_id);
        self.pty_write_receivers.remove(&session_id);
        self.terminal_headers.remove(&session_id);
        self.cache.effective_shell_labels.remove(&session_id);
        self.cache.restore_shell_fallbacks.remove(&session_id);
        self.output_dispatcher.remove_session(session_id);
        self.polling.output_prepare_in_flight.remove(&session_id);
        self.with_detector(|detector| detector.stop_monitoring(session_id));
        self.with_session_manager(|manager| {
            if let Err(error) = manager.close_session(session_id) {
                warn!(
                    ?session_id,
                    %error,
                    "Failed to discard unattached bootstrapped session"
                );
            }
        });
    }

    fn attach_bootstrapped_session(
        &mut self,
        session: Session,
        session_name: &str,
        target_pane: Option<PaneId>,
        group: Option<&String>,
        color: Option<&String>,
    ) -> bool {
        let session_id = session.id;
        self.create_terminal_view_for_session(session_id);

        let added = match target_pane.clone() {
            Some(pane_id) => {
                if self
                    .workspace
                    .add_session_to_pane(session.clone(), pane_id.clone())
                {
                    true
                } else {
                    warn!(
                        ?session_id,
                        ?pane_id,
                        "Reserved pane unavailable when session bootstrap completed; falling back"
                    );
                    self.workspace.add_session(session.clone())
                }
            }
            None => self.workspace.add_session(session.clone()),
        };

        if !added {
            self.discard_bootstrapped_session(session_id);
            warn!(
                ?session_id,
                "Discarded bootstrapped session because the workspace could not attach it"
            );
            return false;
        }

        self.mark_layout_cache_dirty();
        let header = self.build_terminal_header(&session, session_name, group, color);
        self.terminal_headers.insert(session_id, header);
        self.output_dispatcher.mark_ready(session_id);
        self.with_session_manager(|manager| manager.mark_output_pending(session_id));
        true
    }

    fn start_bootstrapped_session_monitoring(
        &mut self,
        session_id: SessionId,
        child_pid: Option<u32>,
    ) {
        if let Some(pid) = child_pid {
            self.with_detector(|detector| {
                if let Err(error) = detector.start_monitoring(session_id, pid) {
                    warn!(
                        ?session_id,
                        %error,
                        "Failed to start monitoring bootstrapped session"
                    );
                }
            });
        }
    }

    fn finalize_created_session_bootstrap(
        &mut self,
        bootstrapped: SessionBootstrapResult,
        target_pane: Option<PaneId>,
        cx: &mut Context<Self>,
    ) {
        self.start_bootstrapped_session_monitoring(bootstrapped.session_id, bootstrapped.child_pid);
        self.sync_manager_session_shell(
            bootstrapped.session_id,
            bootstrapped.request.requested_shell.clone(),
        );
        self.record_effective_session_shell(
            bootstrapped.session_id,
            bootstrapped.request.launch_shell.as_deref(),
        );
        self.record_restored_shell_warning(
            bootstrapped.session_id,
            bootstrapped.request.shell_warning.clone(),
            self.effective_shell_label_for_launch_shell(
                bootstrapped.request.launch_shell.as_deref(),
            ),
        );

        let mut session = bootstrapped.session.clone();
        session.shell = bootstrapped.request.requested_shell.clone();

        if !self.attach_bootstrapped_session(
            session,
            &bootstrapped.request.session_name,
            target_pane.clone(),
            None,
            None,
        ) {
            return;
        }

        self.select_session_with_cx(bootstrapped.session_id, cx);

        if let Some(pane_id) = target_pane {
            info!(
                name = %bootstrapped.request.session_name,
                ?pane_id,
                "Created new session in pane via background bootstrap"
            );
        } else {
            info!(
                name = %bootstrapped.request.session_name,
                "Created new session via background bootstrap"
            );
        }

        self.refresh_derived_ui_state();
        self.save_state_to_disk(cx);
        cx.notify();
    }

    fn finalize_restored_session_bootstrap(
        &mut self,
        bootstrapped: SessionBootstrapResult,
        plan: RestoreSessionPlan,
    ) {
        self.start_bootstrapped_session_monitoring(bootstrapped.session_id, bootstrapped.child_pid);
        self.sync_manager_session_shell(
            bootstrapped.session_id,
            bootstrapped.request.requested_shell.clone(),
        );
        self.record_effective_session_shell(
            bootstrapped.session_id,
            bootstrapped.request.launch_shell.as_deref(),
        );
        self.record_restored_shell_warning(
            bootstrapped.session_id,
            bootstrapped.request.shell_warning.clone(),
            self.effective_shell_label_for_launch_shell(
                bootstrapped.request.launch_shell.as_deref(),
            ),
        );
        let restore_cli = self
            .effective_user_settings()
            .general
            .restore_cli_on_startup;
        let cli_type = if restore_cli {
            restore_plan_cli_type(&plan)
        } else {
            CliType::GenericShell
        };
        self.clipboard
            .clipboard_service
            .set_session_cli_type(bootstrapped.session_id, cli_type);

        if restore_cli && (plan.codex_execution_mode.is_some() || plan.codex_started_at.is_some()) {
            let codex_execution_mode = plan.codex_execution_mode;
            let codex_started_at = plan.codex_started_at;
            if let Ok(manager) = self.session_manager.lock() {
                manager.with_session_state_mut(bootstrapped.session_id, |state| {
                    state.session.codex_execution_mode = codex_execution_mode;
                    state.session.codex_started_at = codex_started_at;
                });
            }
        }

        let mut session = bootstrapped.session;
        session.shell = bootstrapped.request.requested_shell.clone();
        session.group = plan.group.clone();
        session.color = plan.color.clone();
        session.codex_execution_mode = if restore_cli {
            plan.codex_execution_mode
        } else {
            None
        };
        session.codex_started_at = if restore_cli {
            plan.codex_started_at
        } else {
            None
        };

        if !self.attach_bootstrapped_session(
            session,
            &plan.session_name,
            None,
            plan.group.as_ref(),
            plan.color.as_ref(),
        ) {
            return;
        }

        if plan.group.is_some() || plan.color.is_some() {
            self.with_session_manager(|manager| {
                let _ = manager.set_session_group(
                    bootstrapped.session_id,
                    plan.group.clone(),
                    plan.color.clone(),
                );
            });
        }
    }

    /// Enqueue resume commands to be dispatched when each session's shell
    /// produces its first output (prompt-aware dispatch).  The commands are
    /// stored in `polling.pending_resume_commands` and flushed by
    /// `dispatch_pending_resume_commands()` — either when the output pipeline
    /// delivers the first real bytes for a session, or after a fallback timeout.
    fn enqueue_restored_resume_commands(
        &mut self,
        pending_commands: Vec<(SessionId, Vec<String>)>,
    ) {
        let now = std::time::Instant::now();
        let mut enqueued_sessions = Vec::new();
        for (session_id, commands) in pending_commands {
            if !commands.is_empty() {
                info!(
                    ?session_id,
                    command_count = commands.len(),
                    "Enqueued resume commands (prompt-aware dispatch)"
                );
                self.polling
                    .pending_resume_commands
                    .insert(session_id, (now, commands));
                enqueued_sessions.push(session_id);
            }
        }

        // Output or the first layout pass may have completed while the restore
        // batch was still being assembled. Dispatch immediately when both
        // readiness signals are already present.
        for session_id in enqueued_sessions {
            self.dispatch_pending_resume_commands_for_session(session_id);
        }
    }

    /// Dispatch pending resume commands for a specific session.
    ///
    /// Called when the output pipeline delivers real bytes for this session or
    /// after PTY sizing completes. Both readiness signals must be present.
    pub(super) fn dispatch_pending_resume_commands_for_session(&mut self, session_id: SessionId) {
        if !resume_command_is_ready(
            self.polling.resume_shell_ready.contains(&session_id),
            self.cache.pty_sizes.contains_key(&session_id),
        ) {
            return;
        }

        self.dispatch_pending_resume_commands_now(session_id);
    }

    fn dispatch_pending_resume_commands_now(&mut self, session_id: SessionId) {
        let Some((_, commands)) = self.polling.pending_resume_commands.remove(&session_id) else {
            return;
        };
        self.polling.resume_shell_ready.remove(&session_id);
        info!(
            ?session_id,
            command_count = commands.len(),
            "Dispatching resume commands (shell produced output)"
        );
        if let Ok(manager) = self.session_manager.lock() {
            for command in commands {
                if let Err(error) = manager.send_input(session_id, command.as_bytes()) {
                    warn!(?session_id, %error, "Failed to send resume command");
                }
            }
            manager.mark_output_pending(session_id);
        }
    }

    /// Check for timed-out pending resume commands and dispatch them.
    ///
    /// This is the safety-net path: if a shell does not produce any PTY output
    /// within `RESUME_COMMAND_FALLBACK_TIMEOUT`, the resume command is sent
    /// anyway so the session does not remain stuck.
    pub(super) fn dispatch_timed_out_resume_commands(&mut self) {
        let expired: Vec<SessionId> = self
            .polling
            .pending_resume_commands
            .iter()
            .filter(|(_, (enqueued_at, _))| {
                enqueued_at.elapsed() >= RESUME_COMMAND_FALLBACK_TIMEOUT
            })
            .map(|(session_id, _)| *session_id)
            .collect();
        for session_id in expired {
            let visible = self.workspace.visible_session_ids().contains(&session_id);
            if !timed_out_resume_can_dispatch(
                visible,
                self.cache.pty_sizes.contains_key(&session_id),
            ) {
                info!(
                    ?session_id,
                    "Resume timeout reached, waiting for visible pane PTY size"
                );
                if let Some((enqueued_at, _)) =
                    self.polling.pending_resume_commands.get_mut(&session_id)
                {
                    *enqueued_at = std::time::Instant::now();
                }
                continue;
            }
            info!(
                ?session_id,
                "Resume command fallback timeout — dispatching without prompt"
            );
            self.dispatch_pending_resume_commands_now(session_id);
        }
    }

    fn spawn_create_session_bootstrap(
        &mut self,
        request: SessionBootstrapRequest,
        reserved_number: u64,
        target_pane: Option<PaneId>,
        cx: &mut Context<Self>,
    ) {
        let session_manager = self.session_manager.clone();
        let session_name = request.session_name.clone();

        cx.spawn(async move |this: gpui::WeakEntity<Self>, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { bootstrap_session(session_manager, request) })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.release_session_create_reservation(reserved_number, target_pane.clone());
                match result {
                    Ok(bootstrapped) => {
                        this.close_session_creation_modal();
                        this.finalize_created_session_bootstrap(
                            bootstrapped,
                            target_pane.clone(),
                            cx,
                        );
                    }
                    Err(error) => {
                        if let Some(modal) = this.modals.session_creation.as_mut() {
                            modal.pending = false;
                            modal.error = Some(format!("Failed to create session: {}", error));
                        }
                        warn!(
                            name = %session_name,
                            %error,
                            "Failed to create session via background bootstrap"
                        );
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn build_restore_plan(
        state: codirigent_core::AppState,
        fallback_dir: PathBuf,
    ) -> Option<RestorePlan> {
        let codirigent_core::AppState {
            sessions: saved_sessions,
            layout,
            pane_stacks,
            pane_tab_groups,
            ..
        } = state;

        if saved_sessions.is_empty() {
            return None;
        }

        let pane_stacks = if pane_stacks.is_empty() {
            legacy_pane_stacks_from_groups(&saved_sessions, &pane_tab_groups)
        } else {
            pane_stacks
        };

        let mut used_names = std::collections::HashSet::new();
        let mut used_claude_ids: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut used_codex_ids: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut used_gemini_ids: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut sessions = Vec::with_capacity(saved_sessions.len());

        for saved in saved_sessions {
            let working_dir = if saved.working_directory.exists() {
                saved.working_directory.clone()
            } else {
                fallback_dir.clone()
            };

            let mut session_name = saved.name.clone();
            if used_names.contains(&session_name) {
                let mut counter = 2;
                loop {
                    let candidate = format!("{} ({})", saved.name, counter);
                    if !used_names.contains(&candidate) {
                        session_name = candidate;
                        break;
                    }
                    counter += 1;
                }
                warn!(
                    original = %saved.name,
                    renamed = %session_name,
                    "Renamed duplicate session name during restoration"
                );
            }
            used_names.insert(session_name.clone());

            let claude_resume = saved
                .claude_session_id
                .as_ref()
                .filter(|id| used_claude_ids.insert((*id).clone()))
                .and_then(|claude_id| {
                    let permission_mode = if is_safe_cli_session_id(claude_id) {
                        read_claude_permission_mode(claude_id).unwrap_or_default()
                    } else {
                        String::new()
                    };
                    let extra_args = if permission_mode == "bypassPermissions" {
                        vec!["--dangerously-skip-permissions"]
                    } else {
                        Vec::new()
                    };
                    build_resume_command("claude", claude_id, &extra_args)
                });

            let codex_resume = saved
                .codex_session_id
                .as_ref()
                .filter(|id| used_codex_ids.insert((*id).clone()))
                .and_then(|codex_id| {
                    let mode = resolve_saved_codex_execution_mode(
                        saved.codex_execution_mode,
                        &working_dir,
                        codex_id,
                    );
                    build_codex_resume_command(codex_id, mode)
                });

            let gemini_resume = saved
                .gemini_session_id
                .as_ref()
                .filter(|id| used_gemini_ids.insert((*id).clone()))
                .and_then(|gemini_id| build_resume_command("gemini", gemini_id, &[]));

            sessions.push(RestoreSessionPlan {
                original_session_id: saved.id,
                session_name,
                working_dir,
                shell: saved.shell,
                group: saved.group,
                color: saved.color,
                claude_resume,
                codex_resume,
                codex_execution_mode: saved.codex_execution_mode,
                codex_started_at: saved.codex_started_at,
                gemini_resume,
            });
        }

        Some(RestorePlan {
            layout,
            pane_stacks,
            sessions,
        })
    }

    fn apply_restore_plan(&mut self, plan: RestorePlan, cx: &mut Context<Self>) {
        if plan.sessions.is_empty() {
            self.polling.restore_in_flight = false;
            return;
        }

        info!(count = plan.sessions.len(), "Restoring sessions from disk");

        let session_count = plan.sessions.len();
        let desired_layout = plan.layout.clone();
        let desired_pane_stacks = plan.pane_stacks.clone();
        let staging_layout = staging_layout_for_restore(&desired_layout, session_count);
        let reapply_saved_layout = staging_layout != desired_layout;
        let restore_batches = {
            let mut remaining = plan.sessions.into_iter();
            let mut batches = Vec::new();
            loop {
                let batch = remaining
                    .by_ref()
                    .take(2)
                    .map(|plan| {
                        let (launch_shell, shell_warning) =
                            self.restore_launch_shell(plan.shell.as_deref());
                        let request = SessionBootstrapRequest {
                            session_name: plan.session_name.clone(),
                            working_dir: plan.working_dir.clone(),
                            requested_shell: plan.shell.clone(),
                            launch_shell,
                            shell_warning,
                        };
                        (plan, request)
                    })
                    .collect::<Vec<_>>();
                if batch.is_empty() {
                    break;
                }
                batches.push(batch);
            }
            batches
        };
        self.apply_restored_layout_mode(&staging_layout);
        self.polling.restore_in_flight = true;
        let session_manager = self.session_manager.clone();
        cx.spawn(async move |this: gpui::WeakEntity<Self>, cx| {
            let mut restored_session_ids = std::collections::HashMap::new();
            let mut pending_resume_commands: Vec<(SessionId, Vec<String>)> = Vec::new();
            let total_batches = restore_batches.len();
            for (batch_index, batch) in restore_batches.into_iter().enumerate() {
                let is_last_batch = batch_index + 1 == total_batches;
                let desired_layout = desired_layout.clone();
                let session_manager = session_manager.clone();

                let completions = cx
                    .background_executor()
                    .spawn(async move {
                        batch
                            .into_iter()
                            .map(|(plan, request)| CompletedRestoreBootstrap {
                                plan,
                                result: bootstrap_session(session_manager.clone(), request),
                            })
                            .collect::<Vec<_>>()
                    })
                    .await;

                let _ = this.update(cx, |this, cx| {
                    for completion in completions {
                        match completion.result {
                            Ok(bootstrapped) => {
                                let restored_session_id = bootstrapped.session_id;
                                let original_session_id = completion.plan.original_session_id;
                                let restore_cli = this
                                    .effective_user_settings()
                                    .general
                                    .restore_cli_on_startup;
                                if restore_cli {
                                    let commands = restore_resume_commands(&completion.plan)
                                        .into_iter()
                                        .map(str::to_owned)
                                        .collect::<Vec<_>>();
                                    if !commands.is_empty() {
                                        pending_resume_commands
                                            .push((restored_session_id, commands));
                                    }
                                }
                                this.finalize_restored_session_bootstrap(
                                    bootstrapped,
                                    completion.plan,
                                );
                                restored_session_ids
                                    .insert(original_session_id, restored_session_id);
                            }
                            Err(error) => {
                                warn!(
                                    name = %completion.plan.session_name,
                                    %error,
                                    "Failed to restore session via background bootstrap"
                                );
                            }
                        }
                    }

                    if is_last_batch {
                        if reapply_saved_layout {
                            this.apply_restored_layout_mode(&desired_layout);
                        }
                        this.workspace
                            .restore_pane_stacks(&desired_pane_stacks, &restored_session_ids);
                        if let Some(focused_id) = this.workspace.focused_session_id() {
                            this.selection.selected_session_id = Some(focused_id);
                            this.drawer.set_selected_session(Some(focused_id));
                            this.sync_layout_derived_state();
                            this.sync_file_tree_to_focused_session(cx);
                        }
                        this.enqueue_restored_resume_commands(std::mem::take(
                            &mut pending_resume_commands,
                        ));
                        this.polling.restore_in_flight = false;
                        info!("Session restoration complete");
                        // Persist immediately so any session_uuids generated for
                        // legacy state (via serde default) are stable on the next
                        // restart and do not change between restarts.
                        this.save_state_to_disk(cx);
                    }

                    this.refresh_derived_ui_state();
                    cx.notify();
                });

                if !is_last_batch {
                    cx.background_executor()
                        .timer(Duration::from_millis(1))
                        .await;
                }
            }
        })
        .detach();
    }

    fn apply_restored_layout_mode(&mut self, layout: &LayoutMode) {
        match layout {
            LayoutMode::SplitTree { root } => self.workspace.set_split_tree(root.clone()),
            _ => {
                if let Some(profile) = layout_profile_for_restore(layout) {
                    self.workspace.set_layout(profile);
                }
            }
        }

        if let Some(profile) = layout_profile_for_restore(layout) {
            self.top_bar.set_active_layout(profile);
        } else if let Some(profile_id) = self
            .top_bar
            .profile_manager
            .list_profiles()
            .iter()
            .find(|profile| profile.layout == *layout)
            .map(|profile| profile.id.clone())
        {
            self.top_bar.set_active_profile_id(&profile_id);
        } else {
            self.top_bar.set_active_profile_id("__none__");
        }

        self.mark_layout_cache_dirty();
    }

    /// Create a new terminal session in the focused pane.
    pub fn create_session(&mut self, cx: &mut Context<Self>) {
        self.open_session_creation_modal(None);
        cx.notify();
    }

    /// Create a new session in a specific visible pane.
    pub fn create_session_in_pane(&mut self, pane_id: PaneId, cx: &mut Context<Self>) {
        self.open_session_creation_modal(Some(pane_id));
        cx.notify();
    }

    /// Create a new session in a specific split tree slot.
    pub fn create_session_in_slot(&mut self, slot: SlotId, cx: &mut Context<Self>) {
        self.open_session_creation_modal(Some(PaneId::SplitSlot { slot }));
        cx.notify();
    }

    pub(super) fn create_session_with_shell(
        &mut self,
        target_pane: Option<PaneId>,
        requested_shell: Option<String>,
        cx: &mut Context<Self>,
    ) {
        // Find the lowest available session number (reuse gaps from closed sessions)
        if let Some(PaneId::SplitSlot { slot }) = target_pane {
            if self.polling.pending_session_bootstrap_slots.contains(&slot) {
                warn!(?slot, "Session creation already pending for slot");
                return;
            }
            self.polling.pending_session_bootstrap_slots.insert(slot);
        }
        let num = next_available_session_number(
            self.workspace.sessions(),
            &self.polling.pending_session_bootstrap_numbers,
        );
        self.polling.pending_session_bootstrap_numbers.insert(num);
        let name = format!("{}{}", SESSION_NAME_PREFIX, num);
        self.next_session_id = self.next_session_id.max(num + 1);

        let working_dir = self
            .effective_user_settings()
            .general
            .default_working_dir
            .clone()
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .or_else(|| self.project.project_root.clone())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(std::env::temp_dir);

        let request = SessionBootstrapRequest {
            session_name: name,
            working_dir,
            launch_shell: self.launch_shell_for_requested_shell(requested_shell.as_deref()),
            requested_shell,
            shell_warning: None,
        };
        self.spawn_create_session_bootstrap(request, num, target_pane, cx);
    }

    /// Restore sessions from disk on startup without blocking the UI thread.
    pub(super) fn spawn_restore_sessions_from_disk(&mut self, cx: &mut Context<Self>) {
        if self.polling.restore_in_flight {
            return;
        }

        self.polling.restore_in_flight = true;
        let storage = self.persistence.storage.clone();
        let fallback_dir = self
            .project
            .project_root
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));

        cx.spawn(async move |this: gpui::WeakEntity<Self>, cx| {
            let restore_plan = cx
                .background_executor()
                .spawn(async move {
                    let state = match storage.load_state() {
                        Ok(state) => state,
                        Err(e) => {
                            info!("No saved state to restore: {}", e);
                            return None;
                        }
                    };
                    WorkspaceView::build_restore_plan(state, fallback_dir)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                if let Some(plan) = restore_plan {
                    this.apply_restore_plan(plan, cx);
                } else {
                    this.polling.restore_in_flight = false;
                }
            });
        })
        .detach();
    }

    /// Close the focused session.
    pub fn close_focused_session(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.workspace.focused_session_id() {
            self.close_session(id, cx);
        }
    }

    /// Close a specific session by ID.
    pub fn close_session(&mut self, id: SessionId, cx: &mut Context<Self>) {
        // Stop monitoring
        self.with_detector(|detector| {
            detector.stop_monitoring(id);
        });

        // Remove terminal view and PTY writer
        self.terminals.remove(&id);
        self.pty_write_receivers.remove(&id);

        // Close PTY session
        self.with_session_manager(|manager| {
            if let Err(e) = manager.close_session(id) {
                warn!("Failed to close session {}: {}", id, e);
            }
        });

        // Clean up compaction state
        if let Ok(mut svc) = self.persistence.compaction.lock() {
            svc.end_compaction(id);
        }
        self.cache.compaction_start_times.remove(&id);
        if let Ok(mut readers) = self.cli_readers.lock() {
            readers.cached_status.remove(&id);
        }
        self.polling.shell_input_buffers.remove(&id);
        self.polling.pending_resume_commands.remove(&id);
        self.polling.resume_shell_ready.remove(&id);
        self.cache.effective_shell_labels.remove(&id);
        self.cache.restore_shell_fallbacks.remove(&id);
        self.cache.pty_sizes.remove(&id);

        // Remove from output dispatcher tracking (ready/in-flight sets)
        self.output_dispatcher.remove_session(id);

        // Remove the terminal header for this session
        self.terminal_headers.remove(&id);

        // Remove from workspace
        self.workspace.remove_session(id);
        self.mark_layout_cache_dirty();
        self.event_bus
            .publish(CodirigentEvent::SessionClosed { id });
        info!(?id, "Closed session");
        self.refresh_derived_ui_state();
        self.save_state_to_disk(cx);
        cx.notify();
    }
}
