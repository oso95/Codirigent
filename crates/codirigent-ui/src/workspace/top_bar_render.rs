//! Top bar rendering for workspace.
//!
//! This module handles rendering of the top bar with session tabs,
//! layout controls, and window controls.

use crate::title_bar::TitleBar;
use crate::toolbar::CustomLayoutMode;
use crate::workspace::gpui::WorkspaceView;
use gpui::{div, px, Context, IntoElement, ParentElement, Styled, Window, WindowControlArea};

impl WorkspaceView {
    /// token counter, right-panel toggle, and window controls.
    pub(super) fn render_top_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme();
        let bg: gpui::Hsla = theme.header_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let active: gpui::Hsla = theme.active.into();
        let _primary: gpui::Hsla = theme.primary.into();

        // Clone tab data and state before building the element tree
        let tabs: Vec<(usize, String, bool, bool)> = self
            .top_bar
            .tabs()
            .iter()
            .enumerate()
            .map(|(i, t)| (i, t.label.clone(), t.is_active, t.is_user_saved))
            .collect();
        let right_panel_open = self.top_bar.is_right_panel_open();

        let mut bar = div()
            .id("top-bar")
            .h(px(crate::top_bar::TopBar::HEIGHT))
            .w_full()
            .bg(bg)
            .border_b_1()
            .border_color(border_color)
            .flex()
            .items_center()
            .px_3()
            .gap_2();

        // --- Left section ---

        // Layout tab pills (from profile manager)
        let mut tab_row = div().flex().gap_1().items_center();
        for (idx, label, is_active, is_user_saved) in &tabs {
            let tab_bg = if *is_active {
                active
            } else {
                gpui::Hsla::transparent_black()
            };
            let tab_color = if *is_active { fg } else { muted };
            let tab_idx = *idx;

            let mut tab_pill = div()
                .id(SharedString::from(format!("top-bar-tab-{}", tab_idx)))
                .px_3()
                .py_1()
                .rounded_md()
                .bg(tab_bg)
                .text_xs()
                .font_weight(if *is_active {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(tab_color)
                .cursor_pointer()
                .hover(|style| style.bg(active.opacity(0.5)))
                .flex()
                .items_center()
                .gap_1()
                .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                    this.top_bar.click_tab(tab_idx);
                    this.process_top_bar_events();
                    cx.notify();
                }))
                .child(label.clone());

            // Add remove button for user-saved profiles
            if *is_user_saved {
                let label_for_confirm = label.clone();
                tab_pill = tab_pill.child(
                    div()
                        .id(SharedString::from(format!(
                            "top-bar-tab-remove-{}",
                            tab_idx
                        )))
                        .text_xs()
                        .text_color(muted)
                        .cursor_pointer()
                        .hover(|style| {
                            style.text_color(gpui::Hsla {
                                h: 0.0,
                                s: 0.8,
                                l: 0.6,
                                a: 1.0,
                            })
                        })
                        .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                            // Set pending deletion to show confirmation dialog
                            this.pending_profile_deletion =
                                Some((tab_idx, label_for_confirm.clone()));
                            cx.notify();
                        }))
                        .child("\u{00d7}"), // × character
                );
            }

            tab_row = tab_row.child(tab_pill);
        }

        // "+" button to open the custom layout picker
        tab_row = tab_row.child(
            div()
                .id("top-bar-tab-custom")
                .px_3()
                .py_1()
                .rounded_md()
                .bg(gpui::Hsla::transparent_black())
                .text_xs()
                .text_color(muted)
                .cursor_pointer()
                .hover(|style| style.bg(active.opacity(0.5)))
                .flex()
                .items_center()
                .gap_1()
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.top_bar.request_custom_layout();
                    this.process_top_bar_events();
                    cx.notify();
                }))
                .child("+"),
        );

        bar = bar.child(tab_row);

        // --- Spacer ---
        bar = bar.child(div().flex_1());

        // --- Right section ---

        // Right panel toggle
        let panel_color = if right_panel_open { fg } else { muted };
        bar = bar.child(
            div()
                .id("top-bar-right-panel")
                .px_2()
                .py_1()
                .rounded_md()
                .text_xs()
                .text_color(panel_color)
                .font_family(icons::LUCIDE_FONT_FAMILY)
                .cursor_pointer()
                .hover(|style| style.bg(active.opacity(0.3)))
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.top_bar.toggle_right_panel();
                    this.process_top_bar_events();
                    cx.notify();
                }))
                .child(icons::columns_3()),
        );

        bar
    }

    /// Render a terminal header for a session.
    ///
    /// Returns a GPUI element representing the terminal header with session name,
    /// status indicator, task badge, and context usage.
}
