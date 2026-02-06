//! Sessions settings panel (project-level).
//!
//! Max concurrent sessions, default CLI, auto-cleanup toggle.

use super::controls::{
    setting_dropdown, setting_number, setting_row, setting_toggle, settings_section_header,
};
use super::page::SettingsPage;
use crate::theme::CodirigentTheme;
use gpui::{div, IntoElement, ParentElement, Styled};

/// Render the Sessions settings panel.
pub fn render_sessions_panel(
    page: &SettingsPage,
    theme: &CodirigentTheme,
) -> impl IntoElement {
    let sessions = &page.project_config.sessions;

    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(settings_section_header("Session Limits", theme, true))
        .child(setting_row(
            "Max concurrent sessions",
            "Maximum number of sessions running simultaneously (1-16)",
            theme,
            setting_number(&sessions.max_concurrent.to_string(), 1.0, 16.0, theme),
        ))
        .child(setting_row(
            "Default CLI",
            "CLI tool used for new sessions",
            theme,
            setting_dropdown(
                &["claude", "codex", "gemini"],
                &sessions.default_cli,
                theme,
            ),
        ))
        .child(settings_section_header("Cleanup", theme, false))
        .child(setting_row(
            "Auto-cleanup idle sessions",
            "Automatically close sessions that have been idle for a long time",
            theme,
            setting_toggle(sessions.auto_cleanup, theme),
        ))
}
