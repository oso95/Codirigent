//! GPUI rendering components for WorkspaceView.
//!
//! This module coordinates rendering across component modules:
//! - [`terminal_render`] - Terminal canvas rendering (text, cursor, backgrounds)
//! - [`drawer_render`] - Drawer panels (sessions, files, worktrees)
//! - [`grid_render`] - Workspace grid and session cell layout
//! - [`icon_rail_render`] - Left sidebar icon rail
//! - [`task_board_render`] - Right task board panel
//! - [`top_bar_render`] - Top bar and session tabs
//! - [`modal_render`] - Action modals and dialogs
//! - [`icon_utils`] - Icon rendering utilities
//!
//! This file contains:
//! - Title bar rendering
//! - Terminal headers and empty cells
//! - Session menus

use super::gpui::WorkspaceView;
use crate::icons;
use crate::title_bar::TitleBar;
use gpui::{
    div, prelude::FluentBuilder, px, ClickEvent, Context, FontWeight, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, WindowControlArea,
};
use tracing::info;

impl WorkspaceView {
    /// Render the title bar with window controls (minimize, maximize, close).
    ///
    /// This is a 32px bar with a dedicated drag region on the left and native
    /// window controls on the right.
    pub(super) fn render_title_bar(
        &mut self,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.workspace().theme();
        let bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();

        // The bar hosts a dedicated drag region plus caption buttons.
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
            .gap_2();

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
        }

        // Drag region: how the user moves the window by clicking the title bar.
        //
        // macOS: Use GPUI's `WindowControlArea::Drag` — it returns HTCAPTION via
        //   the native hit-test and the OS handles drag + double-click-to-zoom.
        //
        // Windows: Do NOT use `WindowControlArea::Drag`. GPUI 0.2.x has two
        //   issues: (1) WM_NCHITTEST returns HTCAPTION while GPUI holds RefCell
        //   borrows, causing a freeze when DefWindowProc enters a modal drag
        //   loop; (2) GPUI re-dispatches WM_NCLBUTTONDOWN through its element
        //   tree, so on_mouse_down handlers eat resize events for the top edge
        //   (which overlaps the title bar).
        //
        //   Fix: a Win32 window subclass (see `platform_drag.rs`) intercepts
        //   NC messages before GPUI. Drag uses a custom WM_APP message that the
        //   subclass routes to DefWindowProc(WM_NCLBUTTONDOWN, HTCAPTION).
        //   Resize edges go straight to DefWindowProc, bypassing GPUI.
        let mut drag_region = div().flex().items_center().gap_2().flex_1().h_full();

        #[cfg(target_os = "macos")]
        {
            drag_region = drag_region
                .window_control_area(WindowControlArea::Drag)
                .on_mouse_down(MouseButton::Left, |event: &MouseDownEvent, window, _cx| {
                    if event.click_count == 2 {
                        window.titlebar_double_click();
                    }
                });
        }

        #[cfg(target_os = "windows")]
        {
            use raw_window_handle::HasWindowHandle;
            let raw_handle =
                HasWindowHandle::window_handle(window)
                    .ok()
                    .map(|h| match h.as_raw() {
                        raw_window_handle::RawWindowHandle::Win32(win32) => win32.hwnd.get(),
                        _ => 0,
                    });
            if let Some(hwnd) = raw_handle {
                // Install the window subclass that routes WM_NCLBUTTONDOWN
                // for resize edges directly to DefWindowProc (bypassing
                // GPUI's element dispatch) and handles our custom drag
                // message. Safe to call every frame — only installs once.
                crate::platform_drag::install_drag_subclass(hwnd);

                drag_region = drag_region.on_mouse_down(
                    MouseButton::Left,
                    move |event: &MouseDownEvent, window, _cx| {
                        if event.click_count == 2 {
                            window.titlebar_double_click();
                        } else {
                            crate::platform_drag::begin_title_bar_drag(hwnd);
                        }
                    },
                );
            }
        }

        // Logo (3x3 grid matching logo-primary-dark.svg)
        drag_region =
            drag_region.child(div().flex_shrink_0().ml_2().child(self.render_logo_small()));
        drag_region = drag_region.child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(fg)
                .ml_2()
                .child(TitleBar::LOGO_TEXT),
        );

        // Spacer - fills remaining space so window controls stay on the right.
        drag_region = drag_region.child(div().flex_1());
        bar = bar.child(drag_region);

        // Window controls (Windows/Linux)
        // Uses native Segoe icon fonts and WindowControlArea for OS-level handling.
        #[cfg(not(target_os = "macos"))]
        {
            /// Windows-standard close-button hover red (matches OS titlebar).
            const CLOSE_BUTTON_RED: gpui::Rgba = gpui::Rgba {
                r: 232.0 / 255.0,
                g: 17.0 / 255.0,
                b: 32.0 / 255.0,
                a: 1.0,
            };

            let icon_font = "Segoe MDL2 Assets";
            let close_hover_bg: gpui::Hsla = CLOSE_BUTTON_RED.into();

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

    /// Render empty cell inline with pre-computed colors (returns Stateful<Div>).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_empty_cell_inline_with_colors(
        &mut self,
        pane_id: codirigent_core::PaneId,
        index: usize,
        position: codirigent_core::GridPosition,
        panel_bg: gpui::Hsla,
        border_color: gpui::Hsla,
        muted: gpui::Hsla,
        cell_height: f32,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let is_drop_target = self
            .selection
            .drag
            .as_ref()
            .and_then(|drag| drag.target.as_ref())
            .is_some_and(|target| target.pane_id == pane_id && target.index == index);
        let current_border = if is_drop_target {
            let primary: gpui::Hsla = self.workspace().theme().primary.into();
            primary
        } else {
            border_color
        };

        let empty = div()
            .id(SharedString::from(format!(
                "empty-cell-{}-{}",
                position.row, position.col
            )))
            .w_full()
            .h(px(cell_height))
            .bg(panel_bg)
            .border_color(current_border)
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
                    .child(super::types::EMPTY_CELL_MESSAGE),
            );

        if is_drop_target {
            empty.border_2()
        } else {
            empty.border_1()
        }
    }

    /// Render the session context menu (right-click dropdown).
    ///
    /// Displays a floating dropdown near the session row with options for
    /// rename, group management, and session termination.
    pub(super) fn render_session_menu(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let session_id = self.selection.session_menu_open?;

        let theme = self.workspace().theme().clone();
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let hover_bg: gpui::Hsla = theme.active.into();
        let destructive = super::types::DESTRUCTIVE_ITEM_COLOR;
        let orange: gpui::Hsla = theme.orange.into();

        // Check if this session has a group and collect metadata shown in the dropdown.
        let (session_group, session_shell, project_name, cli_name) = {
            let session = self.workspace().session(session_id)?;
            (
                session.group.clone(),
                session.shell.clone(),
                super::gpui::session_project_name(session)
                    .unwrap_or_else(|| "Unknown project".to_string()),
                self.session_cli_display_name(session_id),
            )
        };
        let has_group = session_group.is_some();
        let (shell_label, shell_warning) =
            self.session_shell_display(session_id, session_shell.as_deref());

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

        let top_offset = self.selection.session_menu_anchor_y.unwrap_or_else(|| {
            crate::title_bar::TitleBar::DEFAULT_HEIGHT
                + crate::top_bar::TopBar::HEIGHT
                + super::types::DRAWER_HEADER_HEIGHT
                + self.session_drawer_row_offset(session_id).unwrap_or(0.0)
        });

        // Transparent click-away backdrop (no dark overlay)
        let backdrop = div()
            .id("session-menu-backdrop")
            .occlude()
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _window, cx| {
                    this.close_session_menu(cx);
                    cx.stop_propagation();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _: &MouseDownEvent, _window, cx| {
                    this.close_session_menu(cx);
                    cx.stop_propagation();
                }),
            );

        // Build dropdown menu
        let mut dropdown = div()
            .w(px(240.0))
            .bg(panel_bg)
            .border_1()
            .border_color(border_color)
            .rounded_md()
            .overflow_hidden()
            .shadow_lg()
            .flex()
            .flex_col()
            .py_1();

        dropdown = dropdown
            .child(
                div()
                    .px_3()
                    .pt_2()
                    .pb_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted.opacity(0.6))
                            .child("PROJECT"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(fg)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(project_name),
                    )
                    .when_some(cli_name, |el, cli_name| {
                        el.child(div().text_xs().text_color(muted.opacity(0.6)).child("CLI"))
                            .child(div().text_sm().text_color(fg).child(cli_name))
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted.opacity(0.6))
                            .child("SHELL"),
                    )
                    .child(div().text_sm().text_color(fg).child(shell_label))
                    .when_some(shell_warning, |el, warning| {
                        el.child(div().text_xs().text_color(orange).child(warning))
                    }),
            )
            .child(div().h(px(1.0)).mx_2().my_1().bg(border_color));

        // Rename
        dropdown = dropdown.child(self.render_menu_item(
            "Rename",
            session_id,
            SessionMenuAction::Rename,
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
            hover_bg,
            destructive,
            cx,
        ));

        // Position dropdown: at click position (tab right-click) or next to the drawer (default).
        let left_offset = self
            .selection
            .session_menu_anchor_x
            .unwrap_or_else(|| crate::icon_rail::IconRail::WIDTH + self.drawer.width() - 8.0);

        Some(
            div()
                .id("session-menu-container")
                .occlude()
                .absolute()
                .inset_0()
                .child(backdrop)
                .child(
                    div()
                        .occlude()
                        .absolute()
                        .left(px(left_offset))
                        .top(px(top_offset))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|_this, _: &MouseDownEvent, _window, cx| {
                                cx.stop_propagation();
                            }),
                        )
                        .child(dropdown),
                ),
        )
    }

    // Drawer, file tree, session rows, and group headers are in drawer_render.rs
}

/// Session menu actions.
#[derive(Debug, Clone)]
pub(super) enum SessionMenuAction {
    Rename,
    AssignToGroup(String),
    NewGroup,
    RemoveGroup,
    EndSession,
}
