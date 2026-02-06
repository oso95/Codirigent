//! Main settings layout renderer.
//!
//! Renders the two-column layout: category sidebar (220px) on the left,
//! scrollable content area on the right.

use super::page::{SettingsCategory, SettingsPage};
use crate::theme::CodirigentTheme;
use gpui::{div, px, IntoElement, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled};

/// Width of the category sidebar in pixels.
const SIDEBAR_WIDTH: f32 = 220.0;

/// Render the full settings page layout.
pub fn render_settings_page(
    page: &SettingsPage,
    theme: &CodirigentTheme,
) -> impl IntoElement {
    let bg: gpui::Hsla = theme.background.into();
    let panel_bg: gpui::Hsla = theme.panel_background.into();
    let fg: gpui::Hsla = theme.foreground.into();
    let _muted: gpui::Hsla = theme.muted.into();
    let accent: gpui::Hsla = theme.primary.into();
    let border: gpui::Hsla = theme.border.into();
    let active_cat = page.active_category();

    // Category sidebar
    let mut sidebar = div()
        .w(px(SIDEBAR_WIDTH))
        .h_full()
        .flex()
        .flex_col()
        .bg(panel_bg)
        .border_r_1()
        .border_color(border)
        .py_2();

    // Back button at top
    sidebar = sidebar.child(
        div()
            .id("settings-back")
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .cursor_pointer()
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(accent)
                    .child("\u{2190} Back to workspace"),
            ),
    );

    // Separator
    sidebar = sidebar.child(
        div().h(px(1.0)).mx_3().my_2().bg(border),
    );

    // Category items
    for cat in SettingsCategory::ALL {
        let is_active = cat == active_cat;
        let text_color = if is_active { accent } else { fg };
        let item_bg = if is_active {
            gpui::Hsla { a: 0.1, ..accent }
        } else {
            gpui::Hsla { a: 0.0, ..fg }
        };
        let label = cat.label().to_string();

        sidebar = sidebar.child(
            div()
                .id(gpui::SharedString::from(format!("settings-cat-{}", cat.label())))
                .flex()
                .flex_row()
                .items_center()
                .px_3()
                .py(px(6.0))
                .mx_2()
                .rounded_md()
                .bg(item_bg)
                .cursor_pointer()
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(text_color)
                        .child(label),
                ),
        );
    }

    // Content area
    let content = div()
        .id("settings-content-scroll")
        .flex_1()
        .h_full()
        .flex()
        .flex_col()
        .overflow_y_scroll()
        .p_6()
        .child(
            // Title
            div()
                .text_size(px(18.0))
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(fg)
                .mb_4()
                .child(format!("{} Settings", active_cat.label())),
        )
        .child(render_category_content(page, theme));

    // Main layout
    div()
        .id("settings-page")
        .size_full()
        .flex()
        .flex_row()
        .bg(bg)
        .child(sidebar)
        .child(content)
}

/// Render the content for the active category.
pub fn render_category_content(
    page: &SettingsPage,
    theme: &CodirigentTheme,
) -> impl IntoElement {
    match page.active_category() {
        SettingsCategory::General => {
            super::general::render_general_panel(page, theme).into_any_element()
        }
        SettingsCategory::Appearance => {
            super::appearance::render_appearance_panel(page, theme).into_any_element()
        }
        SettingsCategory::Terminal => {
            super::terminal::render_terminal_panel(page, theme).into_any_element()
        }
        SettingsCategory::KeyboardShortcuts => {
            super::shortcuts::render_shortcuts_panel(page, theme).into_any_element()
        }
        SettingsCategory::Sessions => {
            super::sessions::render_sessions_panel(page, theme).into_any_element()
        }
        SettingsCategory::Advanced => {
            super::advanced::render_advanced_panel(page, theme).into_any_element()
        }
    }
}
