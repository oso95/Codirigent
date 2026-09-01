//! Modal dialogs rendering for workspace.
//!
//! This module handles rendering of modal dialogs including
//! custom layout modal and session action modal.

use super::gpui::WorkspaceView;
use super::types::MODAL_FIELD_HEIGHT;
use crate::components::text_input::{text_input, TextInputStyle};
use crate::icons;
use crate::toolbar::CustomLayoutMode;
use codirigent_core::{LayoutNode, SlotId, SplitDirection};
use gpui::{
    div, prelude::FluentBuilder, px, relative, ClickEvent, Context, FontWeight, Image, ImageFormat,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ObjectFit, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, StyledImage,
};
use std::sync::Arc;

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

        // Mode tab bar — helpers to keep the active/inactive state consistent.
        let tab_color = |mode| if current_mode == mode { primary } else { muted };
        let tab_border = |mode| {
            if current_mode == mode {
                primary
            } else {
                gpui::Hsla::transparent_black()
            }
        };
        let grid_tab_color = tab_color(CustomLayoutMode::Grid);
        let grid_tab_border = tab_border(CustomLayoutMode::Grid);
        let split_tab_color = tab_color(CustomLayoutMode::Split);
        let split_tab_border = tab_border(CustomLayoutMode::Split);

        let mode_tabs = div()
            .flex()
            .flex_row()
            .border_b_1()
            .border_color(border_color)
            .child(
                div()
                    .id("mode-tab-grid")
                    .flex_1()
                    .h(px(MODAL_FIELD_HEIGHT))
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
                    .h(px(MODAL_FIELD_HEIGHT))
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

        // Apply-only button keeps the layout for the current workspace.
        let apply_button = div()
            .id("custom-layout-apply")
            .px_4()
            .py_2()
            .border_1()
            .border_color(border_color)
            .rounded_md()
            .text_sm()
            .text_color(fg)
            .cursor_pointer()
            .hover(|style| style.bg(border_color.opacity(0.1)))
            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                this.apply_custom_layout_from_picker(cx);
                cx.notify();
            }))
            .child(self.aligned_icon_label_row(
                icons::check(),
                fg,
                12.0,
                "Apply",
                fg,
                14.0,
                FontWeight::MEDIUM,
                16.0,
                4.0,
            ));

        let save_and_apply_button = div()
            .id("custom-layout-save-and-apply")
            .px_4()
            .py_2()
            .bg(primary)
            .rounded_md()
            .text_sm()
            .text_color(gpui::Hsla::white())
            .cursor_pointer()
            .hover(|style| style.bg(primary.opacity(0.8)))
            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                this.save_and_apply_custom_layout_from_picker(cx);
                cx.notify();
            }))
            .child(self.aligned_icon_label_row(
                icons::plus(),
                gpui::Hsla::white(),
                12.0,
                "Save + Apply",
                gpui::Hsla::white(),
                14.0,
                FontWeight::MEDIUM,
                16.0,
                4.0,
            ));

        Some(
            div()
                .id("custom-layout-modal-overlay")
                .occlude()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::Hsla::black().opacity(0.5))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_this, _: &MouseDownEvent, _window, cx| {
                        cx.stop_propagation();
                    }),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.custom_picker.close();
                    cx.notify();
                }))
                .child(
                    div()
                        .id("custom-layout-modal")
                        .occlude()
                        .w(px(400.0))
                        .bg(bg)
                        .border_1()
                        .border_color(border_color)
                        .rounded_lg()
                        .flex()
                        .flex_col()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|_this, _: &MouseDownEvent, _window, cx| {
                                cx.stop_propagation();
                            }),
                        )
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
                                .child(apply_button)
                                .child(save_and_apply_button),
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

                        text_input(display_value, is_focused, has_error, &input_style)
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

                        text_input(display_value, is_focused, has_error, &input_style)
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
        let slot_display_numbers: std::collections::HashMap<SlotId, usize> = tree
            .slots_in_order()
            .into_iter()
            .enumerate()
            .map(|(index, slot)| (slot, index + 1))
            .collect();

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
                        &slot_display_numbers,
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
        slot_display_numbers: &std::collections::HashMap<SlotId, usize>,
        primary: gpui::Hsla,
        preview_bg: gpui::Hsla,
        border_color: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        match node {
            LayoutNode::Leaf { slot } => {
                let is_selected = selected == Some(*slot);
                let slot_id = *slot;
                let slot_num = slot_display_numbers
                    .get(&slot_id)
                    .copied()
                    .unwrap_or(slot_id.0 as usize + 1);

                let cell_bg = if is_selected {
                    primary.opacity(0.15)
                } else {
                    preview_bg
                };
                let cell_border = if is_selected { primary } else { border_color };

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
                    slot_display_numbers,
                    primary,
                    preview_bg,
                    border_color,
                    cx,
                );
                let second_elem = Self::render_split_preview_node(
                    second,
                    selected,
                    slot_display_numbers,
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

    /// Render the session action modal for rename/group.
    pub(super) fn render_session_action_modal(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let modal = self.modals.session_action.clone()?;

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

        let input_value = if self.modals.cursor_blink_on {
            let cursor = modal.cursor_position.min(modal.input.chars().count());
            let cursor_byte = modal
                .input
                .char_indices()
                .nth(cursor)
                .map(|(i, _)| i)
                .unwrap_or(modal.input.len());
            let mut out = String::with_capacity(modal.input.len() + 1);
            out.push_str(&modal.input[..cursor_byte]);
            out.push('|');
            out.push_str(&modal.input[cursor_byte..]);
            out
        } else {
            modal.input.clone()
        };

        Some(
            div()
                .id("session-action-overlay")
                .occlude()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::Hsla::black().opacity(0.5))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_this, _: &MouseDownEvent, _window, cx| {
                        cx.stop_propagation();
                    }),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.close_session_action_modal();
                    cx.notify();
                }))
                .child(
                    div()
                        .id("session-action-modal")
                        .occlude()
                        .w(px(420.0))
                        .bg(panel_bg)
                        .border_1()
                        .border_color(border_color)
                        .rounded_lg()
                        .flex()
                        .flex_col()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|_this, _: &MouseDownEvent, _window, cx| {
                                cx.stop_propagation();
                            }),
                        )
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

    /// Render the session creation modal with per-session shell selection.
    pub(super) fn render_session_creation_modal(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let modal = self.modals.session_creation.clone()?;

        let theme = self.workspace().theme();
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let primary: gpui::Hsla = theme.primary.into();
        let warning: gpui::Hsla = theme.orange.into();
        let row_hover: gpui::Hsla = theme.hover.into();
        let error_color: gpui::Hsla = gpui::Hsla::red();
        let modal_pending = modal.pending;

        Some(
            div()
                .id("session-create-overlay")
                .occlude()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::Hsla::black().opacity(0.5))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_this, _: &MouseDownEvent, _window, cx| {
                        cx.stop_propagation();
                    }),
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                    if !modal_pending {
                        this.close_session_creation_modal();
                        cx.notify();
                    }
                }))
                .child(
                    div()
                        .id("session-create-modal")
                        .occlude()
                        .w(px(460.0))
                        .bg(panel_bg)
                        .border_1()
                        .border_color(border_color)
                        .rounded_lg()
                        .flex()
                        .flex_col()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|_this, _: &MouseDownEvent, _window, cx| {
                                cx.stop_propagation();
                            }),
                        )
                        .on_click(cx.listener(|_this, _: &ClickEvent, _window, cx| {
                            cx.stop_propagation();
                        }))
                        .child(
                            div()
                                .h(px(48.0))
                                .px_4()
                                .border_b_1()
                                .border_color(border_color)
                                .flex()
                                .items_center()
                                .child(self.aligned_icon_label_row_with_offset(
                                    icons::terminal(),
                                    fg,
                                    16.0,
                                    "Create Session",
                                    fg,
                                    16.0,
                                    FontWeight::SEMIBOLD,
                                    20.0,
                                    8.0,
                                    3.0,
                                )),
                        )
                        .child(
                            div()
                                .p_4()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(muted)
                                        .child("Choose which shell to use for this session."),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(fg)
                                        .child("Shell"),
                                )
                                .child({
                                    let shell_sections =
                                        self.shell_picker_sections(&modal.shell_options);
                                    let mut list = div().flex().flex_col().gap_2();

                                    for (section_index, section) in shell_sections.iter().enumerate() {
                                        if section_index > 0 {
                                            list = list.child(
                                                div()
                                                    .h(px(1.0))
                                                    .my_1()
                                                    .bg(border_color.opacity(0.5)),
                                            );
                                        }
                                        if let Some(title) = section.title {
                                            list = list.child(
                                                div()
                                                    .px_1()
                                                    .pt(px(4.0))
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(muted.opacity(0.8))
                                                    .child(title),
                                            );
                                        }

                                        for option in &section.options {
                                            let index = option.source_index;
                                            let is_selected = index == modal.selected_shell_index;
                                            let option_hint = if option.raw_value.is_empty() {
                                                "Use the default shell setting or the platform default."
                                            } else {
                                                "Open the session in this shell."
                                            };
                                            let option_border =
                                                if is_selected { primary } else { border_color };
                                            let option_bg = if is_selected {
                                                primary.opacity(0.12)
                                            } else {
                                                gpui::Hsla::transparent_black()
                                            };

                                            list = list.child(
                                                div()
                                                    .id(SharedString::from(format!(
                                                        "session-shell-option-{}",
                                                        index
                                                    )))
                                                    .w_full()
                                                    .p_3()
                                                    .border_1()
                                                    .border_color(option_border)
                                                    .rounded_md()
                                                    .bg(option_bg)
                                                    .cursor_pointer()
                                                    .hover(|style| style.bg(row_hover))
                                                    .on_click(cx.listener({
                                                        move |this, _: &ClickEvent, _window, cx| {
                                                            if modal_pending {
                                                                return;
                                                            }
                                                            if let Some(active) =
                                                                this.modals.session_creation.as_mut()
                                                            {
                                                                active.selected_shell_index = index;
                                                                active.error = None;
                                                            }
                                                            cx.notify();
                                                        }
                                                    }))
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .items_start()
                                                            .gap_3()
                                                            .child(
                                                                div()
                                                                    .mt_px()
                                                                    .w(px(14.0))
                                                                    .h(px(14.0))
                                                                    .rounded_full()
                                                                    .border_1()
                                                                    .border_color(if is_selected {
                                                                        primary
                                                                    } else {
                                                                        muted
                                                                    })
                                                                    .bg(if is_selected {
                                                                        primary
                                                                    } else {
                                                                        gpui::Hsla::transparent_black()
                                                                    }),
                                                            )
                                                            .child(
                                                                div()
                                                                    .flex_1()
                                                                    .flex()
                                                                    .flex_col()
                                                                    .gap_1()
                                                                    .child(
                                                                        div()
                                                                            .text_sm()
                                                                            .font_weight(
                                                                                FontWeight::MEDIUM,
                                                                            )
                                                                            .text_color(fg)
                                                                            .child(
                                                                                option.label.clone(),
                                                                            ),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .text_xs()
                                                                            .text_color(
                                                                                if option.raw_value.is_empty()
                                                                                {
                                                                                    warning.opacity(0.9)
                                                                                } else {
                                                                                    muted
                                                                                },
                                                                            )
                                                                            .child(option_hint),
                                                                    ),
                                                            ),
                                                    ),
                                            );
                                        }
                                    }

                                    div()
                                        .id("session-creation-shell-scroll")
                                        .flex()
                                        .flex_col()
                                        .overflow_y_scroll()
                                        .max_h(px(220.0))
                                        .pr_1()
                                        .child(list)
                                })
                                .when_some(modal.error.clone(), |this, error| {
                                    this.child(div().text_sm().text_color(error_color).child(error))
                                }),
                        )
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
                                        .id("session-create-cancel")
                                        .px_4()
                                        .py_2()
                                        .border_1()
                                        .border_color(border_color)
                                        .rounded_md()
                                        .text_sm()
                                        .text_color(fg)
                                        .when(!modal_pending, |this| this.cursor_pointer())
                                        .when(!modal_pending, |this| {
                                            this.hover(|style| style.bg(border_color.opacity(0.1)))
                                        })
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, _window, cx| {
                                                if !modal_pending {
                                                    this.close_session_creation_modal();
                                                    cx.notify();
                                                }
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
                                        .id("session-create-apply")
                                        .px_4()
                                        .py_2()
                                        .bg(if modal_pending {
                                            primary.opacity(0.6)
                                        } else {
                                            primary
                                        })
                                        .rounded_md()
                                        .text_sm()
                                        .text_color(gpui::Hsla::white())
                                        .when(!modal_pending, |this| this.cursor_pointer())
                                        .when(!modal_pending, |this| {
                                            this.hover(|style| style.bg(primary.opacity(0.8)))
                                        })
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, _window, cx| {
                                                if !modal_pending {
                                                    this.apply_session_creation_modal(cx);
                                                }
                                            },
                                        ))
                                        .child(self.aligned_icon_label_row_with_offset(
                                            icons::plus(),
                                            gpui::Hsla::white(),
                                            12.0,
                                            if modal_pending {
                                                "Creating..."
                                            } else {
                                                "Create"
                                            },
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
