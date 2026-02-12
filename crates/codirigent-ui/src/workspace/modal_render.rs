//! Modal dialogs rendering for workspace.
//!
//! This module handles rendering of modal dialogs including
//! custom layout modal and session action modal.

use crate::components::text_input::{text_input, TextInputStyle};
use crate::icons;
use crate::layout::LayoutProfile;
use crate::toolbar::CustomLayoutMode;
use super::gpui::WorkspaceView;
use super::types::{SessionActionKind, SessionActionModal};
use super::render::SessionMenuAction;
use codirigent_core::{LayoutNode, SessionId, SlotId, SplitDirection};
use std::sync::Arc;
use gpui::{
    div, prelude::FluentBuilder, px, relative, ClickEvent, Context, FontWeight, Image,
    ImageFormat, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ObjectFit,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, StyledImage,
};
use std::cell::Cell;
use std::rc::Rc;

impl WorkspaceView {
    /// interactive split tree builder with preview.
    pub(super) fn render_custom_layout_modal(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let picker = &self.custom_picker;

        if !picker.is_open {
            return None;
        }

        let theme = self.workspace().theme();
        let bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let primary: gpui::Hsla = theme.primary.into();
        let error_color: gpui::Hsla = gpui::Hsla::red();
        let current_mode = picker.mode;
        let picker_error = picker.error.clone();

        // Mode tab bar
        let grid_tab_color = if current_mode == CustomLayoutMode::Grid {
            primary
        } else {
            muted
        };
        let split_tab_color = if current_mode == CustomLayoutMode::Split {
            primary
        } else {
            muted
        };
        let transparent = gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.0,
            a: 0.0,
        };
        let grid_tab_border = if current_mode == CustomLayoutMode::Grid {
            primary
        } else {
            transparent
        };
        let split_tab_border = if current_mode == CustomLayoutMode::Split {
            primary
        } else {
            transparent
        };

        let mode_tabs = div()
            .flex()
            .flex_row()
            .border_b_1()
            .border_color(border_color)
            .child(
                div()
                    .id("mode-tab-grid")
                    .flex_1()
                    .h(px(36.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(grid_tab_color)
                    .border_b_2()
                    .border_color(grid_tab_border)
                    .cursor_pointer()
                    .hover(|style| style.bg(border_color.opacity(0.1)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                        this.custom_picker.set_mode(CustomLayoutMode::Grid);
                        cx.notify();
                    }))
                    .child("Grid"),
            )
            .child(
                div()
                    .id("mode-tab-split")
                    .flex_1()
                    .h(px(36.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(split_tab_color)
                    .border_b_2()
                    .border_color(split_tab_border)
                    .cursor_pointer()
                    .hover(|style| style.bg(border_color.opacity(0.1)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                        this.custom_picker.set_mode(CustomLayoutMode::Split);
                        cx.notify();
                    }))
                    .child("Split"),
            );

        // Content area depends on mode
        let content = match current_mode {
            CustomLayoutMode::Grid => self.render_grid_builder_content(cx).into_any_element(),
            CustomLayoutMode::Split => self.render_split_builder_content(cx).into_any_element(),
        };

        // Apply button handler dispatches on mode
        let apply_button = div()
            .id("custom-layout-apply")
            .px_4()
            .py_2()
            .bg(primary)
            .rounded_md()
            .text_sm()
            .text_color(gpui::Hsla::white())
            .cursor_pointer()
            .hover(|style| style.bg(primary.opacity(0.8)))
            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                match this.custom_picker.mode {
                    CustomLayoutMode::Grid => {
                        if let Some((rows, cols)) = this.custom_picker.validate() {
                            this.custom_picker.close();
                            let profile = crate::layout::LayoutProfile::Custom { rows, cols };
                            this.workspace.set_layout(profile);
                        }
                    }
                    CustomLayoutMode::Split => {
                        if let Some(tree) = this.custom_picker.validate_split() {
                            this.custom_picker.close();
                            this.workspace.set_split_tree(tree);
                        }
                    }
                }
                cx.notify();
            }))
            .child(self.aligned_icon_label_row(
                icons::check(),
                gpui::Hsla::white(),
                12.0,
                "Apply",
                gpui::Hsla::white(),
                14.0,
                FontWeight::MEDIUM,
                16.0,
                4.0,
            ));

        Some(
            div()
                .id("custom-layout-modal-overlay")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::Hsla::black().opacity(0.5))
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.custom_picker.close();
                    cx.notify();
                }))
                .child(
                    div()
                        .id("custom-layout-modal")
                        .w(px(400.0))
                        .bg(bg)
                        .border_1()
                        .border_color(border_color)
                        .rounded_lg()
                        .flex()
                        .flex_col()
                        .on_click(cx.listener(|_this, _: &ClickEvent, _window, cx| {
                            cx.stop_propagation();
                        }))
                        // Header
                        .child(
                            div()
                                .h(px(48.0))
                                .px_4()
                                .border_b_1()
                                .border_color(border_color)
                                .flex()
                                .items_center()
                                .child(self.aligned_icon_label_row(
                                    icons::layout_grid(),
                                    fg,
                                    16.0,
                                    "Custom Layout",
                                    fg,
                                    16.0,
                                    FontWeight::SEMIBOLD,
                                    20.0,
                                    8.0,
                                )),
                        )
                        // Mode tabs
                        .child(mode_tabs)
                        // Content
                        .child(content)
                        // Error message (shared across modes)
                        .when_some(picker_error, |this, error| {
                            this.child(
                                div()
                                    .px_4()
                                    .pb_2()
                                    .child(div().text_sm().text_color(error_color).child(error)),
                            )
                        })
                        // Footer with buttons
                        .child(
                            div()
                                .h(px(60.0))
                                .px_4()
                                .border_t_1()
                                .border_color(border_color)
                                .flex()
                                .items_center()
                                .justify_end()
                                .gap_2()
                                // Cancel button
                                .child(
                                    div()
                                        .id("custom-layout-cancel")
                                        .px_4()
                                        .py_2()
                                        .border_1()
                                        .border_color(border_color)
                                        .rounded_md()
                                        .text_sm()
                                        .text_color(fg)
                                        .cursor_pointer()
                                        .hover(|style| style.bg(border_color.opacity(0.1)))
                                        .on_click(cx.listener(
                                            |this, _: &ClickEvent, _window, cx| {
                                                this.custom_picker.close();
                                                cx.notify();
                                            },
                                        ))
                                        .child(self.aligned_icon_label_row(
                                            icons::x(),
                                            fg,
                                            12.0,
                                            "Cancel",
                                            fg,
                                            14.0,
                                            FontWeight::MEDIUM,
                                            16.0,
                                            4.0,
                                        )),
                                )
                                // Apply button
                                .child(apply_button),
                        ),
                ),
        )
    }

    /// Render the grid builder content (rows/cols inputs + grid preview).
    fn render_grid_builder_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let picker = &self.custom_picker;
        let theme = self.workspace().theme();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let primary: gpui::Hsla = theme.primary.into();
        let error_color: gpui::Hsla = gpui::Hsla::red();
        let input_bg: gpui::Hsla = theme.terminal_background.into();

        let rows_value = picker.rows_input.clone();
        let cols_value = picker.cols_input.clone();
        let has_error = picker.error.is_some();
        let focused_input = picker.focused_input();
        let input_style = TextInputStyle {
            height: 36.0,
            padding_x: 12.0,
            bg: input_bg,
            border: border_color,
            focus_border: primary,
            error_border: error_color,
            text: fg,
        };

        div()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            // Rows input
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_sm().text_color(muted).child("Rows (1-10):"))
                    .child({
                        let is_focused = focused_input == Some(0);
                        let display_value = if is_focused {
                            format!("{}|", rows_value.clone())
                        } else {
                            rows_value.clone()
                        };

                        text_input(
                            "rows-input",
                            display_value,
                            is_focused,
                            has_error,
                            &input_style,
                        )
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _event, _window, cx| {
                                this.custom_picker.set_focus(0);
                                cx.notify();
                            }),
                        )
                    }),
            )
            // Columns input
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_sm().text_color(muted).child("Columns (1-10):"))
                    .child({
                        let is_focused = focused_input == Some(1);
                        let display_value = if is_focused {
                            format!("{}|", cols_value.clone())
                        } else {
                            cols_value.clone()
                        };

                        text_input(
                            "cols-input",
                            display_value,
                            is_focused,
                            has_error,
                            &input_style,
                        )
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _event, _window, cx| {
                                this.custom_picker.set_focus(1);
                                cx.notify();
                            }),
                        )
                    }),
            )
            // Preview grid
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_sm().text_color(muted).child("Preview:"))
                    .child(self.render_grid_preview(&rows_value, &cols_value, theme)),
            )
    }

    /// Render the split builder content (action buttons + interactive preview).
    fn render_split_builder_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let primary: gpui::Hsla = theme.primary.into();
        let preview_bg: gpui::Hsla = theme.terminal_background.into();

        let has_selection = self.custom_picker.selected_slot.is_some();
        let pane_count = self.custom_picker.split_tree.leaf_count();
        let can_remove = has_selection && pane_count > 1;
        let tree = self.custom_picker.split_tree.clone();
        let selected = self.custom_picker.selected_slot;

        // Action button styling helper
        let btn_opacity = if has_selection { 1.0 } else { 0.5 };
        let remove_opacity = if can_remove { 1.0 } else { 0.5 };

        div()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            // Action buttons row
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    // Split Horizontal button
                    .child(
                        div()
                            .id("split-h-btn")
                            .px_3()
                            .py_1()
                            .border_1()
                            .border_color(border_color)
                            .rounded_md()
                            .text_xs()
                            .text_color(fg)
                            .cursor_pointer()
                            .opacity(btn_opacity)
                            .hover(|style| style.bg(border_color.opacity(0.1)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.custom_picker
                                    .split_selected(SplitDirection::Horizontal);
                                cx.notify();
                            }))
                            .child(self.aligned_icon_label_row(
                                icons::columns_3(),
                                fg,
                                11.0,
                                "Split H",
                                fg,
                                12.0,
                                FontWeight::MEDIUM,
                                14.0,
                                3.0,
                            )),
                    )
                    // Split Vertical button
                    .child(
                        div()
                            .id("split-v-btn")
                            .px_3()
                            .py_1()
                            .border_1()
                            .border_color(border_color)
                            .rounded_md()
                            .text_xs()
                            .text_color(fg)
                            .cursor_pointer()
                            .opacity(btn_opacity)
                            .hover(|style| style.bg(border_color.opacity(0.1)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.custom_picker.split_selected(SplitDirection::Vertical);
                                cx.notify();
                            }))
                            .child(self.aligned_icon_label_row(
                                icons::layout_grid(),
                                fg,
                                11.0,
                                "Split V",
                                fg,
                                12.0,
                                FontWeight::MEDIUM,
                                14.0,
                                3.0,
                            )),
                    )
                    // Remove button
                    .child(
                        div()
                            .id("split-remove-btn")
                            .px_3()
                            .py_1()
                            .border_1()
                            .border_color(border_color)
                            .rounded_md()
                            .text_xs()
                            .text_color(fg)
                            .cursor_pointer()
                            .opacity(remove_opacity)
                            .hover(|style| style.bg(border_color.opacity(0.1)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.custom_picker.remove_selected();
                                cx.notify();
                            }))
                            .child(self.aligned_icon_label_row(
                                icons::x_circle(),
                                fg,
                                11.0,
                                "Remove",
                                fg,
                                12.0,
                                FontWeight::MEDIUM,
                                14.0,
                                3.0,
                            )),
                    ),
            )
            // Pane count info
            .child(div().text_xs().text_color(muted).child(format!(
                "{} pane{}",
                pane_count,
                if pane_count == 1 { "" } else { "s" }
            )))
            // Preview label
            .child(div().text_sm().text_color(muted).child("Preview:"))
            // Interactive preview
            .child(
                div()
                    .w_full()
                    .h(px(200.0))
                    .border_1()
                    .border_color(border_color)
                    .rounded_md()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(Self::render_split_preview_node(
                        &tree,
                        selected,
                        primary,
                        preview_bg,
                        border_color,
                        cx,
                    )),
            )
    }

    /// Recursively render a preview node for the split builder.
    fn render_split_preview_node(
        node: &LayoutNode,
        selected: Option<SlotId>,
        primary: gpui::Hsla,
        preview_bg: gpui::Hsla,
        border_color: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        match node {
            LayoutNode::Leaf { slot } => {
                let is_selected = selected == Some(*slot);
                let slot_id = *slot;
                let slot_num = slot.0 + 1; // 1-indexed display

                let cell_bg = if is_selected {
                    primary.opacity(0.15)
                } else {
                    preview_bg
                };
                let cell_border = if is_selected { primary } else { border_color };
                let cell_border_width = if is_selected { 2.0 } else { 1.0 };

                let mut cell = div()
                    .id(SharedString::from(format!("preview-slot-{}", slot_id.0)))
                    .flex_1()
                    .m(px(2.0))
                    .bg(cell_bg)
                    .rounded_sm()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(if is_selected { primary } else { border_color })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        this.custom_picker.select_slot(slot_id);
                        cx.notify();
                    }))
                    .child(format!("{}", slot_num));

                if is_selected {
                    cell = cell.border_2().border_color(cell_border);
                } else {
                    cell = cell.border_1().border_color(cell_border);
                }

                cell.into_any_element()
            }
            LayoutNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let first_elem = Self::render_split_preview_node(
                    first,
                    selected,
                    primary,
                    preview_bg,
                    border_color,
                    cx,
                );
                let second_elem = Self::render_split_preview_node(
                    second,
                    selected,
                    primary,
                    preview_bg,
                    border_color,
                    cx,
                );

                let first_flex = *ratio * 1000.0;
                let second_flex = (1.0 - *ratio) * 1000.0;

                let container = match direction {
                    SplitDirection::Horizontal => {
                        let mut first_div = div().flex().flex_col().size_full();
                        first_div.style().flex_grow = Some(first_flex);
                        first_div.style().flex_shrink = Some(1.0);
                        first_div.style().flex_basis = Some(relative(0.).into());
                        let first_div = first_div.child(first_elem);

                        let mut second_div = div().flex().flex_col().size_full();
                        second_div.style().flex_grow = Some(second_flex);
                        second_div.style().flex_shrink = Some(1.0);
                        second_div.style().flex_basis = Some(relative(0.).into());
                        let second_div = second_div.child(second_elem);

                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .size_full()
                            .child(first_div)
                            .child(second_div)
                    }
                    SplitDirection::Vertical => {
                        let mut first_div = div().flex().flex_row().size_full();
                        first_div.style().flex_grow = Some(first_flex);
                        first_div.style().flex_shrink = Some(1.0);
                        first_div.style().flex_basis = Some(relative(0.).into());
                        let first_div = first_div.child(first_elem);

                        let mut second_div = div().flex().flex_row().size_full();
                        second_div.style().flex_grow = Some(second_flex);
                        second_div.style().flex_shrink = Some(1.0);
                        second_div.style().flex_basis = Some(relative(0.).into());
                        let second_div = second_div.child(second_elem);

                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .size_full()
                            .child(first_div)
                            .child(second_div)
                    }
                };

                container.into_any_element()
            }
        }
    }

    /// Render a preview of the grid layout.
    fn render_grid_preview(
        &self,
        rows_str: &str,
        cols_str: &str,
        theme: &crate::theme::CodirigentTheme,
    ) -> impl IntoElement {
        let border_color: gpui::Hsla = theme.border.into();
        let preview_bg: gpui::Hsla = theme.terminal_background.into();

        // Parse dimensions or use defaults
        let rows: u32 = rows_str.parse().unwrap_or(2).clamp(1, 10);
        let cols: u32 = cols_str.parse().unwrap_or(2).clamp(1, 10);

        let cell_size = 30.0;
        let gap = 4.0;

        let mut grid = div().flex().flex_col().gap(px(gap));

        for _ in 0..rows {
            let mut row = div().flex().flex_row().gap(px(gap));

            for _ in 0..cols {
                row = row.child(
                    div()
                        .w(px(cell_size))
                        .h(px(cell_size))
                        .bg(preview_bg)
                        .border_1()
                        .border_color(border_color)
                        .rounded_sm(),
                );
            }

            grid = grid.child(row);
        }

        grid
    }

    /// Render a small logo for the title bar using the embedded PNG.
    pub(super) fn render_logo_small(&self) -> impl IntoElement {
        // The PNG (120x120 / 240x240 @2x) has ~25% built-in padding around
        // the 3x3 grid.  We render it slightly oversized so the visible grid
        // fills roughly 20px, which looks balanced in the 32px title bar.
        let logo_size = 24.0;
        let image = Arc::new(Image::from_bytes(
            ImageFormat::Png,
            crate::splash_screen::LOGO_PNG_BYTES.to_vec(),
        ));
        gpui::img(image)
            .w(px(logo_size))
            .h(px(logo_size))
            .object_fit(ObjectFit::Contain)
    }

    /// Parse a group color string into Hsla.
    fn parse_group_color(&self, color: &str) -> Option<gpui::Hsla> {
        match color.to_lowercase().as_str() {
            "teal" | "blue-green" => Some(gpui::Hsla {
                h: 0.52,
                s: 0.70,
                l: 0.60,
                a: 1.0,
            }),
            "coral" | "orange-red" => Some(gpui::Hsla {
                h: 0.03,
                s: 0.80,
                l: 0.62,
                a: 1.0,
            }),
            "orange" => Some(gpui::Hsla {
                h: 0.08,
                s: 0.90,
                l: 0.60,
                a: 1.0,
            }),
            "blue" => Some(gpui::Hsla {
                h: 0.60,
                s: 0.70,
                l: 0.60,
                a: 1.0,
            }),
            "purple" => Some(gpui::Hsla {
                h: 0.75,
                s: 0.60,
                l: 0.65,
                a: 1.0,
            }),
            "green" => Some(gpui::Hsla {
                h: 0.33,
                s: 0.60,
                l: 0.55,
                a: 1.0,
            }),
            "yellow" => Some(gpui::Hsla {
                h: 0.15,
                s: 0.80,
                l: 0.65,
                a: 1.0,
            }),
            "red" => Some(gpui::Hsla {
                h: 0.0,
                s: 0.80,
                l: 0.60,
                a: 1.0,
            }),
            _ => None,
        }
    }

    /// Render session context menu (dropdown near the trigger button).
    /// Render the session action modal for rename/group.
    pub(super) fn render_session_action_modal(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let modal = self.session_action_modal.clone()?;

        let theme = self.workspace().theme();
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let primary: gpui::Hsla = theme.primary.into();
        let input_bg: gpui::Hsla = theme.terminal_background.into();
        let error_color: gpui::Hsla = gpui::Hsla::red();
        let input_style = TextInputStyle {
            height: 36.0,
            padding_x: 12.0,
            bg: input_bg,
            border: border_color,
            focus_border: primary,
            error_border: error_color,
            text: fg,
        };

        let title = match modal.kind {
            super::types::SessionActionKind::Rename => "Rename Session",
            super::types::SessionActionKind::AssignGroup => "Assign Group",
        };
        let title_icon = match modal.kind {
            super::types::SessionActionKind::Rename => icons::pencil(),
            super::types::SessionActionKind::AssignGroup => icons::users(),
        };
        let label = match modal.kind {
            super::types::SessionActionKind::Rename => "Session Name:",
            super::types::SessionActionKind::AssignGroup => "Group Name:",
        };

        // Always show cursor since modal input is always focused
        let input_value = if modal.input.is_empty() {
            "|".to_string()
        } else {
            format!("{}|", modal.input)
        };

        Some(
            div()
                .id("session-action-overlay")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::Hsla::black().opacity(0.5))
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.close_session_action_modal();
                    cx.notify();
                }))
                .child(
                    div()
                        .id("session-action-modal")
                        .w(px(420.0))
                        .bg(panel_bg)
                        .border_1()
                        .border_color(border_color)
                        .rounded_lg()
                        .flex()
                        .flex_col()
                        // Prevent closing when clicking modal content
                        .on_click(cx.listener(|_this, _: &ClickEvent, _window, cx| {
                            cx.stop_propagation();
                        }))
                        // Header
                        .child(
                            div()
                                .h(px(48.0))
                                .px_4()
                                .border_b_1()
                                .border_color(border_color)
                                .flex()
                                .items_center()
                                .child(self.aligned_icon_label_row_with_offset(
                                    title_icon,
                                    fg,
                                    16.0,
                                    title,
                                    fg,
                                    16.0,
                                    FontWeight::SEMIBOLD,
                                    20.0,
                                    8.0,
                                    3.0,
                                )),
                        )
                        // Content
                        .child(
                            div()
                                .p_4()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(div().text_sm().text_color(muted).child(label))
                                .child(
                                    text_input(
                                        "session-action-input",
                                        input_value,
                                        true, // Always focused in modal
                                        modal.error.is_some(),
                                        &input_style,
                                    )
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|_this, _event, _window, cx| {
                                            // Input is always focused in this modal
                                            cx.stop_propagation();
                                        }),
                                    ),
                                )
                                .when_some(modal.error.clone(), |this, error| {
                                    this.child(div().text_sm().text_color(error_color).child(error))
                                }),
                        )
                        // Footer
                        .child(
                            div()
                                .h(px(60.0))
                                .px_4()
                                .border_t_1()
                                .border_color(border_color)
                                .flex()
                                .items_center()
                                .justify_end()
                                .gap_2()
                                .child(
                                    div()
                                        .id("session-action-cancel")
                                        .px_4()
                                        .py_2()
                                        .border_1()
                                        .border_color(border_color)
                                        .rounded_md()
                                        .text_sm()
                                        .text_color(fg)
                                        .cursor_pointer()
                                        .hover(|style| style.bg(border_color.opacity(0.1)))
                                        .on_click(cx.listener(
                                            |this, _: &ClickEvent, _window, cx| {
                                                this.close_session_action_modal();
                                                cx.notify();
                                            },
                                        ))
                                        .child(self.aligned_icon_label_row_with_offset(
                                            icons::x(),
                                            fg,
                                            12.0,
                                            "Cancel",
                                            fg,
                                            14.0,
                                            FontWeight::MEDIUM,
                                            16.0,
                                            4.0,
                                            3.0,
                                        )),
                                )
                                .child(
                                    div()
                                        .id("session-action-apply")
                                        .px_4()
                                        .py_2()
                                        .bg(primary)
                                        .rounded_md()
                                        .text_sm()
                                        .text_color(gpui::Hsla::white())
                                        .cursor_pointer()
                                        .hover(|style| style.bg(primary.opacity(0.8)))
                                        .on_click(cx.listener(
                                            |this, _: &ClickEvent, _window, cx| {
                                                this.apply_session_action_modal(cx);
                                            },
                                        ))
                                        .child(self.aligned_icon_label_row_with_offset(
                                            icons::check(),
                                            gpui::Hsla::white(),
                                            12.0,
                                            "Apply",
                                            gpui::Hsla::white(),
                                            14.0,
                                            FontWeight::MEDIUM,
                                            16.0,
                                            4.0,
                                            3.0,
                                        )),
                                ),
                        ),
                ),
        )
    }
}
