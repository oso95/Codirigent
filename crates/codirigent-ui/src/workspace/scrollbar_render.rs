use super::gpui::WorkspaceView;
use super::terminal_render::TerminalCanvasMetrics;
use super::types::TERMINAL_CONTENT_PADDING;
use crate::theme::CodirigentTheme;
use codirigent_core::SessionId;
use gpui::{
    div, px, Context, Focusable, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, SharedString, StatefulInteractiveElement, Styled,
};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

impl WorkspaceView {
    pub(super) fn render_terminal_scrollbar(
        &mut self,
        session_id: SessionId,
        theme: &CodirigentTheme,
        canvas_metrics: Rc<Cell<TerminalCanvasMetrics>>,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let terminal_view = self.terminals.get(&session_id)?;

        // Approximate track height from terminal grid dimensions at render
        // time. The Rc<Cell<>> canvas_metrics is only populated during
        // prepaint (after the element tree is built), so reading it here
        // would always yield 0. Mouse handlers read the Rc lazily (after
        // prepaint) and prefer the exact content_height when available.
        let track_height = terminal_view.rows() as f32 * terminal_view.cell_height();
        if track_height <= 0.0 {
            return None;
        }

        let scrollbar = terminal_view.scrollbar();
        let (thumb_height, thumb_top) = terminal_view.scrollbar_thumb_metrics(track_height);
        let dragging = scrollbar.dragging.is_some();
        let track_width = if scrollbar.hovered || dragging {
            12.0
        } else {
            8.0
        };
        let track_opacity = if scrollbar.opacity > 0.0 || scrollbar.hovered || dragging {
            scrollbar.opacity.max(0.25)
        } else {
            0.0
        };
        let track_bg: gpui::Hsla = theme.border.into();
        let thumb_bg: gpui::Hsla = theme.primary.into();
        let marker_bg: gpui::Hsla = theme.orange.into();
        let marker_fractions = terminal_view.search_marker_fractions();

        let metrics_for_track = Rc::clone(&canvas_metrics);
        let metrics_for_thumb = Rc::clone(&canvas_metrics);

        let mut track = div()
            .id(SharedString::from(format!(
                "terminal-scrollbar-track-{}",
                session_id.0
            )))
            .occlude()
            .absolute()
            .top(px(TERMINAL_CONTENT_PADDING))
            .bottom(px(TERMINAL_CONTENT_PADDING))
            .right(px(0.0))
            .w(px(track_width))
            .rounded_lg()
            .bg(track_bg.opacity(if scrollbar.hovered || dragging {
                0.18
            } else {
                0.1
            }))
            .opacity(track_opacity)
            .cursor_pointer()
            .on_hover(cx.listener(move |this, hovered: &bool, _window, cx| {
                if let Some(terminal_view) = this.terminals.get_mut(&session_id) {
                    terminal_view.set_scrollbar_hovered(*hovered);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    let metrics = metrics_for_track.get();
                    let effective_track = if metrics.content_height > 0.0 {
                        metrics.content_height
                    } else {
                        track_height
                    };
                    let origin_y = metrics.origin_y;
                    let pointer_y: f32 = event.position.y.into();
                    let relative_y = pointer_y - origin_y;
                    if let Some(terminal_view) = this.terminals.get_mut(&session_id) {
                        let target = terminal_view.scrollbar_offset_for_pointer(
                            relative_y,
                            effective_track,
                            None,
                        );
                        if target != terminal_view.display_offset() {
                            terminal_view.scroll_to_offset(target);
                        }
                        window.focus(&this.focus_handle(cx));
                        this.select_session_with_cx(session_id, cx);
                        cx.notify();
                    }
                    cx.stop_propagation();
                }),
            );

        for (index, fraction) in marker_fractions.into_iter().enumerate() {
            let top = (track_height - 2.0).max(0.0) * fraction;
            track = track.child(
                div()
                    .id(gpui::SharedString::from(format!(
                        "terminal-scrollbar-marker-{session_id}-{index}"
                    )))
                    .absolute()
                    .top(px(top))
                    .left(px(0.0))
                    .right(px(0.0))
                    .h(px(2.0))
                    .bg(marker_bg.opacity(0.85)),
            );
        }

        let thumb = div()
            .id(SharedString::from(format!(
                "terminal-scrollbar-thumb-{}",
                session_id.0
            )))
            .occlude()
            .absolute()
            .top(px(thumb_top))
            .left(px(0.0))
            .right(px(0.0))
            .h(px(thumb_height))
            .rounded_lg()
            .bg(thumb_bg.opacity(0.9))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    let metrics = metrics_for_thumb.get();
                    let effective_track = if metrics.content_height > 0.0 {
                        metrics.content_height
                    } else {
                        track_height
                    };
                    let origin_y = metrics.origin_y;
                    let pointer_y: f32 = event.position.y.into();
                    let relative_y = pointer_y - origin_y;

                    if let Some(terminal_view) = this.terminals.get_mut(&session_id) {
                        let (_, current_thumb_top) =
                            terminal_view.scrollbar_thumb_metrics(effective_track);
                        let thumb_offset = (relative_y - current_thumb_top).max(0.0);
                        terminal_view.start_scrollbar_drag(thumb_offset);
                        this.selection.terminal_scrollbar_drag =
                            Some(super::types::TerminalScrollbarDragState {
                                session_id,
                                track_top: origin_y,
                                track_height: effective_track,
                            });
                        this.select_session_with_cx(session_id, cx);
                        window.focus(&this.focus_handle(cx));
                        cx.notify();
                    }

                    cx.stop_propagation();
                }),
            );

        Some(track.child(thumb).into_any_element())
    }

    pub(super) fn update_terminal_scrollbar_fades(&mut self) -> bool {
        let now = Instant::now();
        let mut changed = false;
        for terminal_view in self.terminals.values_mut() {
            changed |= terminal_view.fade_scrollbar_if_idle(now);
        }
        changed
    }
}
