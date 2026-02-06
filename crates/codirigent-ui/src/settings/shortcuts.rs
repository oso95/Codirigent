//! Keyboard shortcuts settings panel.
//!
//! Table of action name + current binding pairs with recording mode.

use super::controls::settings_section_header;
use super::page::SettingsPage;
use crate::theme::CodirigentTheme;
use gpui::{div, px, IntoElement, InteractiveElement, ParentElement, Styled};

/// Render the Keyboard Shortcuts settings panel.
pub fn render_shortcuts_panel(
    page: &SettingsPage,
    theme: &CodirigentTheme,
) -> impl IntoElement {
    let fg: gpui::Hsla = theme.foreground.into();
    let muted: gpui::Hsla = theme.muted.into();
    let accent: gpui::Hsla = theme.primary.into();
    let panel_bg: gpui::Hsla = theme.panel_background.into();
    let border: gpui::Hsla = theme.border.into();

    let bindings = &page.user_settings.keybindings;
    let recording = &page.recording_shortcut;

    let mut container = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(settings_section_header("Keyboard Shortcuts", theme, true));

    // Table header
    container = container.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .px_2()
            .py_1()
            .child(
                div()
                    .flex_1()
                    .text_size(px(11.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(muted)
                    .child("ACTION"),
            )
            .child(
                div()
                    .w(px(160.0))
                    .text_size(px(11.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(muted)
                    .child("BINDING"),
            ),
    );

    // Sorted binding rows
    let mut sorted_bindings: Vec<_> = bindings.iter().collect();
    sorted_bindings.sort_by_key(|(k, _)| (*k).clone());

    for (action, binding) in sorted_bindings {
        let is_recording = recording.as_deref() == Some(action.as_str());
        let display_binding = if is_recording {
            "Press a key...".to_string()
        } else {
            binding.clone()
        };
        let binding_color = if is_recording { accent } else { fg };
        let action_label = humanize_action_name(action);

        container = container.child(
            div()
                .id(gpui::SharedString::from(format!("shortcut-{}", action)))
                .flex()
                .flex_row()
                .items_center()
                .px_2()
                .py(px(6.0))
                .rounded_md()
                .hover(|s| s.bg(gpui::Hsla { a: 0.05, ..fg }))
                .cursor_pointer()
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.0))
                        .text_color(fg)
                        .child(action_label),
                )
                .child(
                    div()
                        .w(px(160.0))
                        .text_size(px(12.0))
                        .text_color(binding_color)
                        .bg(panel_bg)
                        .border_1()
                        .border_color(if is_recording { accent } else { border })
                        .rounded_md()
                        .px_2()
                        .py_1()
                        .child(display_binding),
                ),
        );
    }

    container
}

/// Convert snake_case action name to human-readable label.
fn humanize_action_name(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_humanize_action_name() {
        assert_eq!(humanize_action_name("new_session"), "New Session");
        assert_eq!(humanize_action_name("toggle_sidebar"), "Toggle Sidebar");
        assert_eq!(humanize_action_name("switch_session_1"), "Switch Session 1");
    }
}
