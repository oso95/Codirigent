//! Terminal settings panel.
//!
//! Font family, font size, cursor style, line height.

use super::controls::{
    setting_dropdown, setting_number, setting_row, setting_text, settings_section_header,
};
use super::page::SettingsPage;
use crate::theme::CodirigentTheme;
use gpui::{div, IntoElement, ParentElement, Styled};

/// Render the Terminal settings panel.
pub fn render_terminal_panel(
    page: &SettingsPage,
    theme: &CodirigentTheme,
) -> impl IntoElement {
    let terminal = &page.user_settings.terminal;

    div()
        .flex()
        .flex_col()
        .gap_1()
        // Font section
        .child(settings_section_header("Font", theme, true))
        .child(setting_row(
            "Font family",
            "Monospace font for terminal rendering",
            theme,
            setting_text(&terminal.font_family, "JetBrains Mono", theme),
        ))
        .child(setting_row(
            "Font size",
            "Terminal font size in points (8-24)",
            theme,
            setting_number(&terminal.font_size.to_string(), 8.0, 24.0, theme),
        ))
        // Cursor section
        .child(settings_section_header("Cursor", theme, false))
        .child(setting_row(
            "Cursor style",
            "Shape of the terminal cursor",
            theme,
            setting_dropdown(
                &["block", "underline", "bar"],
                &terminal.cursor_style,
                theme,
            ),
        ))
        // Layout section
        .child(settings_section_header("Layout", theme, false))
        .child(setting_row(
            "Line height",
            "Line height multiplier (1.0-2.5)",
            theme,
            setting_number(&format!("{:.1}", terminal.line_height), 1.0, 2.5, theme),
        ))
}
