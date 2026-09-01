//! Toast notification rendering for auto-update UI.
//!
//! Renders a small overlay in the bottom-right corner of the workspace
//! showing update status: available, downloading, ready to apply, or
//! post-update confirmation.

use super::gpui::WorkspaceView;
use gpui::{
    div, px, ClickEvent, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled,
};

/// Keep pointer input inside an update toast from reaching the workspace underneath it.
fn prevent_update_toast_pointer_passthrough<E: InteractiveElement>(element: E) -> E {
    element.occlude()
}

impl WorkspaceView {
    /// Render the auto-update toast notification.
    ///
    /// Returns `None` when there is nothing to show (all update state is
    /// `None` or the user has dismissed the toast).
    pub(super) fn render_update_toast(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        // Determine which toast variant to show (priority order).
        let variant = if let Some(ref staged) = self.staged_update {
            ToastVariant::ReadyToApply {
                version: staged.version.to_string(),
            }
        } else if let Some(percent) = self.update_download_progress {
            ToastVariant::Downloading { percent }
        } else if let Some(ref info) = self.update_info {
            if self.update_dismissed {
                return None;
            }
            ToastVariant::UpdateAvailable {
                version: info.version.to_string(),
            }
        } else {
            return None;
        };

        let theme = self.workspace().theme();
        let panel_bg: gpui::Hsla = theme.panel_background.into();
        let border_color: gpui::Hsla = theme.border.into();
        let fg: gpui::Hsla = theme.foreground.into();
        let muted: gpui::Hsla = theme.muted.into();
        let primary: gpui::Hsla = theme.primary.into();

        let mut toast = prevent_update_toast_pointer_passthrough(
            div()
                .id("update-toast")
                .absolute()
                .bottom(px(16.0))
                .right(px(16.0))
                .bg(panel_bg)
                .border_1()
                .border_color(border_color)
                .rounded_lg()
                .shadow_lg()
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .max_w(px(320.0))
                .min_w(px(240.0)),
        );

        match variant {
            ToastVariant::UpdateAvailable { version } => {
                toast = toast
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(fg)
                                    .child(SharedString::from(format!(
                                        "Update available (v{})",
                                        version
                                    ))),
                            )
                            .child(self.render_dismiss_button(muted, cx)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("A new version of Codirigent is available."),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .justify_end()
                            .child(self.render_toast_button(
                                "update-btn",
                                "Update",
                                primary,
                                gpui::Hsla::white(),
                                |this: &mut Self,
                                 _: &ClickEvent,
                                 _window,
                                 cx: &mut gpui::Context<Self>| {
                                    if let Some(svc) = &this.update_service {
                                        svc.start_download();
                                    }
                                    cx.notify();
                                },
                                cx,
                            )),
                    );
            }
            ToastVariant::Downloading { percent } => {
                toast = toast
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(fg)
                            .child(SharedString::from(format!("Downloading... {}%", percent))),
                    )
                    .child(self.render_progress_bar(percent, primary, border_color))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .justify_end()
                            .child(self.render_toast_button(
                                "cancel-download-btn",
                                "Cancel",
                                border_color,
                                fg,
                                |this: &mut Self,
                                 _: &ClickEvent,
                                 _window,
                                 cx: &mut gpui::Context<Self>| {
                                    if let Some(svc) = &this.update_service {
                                        svc.cancel_download();
                                    }
                                    this.update_download_progress = None;
                                    // Restore update_info from service state
                                    if let Some(svc) = &this.update_service {
                                        if let codirigent_updater::UpdateState::UpdateAvailable(
                                            info,
                                        ) = svc.state()
                                        {
                                            this.update_info = Some(info);
                                        }
                                    }
                                    cx.notify();
                                },
                                cx,
                            )),
                    );
            }
            ToastVariant::ReadyToApply { version } => {
                toast = toast
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(fg)
                                    .child(SharedString::from(format!(
                                        "Update ready (v{})",
                                        version
                                    ))),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("Restart to apply the update."),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .justify_end()
                            .child(self.render_toast_button(
                                "later-btn",
                                "Later",
                                border_color,
                                fg,
                                |this: &mut Self,
                                 _: &ClickEvent,
                                 _window,
                                 cx: &mut gpui::Context<Self>| {
                                    this.update_dismissed = true;
                                    cx.notify();
                                },
                                cx,
                            ))
                            .child(self.render_toast_button(
                                "restart-btn",
                                "Restart Now",
                                primary,
                                gpui::Hsla::white(),
                                |this: &mut Self,
                                 _: &ClickEvent,
                                 _window,
                                 cx: &mut gpui::Context<Self>| {
                                    if let Some(svc) = &this.update_service {
                                        match svc.apply() {
                                            Ok(()) => {
                                                cx.quit();
                                            }
                                            Err(e) => {
                                                tracing::error!("Failed to apply update: {}", e);
                                            }
                                        }
                                    }
                                },
                                cx,
                            )),
                    );
            }
        }

        Some(toast)
    }

    /// Render a small dismiss (X) button for the toast.
    fn render_dismiss_button(&self, muted: gpui::Hsla, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("dismiss-update-toast")
            .text_xs()
            .text_color(muted)
            .cursor_pointer()
            .hover(|style| style.text_color(muted.opacity(0.7)))
            .px_1()
            .rounded_sm()
            .child("\u{2715}") // Unicode X mark
            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                this.update_dismissed = true;
                cx.notify();
            }))
    }

    /// Render a styled button for the toast.
    fn render_toast_button(
        &self,
        id: &str,
        label: &str,
        bg: gpui::Hsla,
        text: gpui::Hsla,
        on_click: impl Fn(&mut Self, &ClickEvent, &mut gpui::Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(SharedString::from(id.to_string()))
            .px_3()
            .py(px(4.0))
            .rounded_md()
            .bg(bg)
            .text_xs()
            .text_color(text)
            .cursor_pointer()
            .hover(|style| style.opacity(0.85))
            .on_click(cx.listener(on_click))
            .child(SharedString::from(label.to_string()))
    }

    /// Render a simple progress bar.
    fn render_progress_bar(
        &self,
        percent: u8,
        fill_color: gpui::Hsla,
        track_color: gpui::Hsla,
    ) -> impl IntoElement {
        let width_pct = (percent as f32).clamp(0.0, 100.0);
        div()
            .w_full()
            .h(px(4.0))
            .rounded_sm()
            .bg(track_color)
            .child(
                div()
                    .h_full()
                    .rounded_sm()
                    .bg(fill_color)
                    .w(gpui::relative(width_pct / 100.0)),
            )
    }
}

/// Toast content variants.
enum ToastVariant {
    UpdateAvailable { version: String },
    Downloading { percent: u8 },
    ReadyToApply { version: String },
}

#[cfg(test)]
mod tests {
    use super::prevent_update_toast_pointer_passthrough;
    use gpui::{
        div, point, px, size, InteractiveElement, Modifiers, MouseButton, ParentElement, Styled,
        TestAppContext,
    };
    use std::{cell::Cell, rc::Rc};

    #[gpui::test]
    fn update_toast_click_does_not_reach_content_behind_it(cx: &mut TestAppContext) {
        let behind_mouse_downs = Rc::new(Cell::new(0));
        let toast_mouse_downs = Rc::new(Cell::new(0));
        let visual_cx = cx.add_empty_window();

        visual_cx.draw(point(px(0.0), px(0.0)), size(px(100.0), px(100.0)), {
            let behind_mouse_downs = behind_mouse_downs.clone();
            let toast_mouse_downs = toast_mouse_downs.clone();

            move |_, _| {
                let behind_mouse_downs = behind_mouse_downs.clone();
                let toast_mouse_downs = toast_mouse_downs.clone();

                div()
                    .relative()
                    .size_full()
                    .child(div().absolute().inset_0().on_mouse_down(
                        MouseButton::Left,
                        move |_, _, _| {
                            behind_mouse_downs.set(behind_mouse_downs.get() + 1);
                        },
                    ))
                    .child(
                        prevent_update_toast_pointer_passthrough(
                            div()
                                .id("test-update-toast")
                                .debug_selector(|| "test-update-toast".to_string())
                                .absolute()
                                .left(px(20.0))
                                .top(px(20.0))
                                .w(px(60.0))
                                .h(px(60.0)),
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            move |_, _, _| {
                                toast_mouse_downs.set(toast_mouse_downs.get() + 1);
                            },
                        ),
                    )
            }
        });

        let toast_bounds = visual_cx
            .debug_bounds("test-update-toast")
            .expect("test update toast should be rendered");
        visual_cx.simulate_mouse_down(toast_bounds.center(), MouseButton::Left, Modifiers::none());

        assert_eq!(
            toast_mouse_downs.get(),
            1,
            "toast should receive its own mouse input"
        );
        assert_eq!(
            behind_mouse_downs.get(),
            0,
            "toast click must not reach the content behind it"
        );
    }
}
