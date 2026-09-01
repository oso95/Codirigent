//! Editor and font detection utilities.
//!
//! This module provides functions for:
//! - Detecting installed code editors (GUI and terminal-based)
//! - Detecting monospace fonts available on the system
//! - Checking if an editor is terminal-based

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Windows process creation flag: suppress console window for spawned processes.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(windows)]
fn system_where_executable() -> std::path::PathBuf {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("System32")
        .join("where.exe")
}

/// Known GUI-based code editors with CLI support.
pub(super) const KNOWN_GUI_EDITORS: &[&str] =
    &["code", "zed", "cursor", "windsurf", "codium", "subl"];

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
    let mut found = HashSet::new();

    // Pass 1: use which/where (finds editors already on PATH)
    for editor in KNOWN_GUI_EDITORS
        .iter()
        .chain(KNOWN_TERMINAL_EDITORS.iter())
    {
        let on_path = {
            #[cfg(windows)]
            {
                let mut cmd = std::process::Command::new(system_where_executable());
                cmd.arg(editor)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(CREATE_NO_WINDOW);
                cmd.status().map(|s| s.success()).unwrap_or(false)
            }

            #[cfg(not(windows))]
            {
                std::process::Command::new("which")
                    .arg(editor)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            }
        };
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

/// Symbol and dingbat font families that pass monospace width checks but
/// render pictographs instead of text glyphs.
fn is_symbol_font(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("wingding")
        || lower.contains("webding")
        || lower.contains("dingbat")
        || lower.contains("emoji")
}

/// Detect installed monospace fonts by querying the GPUI text system.
///
/// Enumerates all system fonts and filters for monospace by comparing
/// the advance width of 'm' vs 'i' — in a monospace font these are equal.
#[cfg(target_os = "windows")]
pub(super) fn detect_monospace_fonts(text_system: &gpui::TextSystem) -> Vec<String> {
    let all_names: Vec<String> = text_system
        .all_font_names()
        .into_iter()
        .map(|name| name.to_string())
        .collect();

    let all_names_lower: HashSet<String> =
        all_names.iter().map(|name| name.to_lowercase()).collect();

    // Avoid probing missing fonts with resolve_font() on Windows because GPUI
    // logs each miss at error level.
    let preferred = [
        // Nerd Font variants (checked first since users install them intentionally)
        "JetBrainsMono Nerd Font",
        "FiraCode Nerd Font",
        "CaskaydiaCove Nerd Font",
        "CaskaydiaMono Nerd Font",
        "Hack Nerd Font",
        "MesloLGS Nerd Font",
        "MesloLGM Nerd Font",
        "SourceCodePro Nerd Font",
        "Inconsolata Nerd Font",
        "DejaVuSansM Nerd Font",
        "DroidSansM Nerd Font",
        "RobotoMono Nerd Font",
        "UbuntuMono Nerd Font",
        "Mononoki Nerd Font",
        "ProFontWindows Nerd Font",
        // Standard monospace fonts
        "Cascadia Code",
        "Consolas",
        "JetBrains Mono",
        "Fira Code",
        "Source Code Pro",
        "Inconsolata",
        "Lucida Console",
        "Courier New",
    ];

    let mut monospace: Vec<String> = preferred
        .iter()
        .filter(|name| all_names_lower.contains(&name.to_lowercase()))
        .map(|name| (*name).to_string())
        .collect();

    // Catch any remaining Nerd Font that wasn't in the preferred list.
    let nerd_fonts: Vec<String> = all_names
        .iter()
        .filter(|name| {
            let lower = name.to_lowercase();
            lower.contains("nerd font") && !is_symbol_font(name)
        })
        .cloned()
        .collect();
    monospace.extend(nerd_fonts);

    // Fallback heuristic when none of the above are available.
    if monospace.is_empty() {
        monospace = all_names
            .iter()
            .filter(|name| {
                let lower = name.to_lowercase();
                (lower.contains("mono") || lower.contains("code")) && !is_symbol_font(name)
            })
            .cloned()
            .collect();
    }

    if monospace.is_empty() {
        monospace.push("Consolas".to_string());
    }

    monospace.sort();
    monospace.dedup();
    monospace
}

#[cfg(not(target_os = "windows"))]
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

        if is_symbol_font(name) {
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
/// the list of known terminal editors. Handles both Unix (`/`) and
/// Windows (`\`) path separators via `std::path::Path`.
pub(super) fn is_terminal_editor(editor: &str) -> bool {
    let base = editor.rsplit(['/', '\\']).next().unwrap_or(editor);
    let base = std::path::Path::new(base)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(base);
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

    #[test]
    fn test_is_terminal_editor_windows_path() {
        assert!(is_terminal_editor("C:\\Users\\user\\nvim.exe"));
        assert!(is_terminal_editor("C:\\tools\\vim.exe"));
        assert!(!is_terminal_editor("C:\\Users\\user\\code.exe"));
        assert!(!is_terminal_editor("C:\\Program Files\\Zed\\zed.exe"));
    }

    #[test]
    fn test_is_terminal_editor_unix_absolute_path() {
        assert!(is_terminal_editor("/usr/local/bin/nvim"));
        assert!(is_terminal_editor("/usr/local/bin/nano"));
        assert!(!is_terminal_editor("/usr/local/bin/code"));
    }

    #[test]
    fn test_is_symbol_font() {
        assert!(is_symbol_font("Wingdings"));
        assert!(is_symbol_font("Wingdings 2"));
        assert!(is_symbol_font("Webdings"));
        assert!(is_symbol_font("Apple Color Emoji"));
        assert!(is_symbol_font("Noto Color Emoji"));
        assert!(is_symbol_font("Zapf Dingbats"));
        assert!(!is_symbol_font("JetBrains Mono"));
        assert!(!is_symbol_font("Menlo"));
        assert!(!is_symbol_font("Consolas"));
        assert!(!is_symbol_font("Fira Code"));
    }

    #[cfg(unix)]
    #[test]
    fn test_is_executable() {
        use std::path::Path;
        assert!(is_executable(Path::new("/bin/sh")));
        assert!(!is_executable(Path::new("/nonexistent_binary_abc123")));
    }
}
