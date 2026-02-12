//! Icon rail (left sidebar) rendering for workspace.
//!
//! This module handles rendering of the left sidebar icon rail,
//! including navigation icons and layout controls.

use crate::icons;
use crate::workspace::gpui::WorkspaceView;
use gpui::{
    div, px, ClickEvent, Context, FontWeight, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, Styled,
};

impl WorkspaceView {
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
                let btn_bg = if is_active {
                    active_bg
                } else {
                    gpui::Hsla::transparent_black()
                };
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
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.icon_rail
                                .toggle_panel(crate::icon_rail::DrawerPanel::Sessions);
                            this.process_icon_rail_events();
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(20.0))
                            .text_color(btn_fg)
                            .font_family(icons::LUCIDE_FONT_FAMILY)
                            .child(icons::terminal()),
                    )
            })
            // Files button
            .child({
                let is_active = active_panel == Some(crate::icon_rail::DrawerPanel::Files);
                let btn_bg = if is_active {
                    active_bg
                } else {
                    gpui::Hsla::transparent_black()
                };
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
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.icon_rail
                                .toggle_panel(crate::icon_rail::DrawerPanel::Files);
                            this.process_icon_rail_events();
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(20.0))
                            .text_color(btn_fg)
                            .font_family(icons::LUCIDE_FONT_FAMILY)
                            .child(icons::folder_tree()),
                    )
            })
            // Worktrees button
            .child({
                let is_active = active_panel == Some(crate::icon_rail::DrawerPanel::Worktrees);
                let btn_bg = if is_active {
                    active_bg
                } else {
                    gpui::Hsla::transparent_black()
                };
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
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.icon_rail
                                .toggle_panel(crate::icon_rail::DrawerPanel::Worktrees);
                            this.process_icon_rail_events();
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(20.0))
                            .text_color(btn_fg)
                            .font_family(icons::LUCIDE_FONT_FAMILY)
                            .child(icons::git_branch()),
                    )
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
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.open_settings();
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .text_size(px(20.0))
                            .text_color(muted)
                            .font_family(icons::LUCIDE_FONT_FAMILY)
                            .child(icons::settings()),
                    ),
            )
    }

    /// Render the expandable drawer panel (288px).
}
