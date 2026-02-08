//! Advanced settings panel (project-level).
//!
//! Scheduler mode, auto-assign, verification, git settings.

use super::controls::{
    setting_dropdown, setting_number, setting_row, setting_toggle, settings_section_header,
};
use super::page::SettingsPage;
use crate::theme::CodirigentTheme;
use gpui::{div, IntoElement, ParentElement, Styled};

/// Render the Advanced settings panel.
pub fn render_advanced_panel(page: &SettingsPage, theme: &CodirigentTheme) -> impl IntoElement {
    let scheduler = &page.project_config.scheduler;
    let verification = &page.project_config.verification;
    let git = &page.project_config.git;

    let scheduler_mode = format!("{:?}", scheduler.mode);

    div()
        .flex()
        .flex_col()
        .gap_1()
        // Scheduler section
        .child(settings_section_header("Scheduler", theme, true))
        .child(setting_row(
            "Scheduler mode",
            "Task scheduling strategy",
            theme,
            setting_dropdown(
                &["Fifo", "Priority", "Dependency", "Smart"],
                &scheduler_mode,
                theme,
            ),
        ))
        .child(setting_row(
            "Auto-assign tasks",
            "Automatically assign tasks to idle sessions",
            theme,
            setting_toggle(scheduler.auto_assign, theme),
        ))
        // Verification section
        .child(settings_section_header("Verification", theme, false))
        .child(setting_row(
            "Enable verification",
            "Run verification after task completion",
            theme,
            setting_toggle(verification.enabled, theme),
        ))
        .child(setting_row(
            "Auto-detect commands",
            "Auto-detect test/lint commands based on project type",
            theme,
            setting_toggle(verification.auto_detect, theme),
        ))
        .child(setting_row(
            "Max retries",
            "Maximum retry attempts before blocking (1-10)",
            theme,
            setting_number(&verification.max_retries.to_string(), 1.0, 10.0, theme),
        ))
        // Git section
        .child(settings_section_header("Git", theme, false))
        .child(setting_row(
            "Use worktrees",
            "Isolate sessions in separate git worktrees",
            theme,
            setting_toggle(git.use_worktrees, theme),
        ))
        .child(setting_row(
            "Auto-commit",
            "Automatically commit changes after task completion",
            theme,
            setting_toggle(git.auto_commit, theme),
        ))
}
