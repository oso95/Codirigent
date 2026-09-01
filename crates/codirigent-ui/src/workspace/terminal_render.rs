//! Terminal content rendering for WorkspaceView.
//!
//! Contains the canvas-based terminal rendering pipeline including:
//! - Background rectangle rendering
//! - Text painting
//! - Cursor rendering (block, hollow, beam, underline)
//! - IME pre-edit text overlay

use super::gpui::WorkspaceView;
use crate::icons;
use crate::terminal_view::CursorShape;
use crate::theme::CodirigentTheme;
use codirigent_core::SessionId;
use gpui::{div, px, Entity, FocusHandle, IntoElement, ParentElement, Styled};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Copy, Default)]
pub(super) struct TerminalCanvasMetrics {
    pub(super) origin_x: f32,
    pub(super) origin_y: f32,
    pub(super) content_height: f32,
}

impl WorkspaceView {
    pub(super) fn render_terminal_content(
        &mut self,
        session_id: SessionId,
        theme: &CodirigentTheme,
        ime_context: Option<(Entity<WorkspaceView>, FocusHandle, bool, bool)>,
        window: &mut gpui::Window,
    ) -> (gpui::AnyElement, Rc<Cell<TerminalCanvasMetrics>>) {
        // Shared cell for canvas origin (updated during prepaint)
        let canvas_metrics: Rc<Cell<TerminalCanvasMetrics>> =
            Rc::new(Cell::new(TerminalCanvasMetrics::default()));

        // IME pre-edit text should only be shown in the focused terminal pane.
        let ime_preedit_text = if matches!(ime_context.as_ref(), Some((_, _, true, true))) {
            self.ime_preedit_text.clone()
        } else {
            None
        };

        // Get the terminal view for this session.
        let Some(terminal_view) = self.terminals_mut().get_mut(&session_id) else {
            // No terminal yet, show placeholder
            let terminal_bg: gpui::Hsla = theme.terminal_background.into();
            let terminal_fg: gpui::Hsla = theme.terminal_foreground.into();
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
                canvas_metrics,
            );
        };

        // Use cached theme colors (pre-converted in TerminalView constructor/set_theme)
        let terminal_bg = terminal_view.terminal_bg_hsla();
        let terminal_fg = terminal_view.terminal_fg_hsla();
        let cell_width = terminal_view.cell_width();
        let cell_height = terminal_view.cell_height();
        let font_size = terminal_view.font_size();
        let font_family_str = terminal_view.font_family().to_owned();

        // Rebuild row caches first — this also refreshes cached_cursor_viewport_pos.
        let cached_rows = terminal_view.render_rows();
        let shaped_rows = terminal_view.shaped_rows(window.text_system());
        let selection_rects = terminal_view.selection_rects_hsla();
        let search_rects = terminal_view.search_highlight_rects_hsla();

        // Both read from cache — pure field access, no terminal state touch.
        let cursor_rect = terminal_view.cursor_rect();
        let ime_anchor = terminal_view.ime_anchor_pos();

        // Pre-convert cursor color (cursor position changes per-frame so not cacheable in content)
        let cursor_data = cursor_rect.map(|c| {
            let color: gpui::Hsla = c.color.into();
            (c, color)
        });

        let font_family: gpui::SharedString = font_family_str.into();
        let font_size_px = px(font_size);

        let ime_preedit = if let (Some(preedit), Some((anchor_x, anchor_y))) =
            (ime_preedit_text.as_ref(), ime_anchor)
        {
            if preedit.is_empty() {
                None
            } else {
                let font = gpui::Font {
                    family: font_family.clone(),
                    features: gpui::FontFeatures::default(),
                    fallbacks: None,
                    weight: gpui::FontWeight::NORMAL,
                    style: gpui::FontStyle::Normal,
                };
                let preedit_text: gpui::SharedString = preedit.clone().into();
                let preedit_run = gpui::TextRun {
                    len: preedit_text.len(),
                    font,
                    color: terminal_fg,
                    background_color: None,
                    underline: Some(gpui::UnderlineStyle {
                        thickness: px(1.0),
                        color: Some(terminal_fg),
                        wavy: false,
                    }),
                    strikethrough: None,
                };
                let shaped = window.text_system().shape_line(
                    preedit_text,
                    font_size_px,
                    &[preedit_run],
                    None,
                );
                Some((anchor_x, anchor_y, shaped))
            }
        } else {
            None
        };

        // Clone Rc for capture into the canvas prepaint closure
        let canvas_metrics_for_prepaint = Rc::clone(&canvas_metrics);

        // Capture IME context for paint closure
        let ime_context_for_paint = ime_context.clone();

        // Build canvas element that paints directly.
        let terminal_canvas = gpui::canvas(
            // Prepaint: translate bounds to canvas origin only.
            move |bounds, _window: &mut gpui::Window, _cx: &mut gpui::App| {
                // Store origin as f32 for arithmetic (Pixels doesn't support Add in gpui 0.2.1)
                let origin_x: f32 = bounds.origin.x.into();
                let origin_y: f32 = bounds.origin.y.into();
                let padding = super::types::TERMINAL_CONTENT_PADDING;
                let ox = origin_x + padding;
                let oy = origin_y + padding;
                let content_height: f32 = bounds.size.height.into();
                let content_height = (content_height - (padding * 2.0)).max(0.0);

                // Store origin and actual rendered content height for
                // mouse coordinate translation and scrollbar geometry.
                canvas_metrics_for_prepaint.set(TerminalCanvasMetrics {
                    origin_x: ox,
                    origin_y: oy,
                    content_height,
                });

                (
                    ox,
                    oy,
                    cached_rows,
                    shaped_rows,
                    selection_rects,
                    search_rects,
                    ime_preedit,
                    cursor_data,
                    cell_width,
                    cell_height,
                )
            },
            // Paint: draw backgrounds, text, and cursor
            move |bounds: gpui::Bounds<gpui::Pixels>,
                  prepaint_data,
                  window: &mut gpui::Window,
                  cx: &mut gpui::App| {
                let (
                    ox,
                    oy,
                    cached_rows,
                    shaped_rows,
                    selection_rects,
                    search_rects,
                    ime_preedit,
                    cursor_data,
                    cell_w,
                    cell_h,
                ) = prepaint_data;

                // Register input handler for IME if context is provided and it's the focused pane
                if let Some((ref entity, ref focus_handle, is_focused, input_enabled)) =
                    ime_context_for_paint
                {
                    if is_focused && input_enabled {
                        window.handle_input(
                            focus_handle,
                            gpui::ElementInputHandler::new(bounds, entity.clone()),
                            cx,
                        );
                    }
                }

                // 0. Fill canvas with terminal background so empty rows (default-color cells
                //    that produce no bg_rects_hsla entries) don't show panel_bg below them.
                //    This fixes the visual "cut-off" after dragging to a larger pane.
                window.paint_quad(gpui::fill(bounds, terminal_bg));

                // 1. Paint background rectangles
                for row in &cached_rows {
                    for (rect_row, start_col, end_col, bg_color) in row.bg_rects_hsla.iter() {
                        let rect_x = ox + *start_col as f32 * cell_w;
                        let rect_y = oy + *rect_row as f32 * cell_h;
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
                }

                // 2. Paint selection overlay without rebuilding terminal row caches.
                for (rect_row, start_col, end_col, bg_color) in &selection_rects {
                    let rect_x = ox + *start_col as f32 * cell_w;
                    let rect_y = oy + *rect_row as f32 * cell_h;
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

                // 3. Paint search highlights.
                for (rect_row, start_col, end_col, bg_color) in &search_rects {
                    let rect_x = ox + *start_col as f32 * cell_w;
                    let rect_y = oy + *rect_row as f32 * cell_h;
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

                // 4. Paint shaped text runs
                for row in &shaped_rows {
                    for (line_row, start_col, shaped_line) in row.iter() {
                        let text_x = ox + *start_col as f32 * cell_w;
                        let text_y = oy + *line_row as f32 * cell_h;
                        let text_origin = gpui::Point {
                            x: px(text_x),
                            y: px(text_y),
                        };
                        let _ = shaped_line.paint(text_origin, px(cell_h), window, cx);
                    }
                }

                // 5. Paint IME pre-edit text at the cursor position.
                if let Some((preedit_x, preedit_y, preedit_line)) = &ime_preedit {
                    let preedit_origin = gpui::Point {
                        x: px(ox + *preedit_x),
                        y: px(oy + *preedit_y),
                    };
                    let _ = preedit_line.paint(preedit_origin, px(cell_h), window, cx);
                }

                // 6. Paint cursor
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

        (element, canvas_metrics)
    }
}
