//! Shared UI primitive builders for the workspace.
//!
//! This module contains reusable UI building blocks extracted from render.rs
//! to reduce duplication and improve maintainability.

use crate::theme::CodirigentTheme;
use gpui::{
    div, px, ClickEvent, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window,
};

/// Create a modal container with overlay and centered panel.
///
/// This is the standard pattern for modal dialogs in the workspace.
/// The overlay is a semi-transparent black background that covers the entire screen.
/// The panel is centered and has a fixed width.
///
/// # Parameters
/// - `id`: Unique identifier for the modal
/// - `width`: Width of the modal panel in pixels
/// - `theme`: Theme for styling
/// - `header`: Header content
/// - `content`: Main content area
/// - `footer`: Footer with action buttons
pub fn modal_container<H, C, F>(
    overlay_id: impl Into<SharedString>,
    panel_id: impl Into<SharedString>,
    width: f32,
    theme: &CodirigentTheme,
    header: H,
    content: C,
    footer: F,
) -> impl IntoElement
where
    H: IntoElement,
    C: IntoElement,
    F: IntoElement,
{
    let bg: gpui::Hsla = theme.panel_background.into();
    let border_color: gpui::Hsla = theme.border.into();

    div()
        .id(overlay_id.into())
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::Hsla::black().opacity(0.5))
        .child(
            div()
                .id(panel_id.into())
                .w(px(width))
                .bg(bg)
                .border_1()
                .border_color(border_color)
                .rounded_lg()
                .flex()
                .flex_col()
                .child(header)
                .child(content)
                .child(footer),
        )
}

/// Create a section header with icon, label, and count badge.
///
/// This is used for task board section headers (QUEUE, RUNNING, REVIEW, DONE).
///
/// # Parameters
/// - `icon`: Lucide icon component
/// - `label`: Section label text
/// - `count`: Number of items in section
/// - `theme`: Theme for styling
/// - `icon_size`: Size of the icon in pixels
/// - `label_row_height`: Height of the label row in pixels
/// - `icon_y_offset`: Vertical offset for icon alignment
pub fn section_header_with_count(
    icon: impl IntoElement,
    label: impl Into<SharedString>,
    count: usize,
    theme: &CodirigentTheme,
    icon_size: f32,
    label_row_height: f32,
) -> impl IntoElement {
    let muted: gpui::Hsla = theme.muted.into();
    let active_bg: gpui::Hsla = theme.active.into();

    div()
        .flex()
        .justify_between()
        .items_center()
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(icon)
                .child(
                    div()
                        .h(px(label_row_height))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(icon_size))
                                .font_weight(FontWeight::BOLD)
                                .text_color(muted)
                                .child(label.into()),
                        ),
                ),
        )
        .child(
            div()
                .px(px(6.0))
                .rounded_full()
                .bg(active_bg)
                .child(div().text_xs().text_color(muted).child(format!("{}", count))),
        )
}

/// Create a rounded action button with hover effect.
///
/// This is the standard pattern for action buttons in modals and toolbars.
///
/// # Parameters
/// - `id`: Unique identifier for the button
/// - `label`: Button label text
/// - `theme`: Theme for styling
/// - `cx`: GPUI context
/// - `on_click`: Click handler
pub fn action_button<V: 'static>(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    theme: &CodirigentTheme,
    cx: &mut Context<V>,
    on_click: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let primary: gpui::Hsla = theme.primary.into();

    div()
        .id(id.into())
        .px_4()
        .py_2()
        .bg(primary)
        .rounded_md()
        .text_sm()
        .text_color(gpui::Hsla::white())
        .cursor_pointer()
        .hover(|style| style.bg(primary.opacity(0.8)))
        .on_click(cx.listener(on_click))
        .child(label.into())
}

/// Format input text with cursor display.
///
/// Shows the input text with a blinking cursor if focused, or placeholder if empty.
///
/// # Parameters
/// - `input`: Current input text
/// - `focused`: Whether the input is focused
/// - `placeholder`: Placeholder text when empty
///
/// # Returns
/// Formatted string with cursor if focused
pub fn format_input_with_cursor(input: &str, focused: bool, placeholder: &str) -> String {
    if input.is_empty() {
        if focused {
            "|".to_string()
        } else {
            placeholder.to_string()
        }
    } else if focused {
        format!("{}|", input)
    } else {
        input.to_string()
    }
}
