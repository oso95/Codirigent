//! Editor and font detection utilities.
//!
//! This module provides functions for:
//! - Detecting installed code editors (GUI and terminal-based)
//! - Detecting monospace fonts available on the system
//! - Checking if an editor is terminal-based

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Known GUI-based code editors with CLI support.
pub(super) const KNOWN_GUI_EDITORS: &[&str] = &["code", "zed", "cursor", "windsurf", "codium", "subl"];

/// Known terminal-based code editors.
pub(super) const KNOWN_TERMINAL_EDITORS: &[&str] =
    &["vim", "nvim", "vi", "nano", "emacs", "helix", "hx", "micro"];

/// Returns additional directories where editor CLI tools are commonly installed,
/// beyond what the default PATH includes (especially for macOS GUI apps which
/// inherit a minimal PATH).
pub(super) fn extra_editor_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/usr/local/bin"));
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
    }

    #[cfg(target_os = "linux")]
    {
        dirs.push(PathBuf::from("/usr/local/bin"));
        dirs.push(PathBuf::from("/snap/bin"));
        dirs.push(PathBuf::from("/var/lib/flatpak/exports/bin"));
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            dirs.push(home.join(".local/bin"));
            dirs.push(home.join(".local/share/flatpak/exports/bin"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            dirs.push(local.join("Programs/Microsoft VS Code/bin"));
            dirs.push(local.join("Programs/Cursor/bin"));
            dirs.push(local.join("Programs/Windsurf/bin"));
            dirs.push(local.join("Programs/VSCodium/bin"));
        }
        if let Some(pf) = std::env::var_os("ProgramFiles") {
            let pf = PathBuf::from(pf);
            dirs.push(pf.join("Microsoft VS Code/bin"));
            dirs.push(pf.join("Sublime Text"));
            dirs.push(pf.join("Sublime Text 3"));
        }
    }

    dirs
}

/// Checks whether a path exists and is executable.
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.is_file()
        && path
            .metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

/// On Windows, existence implies executability for `.exe`/`.cmd` files.
#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// Detects installed code editors by checking PATH and common installation directories.
///
/// Returns a list of editor command names in a consistent order (GUI editors first,
/// then terminal editors) with no duplicates.
pub(super) fn detect_installed_editors() -> Vec<String> {
    let check_cmd = if cfg!(windows) { "where" } else { "which" };
    let mut found = HashSet::new();

    // Pass 1: use which/where (finds editors already on PATH)
    for editor in KNOWN_GUI_EDITORS
        .iter()
        .chain(KNOWN_TERMINAL_EDITORS.iter())
    {
        let on_path = std::process::Command::new(check_cmd)
            .arg(editor)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if on_path {
            found.insert(*editor);
        }
    }

    // Pass 2: check extra directories for editors not found in Pass 1
    let extra_dirs = extra_editor_dirs();
    for editor in KNOWN_GUI_EDITORS
        .iter()
        .chain(KNOWN_TERMINAL_EDITORS.iter())
    {
        if found.contains(editor) {
            continue;
        }
        for dir in &extra_dirs {
            if is_executable(&dir.join(editor)) {
                found.insert(*editor);
                break;
            }
        }
    }

    // Return in original order (GUI first, then terminal) to keep the dropdown consistent
    KNOWN_GUI_EDITORS
        .iter()
        .chain(KNOWN_TERMINAL_EDITORS.iter())
        .filter(|e| found.contains(*e))
        .map(|s| s.to_string())
        .collect()
}

/// Detect installed monospace fonts by querying the GPUI text system.
///
/// Enumerates all system fonts and filters for monospace by comparing
/// the advance width of 'm' vs 'i' — in a monospace font these are equal.
pub(super) fn detect_monospace_fonts(text_system: &gpui::TextSystem) -> Vec<String> {
    use gpui::{px, Font, FontFeatures, FontStyle, FontWeight};

    let all_names = text_system.all_font_names();
    let font_size = px(14.0);
    let mut monospace = Vec::new();

    for name in &all_names {
        // Skip internal/system fonts (names starting with '.')
        if name.starts_with('.') {
            continue;
        }

        let font = Font {
            family: name.clone().into(),
            features: FontFeatures::default(),
            fallbacks: None,
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
        };

        let font_id = text_system.resolve_font(&font);

        let adv_m = text_system.advance(font_id, font_size, 'm');
        let adv_i = text_system.advance(font_id, font_size, 'i');

        if let (Ok(am), Ok(ai)) = (adv_m, adv_i) {
            if (f32::from(am.width) - f32::from(ai.width)).abs() < 0.01 {
                monospace.push(name.clone());
            }
        }
    }

    monospace.sort();
    monospace.dedup();
    monospace
}

/// Checks if an editor command is a terminal-based editor.
///
/// Extracts the base command name (ignoring path) and checks against
/// the list of known terminal editors.
pub(super) fn is_terminal_editor(editor: &str) -> bool {
    let base = editor.rsplit('/').next().unwrap_or(editor);
    KNOWN_TERMINAL_EDITORS.contains(&base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn test_extra_editor_dirs_returns_entries() {
        let dirs = extra_editor_dirs();
        assert!(
            !dirs.is_empty(),
            "extra_editor_dirs should return entries on macOS/Linux"
        );
    }

    #[test]
    fn test_detect_installed_editors_no_duplicates() {
        let editors = detect_installed_editors();
        let unique: HashSet<_> = editors.iter().collect();
        assert_eq!(
            editors.len(),
            unique.len(),
            "detect_installed_editors returned duplicates: {:?}",
            editors
        );
    }

    #[test]
    fn test_detect_installed_editors_gui_before_terminal() {
        let editors = detect_installed_editors();
        let mut seen_terminal = false;
        for e in &editors {
            if KNOWN_TERMINAL_EDITORS.contains(&e.as_str()) {
                seen_terminal = true;
            } else if seen_terminal && KNOWN_GUI_EDITORS.contains(&e.as_str()) {
                panic!(
                    "GUI editor '{}' appeared after terminal editor in {:?}",
                    e, editors
                );
            }
        }
    }

    #[test]
    fn test_is_terminal_editor() {
        assert!(is_terminal_editor("vim"));
        assert!(is_terminal_editor("nvim"));
        assert!(is_terminal_editor("/usr/bin/vim"));
        assert!(!is_terminal_editor("code"));
        assert!(!is_terminal_editor("zed"));
    }
}
