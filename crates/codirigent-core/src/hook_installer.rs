//! Auto-installs Codirigent hooks into supported CLI config files.
//!
//! On first launch (and on updates), Codirigent merges its required hooks into
//! Claude Code's settings file so the user never has to run a manual setup
//! command. The merge is additive and idempotent - existing hooks from other
//! plugins are preserved.
//!
//! # Hooks registered
//!
//! | Event | Matcher | Purpose |
//! |---|---|---|
//! | `UserPromptSubmit` | (all) | Mark session as "working" |
//! | `Notification` | `idle_prompt\|permission_prompt` | Mark as "response_ready" or "needs_attention" |
//! | `Stop` | (all) | Mark session as "response_ready" |
//!
//! # Signal files
//!
//! Each hook invocation writes a tiny JSON signal file to
//! `~/.config/codirigent/signals/<session_id>.json` (Windows:
//! `%APPDATA%\codirigent\signals\`). Codirigent polls this directory to
//! update session status without reading multi-megabyte JSONL logs.

use anyhow::{Context, Result};
use serde_json::{json, Value as JsonValue};
use std::path::{Path, PathBuf};
use toml::Value as TomlValue;
use tracing::{debug, info};

/// Substring that identifies a Codirigent hook command, used to detect and
/// upgrade legacy bare-name registrations.
const HOOK_MARKER: &str = "codirigent-hook";

/// Ensure Codirigent's hooks are present in `~/.claude/settings.json`.
///
/// `hook_binary` should be the full path to the `codirigent-hook` binary.
/// Using the full path avoids relying on PATH, which may not include the
/// binary's directory during Claude Code's hook execution.
///
/// Safe to call on every launch - the function is idempotent.
/// Returns `Ok(true)` if the file was modified, `Ok(false)` if already up to date.
pub fn ensure_hooks_installed(hook_binary: &Path) -> Result<bool> {
    validate_hook_binary(hook_binary)?;
    let settings_path =
        claude_settings_path().context("Could not determine ~/.claude/settings.json path")?;

    let command = shell_escaped_hook_command(hook_binary);
    let mut settings = read_settings(&settings_path)?;
    let modified = merge_hooks(&mut settings, &command)?;

    if modified {
        write_settings(&settings_path, &settings)?;
        info!("Codirigent hooks installed in {}", settings_path.display());
    } else {
        debug!("Codirigent hooks already present, no changes needed");
    }

    Ok(modified)
}

/// Ensure Codirigent's hooks are present in `~/.gemini/settings.json`.
///
/// Gemini CLI uses the same JSON hook structure as Claude Code, so the merge is
/// additive and idempotent. Existing hooks from other tools are preserved.
pub fn ensure_gemini_hooks_installed(hook_binary: &Path) -> Result<bool> {
    validate_hook_binary(hook_binary)?;
    let settings_path =
        gemini_settings_path().context("Could not determine ~/.gemini/settings.json path")?;

    let command = shell_escaped_hook_command(hook_binary);
    let mut settings = read_settings(&settings_path)?;
    let modified = merge_gemini_hooks(&mut settings, &command)?;

    if modified {
        write_settings(&settings_path, &settings)?;
        info!(
            "Codirigent Gemini hooks installed in {}",
            settings_path.display()
        );
    } else {
        debug!("Codirigent Gemini hooks already present, no changes needed");
    }

    Ok(modified)
}

/// Ensure Codirigent's hooks are present in `~/.codex/config.toml`.
///
/// `hook_binary` should be the full path to the `codirigent-hook` binary.
/// Using the full path avoids relying on PATH when Codex invokes notification
/// hooks from user shell environments.
///
/// Safe to call on every launch -- the function is idempotent.
/// Returns `Ok(true)` if the file was modified, `Ok(false)` if already up to date.
pub fn ensure_codex_hooks_installed(hook_binary: &Path) -> Result<bool> {
    validate_hook_binary(hook_binary)?;
    let config_path =
        codex_config_path().context("Could not determine ~/.codex/config.toml path")?;

    let command = hook_binary.to_string_lossy().into_owned();
    let mut settings = read_toml_settings(&config_path)?;
    let modified = merge_codex_notify(&mut settings, &command)?;

    if modified {
        write_toml_settings(&config_path, &settings)?;
        info!("Codirigent hooks installed in {}", config_path.display());
    } else {
        debug!("Codirigent Codex hooks already present, no changes needed");
    }

    Ok(modified)
}

/// Returns the full path to the `codirigent-hook` binary that lives next to
/// the currently-running executable.
///
/// Falls back to the bare binary name `codirigent-hook` (relying on PATH)
/// if the current executable path cannot be determined.
pub fn hook_binary_path() -> PathBuf {
    let hook_name = if cfg!(windows) {
        "codirigent-hook.exe"
    } else {
        "codirigent-hook"
    };
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|dir| dir.join(hook_name)))
        .unwrap_or_else(|| PathBuf::from(hook_name))
}

fn claude_settings_path() -> Option<PathBuf> {
    let home = home_dir()?;
    Some(home.join(".claude").join("settings.json"))
}

fn gemini_settings_path() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("GEMINI_CLI_HOME") {
        return Some(PathBuf::from(home).join("settings.json"));
    }
    home_dir().map(|home| home.join(".gemini").join("settings.json"))
}

fn codex_config_path() -> Option<PathBuf> {
    home_dir().map(|home| home.join(".codex").join("config.toml"))
}

fn validate_hook_binary(hook_binary: &Path) -> Result<()> {
    if hook_binary.is_absolute() && !hook_binary.is_file() {
        anyhow::bail!(
            "Codirigent hook binary does not exist: {}",
            hook_binary.display()
        );
    }
    Ok(())
}

fn shell_escaped_hook_command(hook_binary: &Path) -> String {
    let raw = hook_binary.to_string_lossy().into_owned();

    #[cfg(windows)]
    {
        // Claude and Gemini execute hooks through a POSIX-compatible shell on
        // Windows. Unquoted backslashes are treated as escape characters, so
        // normalize to the forward-slash form accepted by Windows executables.
        let normalized = raw.replace('\\', "/");
        format!("\"{normalized}\"")
    }

    #[cfg(not(windows))]
    {
        if raw.contains(' ') {
            format!("\"{raw}\"")
        } else {
            raw
        }
    }
}

/// Returns the directory where `codirigent-hook` writes signal files.
///
/// | Platform | Path |
/// |---|---|
/// | Windows | `%APPDATA%\codirigent\signals` |
/// | Linux/macOS | `$XDG_CONFIG_HOME/codirigent/signals` (falls back to `~/.config/codirigent/signals`) |
///
/// Note: `%APPDATA%` (roaming app data) is distinct from `%USERPROFILE%` used for
/// `~/.claude`. This is intentional - signal files are transient runtime data that
/// belongs in the per-machine config location, not in the user's profile root.
pub fn hook_signals_dir() -> Option<PathBuf> {
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
            .or_else(|| home_dir().map(|h| h.join(".config")))?;
        Some(config_home.join("codirigent").join("signals"))
    }
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("USERPROFILE").ok().map(PathBuf::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

fn read_settings(path: &Path) -> Result<JsonValue> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))
}

fn write_settings(path: &Path, settings: &JsonValue) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(settings).context("Failed to serialize settings")?;

    // Atomic write: write to a PID-scoped temp file then rename.
    // Using the process ID prevents two concurrent Codirigent instances from
    // clobbering each other's temp file during simultaneous startup.
    let tmp = path.with_file_name(format!(".settings-{}.tmp", std::process::id()));
    std::fs::write(&tmp, &json).with_context(|| format!("Failed to write {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("Failed to rename {} to {}", tmp.display(), path.display()))?;

    Ok(())
}

fn read_toml_settings(path: &Path) -> Result<TomlValue> {
    if !path.exists() {
        return Ok(TomlValue::Table(Default::default()));
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let settings =
        toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(settings)
}

fn write_toml_settings(path: &Path, settings: &TomlValue) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }
    let toml = toml::to_string_pretty(settings).context("Failed to serialize TOML settings")?;

    // Atomic write: write to a PID-scoped temp file then rename.
    // Using the process ID prevents two concurrent Codirigent instances from
    // clobbering each other's temp file during simultaneous startup.
    let tmp = path.with_file_name(format!(".config-{}.tmp", std::process::id()));
    std::fs::write(&tmp, &toml).with_context(|| format!("Failed to write {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("Failed to rename {} to {}", tmp.display(), path.display()))?;

    Ok(())
}

fn merge_codex_notify(settings: &mut TomlValue, command: &str) -> Result<bool> {
    let table = settings
        .as_table_mut()
        .context("codex config root must be a TOML table")?;

    let notify_value = match table.get_mut("notify") {
        None => {
            table.insert(
                "notify".to_string(),
                TomlValue::Array(vec![TomlValue::String(command.to_owned())]),
            );
            return Ok(true);
        }
        Some(v) => v,
    };

    match notify_value {
        TomlValue::Array(arr) => {
            let mut normalized = Vec::with_capacity(arr.len().max(1));
            let mut inserted_codirigent = false;
            let mut modified = false;

            for value in arr.iter() {
                match value.as_str() {
                    Some(existing) if existing == command => {
                        if inserted_codirigent {
                            modified = true;
                            continue;
                        }
                        normalized.push(TomlValue::String(command.to_owned()));
                        inserted_codirigent = true;
                    }
                    Some(existing) if existing.contains(HOOK_MARKER) => {
                        if !inserted_codirigent {
                            normalized.push(TomlValue::String(command.to_owned()));
                            inserted_codirigent = true;
                        }
                        modified = true;
                    }
                    _ => normalized.push(value.clone()),
                }
            }

            if !inserted_codirigent {
                normalized.push(TomlValue::String(command.to_owned()));
                modified = true;
            }

            if !modified && normalized.len() == arr.len() {
                return Ok(false);
            }

            *arr = normalized;
            Ok(true)
        }
        TomlValue::String(existing) => {
            if existing == command {
                Ok(false)
            } else if existing.contains(HOOK_MARKER) {
                *existing = command.to_owned();
                Ok(true)
            } else {
                *notify_value = TomlValue::Array(vec![
                    TomlValue::String(existing.clone()),
                    TomlValue::String(command.to_owned()),
                ]);
                Ok(true)
            }
        }
        _ => {
            *notify_value = TomlValue::Array(vec![TomlValue::String(command.to_owned())]);
            Ok(true)
        }
    }
}

/// Merge Codirigent's hooks into the settings value. Returns true if modified.
///
/// If a hook entry already exists with the correct `command`, it is left
/// unchanged. If a legacy bare-name entry (containing `HOOK_MARKER` but not
/// matching `command`) is found, it is upgraded in place to `command`.
fn merge_hooks(settings: &mut JsonValue, command: &str) -> Result<bool> {
    merge_json_hooks(settings, command, hook_definitions())
}

fn merge_gemini_hooks(settings: &mut JsonValue, command: &str) -> Result<bool> {
    merge_json_hooks(settings, command, gemini_hook_definitions())
}

fn merge_json_hooks(
    settings: &mut JsonValue,
    command: &str,
    definitions: &[(&'static str, &'static str, &'static str)],
) -> Result<bool> {
    let hooks = settings
        .as_object_mut()
        .context("settings.json root must be an object")?
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .context("hooks must be an object")?;

    let mut modified = false;

    for (event, matcher, description) in definitions {
        let event_hooks = hooks
            .entry(event.to_string())
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .with_context(|| format!("{event} hooks must be an array"))?;

        if already_installed(event_hooks, command) {
            continue;
        }

        // Upgrade a legacy bare-name entry rather than adding a duplicate.
        if upgrade_hook(event_hooks, command) {
            debug!("Upgraded Codirigent hook for {event} to full binary path");
            modified = true;
            continue;
        }

        debug!("Adding Codirigent hook for {event} ({description})");
        event_hooks.push(json!({
            "matcher": matcher,
            "hooks": [{ "type": "command", "command": command }]
        }));
        modified = true;
    }

    Ok(modified)
}

fn hook_definitions() -> &'static [(&'static str, &'static str, &'static str)] {
    &[
        ("UserPromptSubmit", "", "mark session as working"),
        (
            "Notification",
            "idle_prompt|permission_prompt",
            "mark session as response_ready or needs_attention",
        ),
        ("Stop", "", "mark session as response_ready"),
    ]
}

fn gemini_hook_definitions() -> &'static [(&'static str, &'static str, &'static str)] {
    &[
        ("BeforeAgent", "", "mark session as working"),
        ("AfterAgent", "", "mark session as response ready"),
        (
            "Notification",
            "",
            "mark session as needs attention or idle",
        ),
    ]
}

/// Returns true if any hook entry already uses exactly `command`.
fn already_installed(event_hooks: &[JsonValue], command: &str) -> bool {
    event_hooks.iter().any(|hook| {
        hook.get("hooks")
            .and_then(|h| h.as_array())
            .map(|cmds| {
                cmds.iter().any(|cmd| {
                    cmd.get("command")
                        .and_then(|c| c.as_str())
                        .map(|s| s == command)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    })
}

/// Upgrades a legacy bare-name hook (containing `HOOK_MARKER`) to `command`.
/// Returns true if any entry was updated.
fn upgrade_hook(event_hooks: &mut [JsonValue], command: &str) -> bool {
    let mut upgraded = false;
    for hook in event_hooks.iter_mut() {
        if let Some(cmds) = hook.get_mut("hooks").and_then(|h| h.as_array_mut()) {
            for cmd in cmds.iter_mut() {
                let current = cmd
                    .get("command")
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_owned());
                if let Some(c) = current {
                    if c.contains(HOOK_MARKER) && c != command {
                        cmd["command"] = json!(command);
                        upgraded = true;
                    }
                }
            }
        }
    }
    upgraded
}

#[cfg(test)]
mod tests {
    use super::*;

    const CMD: &str = "/usr/local/bin/codirigent-hook";
    const CMD2: &str = "/opt/codirigent/codirigent-hook";
    const CMD_SPACES: &str = r#""C:/Program Files/Codirigent/codirigent-hook.exe""#;

    #[test]
    fn missing_absolute_hook_binary_is_rejected() {
        let temp = tempfile::tempdir().expect("hook installer test should create temp dir");
        let missing = temp.path().join("codirigent-hook-missing");
        let error = validate_hook_binary(&missing)
            .expect_err("an absolute path to a missing hook must be rejected");
        assert!(error.to_string().contains("does not exist"));
    }

    #[test]
    fn bare_hook_binary_name_is_allowed() {
        validate_hook_binary(Path::new("codirigent-hook"))
            .expect("a bare hook command may rely on PATH");
    }

    #[cfg(windows)]
    #[test]
    fn windows_hook_path_without_spaces_is_bash_safe() {
        let command = shell_escaped_hook_command(Path::new(
            r"D:\study\codirigent\target\debug\codirigent-hook.exe",
        ));
        assert_eq!(
            command,
            r#""D:/study/codirigent/target/debug/codirigent-hook.exe""#
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_hook_path_with_spaces_is_bash_safe() {
        let command = shell_escaped_hook_command(Path::new(
            r"C:\Program Files\Codirigent\codirigent-hook.exe",
        ));
        assert_eq!(command, CMD_SPACES);
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_hook_path_escaping_is_unchanged() {
        assert_eq!(
            shell_escaped_hook_command(Path::new(CMD)),
            "/usr/local/bin/codirigent-hook"
        );
        assert_eq!(
            shell_escaped_hook_command(Path::new("/opt/Codirigent App/codirigent-hook")),
            r#""/opt/Codirigent App/codirigent-hook""#
        );
    }

    #[test]
    fn fresh_install_adds_three_hooks() {
        let mut settings = json!({});
        let modified = merge_hooks(&mut settings, CMD).expect("hook installer test should succeed");
        assert!(modified);

        let hooks = settings["hooks"]
            .as_object()
            .expect("hook installer test should succeed");
        assert!(hooks.contains_key("UserPromptSubmit"));
        assert!(hooks.contains_key("Notification"));
        assert!(hooks.contains_key("Stop"));
    }

    #[test]
    fn fresh_install_uses_provided_command() {
        let mut settings = json!({});
        merge_hooks(&mut settings, CMD).expect("hook installer test should succeed");
        let cmd = settings["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"]
            .as_str()
            .expect("hook installer test should succeed");
        assert_eq!(cmd, CMD);
    }

    #[test]
    fn idempotent_second_call() {
        let mut settings = json!({});
        merge_hooks(&mut settings, CMD).expect("hook installer test should succeed");
        let modified = merge_hooks(&mut settings, CMD).expect("hook installer test should succeed");
        assert!(!modified, "second call should not modify");
    }

    #[test]
    fn preserves_existing_hooks() {
        let mut settings = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {"matcher": "", "hooks": [{"type": "command", "command": "other-tool"}]}
                ]
            }
        });
        merge_hooks(&mut settings, CMD).expect("hook installer test should succeed");

        let arr = settings["hooks"]["UserPromptSubmit"]
            .as_array()
            .expect("hook installer test should succeed");
        assert_eq!(arr.len(), 2, "existing hook must be preserved");
    }

    #[test]
    fn does_not_duplicate_if_already_present() {
        let mut settings = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {"matcher": "", "hooks": [{"type": "command", "command": CMD}]}
                ],
                "Notification": [
                    {"matcher": "idle_prompt|permission_prompt", "hooks": [{"type": "command", "command": CMD}]}
                ],
                "Stop": [
                    {"matcher": "", "hooks": [{"type": "command", "command": CMD}]}
                ]
            }
        });
        let modified = merge_hooks(&mut settings, CMD).expect("hook installer test should succeed");
        assert!(!modified);

        for event in &["UserPromptSubmit", "Notification", "Stop"] {
            let arr = settings["hooks"][event]
                .as_array()
                .expect("hook installer test should succeed");
            assert_eq!(arr.len(), 1, "{event} must not be duplicated");
        }
    }

    #[test]
    fn upgrades_bare_name_to_full_path() {
        // Simulate a legacy install with just "codirigent-hook" (no path).
        let mut settings = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {"matcher": "", "hooks": [{"type": "command", "command": "codirigent-hook"}]}
                ],
                "Notification": [
                    {"matcher": "idle_prompt|permission_prompt", "hooks": [{"type": "command", "command": "codirigent-hook"}]}
                ],
                "Stop": [
                    {"matcher": "", "hooks": [{"type": "command", "command": "codirigent-hook"}]}
                ]
            }
        });
        let modified = merge_hooks(&mut settings, CMD).expect("hook installer test should succeed");
        assert!(modified, "legacy entry must be upgraded");

        for event in &["UserPromptSubmit", "Notification", "Stop"] {
            let arr = settings["hooks"][event]
                .as_array()
                .expect("hook installer test should succeed");
            // No duplicate added - the existing entry was upgraded in place.
            assert_eq!(arr.len(), 1, "{event} must not grow");
            let cmd = arr[0]["hooks"][0]["command"]
                .as_str()
                .expect("hook installer test should succeed");
            assert_eq!(cmd, CMD, "{event} command must be updated to full path");
        }
    }

    #[test]
    fn idempotent_after_upgrade() {
        let mut settings = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {"matcher": "", "hooks": [{"type": "command", "command": "codirigent-hook"}]}
                ],
                "Notification": [
                    {"matcher": "idle_prompt|permission_prompt", "hooks": [{"type": "command", "command": "codirigent-hook"}]}
                ],
                "Stop": [
                    {"matcher": "", "hooks": [{"type": "command", "command": "codirigent-hook"}]}
                ]
            }
        });
        merge_hooks(&mut settings, CMD).expect("hook installer test should succeed");
        // Second call with same command must be a no-op.
        let modified = merge_hooks(&mut settings, CMD).expect("hook installer test should succeed");
        assert!(!modified, "second call after upgrade must be idempotent");
    }

    #[test]
    fn upgrades_when_binary_path_changes() {
        // Simulate a previous install with CMD, now re-installed to CMD2.
        let mut settings = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {"matcher": "", "hooks": [{"type": "command", "command": CMD}]}
                ],
                "Notification": [
                    {"matcher": "idle_prompt|permission_prompt", "hooks": [{"type": "command", "command": CMD}]}
                ],
                "Stop": [
                    {"matcher": "", "hooks": [{"type": "command", "command": CMD}]}
                ]
            }
        });
        let modified =
            merge_hooks(&mut settings, CMD2).expect("hook installer test should succeed");
        assert!(modified, "changed path must trigger upgrade");

        for event in &["UserPromptSubmit", "Notification", "Stop"] {
            let arr = settings["hooks"][event]
                .as_array()
                .expect("hook installer test should succeed");
            assert_eq!(arr.len(), 1, "{event} must not grow");
            let cmd = arr[0]["hooks"][0]["command"]
                .as_str()
                .expect("hook installer test should succeed");
            assert_eq!(cmd, CMD2);
        }
    }

    #[test]
    fn path_with_spaces_is_stored_quoted() {
        // CMD_SPACES is the already-quoted form; verify merge_hooks stores it as-is
        // and that the quote is detectable (contains the marker).
        let mut settings = json!({});
        merge_hooks(&mut settings, CMD_SPACES).expect("hook installer test should succeed");
        let cmd = settings["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"]
            .as_str()
            .expect("hook installer test should succeed");
        assert_eq!(cmd, CMD_SPACES);
        assert!(cmd.contains(HOOK_MARKER));
    }

    #[test]
    fn unquoted_path_with_spaces_is_upgraded_to_quoted() {
        let unquoted = r"C:\Program Files\Codirigent\codirigent-hook.exe";
        let mut settings = json!({
            "hooks": {
                "UserPromptSubmit": [
                    {"matcher": "", "hooks": [{"type": "command", "command": unquoted}]}
                ],
                "Notification": [
                    {"matcher": "idle_prompt|permission_prompt", "hooks": [{"type": "command", "command": unquoted}]}
                ],
                "Stop": [
                    {"matcher": "", "hooks": [{"type": "command", "command": unquoted}]}
                ]
            }
        });
        let modified =
            merge_hooks(&mut settings, CMD_SPACES).expect("hook installer test should succeed");
        assert!(modified, "unquoted path must be upgraded to quoted form");

        for event in &["UserPromptSubmit", "Notification", "Stop"] {
            let arr = settings["hooks"][event]
                .as_array()
                .expect("hook installer test should succeed");
            assert_eq!(arr.len(), 1, "{event} must not grow");
            let cmd = arr[0]["hooks"][0]["command"]
                .as_str()
                .expect("hook installer test should succeed");
            assert_eq!(cmd, CMD_SPACES, "{event} must be quoted now");
        }
    }

    #[test]
    fn codex_config_adds_notify() {
        let mut settings: TomlValue =
            toml::from_str("").expect("hook installer test should succeed");
        let modified =
            merge_codex_notify(&mut settings, CMD).expect("hook installer test should succeed");
        assert!(modified);
        assert_eq!(
            settings["notify"]
                .as_array()
                .expect("hook installer test should succeed")[0]
                .as_str()
                .expect("hook installer test should succeed"),
            CMD
        );
    }

    #[test]
    fn codex_config_is_idempotent_for_notify_array() {
        let mut settings: TomlValue = toml::from_str(
            r#"
            notify = ["/usr/local/bin/codirigent-hook", "notify-send"]
            "#,
        )
        .expect("hook installer test should succeed");
        let modified =
            merge_codex_notify(&mut settings, CMD).expect("hook installer test should succeed");
        assert!(!modified);
        assert_eq!(
            settings["notify"]
                .as_array()
                .expect("hook installer test should succeed")
                .len(),
            2
        );
        assert_eq!(
            settings["notify"]
                .as_array()
                .expect("hook installer test should succeed")[0]
                .as_str()
                .expect("hook installer test should succeed"),
            "/usr/local/bin/codirigent-hook"
        );
    }

    #[test]
    fn codex_config_upgrades_string_notify_to_array() {
        let mut settings: TomlValue = toml::from_str(r#"notify = "/usr/bin/old""#)
            .expect("hook installer test should succeed");
        let modified =
            merge_codex_notify(&mut settings, CMD).expect("hook installer test should succeed");
        assert!(modified);
        let notify = settings["notify"]
            .as_array()
            .expect("hook installer test should succeed");
        assert_eq!(notify.len(), 2);
        assert_eq!(
            notify[0]
                .as_str()
                .expect("hook installer test should succeed"),
            "/usr/bin/old"
        );
        assert_eq!(
            notify[1]
                .as_str()
                .expect("hook installer test should succeed"),
            CMD
        );
    }

    #[test]
    fn codex_config_replaces_stale_codirigent_paths_in_notify_array() {
        let mut settings: TomlValue = toml::from_str(
            r#"
            notify = [
                "/Volumes/Codirigent 3/Codirigent.app/Contents/MacOS/codirigent-hook",
                "/Applications/Codirigent.app/Contents/MacOS/codirigent-hook",
                "notify-send"
            ]
            "#,
        )
        .expect("hook installer test should succeed");
        let modified =
            merge_codex_notify(&mut settings, CMD).expect("hook installer test should succeed");

        assert!(modified);
        assert_eq!(
            settings["notify"]
                .as_array()
                .expect("hook installer test should succeed"),
            &vec![
                TomlValue::String(CMD.to_string()),
                TomlValue::String("notify-send".to_string())
            ]
        );
    }

    #[test]
    fn codex_config_upgrades_legacy_string_notify_in_place() {
        let mut settings: TomlValue = toml::from_str(
            r#"notify = "/Volumes/Codirigent 3/Codirigent.app/Contents/MacOS/codirigent-hook""#,
        )
        .expect("hook installer test should succeed");
        let modified =
            merge_codex_notify(&mut settings, CMD).expect("hook installer test should succeed");

        assert!(modified);
        assert_eq!(
            settings["notify"]
                .as_str()
                .expect("hook installer test should succeed"),
            CMD
        );
    }

    #[test]
    fn fresh_gemini_install_adds_expected_hooks() {
        let mut settings = json!({});
        let modified =
            merge_gemini_hooks(&mut settings, CMD).expect("hook installer test should succeed");
        assert!(modified);

        let hooks = settings["hooks"]
            .as_object()
            .expect("hook installer test should succeed");
        assert!(hooks.contains_key("BeforeAgent"));
        assert!(hooks.contains_key("AfterAgent"));
        assert!(hooks.contains_key("Notification"));
    }

    #[test]
    fn gemini_install_is_idempotent() {
        let mut settings = json!({});
        merge_gemini_hooks(&mut settings, CMD).expect("hook installer test should succeed");
        let modified =
            merge_gemini_hooks(&mut settings, CMD).expect("hook installer test should succeed");
        assert!(!modified, "second Gemini install should not modify");
    }

    #[test]
    fn gemini_install_upgrades_legacy_bare_name_entries() {
        let mut settings = json!({
            "hooks": {
                "BeforeAgent": [
                    {"matcher": "", "hooks": [{"type": "command", "command": "codirigent-hook"}]}
                ],
                "AfterAgent": [
                    {"matcher": "", "hooks": [{"type": "command", "command": "codirigent-hook"}]}
                ],
                "Notification": [
                    {"matcher": "", "hooks": [{"type": "command", "command": "codirigent-hook"}]}
                ]
            }
        });
        let modified =
            merge_gemini_hooks(&mut settings, CMD).expect("hook installer test should succeed");
        assert!(modified, "legacy Gemini entries must be upgraded");

        for event in &["BeforeAgent", "AfterAgent", "Notification"] {
            let arr = settings["hooks"][event]
                .as_array()
                .expect("hook installer test should succeed");
            assert_eq!(arr.len(), 1, "{event} must not grow");
            let cmd = arr[0]["hooks"][0]["command"]
                .as_str()
                .expect("hook installer test should succeed");
            assert_eq!(cmd, CMD);
        }
    }
}
