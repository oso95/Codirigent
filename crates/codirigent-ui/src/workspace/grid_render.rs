//! Workspace grid rendering and layout.
//!
//! This module handles rendering of the workspace grid layout, including:
//! - Traditional NxM grid layout
//! - Dispatch to split-tree rendering
//! - Session cells with terminals
//! - Empty grid cells and placeholders

use crate::terminal_header::TerminalHeaderRenderHints;
use crate::theme::CodirigentTheme;
use crate::workspace::gpui::WorkspaceView;
use crate::workspace::types::HEADER_HEIGHT;
use codirigent_core::SessionId;
use gpui::{
    div, px, Context, Focusable, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, ScrollWheelEvent, SharedString,
    StatefulInteractiveElement, Styled, Window,
};
use std::rc::Rc;

/// Visual state of a cell during drag-and-drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragVisual {
    /// This cell is being dragged (source) — dim it.
    Source,
    /// This cell is the current drop target — highlight it.
    Target,
}

impl WorkspaceView {
    /// Dispatch workspace rendering to the appropriate layout: split-tree or NxM grid.
    pub(super) fn render_grid_with_headers(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if self.workspace().is_split_tree_mode() {
            self.render_split_tree_layout(window, cx)
        } else {
            self.render_grid_layout(window, cx).into_any_element()
        }
    }

    /// Render the traditional NxM grid layout.
    fn render_grid_layout(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Clone all theme values upfront to avoid borrow issues
        let theme = self.workspace().theme().clone();
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let muted: gpui::Hsla = theme.muted.into();
        let grid_gap = theme.grid_gap;

        let profile = self.workspace().layout_profile();
        let (rows, cols) = profile.dimensions();

        // Get cell height from grid layout (height is calculated from available vertical space)
        let layout = self.grid_layout_with_task_board();
        let cell_height = layout.cell_size().height;

        let mut grid = div().flex_1().flex().flex_col().gap(px(grid_gap));

        for row in 0..rows {
            let mut row_div = div()
                .flex_1() // Equal row heights via flex distribution
                .flex()
                .flex_row()
                .gap(px(grid_gap));

            for col in 0..cols {
                let index = (row * cols + col) as usize;
                let position = codirigent_core::GridPosition { row, col };

                let cell_div = if let Some(info) = self
                    .cache
                    .render_cell_info
                    .iter()
                    .find(|info| info.index == index)
                    .cloned()
                {
                    // Get or create terminal header hints
                    let header_hints =
                        if let Some(header) = self.get_terminal_header(info.session_id) {
                            header.render_hints()
                        } else {
                            let focused =
                                self.workspace().focused_session_id() == Some(info.session_id);
                            self.workspace()
                                .session(info.session_id)
                                .map(|session| {
                                    crate::terminal_header::TerminalHeader::new(
                                        &session.name,
                                        session.status,
                                    )
                                    .with_focused(focused)
                                    .render_hints()
                                })
                                .unwrap_or_else(|| {
                                    crate::terminal_header::TerminalHeader::new(
                                        "Session",
                                        codirigent_core::SessionStatus::Idle,
                                    )
                                    .with_focused(focused)
                                    .render_hints()
                                })
                        };

                    // Render session cell with actual terminal content
                    self.render_session_cell_with_terminal(
                        info.pane_id.clone(),
                        info.session_id,
                        &header_hints,
                        &theme,
                        Some(cell_height),
                        window,
                        cx,
                    )
                } else {
                    // Empty cell - render inline
                    let pane_id = codirigent_core::PaneId::GridCell { index };
                    self.render_empty_cell_inline_with_colors(
                        pane_id,
                        index,
                        position,
                        panel_bg,
                        border_color,
                        muted,
                        cell_height,
                        cx,
                    )
                };

                // Let flex distribute equal widths; use size_full so
                // the child fills the flex-allocated area
                row_div = row_div.child(div().flex_1().size_full().child(cell_div));
            }

            grid = grid.child(row_div);
        }

        grid
    }

    /// Render a session cell with terminal header and actual terminal content.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_session_cell_with_terminal(
        &mut self,
        pane_id: codirigent_core::PaneId,
        session_id: SessionId,
        hints: &TerminalHeaderRenderHints,
        theme: &CodirigentTheme,
        cell_height: Option<f32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        // Uses HEADER_HEIGHT from types.rs
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let cell_border: gpui::Hsla = if hints.is_focused {
            theme.primary.into()
        } else {
            border_color
        };
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let orange: gpui::Hsla = theme.orange.into();

        // Drag-and-drop: find this cell's position in render_cell_info and its logical index.
        // `drag_vec_pos` is the Vec position (for visual state comparison).
        // `drag_logical_index` is `CellInfo.index` (for swap_sessions).
        let drag_vec_pos = self
            .cache
            .render_cell_info
            .iter()
            .position(|c| c.session_id == session_id);
        let drag_logical_index = drag_vec_pos.map(|pos| self.cache.render_cell_info[pos].index);

        let drag_visual = drag_vec_pos.and_then(|_pos| {
            let drag = self.selection.drag.as_ref()?;
            if !drag.active {
                return None;
            }
            if drag.source_index == drag_logical_index.unwrap_or(usize::MAX) {
                Some(DragVisual::Source)
            } else if drag.target.as_ref().map(|target| target.index) == drag_logical_index {
                Some(DragVisual::Target)
            } else {
                None
            }
        });

        // Override border color for drop target
        let cell_border = match drag_visual {
            Some(DragVisual::Target) => {
                let primary: gpui::Hsla = theme.primary.into();
                primary
            }
            _ => cell_border,
        };

        let header = self.render_pane_header(
            pane_id.clone(),
            session_id,
            hints,
            theme,
            panel_bg,
            border_color,
            cell_border,
            fg,
            muted,
            orange,
            drag_logical_index,
            matches!(drag_visual, Some(DragVisual::Source)),
            cx,
        );

        // Mouse-up handling for active-tab drags lives on the workspace root.

        // Render terminal content before building the div tree so the
        // mutable borrow on `self` is released before `cx.listener()`.
        let entity = cx.entity();
        let fh = self.focus_handle(cx);
        let is_focused = self.workspace.focused_session_id() == Some(session_id);
        let input_enabled = !self.has_blocking_modal();
        if let Some(terminal_view) = self.terminals.get_mut(&session_id) {
            terminal_view.set_focused(is_focused && input_enabled);
        }
        let (terminal_content, canvas_origin) = self.render_terminal_content(
            session_id,
            theme,
            Some((entity, fh, is_focused, input_enabled)),
            window,
        );
        let scrollbar_overlay =
            self.render_terminal_scrollbar(session_id, theme, Rc::clone(&canvas_origin), cx);
        let search_overlay = self.render_terminal_search_overlay(session_id, theme, cx);

        // Clone canvas_origin for each mouse handler closure
        let origin_for_down = Rc::clone(&canvas_origin);
        let origin_for_move = Rc::clone(&canvas_origin);
        let mut terminal_stack = div().relative().size_full().child(terminal_content);
        if let Some(scrollbar_overlay) = scrollbar_overlay {
            terminal_stack = terminal_stack.child(scrollbar_overlay);
        }
        if let Some(search_overlay) = search_overlay {
            terminal_stack = terminal_stack.child(search_overlay);
        }

        let mut outer = div()
            .id(SharedString::from(format!("session-cell-{}", session_id.0)))
            .w_full()
            .bg(panel_bg)
            .border_color(cell_border)
            .rounded_lg()
            .flex()
            .flex_col()
            .overflow_hidden()
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.select_session_with_cx(session_id, cx);
                    window.focus(&this.focus_handle(cx));
                    cx.notify();
                }),
            );

        // Drag visual: thicker border for drop target, normal otherwise
        outer = if matches!(drag_visual, Some(DragVisual::Target)) {
            outer.border_2()
        } else {
            outer.border_1()
        };

        // Drag visual: dim the source cell
        if matches!(drag_visual, Some(DragVisual::Source)) {
            outer = outer.opacity(0.5);
        }

        // Fixed height for grid mode, flexible for split tree mode
        if let Some(h) = cell_height {
            outer = outer.h(px(h));
        } else {
            outer = outer.size_full().flex_1();
        }

        let mut terminal_area = div()
            .id(SharedString::from(format!(
                "terminal-area-{}",
                session_id.0
            )))
            .w_full();

        // Fixed height for grid mode, flexible for split tree mode
        if let Some(h) = cell_height {
            terminal_area = terminal_area.h(px(h - HEADER_HEIGHT));
        } else {
            terminal_area = terminal_area.flex_1();
        }

        outer.child(header).child(
            terminal_area
                .relative()
                .overflow_hidden()
                .on_hover(cx.listener(move |this, hovered: &bool, _window, cx| {
                    if !hovered {
                        return;
                    }
                    if let Some(tv) = this.terminals_mut().get_mut(&session_id) {
                        tv.note_scroll_activity();
                        cx.notify();
                    }
                }))
                .on_scroll_wheel(
                    cx.listener(move |this, event: &ScrollWheelEvent, _window, cx| {
                        if let Some(tv) = this.terminals_mut().get_mut(&session_id) {
                            let cell_h: f32 = tv.cell_height();
                            let delta_y: f32 = event.delta.pixel_delta(px(cell_h)).y.into();
                            // Cap lines per event to avoid page-sized jumps from
                            // high-resolution touchpad momentum scrolling.
                            let max_lines: usize = tv.rows().max(1) as usize / 2;
                            let lines = (delta_y.abs() / cell_h)
                                .ceil()
                                .max(1.0)
                                .min(max_lines as f32)
                                as usize;
                            // Positive delta_y = scroll up = show older content (scrollback)
                            if delta_y > 0.0 {
                                tv.scroll_up(lines);
                            } else if delta_y < 0.0 {
                                // Snap to bottom when scrolling down within a small
                                // margin of the live view. Without this, accidentally
                                // scrolling up by even 1 line causes every new output
                                // line to push the viewport further from the bottom.
                                let snap_margin = 3;
                                if tv.display_offset() <= snap_margin + lines {
                                    tv.scroll_to_bottom();
                                } else {
                                    tv.scroll_down(lines);
                                }
                            }
                            cx.notify();
                        }
                    }),
                )
                // Mouse down: start text selection
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                        // Don't start text selection during drag
                        if this.selection.drag.as_ref().is_some_and(|d| d.active) {
                            return;
                        }
                        let metrics = origin_for_down.get();
                        let mouse_x: f32 = event.position.x.into();
                        let mouse_y: f32 = event.position.y.into();
                        let rel_x = mouse_x - metrics.origin_x;
                        let rel_y = mouse_y - metrics.origin_y;

                        if let Some(tv) = this.terminals_mut().get_mut(&session_id) {
                            let cell_pos: Option<(usize, usize)> = tv.pixel_to_cell(rel_x, rel_y);
                            if let Some((row, col)) = cell_pos {
                                tv.start_selection(row, col);
                                this.selection.is_selecting = true;
                                this.selection.selecting_session_id = Some(session_id);
                                cx.notify();
                            }
                        }
                    }),
                )
                // Mouse move: update selection during drag (with auto-scroll)
                .on_mouse_move(
                    cx.listener(move |this, event: &MouseMoveEvent, _window, cx| {
                        // Don't update text selection during drag
                        if this.selection.drag.as_ref().is_some_and(|d| d.active) {
                            return;
                        }
                        if !this.selection.is_selecting {
                            return;
                        }
                        if this.selection.selecting_session_id != Some(session_id) {
                            return;
                        }

                        let metrics = origin_for_move.get();
                        let mouse_x: f32 = event.position.x.into();
                        let mouse_y: f32 = event.position.y.into();
                        let rel_x = mouse_x - metrics.origin_x;
                        let rel_y = mouse_y - metrics.origin_y;

                        if let Some(tv) = this.terminals_mut().get_mut(&session_id) {
                            let (row, col, scroll_dir) = tv.pixel_to_cell_clamped(rel_x, rel_y);

                            // Auto-scroll when dragging above or below the viewport
                            if scroll_dir < 0 {
                                tv.scroll_up(1);
                            } else if scroll_dir > 0 {
                                tv.scroll_down(1);
                            }

                            tv.update_selection(row, col);
                            cx.notify();
                        }
                    }),
                )
                // Right-click: paste from clipboard (standard terminal behavior)
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, _event: &MouseDownEvent, window, cx| {
                        this.handle_paste(&crate::app::Paste, window, cx);
                    }),
                )
                // Mouse up: end selection
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |this, _event: &MouseUpEvent, _window, cx| {
                        if this.selection.is_selecting
                            && this.selection.selecting_session_id == Some(session_id)
                        {
                            // Selection stays active (for copy) until next click or clear
                            this.selection.is_selecting = false;
                            this.selection.selecting_session_id = None;
                            cx.notify();
                        }
                    }),
                )
                .child(terminal_stack),
        )
    }
}
