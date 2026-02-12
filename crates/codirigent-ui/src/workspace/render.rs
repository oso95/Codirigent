//! GPUI rendering components for WorkspaceView.
//!
//! This module contains the rendering logic for the workspace grid, top bar,
//! icon rail, drawer, and right task board panel,
//! separated from the main WorkspaceView to keep file sizes manageable.

use super::gpui::WorkspaceView;
// Import from main branch (terminal rendering)
use crate::terminal_view::CursorShape;
// Imports from feature branch (UI components)
use crate::components::text_input::{text_input, TextInputStyle};
use crate::empty_session::EmptySessionRenderHints;
use crate::icons;
use crate::layout::LayoutProfile;
use crate::terminal_header::TerminalHeaderRenderHints;
use crate::theme::CodirigentTheme;
use crate::title_bar::TitleBar;
use crate::toolbar::CustomLayoutMode;
use codirigent_core::{LayoutNode, Session, SessionId, SlotId, SplitDirection};
use gpui::{
    div, prelude::FluentBuilder, px, relative, ClickEvent, Context, FontWeight, Image, ImageFormat,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ObjectFit, ParentElement, ScrollWheelEvent, SharedString, StatefulInteractiveElement, Styled,
    StyledImage, Window, WindowControlArea,
};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use tracing::info;

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
    fn render_terminal_content(
        &mut self,
        session_id: SessionId,
        theme: &CodirigentTheme,
    ) -> (gpui::AnyElement, Rc<Cell<(f32, f32)>>) {
        let terminal_bg: gpui::Hsla = theme.terminal_background.into();
        let terminal_fg: gpui::Hsla = theme.terminal_foreground.into();

        // Shared cell for canvas origin (updated during prepaint)
        let canvas_origin: Rc<Cell<(f32, f32)>> = Rc::new(Cell::new((0.0, 0.0)));

        // Get the terminal view for this session
        let Some(terminal_view) = self.terminals_mut().get_mut(&session_id) else {
            // No terminal yet, show placeholder
            return (
                div()
                    .flex_1()
                    .bg(terminal_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_base()
                            .text_color(terminal_fg)
                            .font_family(icons::LUCIDE_FONT_FAMILY)
                            .child(icons::terminal()),
                    )
                    .into_any_element(),
                canvas_origin,
            );
        };

        // Capture fallback dimensions (will be overridden by font metrics in prepaint)
        let fallback_cell_width = terminal_view.cell_width();
        let fallback_cell_height = terminal_view.cell_height();
        let font_size = terminal_view.font_size();
        let cursor_rect = terminal_view.cursor_rect();
        let needs_dimension_init = !terminal_view.dimensions_initialized();

        // Get cached content (only recomputes if dirty)
        let content = terminal_view.cached_content().clone();

        // Pre-convert background rects to GPUI colors
        let bg_rects: Vec<(usize, usize, usize, gpui::Hsla)> = content
            .background_rects
            .iter()
            .map(|(row, start, end, color)| (*row, *start, *end, (*color).into()))
            .collect();

        // Pre-convert text runs to GPUI colors
        let text_runs: Vec<(crate::terminal_view::TextRunSegment, gpui::Hsla)> = content
            .text_runs
            .iter()
            .map(|run| {
                let fg: gpui::Hsla = run.foreground.into();
                (run.clone(), fg)
            })
            .collect();

        // Pre-convert cursor
        let cursor_data = cursor_rect.map(|c| {
            let color: gpui::Hsla = c.color.into();
            (c, color)
        });

        let font_family: gpui::SharedString =
            crate::terminal_view::default_terminal_font_family().into();

        // Clone Rc for capture into the canvas prepaint closure
        let canvas_origin_for_prepaint = Rc::clone(&canvas_origin);

        // Build canvas element that paints directly
        let terminal_canvas = gpui::canvas(
            // Prepaint: shape text lines for each row's text runs
            move |bounds, window: &mut gpui::Window, _cx: &mut gpui::App| {
                // Store origin as f32 for arithmetic (Pixels doesn't support Add in gpui 0.2.1)
                let origin_x: f32 = bounds.origin.x.into();
                let origin_y: f32 = bounds.origin.y.into();
                // Must match TERMINAL_CONTENT_PADDING in resize_terminals_to_grid
                let padding = 4.0_f32;
                let ox = origin_x + padding;
                let oy = origin_y + padding;

                // Store origin for mouse coordinate translation
                canvas_origin_for_prepaint.set((ox, oy));

                // Compute cell dimensions from font metrics (Zed pattern)
                // This ensures proper character spacing by using the actual 'm' advance width
                let (cell_width, cell_height) = if needs_dimension_init {
                    crate::terminal_view::compute_cell_dimensions(
                        window.text_system(),
                        crate::terminal_view::default_terminal_font_family(),
                        font_size,
                    )
                } else {
                    (fallback_cell_width, fallback_cell_height)
                };

                // Shape text for each run (prepaint phase)
                let mut shaped_runs: Vec<(usize, usize, gpui::ShapedLine)> =
                    Vec::with_capacity(text_runs.len());
                let font_size_px = px(font_size);

                for (run, fg_color) in &text_runs {
                    let weight = if run.bold {
                        gpui::FontWeight::BOLD
                    } else {
                        gpui::FontWeight::NORMAL
                    };
                    let style = if run.italic {
                        gpui::FontStyle::Italic
                    } else {
                        gpui::FontStyle::Normal
                    };

                    let font = gpui::Font {
                        family: font_family.clone(),
                        features: gpui::FontFeatures::default(),
                        fallbacks: None,
                        weight,
                        style,
                    };

                    let underline = if run.underline {
                        Some(gpui::UnderlineStyle {
                            thickness: px(1.0),
                            color: Some(*fg_color),
                            wavy: false,
                        })
                    } else {
                        None
                    };

                    let strikethrough = if run.strikethrough {
                        Some(gpui::StrikethroughStyle {
                            thickness: px(1.0),
                            color: Some(*fg_color),
                        })
                    } else {
                        None
                    };

                    let text: gpui::SharedString = run.text.clone().into();
                    let text_run = gpui::TextRun {
                        len: text.len(),
                        font,
                        color: *fg_color,
                        background_color: None,
                        underline,
                        strikethrough,
                    };

                    let shaped =
                        window
                            .text_system()
                            .shape_line(text, font_size_px, &[text_run], None);

                    shaped_runs.push((run.row, run.start_col, shaped));
                }

                (
                    ox,
                    oy,
                    bg_rects,
                    shaped_runs,
                    cursor_data,
                    cell_width,
                    cell_height,
                )
            },
            // Paint: draw backgrounds, text, and cursor
            move |_bounds: gpui::Bounds<gpui::Pixels>,
                  prepaint_data: (
                f32,
                f32,
                Vec<(usize, usize, usize, gpui::Hsla)>,
                Vec<(usize, usize, gpui::ShapedLine)>,
                Option<(crate::terminal_view::CursorRect, gpui::Hsla)>,
                f32,
                f32,
            ),
                  window: &mut gpui::Window,
                  cx: &mut gpui::App| {
                let (ox, oy, bg_rects, shaped_runs, cursor_data, cell_w, cell_h) = prepaint_data;

                // 1. Paint background rectangles
                for (row, start_col, end_col, bg_color) in &bg_rects {
                    let rect_x = ox + *start_col as f32 * cell_w;
                    let rect_y = oy + *row as f32 * cell_h;
                    let rect_w = (*end_col - *start_col) as f32 * cell_w;
                    let rect_bounds = gpui::Bounds {
                        origin: gpui::Point {
                            x: px(rect_x),
                            y: px(rect_y),
                        },
                        size: gpui::Size {
                            width: px(rect_w),
                            height: px(cell_h),
                        },
                    };
                    window.paint_quad(gpui::fill(rect_bounds, *bg_color));
                }

                // 2. Paint shaped text runs
                for (row, start_col, shaped_line) in &shaped_runs {
                    let text_x = ox + *start_col as f32 * cell_w;
                    let text_y = oy + *row as f32 * cell_h;
                    let text_origin = gpui::Point {
                        x: px(text_x),
                        y: px(text_y),
                    };
                    let _ = shaped_line.paint(text_origin, px(cell_h), window, cx);
                }

                // 3. Paint cursor
                if let Some((cursor, cursor_color)) = &cursor_data {
                    let cx_pos = ox + cursor.x;
                    let cy_pos = oy + cursor.y;

                    match cursor.shape {
                        CursorShape::Block => {
                            let cursor_bounds = gpui::Bounds {
                                origin: gpui::Point {
                                    x: px(cx_pos),
                                    y: px(cy_pos),
                                },
                                size: gpui::Size {
                                    width: px(cell_w),
                                    height: px(cell_h),
                                },
                            };
                            window.paint_quad(gpui::fill(cursor_bounds, cursor_color.opacity(0.7)));
                        }
                        CursorShape::HollowBlock => {
                            let cursor_bounds = gpui::Bounds {
                                origin: gpui::Point {
                                    x: px(cx_pos),
                                    y: px(cy_pos),
                                },
                                size: gpui::Size {
                                    width: px(cell_w),
                                    height: px(cell_h),
                                },
                            };
                            window.paint_quad(gpui::quad(
                                cursor_bounds,
                                px(0.0),
                                gpui::transparent_black(),
                                px(1.0),
                                *cursor_color,
                                gpui::BorderStyle::default(),
                            ));
                        }
                        CursorShape::Beam => {
                            let cursor_bounds = gpui::Bounds {
                                origin: gpui::Point {
                                    x: px(cx_pos),
                                    y: px(cy_pos),
                                },
                                size: gpui::Size {
                                    width: px(2.0),
                                    height: px(cell_h),
                                },
                            };
                            window.paint_quad(gpui::fill(cursor_bounds, *cursor_color));
                        }
                        CursorShape::Underline => {
                            let cursor_bounds = gpui::Bounds {
                                origin: gpui::Point {
                                    x: px(cx_pos),
                                    y: px(cy_pos + cell_h - 2.0),
                                },
                                size: gpui::Size {
                                    width: px(cell_w),
                                    height: px(2.0),
                                },
                            };
                            window.paint_quad(gpui::fill(cursor_bounds, *cursor_color));
                        }
                    }
                }
            },
        )
        .size_full();

        // Wrap canvas in a container with terminal background
        // Note: size_full() instead of flex_1() because parent has explicit dimensions
        let element = div()
            .size_full()
            .bg(terminal_bg)
            .overflow_hidden()
            .child(terminal_canvas)
            .into_any_element();

        (element, canvas_origin)
    }

    /// Render the title bar with window controls (minimize, maximize, close).
    ///
    /// This is a 32px bar with the logo on the left and native window controls
    /// on the right. The entire bar is a drag region for moving the window.
    pub(super) fn render_title_bar(
        &mut self,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.workspace().theme();
        let bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();

        // The entire bar is a drag region. Caption buttons use .occlude() +
        // their own WindowControlArea to carve out non-drag zones.
        let mut bar = div()
            .id("title-bar")
            .h(px(self.title_bar.height()))
            .w_full()
            .bg(bg)
            .border_b_1()
            .border_color(border_color)
            .flex()
            .items_center()
            .px_3()
            .gap_2()
            .window_control_area(WindowControlArea::Drag);

        // macOS: Native traffic lights are rendered by the OS.
        // Reserve left padding so content doesn't overlap them, and handle
        // double-click to trigger the system zoom behavior (following Zed's approach).
        #[cfg(target_os = "macos")]
        {
            const TRAFFIC_LIGHT_PADDING: f32 = 71.0;

            bar = if window.is_fullscreen() {
                bar.pl_2()
            } else {
                bar.pl(px(TRAFFIC_LIGHT_PADDING))
            };

            bar = bar.on_click(|event: &ClickEvent, window, _cx| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            });
        }

        // Windows: GPUI 0.2.1 has a stale mouse_hit_test issue in WM_NCHITTEST,
        // so WindowControlArea::Drag alone doesn't reliably initiate drags.
        // Work around by sending WM_NCLBUTTONDOWN(HTCAPTION) on mouse-down.
        #[cfg(target_os = "windows")]
        {
            use raw_window_handle::HasWindowHandle;
            let raw_handle = window.window_handle().ok().map(|h| match h.as_raw() {
                raw_window_handle::RawWindowHandle::Win32(win32) => win32.hwnd.get() as isize,
                _ => 0,
            });
            if let Some(hwnd) = raw_handle {
                bar = bar.on_mouse_down(MouseButton::Left, move |_event, _window, _cx| {
                    crate::platform_drag::begin_title_bar_drag(hwnd);
                });
            }
        }

        // Logo (3x3 grid matching logo-primary-dark.svg)
        bar = bar.child(div().flex_shrink_0().ml_2().child(self.render_logo_small()));
        bar = bar.child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(fg)
                .ml_2()
                .child(TitleBar::LOGO_TEXT),
        );

        // Spacer — fills remaining space so window controls stay on the right
        bar = bar.child(div().flex_1());

        // Window controls (Windows/Linux)
        // Uses native Segoe icon fonts and WindowControlArea for OS-level handling.
        #[cfg(not(target_os = "macos"))]
        {
            let icon_font = "Segoe MDL2 Assets";

            let close_hover_bg: gpui::Hsla = gpui::Rgba {
                r: 232.0 / 255.0,
                g: 17.0 / 255.0,
                b: 32.0 / 255.0,
                a: 1.0,
            }
            .into();

            let maximize_icon = if window.is_maximized() {
                "\u{e923}" // Restore
            } else {
                "\u{e922}" // Maximize
            };

            let controls = div()
                .flex()
                .flex_row()
                .items_center()
                .font_family(icon_font)
                // Minimize
                .child(
                    div()
                        .id("window-minimize")
                        .w(px(36.0))
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(10.0))
                        .text_color(fg)
                        .occlude()
                        .hover(|style| style.bg(border_color.opacity(0.2)))
                        .active(|style| style.bg(border_color.opacity(0.3)))
                        .window_control_area(WindowControlArea::Min)
                        .child("\u{e921}"),
                )
                // Maximize / Restore
                .child(
                    div()
                        .id("window-maximize")
                        .w(px(36.0))
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(10.0))
                        .text_color(fg)
                        .occlude()
                        .hover(|style| style.bg(border_color.opacity(0.2)))
                        .active(|style| style.bg(border_color.opacity(0.3)))
                        .window_control_area(WindowControlArea::Max)
                        .child(maximize_icon),
                )
                // Close
                .child(
                    div()
                        .id("window-close")
                        .w(px(36.0))
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(10.0))
                        .text_color(fg)
                        .occlude()
                        .hover(|style| style.bg(close_hover_bg).text_color(gpui::Hsla::white()))
                        .active(|style| {
                            style
                                .bg(close_hover_bg.opacity(0.8))
                                .text_color(gpui::Hsla::white().opacity(0.8))
                        })
                        .window_control_area(WindowControlArea::Close)
                        .child("\u{e8bb}"),
                );
            bar = bar.child(controls);
        }

        bar
    }

    /// Render the unified top bar (replaces separate TitleBar + Toolbar).
    ///
    /// A single 48px bar containing: logo, layout tabs, broadcast toggle,
    pub(super) fn render_terminal_header(
        &self,
        session_id: SessionId,
        hints: &TerminalHeaderRenderHints,
    ) -> impl IntoElement {
        let theme = self.workspace().theme();
        let bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();

        let header_border = if hints.is_focused {
            let primary: gpui::Hsla = theme.primary.into();
            primary
        } else {
            border_color
        };

        let mut header = div()
            .id(SharedString::from(format!(
                "terminal-header-{}",
                session_id.0
            )))
            .h(px(hints.height))
            .w_full()
            .bg(bg)
            .border_b_1()
            .border_color(header_border)
            .flex()
            .items_center()
            .px_2()
            .gap_2();

        // Color indicator bar
        let color_indicator: gpui::Hsla = hints.color_indicator.into();
        header = header.child(
            div()
                .w(px(3.0))
                .h(px(16.0))
                .rounded_sm()
                .bg(color_indicator),
        );

        // Status dot
        let status_color: gpui::Hsla = hints.status.color.into();
        header = header.child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(status_color));

        // Session name
        header = header.child(
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

        // Group badge (if session is in a group)
        if let Some(group) = &hints.group_name {
            let group_color: gpui::Hsla = hints.color_indicator.into();
            let badge_bg = gpui::Hsla {
                h: group_color.h,
                s: group_color.s,
                l: group_color.l,
                a: 0.15,
            };
            let group_label = if group.chars().count() > 12 {
                let truncated: String = group.chars().take(10).collect();
                format!("{}...", truncated)
            } else {
                group.clone()
            };
            header = header.child(
                div()
                    .px(px(5.0))
                    .py_px()
                    .rounded_sm()
                    .bg(badge_bg)
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_1()
                    .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(group_color))
                    .child(div().text_xs().text_color(group_color).child(group_label)),
            );
        }

        // Spacer
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

        header
    }

    /// Render an empty session cell.
    ///
    /// Returns a GPUI element representing an empty grid slot with a dashed border
    /// and a plus icon.
    pub(super) fn render_empty_cell(
        &self,
        hints: &EmptySessionRenderHints,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let bg: gpui::Hsla = hints.background.into();
        let border_color: gpui::Hsla = hints.border.color.into();
        let icon_color: gpui::Hsla = hints.icon_color.into();
        let text_color: gpui::Hsla = hints.text_color.into();
        let position = hints.position;

        div()
            .id(SharedString::from(format!(
                "empty-cell-{}-{}",
                position.row, position.col
            )))
            .flex_1()
            .bg(bg)
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
                info!(?position, "Empty cell clicked");
                this.empty_cells.click(position);
                cx.notify();
            }))
            .child(div().text_xl().text_color(icon_color).child(hints.icon))
            .child(div().text_xs().text_color(text_color).child(hints.message))
    }

    /// This is the updated grid renderer that uses TerminalHeader for sessions
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

    /// Render terminal header inline (returns Stateful<Div> for type consistency).
    fn render_terminal_header_inline(
        &self,
        session_id: SessionId,
        hints: &TerminalHeaderRenderHints,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.workspace().theme();
        let bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();

        let header_border = if hints.is_focused {
            let primary: gpui::Hsla = theme.primary.into();
            primary
        } else {
            border_color
        };

        let mut header = div()
            .id(SharedString::from(format!(
                "terminal-header-{}",
                session_id.0
            )))
            .h(px(hints.height))
            .w_full()
            .bg(bg)
            .border_b_1()
            .border_color(header_border)
            .flex()
            .items_center()
            .px_2()
            .gap_2();

        // Color indicator bar
        let color_indicator: gpui::Hsla = hints.color_indicator.into();
        header = header.child(
            div()
                .w(px(3.0))
                .h(px(16.0))
                .rounded_sm()
                .bg(color_indicator),
        );

        // Status dot
        let status_color: gpui::Hsla = hints.status.color.into();
        header = header.child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(status_color));

        // Session name
        header = header.child(
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

        // Group badge (if session is in a group)
        if let Some(group) = &hints.group_name {
            let group_color: gpui::Hsla = hints.color_indicator.into();
            let badge_bg = gpui::Hsla {
                h: group_color.h,
                s: group_color.s,
                l: group_color.l,
                a: 0.15,
            };
            let group_label = if group.chars().count() > 12 {
                let truncated: String = group.chars().take(10).collect();
                format!("{}...", truncated)
            } else {
                group.clone()
            };
            header = header.child(
                div()
                    .px(px(5.0))
                    .py_px()
                    .rounded_sm()
                    .bg(badge_bg)
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_1()
                    .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(group_color))
                    .child(div().text_xs().text_color(group_color).child(group_label)),
            );
        }

        // Spacer
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

        header
    }

    /// Render empty cell inline with pre-computed colors (returns Stateful<Div>).
    fn render_empty_cell_inline_with_colors(
        &mut self,
        position: codirigent_core::GridPosition,
        panel_bg: gpui::Hsla,
        border_color: gpui::Hsla,
        muted: gpui::Hsla,
        cell_height: f32,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(SharedString::from(format!(
                "empty-cell-{}-{}",
                position.row, position.col
            )))
            .w_full()
            .h(px(cell_height))
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
                info!(?position, "Empty cell clicked");
                this.empty_cells.click(position);
                cx.notify();
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

    /// Render empty cell inline (returns Stateful<Div>).
    fn render_empty_cell_inline(
        &mut self,
        position: codirigent_core::GridPosition,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = self.workspace().theme().clone();
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let muted: gpui::Hsla = theme.muted.into();

        let layout = self.grid_layout_with_task_board();
        let cell_height = layout.cell_size().height;

        self.render_empty_cell_inline_with_colors(
            position,
            panel_bg,
            border_color,
            muted,
            cell_height,
            cx,
        )
    }

    /// Render the custom layout picker modal.
    ///
    /// Displays a modal overlay with tabs for Grid and Split modes.
    /// Grid mode provides rows/columns inputs; Split mode provides an
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
    fn render_logo_small(&self) -> impl IntoElement {
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
    pub(super) fn render_session_menu(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let session_id = self.session_menu_open?;

        let theme = self.workspace().theme().clone();
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let hover_bg: gpui::Hsla = theme.active.into();
        let destructive = gpui::Hsla {
            h: 0.0,
            s: 0.7,
            l: 0.55,
            a: 1.0,
        };

        // Check if this session has a group
        let session_group = self
            .workspace()
            .session(session_id)
            .and_then(|s| s.group.clone());
        let has_group = session_group.is_some();

        // Collect existing group names (deduplicated, sorted)
        let existing_groups: Vec<String> = {
            let mut groups: Vec<String> = self
                .workspace()
                .sessions()
                .iter()
                .filter_map(|s| s.group.clone())
                .filter(|g| !g.is_empty())
                .collect();
            groups.sort();
            groups.dedup();
            groups
        };

        // Compute vertical position based on session's index in the list
        let sessions = self.workspace().sessions().to_vec();
        let mut row_index = 0usize;
        for (i, s) in sessions.iter().enumerate() {
            if s.id == session_id {
                row_index = i;
                break;
            }
        }
        let top_offset = crate::title_bar::TitleBar::DEFAULT_HEIGHT
            + crate::top_bar::TopBar::HEIGHT
            + 40.0   // drawer header
            + (row_index as f32) * 36.0;

        // Transparent click-away backdrop (no dark overlay)
        let backdrop = div()
            .id("session-menu-backdrop")
            .absolute()
            .inset_0()
            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                this.close_session_menu(cx);
            }));

        // Build dropdown menu
        let mut dropdown = div()
            .w(px(180.0))
            .bg(panel_bg)
            .border_1()
            .border_color(border_color)
            .rounded_md()
            .overflow_hidden()
            .shadow_lg()
            .flex()
            .flex_col()
            .py_1();

        // Rename
        dropdown = dropdown.child(self.render_menu_item(
            "Rename",
            session_id,
            SessionMenuAction::Rename,
            &theme,
            hover_bg,
            fg,
            cx,
        ));

        // Separator before groups section
        dropdown = dropdown.child(div().h(px(1.0)).mx_2().my_1().bg(border_color));

        // Existing groups as direct-click options
        if !existing_groups.is_empty() {
            dropdown = dropdown.child(
                div().px_3().pt_1().pb(px(2.0)).child(
                    div()
                        .text_xs()
                        .text_color(muted.opacity(0.6))
                        .child("GROUPS"),
                ),
            );
            for group_name in &existing_groups {
                let is_current = session_group.as_deref() == Some(group_name.as_str());
                let label = if is_current {
                    format!("{} \u{2713}", group_name)
                } else {
                    group_name.clone()
                };
                dropdown = dropdown.child(self.render_menu_item(
                    &label,
                    session_id,
                    SessionMenuAction::AssignToGroup(group_name.clone()),
                    &theme,
                    hover_bg,
                    if is_current { muted } else { fg },
                    cx,
                ));
            }
        }

        // New Group...
        dropdown = dropdown.child(self.render_menu_item(
            "New Group\u{2026}",
            session_id,
            SessionMenuAction::NewGroup,
            &theme,
            hover_bg,
            fg,
            cx,
        ));

        // Remove Group (only if session has a group)
        if has_group {
            dropdown = dropdown.child(self.render_menu_item(
                "Remove Group",
                session_id,
                SessionMenuAction::RemoveGroup,
                &theme,
                hover_bg,
                fg,
                cx,
            ));
        }

        // Separator before destructive action
        dropdown = dropdown.child(div().h(px(1.0)).mx_2().my_1().bg(border_color));

        // End Session (destructive, clearly labeled)
        dropdown = dropdown.child(self.render_menu_item(
            "End Session",
            session_id,
            SessionMenuAction::EndSession,
            &theme,
            hover_bg,
            destructive,
            cx,
        ));

        // Position dropdown to the right of the drawer, aligned with the row
        let left_offset = crate::icon_rail::IconRail::WIDTH + self.drawer.width() - 8.0;

        Some(
            div()
                .id("session-menu-container")
                .absolute()
                .inset_0()
                .child(backdrop)
                .child(
                    div()
                        .absolute()
                        .left(px(left_offset))
                        .top(px(top_offset))
                        .child(dropdown),
                ),
        )
    }

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
            super::gpui::SessionActionKind::Rename => "Rename Session",
            super::gpui::SessionActionKind::AssignGroup => "Assign Group",
        };
        let title_icon = match modal.kind {
            super::gpui::SessionActionKind::Rename => icons::pencil(),
            super::gpui::SessionActionKind::AssignGroup => icons::users(),
        };
        let label = match modal.kind {
            super::gpui::SessionActionKind::Rename => "Session Name:",
            super::gpui::SessionActionKind::AssignGroup => "Group Name:",
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
    pub(super) fn render_drawer(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme();
        let drawer_bg: gpui::Hsla = theme.drawer_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let header_bg: gpui::Hsla = theme.header_background.into();
        let muted: gpui::Hsla = theme.muted.into();
        let width = self.drawer.width();
        let panel = self.drawer.active_panel();

        let panel_title = match panel {
            Some(crate::icon_rail::DrawerPanel::Sessions) => "SESSIONS",
            Some(crate::icon_rail::DrawerPanel::Files) => "EXPLORER",
            Some(crate::icon_rail::DrawerPanel::Worktrees) => "WORKTREES",
            None => "",
        };

        // Build content based on active panel
        let content = match panel {
            Some(crate::icon_rail::DrawerPanel::Sessions) => {
                self.render_drawer_sessions_content(cx).into_any_element()
            }
            Some(crate::icon_rail::DrawerPanel::Worktrees) => {
                self.render_drawer_worktrees_content(cx).into_any_element()
            }
            Some(crate::icon_rail::DrawerPanel::Files) => {
                self.render_drawer_files_content(cx).into_any_element()
            }
            None => {
                let session_label = match self.selected_session_id {
                    Some(id) => format!("Session {}", id.0),
                    None => "No session selected".to_string(),
                };
                div()
                    .flex_1()
                    .overflow_hidden()
                    .p_3()
                    .child(div().text_xs().text_color(muted).child(session_label))
                    .into_any_element()
            }
        };

        div()
            .id("drawer-panel")
            .w(px(width))
            .h_full()
            .bg(drawer_bg)
            .border_r_1()
            .border_color(border_color)
            .flex()
            .flex_col()
            // Header
            .child(
                div()
                    .h(px(40.0))
                    .w_full()
                    .bg(header_bg)
                    .border_b_1()
                    .border_color(border_color)
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(muted)
                            .child(panel_title),
                    )
                    .child(
                        div()
                            .id("drawer-close")
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.icon_rail.close_drawer();
                                    this.process_icon_rail_events();
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .font_family(icons::LUCIDE_FONT_FAMILY)
                                    .child(icons::x()),
                            ),
                    ),
            )
            // Content
            .child(content)
    }

    /// Render the sessions list content for the drawer panel.
    fn render_drawer_sessions_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme().clone();
        let muted: gpui::Hsla = theme.muted.into();
        let _fg: gpui::Hsla = theme.foreground.into();
        let border_color: gpui::Hsla = theme.border.into();
        let header_bg: gpui::Hsla = theme.header_background.into();

        let sessions: Vec<Session> = self.workspace().sessions().to_vec();
        let focused_id = self.workspace().focused_session_id();
        let session_count = sessions.len();

        // Separate ungrouped and grouped sessions
        let mut ungrouped: Vec<&Session> = Vec::new();
        let mut groups: std::collections::BTreeMap<String, Vec<&Session>> =
            std::collections::BTreeMap::new();
        for session in &sessions {
            match &session.group {
                Some(group) if !group.is_empty() => {
                    groups.entry(group.clone()).or_default().push(session);
                }
                _ => ungrouped.push(session),
            }
        }

        let mut content = div().flex_1().overflow_hidden().flex().flex_col();

        // Render ungrouped sessions first
        for session in &ungrouped {
            content = content.child(self.render_session_row(session, focused_id, &theme, cx));
        }

        // Render grouped sessions with headers
        let expanded_map = self.drawer_group_expanded.clone();
        for (group_name, group_sessions) in &groups {
            let color = group_sessions.first().and_then(|s| s.color.clone());
            let expanded = expanded_map.get(group_name).copied().unwrap_or(true);

            content = content.child(self.render_session_group_header(
                group_name,
                color.as_deref(),
                group_sessions.len(),
                expanded,
                &theme,
                cx,
            ));

            if expanded {
                for session in group_sessions {
                    content =
                        content.child(self.render_session_row(session, focused_id, &theme, cx));
                }
            }
        }

        div()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_hidden()
            // Scrollable session list
            .child(content)
            // Footer
            .child(
                div()
                    .p_3()
                    .border_t_1()
                    .border_color(border_color)
                    .bg(header_bg)
                    .child(div().text_xs().text_color(muted).child(format!(
                        "{} session{}",
                        session_count,
                        if session_count == 1 { "" } else { "s" }
                    ))),
            )
    }

    /// Render the worktrees/git panel content for the drawer.
    fn render_drawer_worktrees_content(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme().clone();
        let muted: gpui::Hsla = theme.muted.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let border_color: gpui::Hsla = theme.border.into();
        let header_bg: gpui::Hsla = theme.header_background.into();
        let green = gpui::Hsla {
            h: 0.35,
            s: 0.6,
            l: 0.5,
            a: 1.0,
        };
        let orange = gpui::Hsla {
            h: 0.1,
            s: 0.8,
            l: 0.6,
            a: 1.0,
        };
        let red = gpui::Hsla {
            h: 0.0,
            s: 0.7,
            l: 0.55,
            a: 1.0,
        };
        let blue = gpui::Hsla {
            h: 0.58,
            s: 0.5,
            l: 0.6,
            a: 1.0,
        };

        // Show focused session git info, or first session if none focused
        let focused_id = self.workspace().focused_session_id();
        let sessions: Vec<Session> = self.workspace().sessions().to_vec();
        let session = focused_id
            .and_then(|id| sessions.iter().find(|s| s.id == id))
            .or_else(|| sessions.first());

        let (dir_name, _has_git_info) = match session {
            Some(s) => {
                let dir_name = s
                    .working_directory
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let has_git = s.git_info.is_some();
                (dir_name, has_git)
            }
            None => ("No session".to_string(), false),
        };

        let mut content = div()
            .flex_1()
            .overflow_hidden()
            .flex()
            .flex_col()
            .p_2()
            .gap_2();

        if session.is_none() {
            return div()
                .flex_1()
                .flex()
                .flex_col()
                .overflow_hidden()
                // Sub-header showing current location
                .child(
                    div()
                        .h(px(32.0))
                        .w_full()
                        .bg(header_bg)
                        .border_b_1()
                        .border_color(border_color)
                        .flex()
                        .items_center()
                        .px_3()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(muted)
                                .child("No session selected"),
                        ),
                )
                .child(
                    div().flex_1().p_2().child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("Select a session to view git worktrees"),
                    ),
                );
        }

        let session = session.unwrap();

        let gi = match session.git_info.as_ref() {
            Some(gi) => gi,
            None => {
                return div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    // Sub-header showing current location
                    .child(
                        div()
                            .h(px(32.0))
                            .w_full()
                            .bg(header_bg)
                            .border_b_1()
                            .border_color(border_color)
                            .flex()
                            .items_center()
                            .px_3()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(fg)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(dir_name),
                            ),
                    )
                    .child(
                        div().flex_1().p_2().child(
                            div()
                                .text_xs()
                                .text_color(muted.opacity(0.5))
                                .child("Not a git repository"),
                        ),
                    );
            }
        };

        // Branch + HEAD
        let branch_color = gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.75,
            a: 1.0,
        };
        content = content.child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(branch_color)
                        .font_family(icons::LUCIDE_FONT_FAMILY)
                        .child(icons::git_branch()),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(fg)
                        .child(gi.branch.clone()),
                )
                .when_some(gi.head_sha.as_ref(), |el, sha| {
                    el.child(
                        div()
                            .text_xs()
                            .text_color(muted.opacity(0.5))
                            .child(sha.clone()),
                    )
                }),
        );

        // Staged changes section
        if !gi.staged_files.is_empty() {
            content = content.child(
                div()
                    .mt_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(green)
                    .child(format!("Staged ({})", gi.staged_files.len())),
            );
            for file in &gi.staged_files {
                let (label, color) = Self::change_kind_display(&file.change, green, red, blue);
                content = content.child(
                    div()
                        .pl_2()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(color)
                                .child(label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(fg.opacity(0.8))
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(file.path.clone()),
                        ),
                );
            }
        }

        // Unstaged changes section
        if !gi.unstaged_files.is_empty() {
            content = content.child(
                div()
                    .mt_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(orange)
                    .child(format!("Changes ({})", gi.unstaged_files.len())),
            );
            for file in &gi.unstaged_files {
                let (label, color) = Self::change_kind_display(&file.change, green, red, blue);
                content = content.child(
                    div()
                        .pl_2()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(color)
                                .child(label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(fg.opacity(0.8))
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(file.path.clone()),
                        ),
                );
            }
        }

        // Clean state
        if gi.staged_files.is_empty() && gi.unstaged_files.is_empty() {
            content = content.child(
                div()
                    .mt_1()
                    .text_xs()
                    .text_color(green)
                    .child("Working tree clean"),
            );
        }

        // Worktrees section
        let worktrees = self.worktree_panel.worktrees();
        if !worktrees.is_empty() {
            // Section divider + header
            content = content.child(
                div()
                    .mt_3()
                    .pt_2()
                    .border_t_1()
                    .border_color(border_color.opacity(0.3))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(muted)
                            .child("Worktrees"),
                    ),
            );

            // Collect session lookup data
            let all_sessions: Vec<Session> = self.workspace().sessions().to_vec();

            for wt in worktrees {
                // Icon: star for main, diamond for linked
                let icon = if wt.is_main { "\u{2605}" } else { "\u{25C6}" };
                let icon_color = if wt.is_main { green } else { blue };

                let sha_text = wt
                    .head_sha
                    .as_ref()
                    .map(|s| {
                        if s.len() > 3 {
                            s[..3].to_string()
                        } else {
                            s.clone()
                        }
                    })
                    .unwrap_or_default();

                // Session binding label
                let binding_label = match wt.bound_session {
                    Some(sid) => {
                        let session_name = all_sessions
                            .iter()
                            .find(|s| s.id == sid)
                            .map(|s| s.name.clone())
                            .unwrap_or_else(|| format!("Session {}", sid.0));
                        format!("\u{2192} {}", session_name)
                    }
                    None => "(unbound)".to_string(),
                };

                content = content
                    .child(
                        div()
                            .pl_1()
                            .flex()
                            .items_center()
                            .gap_1()
                            // Icon
                            .child(div().text_xs().text_color(icon_color).child(icon))
                            // Branch name
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(fg)
                                    .child(wt.branch.clone()),
                            )
                            // Short SHA
                            .when(!sha_text.is_empty(), |el| {
                                el.child(
                                    div()
                                        .text_xs()
                                        .text_color(muted.opacity(0.5))
                                        .child(sha_text),
                                )
                            }),
                    )
                    .child(
                        div()
                            .pl_4()
                            .text_xs()
                            .text_color(muted.opacity(0.6))
                            .child(binding_label),
                    );
            }
        }

        // Branches section — show branches not already in a worktree
        let available_branches = self.worktree_panel.available_branches();
        let worktree_branches: Vec<&str> = self
            .worktree_panel
            .worktrees()
            .iter()
            .map(|wt| wt.branch.as_str())
            .collect();
        let extra_branches: Vec<&String> = available_branches
            .iter()
            .filter(|b| !worktree_branches.contains(&b.as_str()))
            .collect();

        if !extra_branches.is_empty() {
            content = content.child(
                div()
                    .mt_3()
                    .pt_2()
                    .border_t_1()
                    .border_color(border_color.opacity(0.3))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(muted)
                            .child("Branches"),
                    ),
            );

            for branch in &extra_branches {
                content = content.child(
                    div()
                        .pl_3()
                        .text_xs()
                        .text_color(muted.opacity(0.6))
                        .child((*branch).clone()),
                );
            }
        }

        // Footer — summary counts
        let wt_count = self.worktree_panel.worktrees().len();
        let branch_count = available_branches.len();
        if wt_count > 0 || branch_count > 0 {
            content = content.child(
                div()
                    .mt_3()
                    .pt_2()
                    .border_t_1()
                    .border_color(border_color.opacity(0.3))
                    .text_xs()
                    .text_color(muted.opacity(0.4))
                    .child(format!(
                        "{} worktree{} \u{00B7} {} branch{}",
                        wt_count,
                        if wt_count == 1 { "" } else { "s" },
                        branch_count,
                        if branch_count == 1 { "" } else { "es" },
                    )),
            );
        }

        // Return with sub-header matching file tree design
        div()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_hidden()
            // Sub-header showing current location
            .child(
                div()
                    .h(px(32.0))
                    .w_full()
                    .bg(header_bg)
                    .border_b_1()
                    .border_color(border_color)
                    .flex()
                    .items_center()
                    .px_3()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(fg)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(dir_name),
                    ),
            )
            // Scrollable content area
            .child(content)
    }

    /// Map a git change kind to a display label and color.
    fn change_kind_display(
        kind: &codirigent_core::GitChangeKind,
        green: gpui::Hsla,
        red: gpui::Hsla,
        blue: gpui::Hsla,
    ) -> (&'static str, gpui::Hsla) {
        match kind {
            codirigent_core::GitChangeKind::Modified => ("M", blue),
            codirigent_core::GitChangeKind::Added => ("A", green),
            codirigent_core::GitChangeKind::Deleted => ("D", red),
            codirigent_core::GitChangeKind::Renamed => ("R", blue),
        }
    }

    /// Render the file tree content for the drawer panel.
    fn render_drawer_files_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme().clone();
        let muted: gpui::Hsla = theme.muted.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let border_color: gpui::Hsla = theme.border.into();
        let header_bg: gpui::Hsla = theme.header_background.into();
        let active_bg: gpui::Hsla = theme.active.into();

        // Project root name for sub-header
        let root_name = self
            .project_root
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Project")
            .to_string();

        let show_hidden = self.file_tree.show_hidden();
        let item_count = self.file_tree.visible_count();

        // Collect items into owned vec for the closure
        let items: Vec<crate::sidebar::FileTreeRenderItem> =
            self.file_tree.visible_items().to_vec();

        // Build scrollable tree rows
        let mut tree_content = div()
            .id("file-tree-scroll")
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col();

        for (idx, item) in items.iter().enumerate() {
            tree_content = tree_content.child(self.render_file_tree_row(idx, item, &theme, cx));
        }

        // Eye icon: show/hide hidden files
        let eye_icon = if show_hidden {
            icons::eye()
        } else {
            icons::eye_off()
        };

        div()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_hidden()
            // Sub-header toolbar
            .child(
                div()
                    .h(px(32.0))
                    .w_full()
                    .bg(header_bg)
                    .border_b_1()
                    .border_color(border_color)
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(fg)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(root_name),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            // Eye toggle
                            .child(
                                div()
                                    .id("file-tree-toggle-hidden")
                                    .cursor_pointer()
                                    .px(px(4.0))
                                    .py(px(2.0))
                                    .rounded_sm()
                                    .hover(|style| style.bg(active_bg))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            let new_val = !this.file_tree.show_hidden();
                                            this.file_tree.set_show_hidden(new_val);
                                            if let Some(tree) = this.file_tree_model.as_mut() {
                                                tree.set_show_hidden(new_val);
                                                if let Err(e) = tree.refresh() {
                                                    tracing::warn!(
                                                        "Failed to refresh file tree: {}",
                                                        e
                                                    );
                                                }
                                            }
                                            this.refresh_file_tree_panel();
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted)
                                            .font_family(icons::LUCIDE_FONT_FAMILY)
                                            .child(eye_icon),
                                    ),
                            )
                            // Refresh button
                            .child(
                                div()
                                    .id("file-tree-refresh")
                                    .cursor_pointer()
                                    .px(px(4.0))
                                    .py(px(2.0))
                                    .rounded_sm()
                                    .hover(|style| style.bg(active_bg))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            if let Some(tree) = this.file_tree_model.as_mut() {
                                                if let Err(e) = tree.refresh() {
                                                    tracing::warn!(
                                                        "Failed to refresh file tree: {}",
                                                        e
                                                    );
                                                }
                                            }
                                            this.refresh_file_tree_panel();
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted)
                                            .font_family(icons::LUCIDE_FONT_FAMILY)
                                            .child(icons::refresh()),
                                    ),
                            ),
                    ),
            )
            // Scrollable tree list
            .child(tree_content)
            // Footer
            .child(
                div()
                    .p_3()
                    .border_t_1()
                    .border_color(border_color)
                    .bg(header_bg)
                    .child(div().text_xs().text_color(muted).child(format!(
                        "{} item{}",
                        item_count,
                        if item_count == 1 { "" } else { "s" }
                    ))),
            )
    }

    /// Render a single file tree row.
    fn render_file_tree_row(
        &mut self,
        idx: usize,
        item: &crate::sidebar::FileTreeRenderItem,
        theme: &CodirigentTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted: gpui::Hsla = theme.muted.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let active_bg: gpui::Hsla = theme.active.into();

        let depth = item.depth as f32;
        let is_dir = item.is_dir;
        let expanded = item.expanded;
        let is_selected = item.is_selected;
        let path = item.path.clone();
        let icon_color: gpui::Hsla = item.icon.color().into();
        let icon_str = item.icon.lucide_icon();
        let name = item.name.clone();

        let name_color = if is_selected { fg } else { muted };
        let row_bg = if is_selected {
            active_bg
        } else {
            gpui::Hsla {
                h: 0.0,
                s: 0.0,
                l: 0.0,
                a: 0.0,
            }
        };

        // Chevron for directories, spacer for files
        let chevron = if is_dir {
            let chevron_str = if expanded {
                icons::chevron_down()
            } else {
                icons::chevron_right()
            };
            div()
                .w(px(14.0))
                .h(px(14.0))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .child(
                    div()
                        .text_color(muted)
                        .font_family(icons::LUCIDE_FONT_FAMILY)
                        .text_size(px(10.0))
                        .child(chevron_str),
                )
        } else {
            div().w(px(14.0)).h(px(14.0)).flex_shrink_0()
        };

        let path_for_click = path.clone();
        let path_for_dbl = path.clone();
        let path_for_ctx = path.clone();

        div()
            .id(SharedString::from(format!("file-tree-row-{}", idx)))
            .h(px(crate::sidebar::FileTreePanel::ITEM_HEIGHT))
            .w_full()
            .pl(px(depth * crate::sidebar::FileTreePanel::INDENT_SIZE + 4.0))
            .pr(px(8.0))
            .flex()
            .items_center()
            .gap(px(4.0))
            .bg(row_bg)
            .cursor_pointer()
            .hover(|style| style.bg(active_bg))
            .on_click(cx.listener(move |this, event: &ClickEvent, _window, cx| {
                if event.click_count() >= 2 && !is_dir {
                    // Double-click on file -> activate (insert path)
                    let ev = crate::sidebar::FileTreeEvent::FileActivated(path_for_dbl.clone());
                    this.handle_file_tree_event(ev, cx);
                } else if is_dir {
                    // Click on directory -> toggle
                    let ev =
                        crate::sidebar::FileTreeEvent::DirectoryToggled(path_for_click.clone());
                    this.handle_file_tree_event(ev, cx);
                } else {
                    // Single click on file -> select
                    let ev = crate::sidebar::FileTreeEvent::FileSelected(path_for_click.clone());
                    this.handle_file_tree_event(ev, cx);
                }
                cx.notify();
            }))
            // Right-click -> context menu
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _window, cx| {
                    this.open_file_tree_context_menu(path_for_ctx.clone(), event.position, cx);
                }),
            )
            // Chevron
            .child(chevron)
            // Icon
            .child(self.centered_lucide_icon(icon_str, icon_color, 12.0))
            // Name
            .child(
                div()
                    .text_xs()
                    .text_color(name_color)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(name),
            )
    }

    /// Render the file tree context menu (right-click menu).
    pub(super) fn render_file_tree_context_menu(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let menu = self.file_tree_context_menu.clone()?;

        let theme = self.workspace().theme().clone();
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let hover_bg: gpui::Hsla = theme.active.into();

        let path_for_insert = menu.path.clone();
        let path_for_copy = menu.path.clone();
        let path_for_task = menu.path.clone();

        // Click-away backdrop (transparent)
        let backdrop = div()
            .id("file-ctx-menu-backdrop")
            .absolute()
            .inset_0()
            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                this.close_file_tree_context_menu(cx);
            }));

        // Menu items
        let insert_item = div()
            .id("ctx-insert-path")
            .h(px(28.0))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .hover(move |style| style.bg(hover_bg))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let path = path_for_insert.clone();
                    this.insert_path_to_terminal(&path);
                    this.close_file_tree_context_menu(cx);
                    cx.stop_propagation();
                }),
            )
            .child(div().text_xs().text_color(fg).child("Insert path"));

        let copy_item = div()
            .id("ctx-copy-path")
            .h(px(28.0))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .hover(move |style| style.bg(hover_bg))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let path = path_for_copy.clone();
                    this.copy_path_to_clipboard(&path);
                    this.close_file_tree_context_menu(cx);
                    cx.stop_propagation();
                }),
            )
            .child(div().text_xs().text_color(fg).child("Copy path"));

        let create_task_item = div()
            .id("ctx-create-task")
            .h(px(28.0))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .hover(move |style| style.bg(hover_bg))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let path = path_for_task.clone();
                    this.open_task_creation_modal_for_file(&path);
                    this.close_file_tree_context_menu(cx);
                    cx.stop_propagation();
                }),
            )
            .child(div().text_xs().text_color(fg).child("Create task"));

        let dropdown = div()
            .w(px(140.0))
            .bg(panel_bg)
            .border_1()
            .border_color(border_color)
            .rounded_md()
            .overflow_hidden()
            .shadow_lg()
            .flex()
            .flex_col()
            .py_1()
            // Prevent clicks on the menu from propagating to elements behind it
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_this, _: &MouseDownEvent, _window, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(insert_item)
            .child(copy_item)
            .child(create_task_item);

        // Position the menu at the click location
        let menu_container = div()
            .absolute()
            .top(menu.position.y)
            .left(menu.position.x)
            .child(dropdown);

        Some(
            div()
                .id("file-tree-context-menu-overlay")
                .absolute()
                .inset_0()
                .child(backdrop)
                .child(menu_container),
        )
    }

    /// Render a single session row in the drawer session list.
    fn render_session_row(
        &mut self,
        session: &Session,
        focused_id: Option<SessionId>,
        theme: &CodirigentTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted: gpui::Hsla = theme.muted.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let status_color: gpui::Hsla = theme.status_color(session.status).into();
        let is_focused = focused_id == Some(session.id);
        let row_bg = if is_focused {
            theme.active.into()
        } else {
            gpui::Hsla::transparent_black()
        };
        let hover_bg: gpui::Hsla = theme.active.into();

        let session_id = session.id;
        let session_name = session.name.clone();
        let context_pct = session.context_usage;

        div()
            .id(SharedString::from(format!("session-row-{}", session_id.0)))
            .h(px(36.0))
            .w_full()
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .bg(row_bg)
            .cursor_pointer()
            .hover(move |style| style.bg(hover_bg))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.select_session(session_id);
                    cx.notify();
                }),
            )
            // Status dot
            .child(
                div()
                    .w(px(8.0))
                    .h(px(8.0))
                    .rounded_full()
                    .bg(status_color)
                    .flex_shrink_0(),
            )
            // Session name (truncated)
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .text_xs()
                    .text_color(if is_focused { fg } else { muted })
                    .child(session_name),
            )
            // Git branch (compact) - between name and context%
            .when_some(session.git_info.as_ref(), |el, gi| {
                let mut branch = gi.branch.clone();
                if branch.chars().count() > 12 {
                    branch = branch.chars().take(9).collect::<String>() + "...";
                }
                let branch_color = muted.opacity(0.5);
                el.child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(div().text_xs().text_color(branch_color).child(branch))
                        .when(gi.dirty_count > 0, |el| {
                            el.child(
                                div()
                                    .text_xs()
                                    .text_color(gpui::Hsla {
                                        h: 0.1,
                                        s: 0.8,
                                        l: 0.6,
                                        a: 1.0,
                                    })
                                    .child(format!("\u{25CF}{}", gi.dirty_count)),
                            )
                        }),
                )
            })
            // Context percentage (if available) with threshold-based coloring
            .when_some(context_pct, |el, pct| {
                let context_color: gpui::Hsla =
                    crate::terminal_header::ContextLevel::from_percentage(pct)
                        .color()
                        .into();
                el.child(
                    div()
                        .text_xs()
                        .text_color(context_color)
                        .flex_shrink_0()
                        .child(format!("{}%", (pct * 100.0) as u32)),
                )
            })
            // Menu button
            .child(
                div()
                    .id(SharedString::from(format!("session-menu-{}", session_id.0)))
                    .w(px(24.0))
                    .h(px(24.0))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .cursor_pointer()
                    .hover(|style| {
                        style.bg(gpui::Hsla {
                            h: 0.0,
                            s: 0.0,
                            l: 1.0,
                            a: 0.1,
                        })
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.open_session_menu(session_id, cx);
                        }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .font_family(icons::LUCIDE_FONT_FAMILY)
                            .child(icons::more_horizontal()),
                    ),
            )
    }

    /// Render a session group header in the drawer session list.
    fn render_session_group_header(
        &mut self,
        group_name: &str,
        color: Option<&str>,
        count: usize,
        expanded: bool,
        theme: &CodirigentTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted: gpui::Hsla = theme.muted.into();

        // Parse group color or use a default
        let bar_color = color
            .and_then(|c| {
                if c.starts_with('#') && c.len() == 7 {
                    let r = u8::from_str_radix(&c[1..3], 16).ok()?;
                    let g = u8::from_str_radix(&c[3..5], 16).ok()?;
                    let b = u8::from_str_radix(&c[5..7], 16).ok()?;
                    Some(gpui::Hsla::from(gpui::Rgba {
                        r: r as f32 / 255.0,
                        g: g as f32 / 255.0,
                        b: b as f32 / 255.0,
                        a: 1.0,
                    }))
                } else {
                    None
                }
            })
            .unwrap_or(muted);

        let chevron = if expanded {
            icons::chevron_down()
        } else {
            icons::chevron_right()
        };

        let group_name_owned = group_name.to_string();
        let group_label = format!("{} ({})", group_name, count);
        let toggle_key = group_name_owned.clone();

        div()
            .id(SharedString::from(format!(
                "group-header-{}",
                group_name_owned
            )))
            .h(px(28.0))
            .w_full()
            .px_3()
            .flex()
            .items_center()
            .gap(px(6.0))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let current = this
                        .drawer_group_expanded
                        .get(&toggle_key)
                        .copied()
                        .unwrap_or(true);
                    this.drawer_group_expanded
                        .insert(toggle_key.clone(), !current);
                    cx.notify();
                }),
            )
            // Color bar
            .child(
                div()
                    .w(px(3.0))
                    .h(px(16.0))
                    .rounded_sm()
                    .bg(bar_color)
                    .flex_shrink_0(),
            )
            .child(self.aligned_icon_label_row(
                chevron,
                muted,
                12.0,
                group_label,
                muted,
                11.0,
                FontWeight::BOLD,
                14.0,
                6.0,
            ))
    }


}

/// Session menu actions.
#[derive(Debug, Clone)]
pub enum SessionMenuAction {
    Rename,
    AssignToGroup(String),
    NewGroup,
    RemoveGroup,
    EndSession,
}
