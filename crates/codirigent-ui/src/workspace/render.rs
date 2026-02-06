//! GPUI rendering components for WorkspaceView.
//!
//! This module contains the rendering logic for the workspace grid, top bar,
//! icon rail, drawer, broadcast bar, and right task board panel,
//! separated from the main WorkspaceView to keep file sizes manageable.

use super::gpui::WorkspaceView;
// Import from main branch (terminal rendering)
use crate::terminal_view::CursorShape;
// Imports from feature branch (UI components)
use crate::components::text_input::{text_input, TextInputStyle};
use crate::empty_session::EmptySessionRenderHints;
use crate::terminal_header::TerminalHeaderRenderHints;
use crate::theme::CodirigentTheme;
use codirigent_core::{Session, SessionId, SessionStatus};
use crate::icons;
use crate::layout::LayoutProfile;
use crate::title_bar::TitleBar;
use gpui::{
    div, px, ClickEvent, Context, FontWeight, Image, ImageFormat, InteractiveElement, IntoElement,
    ObjectFit, ParentElement, Window, WindowControlArea, prelude::FluentBuilder, SharedString,
    StatefulInteractiveElement, Styled, StyledImage, MouseButton, ScrollWheelEvent,
};
use std::sync::Arc;
use tracing::info;

impl WorkspaceView {
    /// Convert core Task to UI TaskItem with status mapping.
    fn core_task_to_ui_item(&self, task: &codirigent_core::Task) -> crate::task_board::TaskItem {
        use crate::task_board::{TaskItem, TaskPriority as UIPriority, TaskStatus as UIStatus};
        use codirigent_core::{TaskPriority as CorePriority, TaskStatus as CoreStatus};

        // Map priority
        let ui_priority = match task.priority {
            CorePriority::Critical | CorePriority::High => UIPriority::High,
            CorePriority::Medium => UIPriority::Medium,
            CorePriority::Low => UIPriority::Low,
        };

        // Map status
        let ui_status = match task.status {
            CoreStatus::Queued => UIStatus::Queued,
            CoreStatus::Assigned | CoreStatus::Working => UIStatus::InProgress,
            CoreStatus::Verifying | CoreStatus::Review => UIStatus::PendingReview,
            CoreStatus::Done => UIStatus::Completed,
            CoreStatus::Blocked => UIStatus::Queued, // Treat blocked as queued in UI
        };

        // Format estimated time
        let estimated_time = task.estimated_minutes.map(|mins| {
            if mins < 60 {
                format!("{}m", mins)
            } else {
                format!("{}h {}m", mins / 60, mins % 60)
            }
        });

        // Format created_at as relative time
        let created_at = {
            let now = chrono::Utc::now();
            let duration = now.signed_duration_since(task.created_at);
            if duration.num_minutes() < 60 {
                Some(format!("{}m ago", duration.num_minutes()))
            } else if duration.num_hours() < 24 {
                Some(format!("{}h ago", duration.num_hours()))
            } else {
                Some(format!("{}d ago", duration.num_days()))
            }
        };

        TaskItem::new(task.id.0.clone(), task.title.clone())
            .with_priority(ui_priority)
            .with_status(ui_status)
            .with_estimated_time(estimated_time.unwrap_or_else(|| "?".to_string()))
            .with_created_at(created_at.unwrap_or_else(|| "now".to_string()))
    }


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
                                .child(
                                    div()
                                        .w(px(8.0))
                                        .h(px(8.0))
                                        .rounded_full()
                                        .bg(status_color),
                                )
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
                        .child(
                            // Terminal content area
                            self.render_terminal_content(info.session_id, &theme),
                        )
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
    fn render_terminal_content(
        &mut self,
        session_id: SessionId,
        theme: &CodirigentTheme,
    ) -> gpui::AnyElement {
        let terminal_bg: gpui::Hsla = theme.terminal_background.into();
        let terminal_fg: gpui::Hsla = theme.terminal_foreground.into();

        // Get the terminal view for this session
        let Some(terminal_view) = self.terminals_mut().get_mut(&session_id) else {
            // No terminal yet, show placeholder
            return div()
                .flex_1()
                .bg(terminal_bg)
                .flex()
                .items_center()
                .justify_center()
                .child(div().text_base().text_color(terminal_fg).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::terminal()))
                .into_any_element();
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

        let font_family: gpui::SharedString = crate::terminal_view::default_terminal_font_family().into();

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
                let mut shaped_runs: Vec<(usize, usize, gpui::ShapedLine)> = Vec::with_capacity(text_runs.len());
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

                    let shaped = window
                        .text_system()
                        .shape_line(text, font_size_px, &[text_run], None);

                    shaped_runs.push((run.row, run.start_col, shaped));
                }

                (ox, oy, bg_rects, shaped_runs, cursor_data, cell_width, cell_height)
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
                let (ox, oy, bg_rects, shaped_runs, cursor_data, cell_w, cell_h) =
                    prepaint_data;

                // 1. Paint background rectangles
                for (row, start_col, end_col, bg_color) in &bg_rects {
                    let rect_x = ox + *start_col as f32 * cell_w;
                    let rect_y = oy + *row as f32 * cell_h;
                    let rect_w = (*end_col - *start_col) as f32 * cell_w;
                    let rect_bounds = gpui::Bounds {
                        origin: gpui::Point { x: px(rect_x), y: px(rect_y) },
                        size: gpui::Size { width: px(rect_w), height: px(cell_h) },
                    };
                    window.paint_quad(gpui::fill(rect_bounds, *bg_color));
                }

                // 2. Paint shaped text runs
                for (row, start_col, shaped_line) in &shaped_runs {
                    let text_x = ox + *start_col as f32 * cell_w;
                    let text_y = oy + *row as f32 * cell_h;
                    let text_origin = gpui::Point { x: px(text_x), y: px(text_y) };
                    let _ = shaped_line.paint(
                        text_origin,
                        px(cell_h),
                        window,
                        cx,
                    );
                }

                // 3. Paint cursor
                if let Some((cursor, cursor_color)) = &cursor_data {
                    let cx_pos = ox + cursor.x;
                    let cy_pos = oy + cursor.y;

                    match cursor.shape {
                        CursorShape::Block => {
                            let cursor_bounds = gpui::Bounds {
                                origin: gpui::Point { x: px(cx_pos), y: px(cy_pos) },
                                size: gpui::Size { width: px(cell_w), height: px(cell_h) },
                            };
                            window.paint_quad(gpui::fill(
                                cursor_bounds,
                                cursor_color.opacity(0.7),
                            ));
                        }
                        CursorShape::HollowBlock => {
                            let cursor_bounds = gpui::Bounds {
                                origin: gpui::Point { x: px(cx_pos), y: px(cy_pos) },
                                size: gpui::Size { width: px(cell_w), height: px(cell_h) },
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
                                origin: gpui::Point { x: px(cx_pos), y: px(cy_pos) },
                                size: gpui::Size { width: px(2.0), height: px(cell_h) },
                            };
                            window.paint_quad(gpui::fill(cursor_bounds, *cursor_color));
                        }
                        CursorShape::Underline => {
                            let cursor_bounds = gpui::Bounds {
                                origin: gpui::Point {
                                    x: px(cx_pos),
                                    y: px(cy_pos + cell_h - 2.0),
                                },
                                size: gpui::Size { width: px(cell_w), height: px(2.0) },
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
        div()
            .size_full()
            .bg(terminal_bg)
            .overflow_hidden()
            .child(terminal_canvas)
            .into_any_element()
    }

    /// Render the title bar with window controls (minimize, maximize, close).
    ///
    /// This is a 32px bar with the logo on the left and native window controls
    /// on the right. The entire bar is a drag region for moving the window.
    pub(super) fn render_title_bar(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
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
            let raw_handle = window.window_handle().ok().map(|h| {
                match h.as_raw() {
                    raw_window_handle::RawWindowHandle::Win32(win32) => {
                        win32.hwnd.get() as isize
                    }
                    _ => 0,
                }
            });
            if let Some(hwnd) = raw_handle {
                bar = bar.on_mouse_down(MouseButton::Left, move |_event, _window, _cx| {
                    crate::platform_drag::begin_title_bar_drag(hwnd);
                });
            }
        }

        // Logo (3x3 grid matching logo-primary-dark.svg)
        bar = bar.child(
            div()
                .flex_shrink_0()
                .ml_2()
                .child(self.render_logo_small()),
        );
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
            }.into();

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
                        .active(|style| style.bg(close_hover_bg.opacity(0.8)).text_color(gpui::Hsla::white().opacity(0.8)))
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
    /// token counter, right-panel toggle, and window controls.
    pub(super) fn render_top_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme();
        let bg: gpui::Hsla = theme.header_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let active: gpui::Hsla = theme.active.into();
        let primary: gpui::Hsla = theme.primary.into();

        // Clone tab data and state before building the element tree
        let tabs: Vec<(usize, String, bool)> = self
            .top_bar
            .tabs()
            .iter()
            .enumerate()
            .map(|(i, t)| (i, t.label.clone(), t.is_active))
            .collect();
        let broadcast_enabled = self.top_bar.is_broadcast_enabled();
        let right_panel_open = self.top_bar.is_right_panel_open();
        let is_custom_layout = self.workspace.layout_profile().is_custom();
        let custom_label = if let LayoutProfile::Custom { rows, cols } = self.workspace.layout_profile() {
            format!("{}x{}", rows, cols)
        } else {
            "Custom".to_string()
        };

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

        // Layout tab pills
        let mut tab_row = div().flex().gap_1().items_center();
        for (idx, label, is_active) in &tabs {
            let tab_bg = if *is_active {
                active
            } else {
                gpui::Hsla::transparent_black()
            };
            let tab_color = if *is_active { fg } else { muted };
            let tab_idx = *idx;

            tab_row = tab_row.child(
                div()
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
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        this.top_bar.click_tab(tab_idx);
                        this.process_top_bar_events();
                        cx.notify();
                    }))
                    .child(label.clone()),
            );
        }

        // Custom layout button
        let custom_bg = if is_custom_layout {
            active
        } else {
            gpui::Hsla::transparent_black()
        };
        let custom_color = if is_custom_layout { fg } else { muted };
        tab_row = tab_row.child(
            div()
                .id("top-bar-tab-custom")
                .px_3()
                .py_1()
                .rounded_md()
                .bg(custom_bg)
                .text_xs()
                .font_weight(if is_custom_layout {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(custom_color)
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
                .child(custom_label),
        );

        bar = bar.child(tab_row);

        // Vertical divider
        bar = bar.child(
            div()
                .w(px(1.0))
                .h(px(20.0))
                .bg(border_color)
                .mx_1(),
        );

        // Broadcast toggle
        let broadcast_color = if broadcast_enabled { primary } else { muted };
        bar = bar.child(
            div()
                .id("top-bar-broadcast")
                .px_2()
                .py_1()
                .rounded_md()
                .flex()
                .items_center()
                .gap_1()
                .cursor_pointer()
                .hover(|style| style.bg(active.opacity(0.3)))
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.top_bar.toggle_broadcast();
                    this.process_top_bar_events();
                    cx.notify();
                }))
                .child(div().text_xs().text_color(broadcast_color).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::zap()))
                .child(div().text_xs().text_color(broadcast_color).child("Broadcast")),
        );

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
            .id(SharedString::from(format!("terminal-header-{}", session_id.0)))
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
        header = header.child(
            div()
                .w(px(8.0))
                .h(px(8.0))
                .rounded_full()
                .bg(status_color),
        );

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
            .id(SharedString::from(format!("empty-cell-{}-{}", position.row, position.col)))
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
            .child(
                div()
                    .text_xl()
                    .text_color(icon_color)
                    .child(hints.icon),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(text_color)
                    .child(hints.message),
            )
    }

    /// This is the updated grid renderer that uses TerminalHeader for sessions
    /// and EmptySessionCell for empty slots, with actual terminal content.
    pub(super) fn render_grid_with_headers(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
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

        // Get grid layout to calculate cell bounds (used for terminal sizing hints)
        let layout = self.grid_layout_with_task_board();

        let mut grid = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(grid_gap));

        for row in 0..rows {
            let mut row_div = div()
                .flex_1()  // Equal row heights via flex distribution
                .flex()
                .flex_row()
                .gap(px(grid_gap));

            for col in 0..cols {
                let index = (row * cols + col) as usize;
                let position = codirigent_core::GridPosition { row, col };

                // Calculate cell bounds from layout (used for terminal sizing hints)
                let cell_bounds = layout.cell_bounds(row, col);

                let cell_div = if let Some(info) = cells.get(index) {
                    // Session cell with terminal header
                    let cell_border = if info.is_focused {
                        primary_color
                    } else {
                        border_color
                    };

                    // Get or create terminal header hints
                    let header_hints = if let Some(header) = self.get_terminal_header(info.session_id) {
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
                        cell_bounds,
                        cx,
                    )
                } else {
                    // Empty cell - render inline
                    self.render_empty_cell_inline_with_colors(position, panel_bg, border_color, muted, cell_bounds, cx)
                };

                // Let flex distribute equal widths; use size_full so
                // the child fills the flex-allocated area
                row_div = row_div.child(
                    div()
                        .flex_1()
                        .size_full()
                        .child(cell_div)
                );
            }

            grid = grid.child(row_div);
        }

        grid
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
        cell_bounds: crate::layout::Bounds,
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
            .id(SharedString::from(format!("terminal-header-{}", session_id.0)))
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
            .child(
                div()
                    .w(px(8.0))
                    .h(px(8.0))
                    .rounded_full()
                    .bg(status_color),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(fg)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(hints.name.clone()),
            )
            .child(div().flex_1());

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

        let terminal_height = cell_bounds.size.height - HEADER_HEIGHT;

        // Render terminal content before building the div tree so the
        // mutable borrow on `self` is released before `cx.listener()`.
        let terminal_content = self.render_terminal_content(session_id, theme);

        div()
            .id(SharedString::from(format!("session-cell-{}", session_id.0)))
            .w(px(cell_bounds.size.width))
            .h(px(cell_bounds.size.height))
            .bg(panel_bg)
            .border_1()
            .border_color(cell_border)
            .rounded_lg()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(header)
            .child(
                div()
                    .w(px(cell_bounds.size.width))
                    .h(px(terminal_height))
                    .overflow_hidden()
                    .on_scroll_wheel(cx.listener(move |this, event: &ScrollWheelEvent, _window, cx| {
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
                    }))
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
            .id(SharedString::from(format!("terminal-header-{}", session_id.0)))
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
        header = header.child(
            div()
                .w(px(8.0))
                .h(px(8.0))
                .rounded_full()
                .bg(status_color),
        );

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
        cell_bounds: crate::layout::Bounds,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(SharedString::from(format!("empty-cell-{}-{}", position.row, position.col)))
            .w(px(cell_bounds.size.width))
            .h(px(cell_bounds.size.height))
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
        let theme = self.workspace().theme();
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let muted: gpui::Hsla = theme.muted.into();

        // Calculate cell bounds from grid layout (accounting for task board height)
        let layout = self.grid_layout_with_task_board();
        let cell_bounds = layout.cell_bounds(position.row, position.col);

        self.render_empty_cell_inline_with_colors(position, panel_bg, border_color, muted, cell_bounds, cx)
    }

    /// Render the custom layout picker modal.
    ///
    /// Displays a modal overlay with input fields for rows and columns when the
    /// custom layout picker is open.
    pub(super) fn render_custom_layout_modal(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
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
        let error_color: gpui::Hsla = gpui::Hsla::red(); // Red for errors
        let input_bg: gpui::Hsla = theme.terminal_background.into();

        let rows_value = picker.rows_input.clone();
        let cols_value = picker.cols_input.clone();
        let has_error = picker.error.is_some();
        let focused_input = picker.focused_input();
        let focus_border: gpui::Hsla = primary;
        let input_style = TextInputStyle {
            height: 36.0,
            padding_x: 12.0,
            bg: input_bg,
            border: border_color,
            focus_border,
            error_border: error_color,
            text: fg,
        };

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
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(div().text_base().text_color(fg).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::layout_grid()))
                                        .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Custom Grid Layout")),
                                ),
                        )
                        // Content
                        .child(
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
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(muted)
                                                .child("Rows (1-10):"),
                                        )
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
                                              .on_mouse_down(MouseButton::Left, cx.listener(|this, _event, _window, cx| {
                                                  this.custom_picker.set_focus(0);
                                                  cx.notify();
                                              }))
                                          }),
                                )
                                // Columns input
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(muted)
                                                .child("Columns (1-10):"),
                                        )
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
                                              .on_mouse_down(MouseButton::Left, cx.listener(|this, _event, _window, cx| {
                                                  this.custom_picker.set_focus(1);
                                                  cx.notify();
                                              }))
                                          }),
                                )
                                // Error message
                                .when_some(picker.error.clone(), |this, error| {
                                    this.child(
                                        div()
                                            .text_sm()
                                            .text_color(error_color)
                                            .child(error),
                                    )
                                })
                                // Preview grid
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(muted)
                                                .child("Preview:"),
                                        )
                                        .child(self.render_grid_preview(&rows_value, &cols_value, theme)),
                                ),
                        )
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
                                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                            this.custom_picker.close();
                                            cx.notify();
                                        }))
                                        .child(div().flex().items_center().gap_1()
                                            .child(div().text_xs().font_family(icons::LUCIDE_FONT_FAMILY).child(icons::x()))
                                            .child("Cancel")),
                                )
                                // Apply button
                                .child(
                                    div()
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
                                            if let Some((rows, cols)) = this.custom_picker.validate() {
                                                this.custom_picker.close();
                                                let profile = crate::layout::LayoutProfile::Custom { rows, cols };
                                                this.workspace.set_layout(profile);
                                                cx.notify();
                                            } else {
                                                cx.notify();
                                            }
                                        }))
                                        .child(div().flex().items_center().gap_1()
                                            .child(div().text_xs().font_family(icons::LUCIDE_FONT_FAMILY).child(icons::check()))
                                            .child("Apply")),
                                ),
                        ),
                ),
        )
    }

    /// Render a preview of the grid layout.
    fn render_grid_preview(&self, rows_str: &str, cols_str: &str, theme: &crate::theme::CodirigentTheme) -> impl IntoElement {
        let border_color: gpui::Hsla = theme.border.into();
        let preview_bg: gpui::Hsla = theme.terminal_background.into();

        // Parse dimensions or use defaults
        let rows: u32 = rows_str.parse().unwrap_or(2).clamp(1, 10);
        let cols: u32 = cols_str.parse().unwrap_or(2).clamp(1, 10);

        let cell_size = 30.0;
        let gap = 4.0;

        let mut grid = div()
            .flex()
            .flex_col()
            .gap(px(gap));

        for _ in 0..rows {
            let mut row = div()
                .flex()
                .flex_row()
                .gap(px(gap));

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

    /// Render the worktree create modal.
    ///
    /// Displays a modal overlay with inputs for creating a new git worktree.
    pub(super) fn render_worktree_modal(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.worktree_panel.is_create_modal_open() {
            return None;
        }

        let theme = self.workspace().theme();
        let bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let primary: gpui::Hsla = theme.primary.into();
        let input_bg: gpui::Hsla = theme.terminal_background.into();
        let focus_border: gpui::Hsla = primary;
        let input_style = TextInputStyle {
            height: 36.0,
            padding_x: 12.0,
            bg: input_bg,
            border: border_color,
            focus_border,
            error_border: border_color,
            text: fg,
        };
        let placeholder_style = TextInputStyle { text: muted, ..input_style };

        let branch_value = self.worktree_panel.branch_input().to_string();
        let base_branch_value = self.worktree_panel.base_branch_input().to_string();
        let use_existing = self.worktree_panel.use_existing_branch();
        let available_branches = self.worktree_panel.available_branches().to_vec();
        let focused_input = self.worktree_panel.focused_input();
        let dropdown_open = self.worktree_panel.is_branch_dropdown_open();

        Some(
            div()
                .id("worktree-modal-overlay")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::Hsla::black().opacity(0.5))
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    // Close modal on overlay click
                    this.handle_worktree_event(crate::sidebar::WorktreeEvent::CancelCreate, cx);
                }))
                .child(
                    div()
                        .id("worktree-modal")
                        .w(px(450.0))
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
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(div().text_base().text_color(fg).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::git_branch()))
                                        .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Create Git Worktree")),
                                ),
                        )
                        // Content
                        .child(
                            div()
                                .p_4()
                                .flex()
                                .flex_col()
                                .gap_4()
                                // Branch type toggle
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(muted)
                                                .child("Branch Type:"),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .gap_2()
                                                // New branch button
                                                .child(
                                                    div()
                                                        .id("worktree-new-branch-btn")
                                                        .px_3()
                                                        .py_2()
                                                        .border_1()
                                                        .border_color(if !use_existing { primary } else { border_color })
                                                        .bg(if !use_existing { primary.opacity(0.1) } else { bg.opacity(0.0) })
                                                        .rounded_md()
                                                        .text_sm()
                                                        .text_color(if !use_existing { primary } else { fg })
                                                        .cursor_pointer()
                                                        .hover(|style| style.bg(primary.opacity(0.05)))
                                                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                                            if this.worktree_panel.use_existing_branch() {
                                                                this.worktree_panel.toggle_use_existing_branch();
                                                                cx.notify();
                                                            }
                                                        }))
                                                        .child(div().flex().items_center().gap_1()
                                                            .child(div().text_xs().font_family(icons::LUCIDE_FONT_FAMILY).child(icons::git_branch()))
                                                            .child("New Branch")),
                                                )
                                                // Existing branch button
                                                .child(
                                                    div()
                                                        .id("worktree-existing-branch-btn")
                                                        .px_3()
                                                        .py_2()
                                                        .border_1()
                                                        .border_color(if use_existing { primary } else { border_color })
                                                        .bg(if use_existing { primary.opacity(0.1) } else { bg.opacity(0.0) })
                                                        .rounded_md()
                                                        .text_sm()
                                                        .text_color(if use_existing { primary } else { fg })
                                                        .cursor_pointer()
                                                        .hover(|style| style.bg(primary.opacity(0.05)))
                                                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                                            if !this.worktree_panel.use_existing_branch() {
                                                                this.worktree_panel.toggle_use_existing_branch();
                                                                cx.notify();
                                                            }
                                                        }))
                                                        .child(div().flex().items_center().gap_1()
                                                            .child(div().text_xs().font_family(icons::LUCIDE_FONT_FAMILY).child(icons::git_fork()))
                                                            .child("Existing Branch")),
                                                ),
                                        ),
                                )
                                // Branch name input (shown for new branch) or selection (shown for existing)
                                .child({
                                    let mut branch_block = div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(muted)
                                                .child(if use_existing { "Select Branch:" } else { "Branch Name:" }),
                                        );

                                    if !use_existing {
                                        let is_focused = focused_input == Some(0);
                                        let display_value = if branch_value.is_empty() {
                                            "e.g., feature/my-feature".to_string()
                                        } else {
                                            branch_value.clone()
                                        };
                                        let display_with_cursor = if is_focused && !display_value.is_empty() {
                                            format!("{}|", display_value)
                                        } else if is_focused {
                                            "|".to_string()
                                        } else {
                                            display_value.clone()
                                        };
                                        let style = if branch_value.is_empty() && !is_focused {
                                            placeholder_style
                                        } else {
                                            input_style
                                        };

                                        branch_block = branch_block.child({
                                            let input = text_input(
                                                "branch-input",
                                                display_with_cursor,
                                                is_focused,
                                                false,
                                                &style,
                                            )
                                            .cursor_pointer();

                                            input.on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _event, _window, cx| {
                                                this.worktree_panel.set_focus(0);
                                                cx.notify();
                                            }))
                                        });
                                    } else {
                                        let display_text = if branch_value.is_empty() {
                                            "Select a branch...".to_string()
                                        } else {
                                            branch_value.clone()
                                        };
                                        let style = if branch_value.is_empty() {
                                            placeholder_style
                                        } else {
                                            input_style
                                        };

                                        let dropdown = {
                                            let input = text_input(
                                                "branch-dropdown",
                                                format!("{} v", display_text),
                                                false,
                                                false,
                                                &style,
                                            )
                                            .cursor_pointer();

                                            input.on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _event, _window, cx| {
                                                this.worktree_panel.toggle_branch_dropdown();
                                                cx.notify();
                                            }))
                                        };

                                        let mut dropdown_container = div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(dropdown);

                                        if dropdown_open {
                                            let mut list = div()
                                                .bg(input_bg)
                                                .border_1()
                                                .border_color(border_color)
                                                .rounded_md()
                                                .flex()
                                                .flex_col();

                                            if available_branches.is_empty() {
                                                list = list.child(
                                                    div()
                                                        .px_3()
                                                        .py_2()
                                                        .text_sm()
                                                        .text_color(muted)
                                                        .child("No branches available"),
                                                );
                                            } else {
                                                for branch in &available_branches {
                                                    let branch_name = branch.clone();
                                                    list = list.child({
                                                        let branch_div = div()
                                                            .px_3()
                                                            .py_2()
                                                            .text_sm()
                                                            .text_color(fg)
                                                            .cursor_pointer()
                                                            .hover(|style| style.bg(primary.opacity(0.1)));

                                                        branch_div.on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, _event, _window, cx| {
                                                                this.worktree_panel.select_existing_branch(branch_name.clone());
                                                                cx.notify();
                                                            }))
                                                            .child(branch.clone())
                                                    });
                                                }
                                            }

                                            dropdown_container = dropdown_container.child(list);
                                        }

                                        branch_block = branch_block.child(dropdown_container);
                                    }

                                    branch_block
                                })
                                // Base branch input (only shown for new branches)
                                .when(!use_existing, |this| {
                                    this.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(muted)
                                                    .child("Base Branch:"),
                                            )
                                            .child({
                                                let is_focused = focused_input == Some(1);
                                                let display_text = if is_focused {
                                                    format!("{}|", base_branch_value)
                                                } else {
                                                    base_branch_value.clone()
                                                };
                                                let input = text_input(
                                                    "base-branch-input",
                                                    display_text,
                                                    is_focused,
                                                    false,
                                                    &input_style,
                                                )
                                                .cursor_pointer();

                                                input.on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _event, _window, cx| {
                                                    this.worktree_panel.set_focus(1);
                                                    cx.notify();
                                                }))
                                            }),
                                    )
                                })
                                // Info message
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child(if use_existing {
                                            "Checkout an existing branch in a new worktree."
                                        } else {
                                            "Create a new branch from the base branch in a new worktree."
                                        }),
                                ),
                        )
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
                                        .id("worktree-cancel")
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
                                            this.handle_worktree_event(crate::sidebar::WorktreeEvent::CancelCreate, cx);
                                        }))
                                        .child(div().flex().items_center().gap_1()
                                            .child(div().text_xs().font_family(icons::LUCIDE_FONT_FAMILY).child(icons::x()))
                                            .child("Cancel")),
                                )
                                // Create button
                                .child(
                                    div()
                                        .id("worktree-create")
                                        .px_4()
                                        .py_2()
                                        .bg(if branch_value.is_empty() { muted } else { primary })
                                        .rounded_md()
                                        .text_sm()
                                        .text_color(gpui::Hsla::white())
                                        .cursor(if branch_value.is_empty() { gpui::CursorStyle::OperationNotAllowed } else { gpui::CursorStyle::PointingHand })
                                        .when(!branch_value.is_empty(), |style| {
                                            style
                                                .hover(|style| style.bg(primary.opacity(0.8)))
                                                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                                    let branch = this.worktree_panel.branch_input().to_string();
                                                    let base_branch = if this.worktree_panel.use_existing_branch() {
                                                        None
                                                    } else {
                                                        Some(this.worktree_panel.base_branch_input().to_string())
                                                    };
                                                    this.handle_worktree_event(
                                                        crate::sidebar::WorktreeEvent::ConfirmCreate { branch, base_branch },
                                                        cx
                                                    );
                                                }))
                                        })
                                        .child(div().flex().items_center().gap_1()
                                            .child(div().text_xs().font_family(icons::LUCIDE_FONT_FAMILY).child(icons::plus()))
                                            .child("Create")),
                                ),
                        ),
                ),
        )
    }

    /// Render a small logo for the title bar using the embedded PNG.
    fn render_logo_small(&self) -> impl IntoElement {
        // Old size was 3*4 + 2*1 = 14px
        let logo_size = 14.0;
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
            "teal" | "blue-green" => Some(gpui::Hsla { h: 0.52, s: 0.70, l: 0.60, a: 1.0 }),
            "coral" | "orange-red" => Some(gpui::Hsla { h: 0.03, s: 0.80, l: 0.62, a: 1.0 }),
            "orange" => Some(gpui::Hsla { h: 0.08, s: 0.90, l: 0.60, a: 1.0 }),
            "blue" => Some(gpui::Hsla { h: 0.60, s: 0.70, l: 0.60, a: 1.0 }),
            "purple" => Some(gpui::Hsla { h: 0.75, s: 0.60, l: 0.65, a: 1.0 }),
            "green" => Some(gpui::Hsla { h: 0.33, s: 0.60, l: 0.55, a: 1.0 }),
            "yellow" => Some(gpui::Hsla { h: 0.15, s: 0.80, l: 0.65, a: 1.0 }),
            "red" => Some(gpui::Hsla { h: 0.0, s: 0.80, l: 0.60, a: 1.0 }),
            _ => None,
        }
    }

    /// Render session context menu (dropdown near the trigger button).
    pub(super) fn render_session_menu(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let session_id = self.session_menu_open?;

        let theme = self.workspace().theme();
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let hover_bg: gpui::Hsla = theme.active.into();
        let destructive = gpui::Hsla { h: 0.0, s: 0.7, l: 0.55, a: 1.0 };

        // Transparent click-away backdrop (no dark overlay)
        let backdrop = div()
            .id("session-menu-backdrop")
            .absolute()
            .inset_0()
            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                this.close_session_menu(cx);
            }));

        // Compact dropdown menu
        let dropdown = div()
            .w(px(180.0))
            .bg(panel_bg)
            .border_1()
            .border_color(border_color)
            .rounded_md()
            .overflow_hidden()
            .shadow_lg()
            .flex()
            .flex_col()
            .py_1()
            .child(self.render_menu_item(
                "Rename",
                session_id,
                SessionMenuAction::Rename,
                theme,
                hover_bg,
                fg,
                cx,
            ))
            .child(self.render_menu_item(
                "Assign Group",
                session_id,
                SessionMenuAction::AssignGroup,
                theme,
                hover_bg,
                fg,
                cx,
            ))
            .child(self.render_menu_item(
                "Remove Group",
                session_id,
                SessionMenuAction::RemoveGroup,
                theme,
                hover_bg,
                fg,
                cx,
            ))
            .child(
                div()
                    .h(px(1.0))
                    .mx_2()
                    .my_1()
                    .bg(border_color),
            )
            .child(self.render_menu_item(
                "Close",
                session_id,
                SessionMenuAction::Close,
                theme,
                hover_bg,
                destructive,
                cx,
            ));

        // Position the dropdown in the left panel area, near the drawer
        let left_offset = crate::icon_rail::IconRail::WIDTH + self.drawer.width() - 180.0 - 8.0;

        Some(
            div()
                .id("session-menu-container")
                .absolute()
                .inset_0()
                .child(backdrop)
                .child(
                    div()
                        .absolute()
                        .left(px(left_offset.max(0.0)))
                        .top(px(100.0))
                        .child(dropdown),
                ),
        )
    }

    /// Render the session action modal for rename/group.
    pub(super) fn render_session_action_modal(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
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
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(div().text_base().text_color(fg).font_family(icons::LUCIDE_FONT_FAMILY).child(title_icon))
                                        .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(fg).child(title)),
                                ),
                        )
                        // Content
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
                                        .child(label),
                                )
                                .child(text_input(
                                    "session-action-input",
                                    input_value,
                                    true,  // Always focused in modal
                                    modal.error.is_some(),
                                    &input_style,
                                ).on_mouse_down(MouseButton::Left, cx.listener(|_this, _event, _window, cx| {
                                    // Input is always focused in this modal
                                    cx.stop_propagation();
                                })))
                                .when_some(modal.error.clone(), |this, error| {
                                    this.child(
                                        div()
                                            .text_sm()
                                            .text_color(error_color)
                                            .child(error),
                                    )
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
                                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                            this.close_session_action_modal();
                                            cx.notify();
                                        }))
                                        .child(div().flex().items_center().gap_1()
                                            .child(div().text_xs().font_family(icons::LUCIDE_FONT_FAMILY).child(icons::x()))
                                            .child("Cancel")),
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
                                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                            this.apply_session_action_modal(cx);
                                        }))
                                        .child(div().flex().items_center().gap_1()
                                            .child(div().text_xs().font_family(icons::LUCIDE_FONT_FAMILY).child(icons::check()))
                                            .child("Apply")),
                                ),
                        ),
                ),
        )
    }

    /// Render the task creation modal.
    pub(super) fn render_task_creation_modal(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let modal = self.task_creation_modal.clone()?;

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

        // Add cursor only to focused field
        let title_focused = modal.focused_field == 0;
        let desc_focused = modal.focused_field == 1;

        let title_value = if title_focused {
            if modal.title.is_empty() {
                "|".to_string()
            } else {
                format!("{}|", modal.title)
            }
        } else {
            if modal.title.is_empty() {
                "Enter task title...".to_string()
            } else {
                modal.title.clone()
            }
        };

        let description_value = if desc_focused {
            if modal.description.is_empty() {
                "|".to_string()
            } else {
                format!("{}|", modal.description)
            }
        } else {
            if modal.description.is_empty() {
                "Enter description...".to_string()
            } else {
                modal.description.clone()
            }
        };

        Some(
            div()
                .id("task-creation-overlay")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::Hsla::black().opacity(0.5))
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.close_task_creation_modal();
                    cx.notify();
                }))
                .child(
                    div()
                        .id("task-creation-modal")
                        .w(px(500.0))
                        .bg(panel_bg)
                        .border_1()
                        .border_color(border_color)
                        .rounded_lg()
                        .flex()
                        .flex_col()
                        // Prevent click from closing modal
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
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(div().text_base().text_color(fg).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::clipboard_plus()))
                                        .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(fg).child("Create New Task")),
                                ),
                        )
                        // Content
                        .child(
                            div()
                                .p_4()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(muted)
                                                .child(format!("Title:{}",
                                                    if modal.focused_field == 0 { " (active)" } else { "" }
                                                )),
                                        )
                                        .child(text_input(
                                            "task-title-input",
                                            title_value,
                                            title_focused,
                                            modal.error.is_some(),
                                            &input_style,
                                        ).on_mouse_down(MouseButton::Left, cx.listener(|this, _event, _window, cx| {
                                            if let Some(modal) = &mut this.task_creation_modal {
                                                modal.focused_field = 0;
                                            }
                                            cx.notify();
                                        }))),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(muted)
                                                .child(format!("Description:{}",
                                                    if modal.focused_field == 1 { " (active)" } else { "" }
                                                )),
                                        )
                                        .child(
                                            div()
                                                .h(px(120.0))
                                                .w_full()
                                                .p_3()
                                                .bg(input_bg)
                                                .border_1()
                                                .border_color(if desc_focused { primary } else { border_color })
                                                .rounded_md()
                                                .text_sm()
                                                .text_color(if desc_focused || !modal.description.is_empty() { fg } else { muted })
                                                .cursor_pointer()
                                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _event, _window, cx| {
                                                    if let Some(modal) = &mut this.task_creation_modal {
                                                        modal.focused_field = 1;
                                                    }
                                                    cx.notify();
                                                }))
                                                .child(description_value),
                                        ),
                                )
                                .when_some(modal.error.clone(), |this, error| {
                                    this.child(
                                        div()
                                            .text_sm()
                                            .text_color(error_color)
                                            .child(error),
                                    )
                                })
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child("Press Tab to switch fields, Enter to create, Esc to cancel"),
                                ),
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
                                        .id("task-creation-cancel")
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
                                            this.close_task_creation_modal();
                                            cx.notify();
                                        }))
                                        .child(div().flex().items_center().gap_1()
                                            .child(div().text_xs().font_family(icons::LUCIDE_FONT_FAMILY).child(icons::x()))
                                            .child("Cancel")),
                                )
                                .child(
                                    div()
                                        .id("task-creation-create")
                                        .px_4()
                                        .py_2()
                                        .bg(primary)
                                        .rounded_md()
                                        .text_sm()
                                        .text_color(gpui::Hsla::white())
                                        .cursor_pointer()
                                        .hover(|style| style.bg(primary.opacity(0.8)))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                            this.apply_task_creation_modal(cx);
                                        }))
                                        .child(div().flex().items_center().gap_1()
                                            .child(div().text_xs().font_family(icons::LUCIDE_FONT_FAMILY).child(icons::plus()))
                                            .child("Create Task")),
                                ),
                        ),
                ),
        )
    }

    /// Render a dropdown menu item with icon.
    fn render_menu_item(
        &self,
        label: &str,
        session_id: SessionId,
        action: SessionMenuAction,
        _theme: &CodirigentTheme,
        hover_bg: gpui::Hsla,
        fg: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.to_string();
        let icon = match action {
            SessionMenuAction::Rename => icons::pencil(),
            SessionMenuAction::AssignGroup => icons::users(),
            SessionMenuAction::RemoveGroup => icons::user_minus(),
            SessionMenuAction::Close => icons::x_circle(),
        };
        div()
            .id(SharedString::from(format!("menu-{:?}-{}", action, session_id.0)))
            .h(px(30.0))
            .px_3()
            .flex()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .hover(move |style| style.bg(hover_bg.opacity(0.1)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                this.handle_session_menu_action(session_id, action, cx);
            }))
            .child(
                div()
                    .text_xs()
                    .text_color(fg)
                    .font_family(icons::LUCIDE_FONT_FAMILY)
                    .child(icon),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(fg)
                    .child(label),
            )
    }

    // =========================================================================
    // New panel render methods (icon rail, drawer, broadcast bar, right task board)
    // =========================================================================

    /// Render the narrow icon rail (56px).
    pub(super) fn render_icon_rail(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme();
        let rail_bg: gpui::Hsla = theme.icon_rail_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let active_bg: gpui::Hsla = theme.active.into();
        let muted: gpui::Hsla = theme.muted.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let active_panel = self.icon_rail.active_panel();

        div()
            .id("icon-rail")
            .w(px(crate::icon_rail::IconRail::WIDTH))
            .h_full()
            .bg(rail_bg)
            .border_r_1()
            .border_color(border_color)
            .flex()
            .flex_col()
            .items_center()
            .py_4()
            .gap_2()
            // Sessions button
            .child({
                let is_active = active_panel == Some(crate::icon_rail::DrawerPanel::Sessions);
                let btn_bg = if is_active { active_bg } else { gpui::Hsla::transparent_black() };
                let btn_fg = if is_active { fg } else { muted };
                div()
                    .id("rail-sessions")
                    .w(px(40.0))
                    .h(px(40.0))
                    .rounded_xl()
                    .bg(btn_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.icon_rail.toggle_panel(crate::icon_rail::DrawerPanel::Sessions);
                        this.process_icon_rail_events();
                        cx.notify();
                    }))
                    .child(div().text_size(px(20.0)).text_color(btn_fg).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::terminal()))
            })
            // Files button
            .child({
                let is_active = active_panel == Some(crate::icon_rail::DrawerPanel::Files);
                let btn_bg = if is_active { active_bg } else { gpui::Hsla::transparent_black() };
                let btn_fg = if is_active { fg } else { muted };
                div()
                    .id("rail-files")
                    .w(px(40.0))
                    .h(px(40.0))
                    .rounded_xl()
                    .bg(btn_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.icon_rail.toggle_panel(crate::icon_rail::DrawerPanel::Files);
                        this.process_icon_rail_events();
                        cx.notify();
                    }))
                    .child(div().text_size(px(20.0)).text_color(btn_fg).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::folder_tree()))
            })
            // Worktrees button
            .child({
                let is_active = active_panel == Some(crate::icon_rail::DrawerPanel::Worktrees);
                let btn_bg = if is_active { active_bg } else { gpui::Hsla::transparent_black() };
                let btn_fg = if is_active { fg } else { muted };
                div()
                    .id("rail-worktrees")
                    .w(px(40.0))
                    .h(px(40.0))
                    .rounded_xl()
                    .bg(btn_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.icon_rail.toggle_panel(crate::icon_rail::DrawerPanel::Worktrees);
                        this.process_icon_rail_events();
                        cx.notify();
                    }))
                    .child(div().text_size(px(20.0)).text_color(btn_fg).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::git_branch()))
            })
            // Spacer
            .child(div().flex_1())
            // Settings button (bottom)
            .child(
                div()
                    .id("rail-settings")
                    .w(px(40.0))
                    .h(px(40.0))
                    .rounded_lg()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(div().text_size(px(20.0)).text_color(muted).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::settings()))
            )
    }

    /// Render the expandable drawer panel (288px).
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
            _ => {
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
                        div().text_xs().font_weight(FontWeight::BOLD)
                            .text_color(muted)
                            .child(panel_title),
                    )
                    .child(
                        div()
                            .id("drawer-close")
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.icon_rail.close_drawer();
                                this.process_icon_rail_events();
                                cx.notify();
                            }))
                            .child(div().text_xs().text_color(muted).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::x())),
                    ),
            )
            // Content
            .child(content)
    }

    /// Render the sessions list content for the drawer panel.
    fn render_drawer_sessions_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme().clone();
        let muted: gpui::Hsla = theme.muted.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let border_color: gpui::Hsla = theme.border.into();
        let header_bg: gpui::Hsla = theme.header_background.into();

        let sessions: Vec<Session> = self.workspace().sessions().to_vec();
        let focused_id = self.workspace().focused_session_id();
        let session_count = sessions.len();

        // Separate ungrouped and grouped sessions
        let mut ungrouped: Vec<&Session> = Vec::new();
        let mut groups: std::collections::BTreeMap<String, Vec<&Session>> = std::collections::BTreeMap::new();
        for session in &sessions {
            match &session.group {
                Some(group) if !group.is_empty() => {
                    groups.entry(group.clone()).or_default().push(session);
                }
                _ => ungrouped.push(session),
            }
        }

        let mut content = div()
            .flex_1()
            .overflow_hidden()
            .flex()
            .flex_col();

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
                    content = content.child(self.render_session_row(session, focused_id, &theme, cx));
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
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div().text_xs().text_color(muted)
                            .child(format!("{} session{}", session_count, if session_count == 1 { "" } else { "s" })),
                    )
                    .child(
                        div()
                            .id("drawer-new-session")
                            .px_2()
                            .py(px(4.0))
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|style| { let c: gpui::Hsla = theme.active.into(); style.bg(c) })
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.create_session(cx);
                            }))
                            .child(
                                div().flex().items_center().gap_1()
                                    .child(div().text_xs().text_color(fg).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::plus()))
                                    .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(fg).child("New Session")),
                            ),
                    ),
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
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                this.select_session(session_id);
                cx.notify();
            }))
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
            // Context percentage (if available)
            .when_some(context_pct, |el, pct| {
                el.child(
                    div()
                        .text_xs()
                        .text_color(muted.opacity(0.6))
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
                    .hover(|style| style.bg(gpui::Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.1 }))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.open_session_menu(session_id, cx);
                    }))
                    .child(
                        div().text_xs().text_color(muted).font_family(icons::LUCIDE_FONT_FAMILY)
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
            .id(SharedString::from(format!("group-header-{}", group_name_owned)))
            .h(px(28.0))
            .w_full()
            .px_3()
            .flex()
            .items_center()
            .gap(px(6.0))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                let current = this.drawer_group_expanded.get(&toggle_key).copied().unwrap_or(true);
                this.drawer_group_expanded.insert(toggle_key.clone(), !current);
                cx.notify();
            }))
            // Color bar
            .child(
                div()
                    .w(px(3.0))
                    .h(px(16.0))
                    .rounded_sm()
                    .bg(bar_color)
                    .flex_shrink_0(),
            )
            // Chevron
            .child(
                div().text_xs().text_color(muted).font_family(icons::LUCIDE_FONT_FAMILY)
                    .child(chevron),
            )
            // Group name + count
            .child(
                div().text_xs().font_weight(FontWeight::BOLD).text_color(muted)
                    .child(group_label),
            )
    }

    /// Render the broadcast input bar (52px, rose-accented).
    pub(super) fn render_broadcast_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme();
        let accent: gpui::Hsla = theme.broadcast_accent.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let input_text = self.broadcast_bar.input().to_string();

        let bar_bg = gpui::Hsla { h: accent.h, s: accent.s, l: 0.05, a: 0.2 };
        let bar_border = gpui::Hsla { a: 0.3, ..accent };

        div()
            .id("broadcast-bar")
            .w_full()
            .h(px(crate::broadcast_bar::BroadcastBar::HEIGHT))
            .bg(bar_bg)
            .border_b_1()
            .border_color(bar_border)
            .flex()
            .items_center()
            .justify_center()
            .px_4()
            .child(
                div()
                    .w_full()
                    .max_w(px(768.0))
                    .flex()
                    .items_center()
                    .gap_2()
                    // Label
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .flex_shrink_0()
                            .child(div().text_sm().text_color(accent).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::radio()))
                            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(accent).child("BROADCAST TO ALL:")),
                    )
                    // Input display
                    .child(
                        div()
                            .flex_1()
                            .h(px(36.0))
                            .bg(gpui::Hsla { h: 0.0, s: 0.0, l: 0.04, a: 1.0 })
                            .border_1()
                            .border_color(gpui::Hsla { a: 0.3, ..accent })
                            .rounded_md()
                            .px_4()
                            .flex()
                            .items_center()
                            .child(
                                if input_text.is_empty() {
                                    div().text_sm().text_color(gpui::Hsla { h: 0.0, s: 0.0, l: 0.35, a: 1.0 })
                                        .child(crate::broadcast_bar::BroadcastBar::PLACEHOLDER)
                                } else {
                                    div().text_sm().text_color(fg).child(input_text)
                                },
                            ),
                    )
                    // Send button
                    .child(
                        div()
                            .id("broadcast-send")
                            .w(px(36.0))
                            .h(px(36.0))
                            .bg(accent)
                            .rounded_md()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.broadcast_bar.submit();
                                this.process_broadcast_events();
                                cx.notify();
                            }))
                            .child(div().text_sm().text_color(gpui::Hsla::white()).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::send())),
                    ),
            )
    }

    /// Render the task board as a right sidebar panel (288px).
    pub(super) fn render_right_task_board(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.workspace().theme();
        let panel_bg: gpui::Hsla = theme.icon_rail_background.into();
        let header_bg: gpui::Hsla = theme.drawer_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let primary: gpui::Hsla = theme.primary.into();
        let active_bg: gpui::Hsla = theme.active.into();

        div()
            .id("right-task-board")
            .w(px(crate::layout::RIGHT_PANEL_WIDTH))
            .h_full()
            .bg(panel_bg)
            .border_l_1()
            .border_color(border_color)
            .flex()
            .flex_col()
            // Header
            .child(
                div()
                    .h(px(48.0))
                    .w_full()
                    .bg(header_bg)
                    .border_b_1()
                    .border_color(border_color)
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .child(
                        div().flex().items_center().gap_2()
                            .child(div().text_xs().text_color(muted).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::list_todo()))
                            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(muted).child("TASKS")),
                    )
                    .child(
                        div().flex().items_center().gap(px(6.0))
                            .px_2()
                            .py(px(2.0))
                            .rounded_md()
                            .bg(primary.opacity(0.1))
                            .border_1()
                            .border_color(primary.opacity(0.2))
                            .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(primary))
                            .child(div().text_color(primary.opacity(0.8)).text_xs().child("Auto")),
                    ),
            )
            // Scrollable content - Running + Queue sections
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_6()
                    // Running section header
                    .child(
                        div().flex().flex_col()
                            .child(
                                div().flex().justify_between().items_center().mb_2()
                                    .child(div().flex().items_center().gap_1()
                                        .child(div().text_xs().text_color(muted).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::play()))
                                        .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(muted).child("RUNNING")))
                                    .child(div().px(px(6.0)).rounded_full().bg(active_bg)
                                        .child(div().text_xs().text_color(muted).child("0"))),
                            )
                            .child(
                                div().text_xs().text_color(muted.opacity(0.5)).child("No running tasks"),
                            ),
                    )
                    // Queue section header
                    .child(
                        div().flex().flex_col()
                            .child(
                                div().flex().justify_between().items_center().mb_2()
                                    .child(div().flex().items_center().gap_1()
                                        .child(div().text_xs().text_color(muted).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::clock()))
                                        .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(muted).child("QUEUE")))
                                    .child(div().px(px(6.0)).rounded_full().bg(active_bg)
                                        .child(div().text_xs().text_color(muted).child("0"))),
                            )
                            .child(
                                div().text_xs().text_color(muted.opacity(0.5)).child("No queued tasks"),
                            ),
                    ),
            )
            // Footer: Add Task button
            .child(
                div()
                    .p_3()
                    .border_t_1()
                    .border_color(border_color)
                    .bg(header_bg)
                    .child(
                        div()
                            .id("add-task-btn")
                            .w_full()
                            .py(px(6.0))
                            .bg(active_bg)
                            .rounded_md()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .child(
                                div().flex().items_center().gap_1()
                                    .child(div().text_xs().text_color(fg).font_family(icons::LUCIDE_FONT_FAMILY).child(icons::plus()))
                                    .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(fg).child("Add Task")),
                            ),
                    ),
            )
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
