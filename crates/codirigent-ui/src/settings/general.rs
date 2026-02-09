//! General settings panel.
//!
//! Editor picker, default shell, working directory, splash screen toggle.

use super::controls::{
    setting_dropdown, setting_path, setting_row, setting_text, setting_toggle,
    settings_section_header,
};
use super::page::SettingsPage;
use crate::theme::CodirigentTheme;
use gpui::{div, IntoElement, ParentElement, Styled};

/// Render the General settings panel.
pub fn render_general_panel(page: &SettingsPage, theme: &CodirigentTheme) -> impl IntoElement {
    let general = &page.user_settings.general;
    let editor_options: Vec<&str> = page.detected_editors.iter().map(|s| s.as_str()).collect();

    div()
        .flex()
        .flex_col()
        .gap_1()
        // Editor section
        .child(settings_section_header("Editor", theme, true))
        .child(setting_row(
            "Default editor",
            "External editor to open files with",
            theme,
            setting_dropdown(
                &editor_options,
                &general.editor_command,
                theme,
            ),
        ))
        // Shell section
        .child(settings_section_header("Shell", theme, false))
        .child(setting_row(
            "Default shell",
            "Shell used for new sessions",
            theme,
            setting_text(&general.default_shell, "powershell", theme),
        ))
        .child(setting_row(
            "Default working directory",
            "Initial directory for new sessions",
            theme,
            setting_path(general.default_working_dir.as_deref().unwrap_or(""), theme),
        ))
        // Startup section
        .child(settings_section_header("Startup", theme, false))
        .child(setting_row(
            "Show splash screen",
            "Display splash screen on application start",
            theme,
            setting_toggle(general.show_splash, theme),
        ))
}
