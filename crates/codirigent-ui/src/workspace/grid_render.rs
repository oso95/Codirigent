//! Workspace grid rendering and layout.
//!
//! This module handles rendering of the workspace grid layout, including:
//! - Traditional NxM grid layout
//! - Split tree (binary tree) layout
//! - Session cells with terminals
//! - Empty cells and placeholders

use crate::empty_session::EmptySessionRenderHints;
use crate::layout::LayoutProfile;
use crate::terminal_header::TerminalHeaderRenderHints;
use crate::terminal_view::CursorShape;
use crate::theme::CodirigentTheme;
use crate::workspace::gpui::WorkspaceView;
use codirigent_core::{LayoutNode, Session, SessionId, SplitDirection};
use gpui::{
    div, px, relative, Context, FontWeight, Image, ImageFormat, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, ParentElement, ScrollWheelEvent,
    Styled, StyledImage,
};
use std::sync::Arc;

impl WorkspaceView {
    /// Render the grid of session panes.
    pub(super) fn render_grid(&mut self) -> impl IntoElement {
        let theme = self.workspace().theme().clone();
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();

        let cells = self.workspace().cell_info();
        let profile = self.workspace().layout_profile();
        let (rows, cols) = profile.dimensions();

        let mut grid = div().flex_1().flex().flex_col().gap(px(theme.grid_gap));

        for row in 0..rows {
            let mut row_div = div().flex_1().flex().flex_row().gap(px(theme.grid_gap));

            for col in 0..cols {
                let index = (row * cols + col) as usize;
                let cell = cells.get(index);

                let cell_div = if let Some(info) = cell {
                    let status_color: gpui::Hsla = theme.status_color(info.status).into();
                    let cell_border = if info.is_focused {
                        let active: gpui::Hsla = theme.active.into();
                        active
                    } else {
                        border_color
                    };

                    div()
                        .flex_1()
                        .bg(panel_bg)
                        .border_1()
                        .border_color(cell_border)
                        .rounded_md()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .child(
                            // Header with session name
                            div()
                                .h(px(28.0))
                                .px_2()
                                .border_b_1()
                                .border_color(border_color)
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(status_color))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(fg)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(info.name.clone()),
                                ),
                        )
                        .child({
                            // Terminal content area
                            let (terminal_content, _canvas_origin) =
                                self.render_terminal_content(info.session_id, &theme);
                            terminal_content
                        })
                } else {
                    // Empty cell
                    div()
                        .flex_1()
                        .bg(panel_bg)
                        .border_1()
                        .border_color(border_color)
                        .rounded_md()
                        .border_dashed()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_base()
                                .text_color(border_color)
                                .font_family(icons::LUCIDE_FONT_FAMILY)
                                .child(icons::plus()),
                        )
                };

                row_div = row_div.child(cell_div);
            }

            grid = grid.child(row_div);
        }

        grid
    }

    /// Render the terminal content for a session using canvas-based rendering.
    ///
    /// Uses GPUI's `canvas()` element to paint terminal cells directly with
    /// `paint_quad()` and `ShapedLine::paint()`, reducing element count from
    /// ~8,000 divs to a single canvas element per terminal.
    ///
    /// Returns `(element, canvas_origin)` where `canvas_origin` is an `Rc<Cell>`
    /// that will contain the canvas origin (with padding) after prepaint, for use
    /// in mouse coordinate translation.
    /// and EmptySessionCell for empty slots, with actual terminal content.
    pub(super) fn render_grid_with_headers(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if self.workspace().is_split_tree_mode() {
            self.render_split_tree_layout(cx).into_any_element()
        } else {
            self.render_grid_layout(cx).into_any_element()
        }
    }

    /// Render the traditional NxM grid layout.
    fn render_grid_layout(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        // Clone all theme values upfront to avoid borrow issues
        let theme = self.workspace().theme().clone();
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let primary_color: gpui::Hsla = theme.primary.into();
        let muted: gpui::Hsla = theme.muted.into();
        let grid_gap = theme.grid_gap;

        // Clone cell info and layout dimensions
        let cells = self.workspace().cell_info();
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

                let cell_div = if let Some(info) = cells.get(index) {
                    // Session cell with terminal header
                    let cell_border = if info.is_focused {
                        primary_color
                    } else {
                        border_color
                    };

                    // Get or create terminal header hints
                    let header_hints =
                        if let Some(header) = self.get_terminal_header(info.session_id) {
                            header.render_hints()
                        } else {
                            // Create default hints from cell info
                            crate::terminal_header::TerminalHeader::new(&info.name, info.status)
                                .with_focused(info.is_focused)
                                .render_hints()
                        };

                    // Render session cell with actual terminal content
                    self.render_session_cell_with_terminal(
                        info.session_id,
                        &header_hints,
                        panel_bg,
                        cell_border,
                        border_color,
                        &theme,
                        Some(cell_height),
                        cx,
                    )
                } else {
                    // Empty cell - render inline
                    self.render_empty_cell_inline_with_colors(
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

    /// Render the split tree layout using recursive binary tree traversal.
    fn render_split_tree_layout(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme().clone();
        let grid_gap = theme.grid_gap;

        // Collect split tree state needed for rendering
        let tree = match self.workspace().layout_state() {
            crate::layout::WorkspaceLayoutState::SplitTree(s) => s.tree().clone(),
            _ => return div().flex_1().into_any_element(),
        };

        // Build a map of slot -> (session_id, is_focused, session_name, session_status)
        let cells = self.workspace().cell_info();
        let focused_session = self.workspace().focused_session_id();
        let split_state = self.workspace().layout_state().as_split_tree();
        let slot_sessions: std::collections::HashMap<
            SlotId,
            (SessionId, bool, String, codirigent_core::SessionStatus),
        > = if let Some(state) = split_state {
            state
                .assignments()
                .iter()
                .filter_map(|(slot, opt_sid)| {
                    let sid = (*opt_sid)?;
                    let session = self.workspace().session(sid)?;
                    let is_focused = focused_session == Some(sid);
                    Some((
                        *slot,
                        (sid, is_focused, session.name.clone(), session.status),
                    ))
                })
                .collect()
        } else {
            std::collections::HashMap::new()
        };

        // Get cell height for the full grid area (used for empty cells)
        let layout = self.grid_layout_with_task_board();
        let cell_height = layout.cell_size().height;

        self.render_split_node(&tree, &slot_sessions, &theme, cell_height, grid_gap, cx)
    }

    /// Recursively render a layout node in the split tree.
    fn render_split_node(
        &mut self,
        node: &LayoutNode,
        slot_sessions: &std::collections::HashMap<
            SlotId,
            (SessionId, bool, String, codirigent_core::SessionStatus),
        >,
        theme: &CodirigentTheme,
        cell_height: f32,
        gap: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let primary_color: gpui::Hsla = theme.primary.into();
        let muted: gpui::Hsla = theme.muted.into();

        match node {
            LayoutNode::Leaf { slot } => {
                if let Some((session_id, is_focused, name, status)) = slot_sessions.get(slot) {
                    let cell_border = if *is_focused {
                        primary_color
                    } else {
                        border_color
                    };

                    let header_hints = if let Some(header) = self.get_terminal_header(*session_id) {
                        header.render_hints()
                    } else {
                        crate::terminal_header::TerminalHeader::new(name, *status)
                            .with_focused(*is_focused)
                            .render_hints()
                    };

                    self.render_session_cell_with_terminal(
                        *session_id,
                        &header_hints,
                        panel_bg,
                        cell_border,
                        border_color,
                        theme,
                        None,
                        cx,
                    )
                    .into_any_element()
                } else {
                    // Empty slot
                    self.render_split_empty_slot(*slot, panel_bg, border_color, muted, cx)
                        .into_any_element()
                }
            }
            LayoutNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                // Render children recursively
                let first_elem =
                    self.render_split_node(first, slot_sessions, theme, cell_height, gap, cx);
                let second_elem =
                    self.render_split_node(second, slot_sessions, theme, cell_height, gap, cx);

                // Use flex ratio to distribute space: first gets `ratio`, second gets `1 - ratio`
                // Multiply by 1000 for precision in flex-grow values
                let first_flex = *ratio * 1000.0;
                let second_flex = (1.0 - *ratio) * 1000.0;

                let container = match direction {
                    SplitDirection::Horizontal => {
                        // Left-to-right split
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
                            .gap(px(gap))
                            .child(first_div)
                            .child(second_div)
                    }
                    SplitDirection::Vertical => {
                        // Top-to-bottom split
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
                            .gap(px(gap))
                            .child(first_div)
                            .child(second_div)
                    }
                };

                container.into_any_element()
            }
        }
    }

    /// Render an empty slot in split tree mode.
    fn render_split_empty_slot(
        &mut self,
        slot: SlotId,
        panel_bg: gpui::Hsla,
        border_color: gpui::Hsla,
        muted: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(SharedString::from(format!("empty-slot-{}", slot.0)))
            .size_full()
            .bg(panel_bg)
            .border_1()
            .border_color(border_color)
            .rounded_lg()
            .border_dashed()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                info!(?slot, "Empty split slot clicked — creating session");
                this.create_session_in_slot(slot, cx);
            }))
            .child(
                div()
                    .text_xl()
                    .text_color(muted)
                    .font_family(icons::LUCIDE_FONT_FAMILY)
                    .child(icons::circle_plus()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("Idle - Ready for next task"),
            )
    }

    /// Render a session cell with terminal header and actual terminal content.
    fn render_session_cell_with_terminal(
        &mut self,
        session_id: SessionId,
        hints: &TerminalHeaderRenderHints,
        panel_bg: gpui::Hsla,
        cell_border: gpui::Hsla,
        border_color: gpui::Hsla,
        theme: &CodirigentTheme,
        cell_height: Option<f32>,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        const HEADER_HEIGHT: f32 = 32.0;
        let fg: gpui::Hsla = theme.foreground.into();

        let header_border = if hints.is_focused {
            let primary: gpui::Hsla = theme.primary.into();
            primary
        } else {
            border_color
        };

        // Color indicator bar
        let color_indicator: gpui::Hsla = hints.color_indicator.into();
        let status_color: gpui::Hsla = hints.status.color.into();

        let mut header = div()
            .id(SharedString::from(format!(
                "terminal-header-{}",
                session_id.0
            )))
            .h(px(hints.height))
            .w_full()
            .bg(panel_bg)
            .border_b_1()
            .border_color(header_border)
            .flex()
            .items_center()
            .px_2()
            .gap_2()
            .child(
                div()
                    .w(px(3.0))
                    .h(px(16.0))
                    .rounded_sm()
                    .bg(color_indicator),
            )
            .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(status_color))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(fg)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(hints.name.clone()),
            );

        // Project/directory name (after session name)
        if let Some(project) = &hints.project_name {
            let muted_fg = gpui::Hsla {
                h: 0.0,
                s: 0.0,
                l: 0.5,
                a: 0.7,
            };
            header = header.child(
                div()
                    .text_xs()
                    .text_color(muted_fg)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(project.clone()),
            );
        }

        // Git branch badge (after session name)
        if let Some(branch) = &hints.git_branch {
            let git_muted = gpui::Hsla {
                h: 0.0,
                s: 0.0,
                l: 0.6,
                a: 0.8,
            };
            let branch_label = if branch.chars().count() > 16 {
                let truncated: String = branch.chars().take(13).collect();
                format!("{}...", truncated)
            } else {
                branch.clone()
            };
            let mut git_badge = div()
                .px(px(4.0))
                .py_px()
                .rounded_sm()
                .bg(gpui::Hsla {
                    h: 0.0,
                    s: 0.0,
                    l: 1.0,
                    a: 0.06,
                })
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(git_muted)
                        .font_family(icons::LUCIDE_FONT_FAMILY)
                        .child(icons::git_branch()),
                )
                .child(div().text_xs().text_color(git_muted).child(branch_label));

            if let Some(count) = hints.git_dirty_count {
                if count > 0 {
                    git_badge = git_badge.child(
                        div()
                            .text_xs()
                            .text_color(gpui::Hsla {
                                h: 0.1,
                                s: 0.8,
                                l: 0.6,
                                a: 1.0,
                            })
                            .child(format!("+{}", count)),
                    );
                }
            }

            header = header.child(git_badge);
        }

        header = header.child(div().flex_1());

        // Task badge (if any)
        if let Some(task) = &hints.task {
            let task_bg: gpui::Hsla = task.bg_color.into();
            let task_color: gpui::Hsla = task.text_color.into();
            header = header.child(
                div()
                    .px_2()
                    .py_px()
                    .rounded_sm()
                    .bg(task_bg)
                    .text_xs()
                    .text_color(task_color)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(task.display_text.clone()),
            );
        }

        // Context usage (if any)
        if let Some(context) = &hints.context {
            let context_color: gpui::Hsla = context.color.into();
            header = header.child(
                div()
                    .text_xs()
                    .text_color(context_color)
                    .child(context.text().to_string()),
            );
        }

        // Render terminal content before building the div tree so the
        // mutable borrow on `self` is released before `cx.listener()`.
        let (terminal_content, canvas_origin) = self.render_terminal_content(session_id, theme);

        // Clone canvas_origin for each mouse handler closure
        let origin_for_down = Rc::clone(&canvas_origin);
        let origin_for_move = Rc::clone(&canvas_origin);

        let mut outer = div()
            .id(SharedString::from(format!("session-cell-{}", session_id.0)))
            .w_full()
            .bg(panel_bg)
            .border_1()
            .border_color(cell_border)
            .rounded_lg()
            .flex()
            .flex_col()
            .overflow_hidden()
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.select_session(session_id);
                    cx.notify();
                }),
            );

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
                .overflow_hidden()
                .on_scroll_wheel(
                    cx.listener(move |this, event: &ScrollWheelEvent, _window, cx| {
                        if let Some(tv) = this.terminals_mut().get_mut(&session_id) {
                            let cell_h: f32 = tv.cell_height();
                            let delta_y: f32 = event.delta.pixel_delta(px(cell_h)).y.into();
                            // GPUI on Windows: positive y = finger/wheel up = show older content
                            if delta_y > 0.0 {
                                let lines = (delta_y / cell_h).ceil().max(1.0) as usize;
                                tv.scroll_up(lines);
                            } else if delta_y < 0.0 {
                                let lines = (-delta_y / cell_h).ceil().max(1.0) as usize;
                                tv.scroll_down(lines);
                            }
                            cx.notify();
                        }
                    }),
                )
                // Mouse down: start text selection
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                        let (ox, oy) = origin_for_down.get();
                        let mouse_x: f32 = event.position.x.into();
                        let mouse_y: f32 = event.position.y.into();
                        let rel_x = mouse_x - ox;
                        let rel_y = mouse_y - oy;

                        if let Some(tv) = this.terminals_mut().get_mut(&session_id) {
                            let cell_pos: Option<(usize, usize)> = tv.pixel_to_cell(rel_x, rel_y);
                            if let Some((row, col)) = cell_pos {
                                tv.start_selection(row, col);
                                this.is_selecting = true;
                                this.selecting_session_id = Some(session_id);
                                cx.notify();
                            }
                        }
                    }),
                )
                // Mouse move: update selection during drag
                .on_mouse_move(
                    cx.listener(move |this, event: &MouseMoveEvent, _window, cx| {
                        if !this.is_selecting {
                            return;
                        }
                        if this.selecting_session_id != Some(session_id) {
                            return;
                        }

                        let (ox, oy) = origin_for_move.get();
                        let mouse_x: f32 = event.position.x.into();
                        let mouse_y: f32 = event.position.y.into();
                        let rel_x = mouse_x - ox;
                        let rel_y = mouse_y - oy;

                        if let Some(tv) = this.terminals_mut().get_mut(&session_id) {
                            let cell_pos: Option<(usize, usize)> = tv.pixel_to_cell(rel_x, rel_y);
                            if let Some((row, col)) = cell_pos {
                                tv.update_selection(row, col);
                                cx.notify();
                            }
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
                        if this.is_selecting && this.selecting_session_id == Some(session_id) {
                            if let Some(tv) = this.terminals_mut().get_mut(&session_id) {
                                tv.end_selection();
                            }
                            this.is_selecting = false;
                            this.selecting_session_id = None;
                            cx.notify();
                        }
                    }),
                )
                .child(terminal_content),
        )
    }

}
