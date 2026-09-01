//! Pane header and tab-strip rendering.
//!
//! This module owns the header UI for workspace panes, including pane-local
//! tabs, session badges, and the pane-local session creation affordance.

use crate::icons;
use crate::terminal_header::TerminalHeaderRenderHints;
use crate::theme::CodirigentTheme;
use crate::workspace::gpui::WorkspaceView;
use codirigent_core::{PaneId, SessionId, SessionStatus};
use gpui::{
    div, px, ClickEvent, Context, FontWeight, InteractiveElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement, SharedString, StatefulInteractiveElement, Styled,
};

impl WorkspaceView {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_pane_header(
        &mut self,
        pane_id: PaneId,
        session_id: SessionId,
        hints: &TerminalHeaderRenderHints,
        theme: &CodirigentTheme,
        panel_bg: gpui::Hsla,
        border_color: gpui::Hsla,
        header_border: gpui::Hsla,
        fg: gpui::Hsla,
        muted: gpui::Hsla,
        orange: gpui::Hsla,
        drag_logical_index: Option<usize>,
        is_drag_source: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let color_indicator: gpui::Hsla = hints.color_indicator.into();
        let show_plus_button = self
            .workspace()
            .pane_active_session_id(pane_id.clone())
            .is_some();

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
            .child(self.render_pane_tab_strip(
                pane_id.clone(),
                session_id,
                hints,
                theme,
                border_color,
                fg,
                muted,
                drag_logical_index,
                cx,
            ));

        if let Some(project) = &hints.project_name {
            header = header.child(
                div()
                    .text_xs()
                    .text_color(muted.opacity(0.7))
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(project.clone()),
            );
        }

        if let Some(cli_name) = &hints.cli_name {
            header = header.child(Self::render_cli_badge(cli_name, border_color, muted, theme));
        }

        if let Some(branch) = &hints.git_branch {
            header = header.child(self.render_git_branch_badge(
                branch,
                hints,
                border_color,
                muted,
                orange,
            ));
        }

        if let Some(shell_label) = &hints.shell_label {
            header = header.child(self.render_shell_badge(
                shell_label,
                hints,
                border_color,
                muted,
                orange,
            ));
        }

        header = header.child(div().flex_1());

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

        if let Some(context) = &hints.context {
            let context_color: gpui::Hsla = context.color.into();
            header = header.child(
                div()
                    .text_xs()
                    .text_color(context_color)
                    .child(context.text().to_string()),
            );
        }

        if show_plus_button {
            header = header.child(self.render_pane_add_button(
                &pane_id,
                session_id,
                border_color,
                fg,
                cx,
            ));
        }

        if is_drag_source {
            header.cursor_grabbing()
        } else {
            header
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_pane_tab_strip(
        &mut self,
        pane_id: PaneId,
        session_id: SessionId,
        hints: &TerminalHeaderRenderHints,
        theme: &CodirigentTheme,
        border_color: gpui::Hsla,
        fg: gpui::Hsla,
        muted: gpui::Hsla,
        drag_logical_index: Option<usize>,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let pane_tab_ids = self.workspace().pane_tab_session_ids(pane_id.clone());
        let mut tab_strip = div().flex().items_center().gap_1().overflow_hidden();

        let tab_status_style = self
            .effective_user_settings()
            .appearance
            .tab_status_style
            .clone();

        for tab_session_id in pane_tab_ids {
            let tab_is_active = tab_session_id == session_id;
            let tab_name = self
                .workspace()
                .session(tab_session_id)
                .map(|session| session.name.clone())
                .unwrap_or_else(|| hints.name.clone());
            let tab_status = self
                .workspace()
                .session(tab_session_id)
                .map(|s| s.status)
                .unwrap_or(SessionStatus::Idle);
            let decoration = super::tab_status_render::render_tab_status(
                &tab_status_style,
                tab_status,
                tab_is_active,
            );

            let tab_bg = if let Some(glow_bg) = decoration.tab_bg {
                if tab_is_active {
                    let mut base: gpui::Hsla = theme.active.into();
                    base.h = glow_bg.h;
                    base.s = glow_bg.s.max(base.s);
                    base
                } else {
                    glow_bg
                }
            } else if tab_is_active {
                theme.active.into()
            } else {
                border_color.opacity(0.35)
            };
            let tab_fg = if tab_is_active {
                fg
            } else {
                muted.opacity(0.9)
            };

            let mut tab = div()
                .id(SharedString::from(format!(
                    "terminal-tab-{}-{}",
                    session_id.0, tab_session_id.0
                )))
                .px_2()
                .h(px(22.0))
                .rounded_md()
                .bg(tab_bg)
                .flex()
                .items_center()
                .gap_1()
                .overflow_hidden()
                .cursor_pointer()
                .on_click(cx.listener({
                    let pane_id = pane_id.clone();
                    move |this, _: &ClickEvent, _window, cx| {
                        if this
                            .workspace
                            .activate_pane_tab(pane_id.clone(), tab_session_id)
                        {
                            this.select_session_with_cx(tab_session_id, cx);
                            this.mark_layout_cache_dirty();
                            this.sync_layout_derived_state();
                            this.save_state_to_disk(cx);
                            cx.notify();
                        }
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                        this.open_session_menu(
                            tab_session_id,
                            Some(event.position.y.into()),
                            Some(event.position.x.into()),
                            cx,
                        );
                        cx.stop_propagation();
                    }),
                );

            // Apply glow border if present
            if let Some(glow_border) = decoration.tab_border {
                tab = tab.border_1().border_color(glow_border);
            }

            // Apply pulse opacity for animated states.
            // 6-step sine-like curve for smooth pulsing (250ms per step = 1.5s cycle).
            if decoration.should_pulse {
                const PULSE_CURVE: [f32; 6] = [1.0, 0.85, 0.55, 0.4, 0.55, 0.85];
                let phase = (self.pulse_counter % 6) as usize;
                tab = tab.opacity(PULSE_CURVE[phase]);
            }

            let is_badge = tab_status_style == "badge";
            let is_glow = tab_status_style == "glow";
            let mut status_child = decoration.child;

            // Prepend dot (before name) for "dot" style or unknown styles
            if !is_badge && !is_glow {
                if let Some(child) = status_child.take() {
                    tab = tab.child(child);
                }
            }

            // Add the name label
            tab = tab.child(
                div()
                    .text_xs()
                    .font_weight(if tab_is_active {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(tab_fg)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(tab_name),
            );

            // Append badge (after name) for "badge" style
            if is_badge {
                if let Some(child) = status_child.take() {
                    tab = tab.child(child);
                }
            }

            if tab_is_active {
                let drag_source_pane_id = pane_id.clone();
                tab = tab
                    .cursor_grab()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                            let pos = crate::layout::Point::new(
                                event.position.x.into(),
                                event.position.y.into(),
                            );
                            this.selection.drag = Some(super::types::DragState {
                                source_session_id: tab_session_id,
                                source_pane_id: drag_source_pane_id.clone(),
                                source_index: drag_logical_index.unwrap_or(0),
                                start_position: pos,
                                current_position: pos,
                                active: false,
                                target: None,
                            });
                            cx.notify();
                        }),
                    )
                    .on_mouse_move(cx.listener(
                        move |this, event: &MouseMoveEvent, _window, cx| {
                            let Some(drag) = &mut this.selection.drag else {
                                return;
                            };
                            if drag.source_session_id != tab_session_id {
                                return;
                            }
                            let pos = crate::layout::Point::new(
                                event.position.x.into(),
                                event.position.y.into(),
                            );
                            drag.update_pointer(pos, &this.cache.render_pane_drop_targets);
                            cx.notify();
                        },
                    ));
            }

            tab_strip = tab_strip.child(tab);
        }

        tab_strip
    }

    fn render_git_branch_badge(
        &mut self,
        branch: &str,
        hints: &TerminalHeaderRenderHints,
        border_color: gpui::Hsla,
        muted: gpui::Hsla,
        _orange: gpui::Hsla,
    ) -> gpui::Div {
        let git_fg = muted.opacity(0.8);
        let git_addition: gpui::Hsla = crate::sidebar::Color::from_hex("#22c55e").into();
        let git_deletion: gpui::Hsla = crate::sidebar::Color::from_hex("#ef4444").into();
        let git_badge_bg = border_color.opacity(0.25);
        let branch_label = if branch.chars().count() > 16 {
            let truncated: String = branch.chars().take(13).collect();
            format!("{}...", truncated)
        } else {
            branch.to_owned()
        };
        let mut git_badge = div()
            .px(px(4.0))
            .py_px()
            .rounded_sm()
            .bg(git_badge_bg)
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(git_fg)
                    .font_family(icons::LUCIDE_FONT_FAMILY)
                    .child(icons::git_branch()),
            )
            .child(div().text_xs().text_color(git_fg).child(branch_label));

        if let Some(count) = hints.git_pending_additions.filter(|count| *count > 0) {
            git_badge = git_badge.child(
                div()
                    .text_xs()
                    .text_color(git_addition)
                    .child(format!("+{}", count)),
            );
        }

        if let Some(count) = hints.git_pending_deletions.filter(|count| *count > 0) {
            git_badge = git_badge.child(
                div()
                    .text_xs()
                    .text_color(git_deletion)
                    .child(format!("-{}", count)),
            );
        }

        git_badge
    }

    fn render_cli_badge(
        cli_name: &str,
        border_color: gpui::Hsla,
        muted: gpui::Hsla,
        theme: &CodirigentTheme,
    ) -> gpui::Div {
        let cli_fg: gpui::Hsla = theme.primary.into();

        div()
            .px(px(4.0))
            .py_px()
            .rounded_sm()
            .bg(border_color.opacity(0.25))
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_1()
            .child(div().text_xs().text_color(muted.opacity(0.6)).child("CLI"))
            .child(
                div()
                    .text_xs()
                    .text_color(cli_fg)
                    .child(cli_name.to_owned()),
            )
    }

    fn render_shell_badge(
        &mut self,
        shell_label: &str,
        hints: &TerminalHeaderRenderHints,
        border_color: gpui::Hsla,
        muted: gpui::Hsla,
        orange: gpui::Hsla,
    ) -> gpui::Div {
        let shell_warning = hints.shell_warning.is_some();
        let shell_fg = if shell_warning {
            orange
        } else {
            muted.opacity(0.8)
        };
        let shell_bg = if shell_warning {
            orange.opacity(0.12)
        } else {
            border_color.opacity(0.25)
        };

        div()
            .px(px(4.0))
            .py_px()
            .rounded_sm()
            .bg(shell_bg)
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(shell_fg)
                    .font_family(icons::LUCIDE_FONT_FAMILY)
                    .child(icons::terminal()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(shell_fg)
                    .child(shell_label.to_owned()),
            )
    }

    fn render_pane_add_button(
        &mut self,
        pane_id: &PaneId,
        session_id: SessionId,
        border_color: gpui::Hsla,
        fg: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(SharedString::from(format!("pane-add-tab-{}", session_id.0)))
            .w(px(20.0))
            .h(px(20.0))
            .rounded_md()
            .bg(border_color.opacity(0.25))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|style| style.bg(border_color.opacity(0.45)))
            .on_click(cx.listener({
                let pane_id = pane_id.clone();
                move |this, _: &ClickEvent, _window, cx| {
                    this.create_session_in_pane(pane_id.clone(), cx);
                }
            }))
            .child(
                div()
                    .text_xs()
                    .font_family(icons::LUCIDE_FONT_FAMILY)
                    .text_color(fg)
                    .child(icons::plus()),
            )
    }
}
