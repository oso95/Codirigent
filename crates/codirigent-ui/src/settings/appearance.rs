//! Appearance settings panel.
//!
//! Theme dropdown, UI font size stepper, grid gap stepper.

use super::controls::{setting_dropdown, setting_number, setting_row, settings_section_header};
use super::page::SettingsPage;
use crate::theme::CodirigentTheme;
use gpui::{div, IntoElement, ParentElement, Styled};

/// Render the Appearance settings panel.
pub fn render_appearance_panel(page: &SettingsPage, theme: &CodirigentTheme) -> impl IntoElement {
    let appearance = &page.user_settings.appearance;

    div()
        .flex()
        .flex_col()
        .gap_1()
        // Theme section
        .child(settings_section_header("Theme", theme, true))
        .child(setting_row(
            "Color theme",
            "Switch between dark and light themes",
            theme,
            setting_dropdown(&["dark", "light"], &appearance.theme, theme),
        ))
        // UI section
        .child(settings_section_header("Interface", theme, false))
        .child(setting_row(
            "UI font size",
            "Font size for interface elements (10-24)",
            theme,
            setting_number(&appearance.font_size.to_string(), 10.0, 24.0, theme),
        ))
        .child(setting_row(
            "Grid gap",
            "Spacing between session panes in pixels (0-16)",
            theme,
            setting_number(&appearance.grid_gap.to_string(), 0.0, 16.0, theme),
        ))
}
