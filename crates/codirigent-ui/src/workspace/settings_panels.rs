//! Interactive settings panels with real dropdown menus and live setting wiring.
//!
//! Each category panel reads current values from `SettingsPage`, builds
//! controls with `cx.listener()` handlers, and applies changes to the
//! running app (theme, terminal cursor, grid gap, etc.) immediately.

use gpui::*;
use std::sync::Arc;

use crate::settings::controls::{setting_row, setting_toggle, settings_section_header};
use crate::settings::SettingsCategory;
use crate::terminal_view::CursorShape;

use super::types::DROPDOWN_TRIGGER_HEIGHT;

impl super::gpui::WorkspaceView {
    /// Render the full settings overlay (sidebar + content area).
    pub(super) fn render_settings_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let page = self.settings_page.as_ref().unwrap();
        let theme = self.workspace.theme();
        let bg: Hsla = theme.background.into();
        let panel_bg: Hsla = theme.panel_background.into();
        let fg: Hsla = theme.foreground.into();
        let primary: Hsla = theme.primary.into();
        let border: Hsla = theme.border.into();
        let active_cat = page.active_category();
        let base_font_size = theme.font_size_base;

        // Category sidebar
        let mut sidebar = div()
            .w(px(220.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(panel_bg)
            .border_r_1()
            .border_color(border)
            .py_2();

        // Back button
        sidebar = sidebar.child(
            div()
                .id("settings-back")
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.close_settings();
                    cx.notify();
                }))
                .child(
                    div()
                        .text_color(primary)
                        .child("\u{2190} Back to workspace"),
                ),
        );

        sidebar = sidebar.child(div().h(px(1.0)).mx_3().my_2().bg(border));

        for cat in SettingsCategory::ALL {
            let is_active = cat == active_cat;
            let text_color = if is_active { primary } else { fg };
            let item_bg = if is_active {
                Hsla { a: 0.1, ..primary }
            } else {
                Hsla { a: 0.0, ..fg }
            };
            let label = cat.label().to_string();

            sidebar = sidebar.child(
                div()
                    .id(SharedString::from(format!("settings-cat-{}", cat.label())))
                    .flex()
                    .flex_row()
                    .items_center()
                    .px_3()
                    .py(px(6.0))
                    .mx_2()
                    .rounded_md()
                    .bg(item_bg)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(ref mut page) = this.settings_page {
                            page.set_category(cat);
                            page.open_dropdown = None;
                        }
                        cx.notify();
                    }))
                    .child(div().text_color(text_color).child(label)),
            );
        }

        let content_child = self.render_settings_content(cx);

        let content = div()
            .id("settings-content-scroll")
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .p_6()
            .child(
                div()
                    .text_size(px(base_font_size + 5.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(fg)
                    .mb_4()
                    .child(format!("{} Settings", active_cat.label())),
            )
            .child(content_child);

        div()
            .id("settings-overlay")
            .flex_1()
            .flex()
            .flex_row()
            .bg(bg)
            .overflow_hidden()
            .text_size(px(base_font_size))
            .child(sidebar)
            .child(content)
    }

    /// Dispatch to the active category's render method.
    fn render_settings_content(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let cat = self.settings_page.as_ref().unwrap().active_category();
        match cat {
            SettingsCategory::General => self.render_general_settings(cx),
            SettingsCategory::Appearance => self.render_appearance_settings(cx),
            SettingsCategory::Terminal => self.render_terminal_settings(cx),
            SettingsCategory::KeyboardShortcuts => self.render_shortcuts_settings(cx),
            SettingsCategory::Sessions => self.render_sessions_settings(cx),
            SettingsCategory::Advanced => self.render_advanced_settings(cx),
        }
    }

    // ── Dropdown helper ────────────────────────────────────────────────

    /// Render a dropdown control using GPUI's deferred/anchored overlay pattern.
    ///
    /// When open, options appear as a floating overlay on top of all other
    /// content using `deferred(anchored(...))`, matching how Zed renders
    /// popover menus.
    fn render_dropdown_control(
        &self,
        dropdown_id: &str,
        options: &[&str],
        selected: &str,
        cx: &mut Context<Self>,
        on_select: impl Fn(&mut Self, String, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let theme = self.workspace.theme();
        let fg: Hsla = theme.foreground.into();
        let panel_bg: Hsla = theme.panel_background.into();
        let border: Hsla = theme.border.into();
        let hover_bg: Hsla = theme.hover.into();
        let accent: Hsla = theme.primary.into();

        let is_open = self
            .settings_page
            .as_ref()
            .and_then(|p| p.open_dropdown.as_deref())
            == Some(dropdown_id);

        let dd_id = dropdown_id.to_string();
        let selected_display = selected.to_string();

        // Trigger button -- stores click position for anchored overlay
        let trigger = div()
            .id(SharedString::from(format!("{}-trigger", dropdown_id)))
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .min_w(px(140.0))
            .h(px(DROPDOWN_TRIGGER_HEIGHT))
            .bg(panel_bg)
            .border_1()
            .border_color(if is_open { accent } else { border })
            .rounded_md()
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener({
                    let dd_id = dd_id.clone();
                    move |this, event: &MouseDownEvent, _, cx| {
                        if let Some(page) = this.settings_page.as_mut() {
                            if page.open_dropdown.as_deref() == Some(&dd_id) {
                                page.open_dropdown = None;
                            } else {
                                page.open_dropdown = Some(dd_id.clone());
                                // Store click position for anchored overlay
                                page.dropdown_click_pos =
                                    (event.position.x / px(1.0), event.position.y / px(1.0));
                            }
                        }
                        cx.notify();
                    }
                }),
            )
            .child(div().text_color(fg).flex_1().child(selected_display))
            .child(div().text_size(px(10.0)).text_color(fg).child(if is_open {
                "\u{25B2}"
            } else {
                "\u{25BC}"
            }));

        // Container with trigger; overlay is rendered separately in render_settings_overlay
        let mut container = div().child(trigger);

        // Floating dropdown overlay via deferred(anchored(...))
        if is_open {
            let on_select = Arc::new(on_select);
            let (click_x, click_y) = self
                .settings_page
                .as_ref()
                .map(|p| p.dropdown_click_pos)
                .unwrap_or((0.0, 0.0));

            let mut options_list = div()
                .min_w(px(140.0))
                .bg(panel_bg)
                .border_1()
                .border_color(border)
                .rounded_md()
                .shadow_md()
                .py_1()
                .flex()
                .flex_col()
                .overflow_hidden();

            for opt in options {
                let opt_str = opt.to_string();
                let is_selected = *opt == selected;
                let cb = on_select.clone();

                options_list = options_list.child(
                    div()
                        .id(SharedString::from(format!("{}-opt-{}", dd_id, opt)))
                        .px_2()
                        .py(px(6.0))
                        .text_color(if is_selected { accent } else { fg })
                        .bg(if is_selected {
                            Hsla { a: 0.1, ..accent }
                        } else {
                            panel_bg
                        })
                        .cursor_pointer()
                        .hover(|s| s.bg(hover_bg))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                cb(this, opt_str.clone(), window, cx);
                                if let Some(page) = this.settings_page.as_mut() {
                                    page.open_dropdown = None;
                                }
                                cx.notify();
                            }),
                        )
                        .child(opt.to_string()),
                );
            }

            // Click-away backdrop (closes dropdown when clicking outside)
            let backdrop = div()
                .id(SharedString::from(format!("{}-backdrop", dd_id)))
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .w(px(9999.0))
                .h(px(9999.0))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.open_dropdown = None;
                        }
                        cx.notify();
                    }),
                );

            // Position below the click, anchored to window with overflow prevention
            let overlay = deferred(
                anchored()
                    .anchor(Corner::TopLeft)
                    .position(point(px(click_x), px(click_y + DROPDOWN_TRIGGER_HEIGHT)))
                    .snap_to_window_with_margin(px(8.0))
                    .child(div().occlude().child(options_list)),
            )
            .with_priority(1);

            container = container
                .child(deferred(backdrop).with_priority(0))
                .child(overlay);
        }

        container
    }

    // ── General ──────────────────────────────────────────────────────

    fn render_general_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let page = self.settings_page.as_ref().unwrap();
        let editor = page.user_settings.general.editor_command.clone();
        let shell = page.user_settings.general.default_shell.clone();
        let working_dir = page.user_settings.general.default_working_dir.clone();
        let show_splash = page.user_settings.general.show_splash;
        let theme = self.workspace.theme();

        // Show actual CWD when no custom path is configured
        let display_dir = working_dir.clone().unwrap_or_else(|| {
            std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| String::new())
        });

        let editor_options: Vec<&str> = page.detected_editors.iter().map(|s| s.as_str()).collect();

        // Build shell options: empty string displays as "(Auto-detect)"
        let shell_display_options: Vec<String> = page
            .detected_shells
            .iter()
            .map(|s| {
                if s.is_empty() {
                    "(Auto-detect)".to_string()
                } else {
                    s.clone()
                }
            })
            .collect();
        let shell_option_refs: Vec<&str> =
            shell_display_options.iter().map(|s| s.as_str()).collect();
        let shell_display = if shell.is_empty() {
            "(Auto-detect)".to_string()
        } else {
            shell.clone()
        };

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(settings_section_header("Editor", theme, true))
            .child(setting_row(
                "Default editor",
                "External editor to open files with",
                theme,
                self.render_dropdown_control(
                    "dd-editor",
                    &editor_options,
                    &editor,
                    cx,
                    |this, val, _, _| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.user_settings.general.editor_command = val;
                            page.user_save_pending = true;
                        }
                    },
                ),
            ))
            .child(settings_section_header("Shell", theme, false))
            .child(setting_row(
                "Default shell",
                "Shell used for new sessions",
                theme,
                self.render_dropdown_control(
                    "dd-shell",
                    &shell_option_refs,
                    &shell_display,
                    cx,
                    |this, val, _, _| {
                        if let Some(page) = this.settings_page.as_mut() {
                            // Map "(Auto-detect)" back to empty string
                            let stored = if val == "(Auto-detect)" {
                                String::new()
                            } else {
                                val
                            };
                            page.user_settings.general.default_shell = stored;
                            page.user_save_pending = true;
                        }
                    },
                ),
            ))
            .child(setting_row(
                "Default working directory",
                "Initial directory for new sessions",
                theme,
                self.render_path_picker("dd-workdir", &display_dir, cx),
            ))
            .child(settings_section_header("Startup", theme, false))
            .child(setting_row(
                "Show splash screen",
                "Display splash screen on application start",
                theme,
                div()
                    .id("toggle-splash")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if let Some(page) = this.settings_page.as_mut() {
                                page.user_settings.general.show_splash =
                                    !page.user_settings.general.show_splash;
                                page.user_save_pending = true;
                            }
                            cx.notify();
                        }),
                    )
                    .child(setting_toggle(show_splash, theme)),
            ))
            .into_any_element()
    }

    // ── Appearance ───────────────────────────────────────────────────

    fn render_appearance_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let page = self.settings_page.as_ref().unwrap();
        let theme_name = page.user_settings.appearance.theme.clone();
        let font_size = page.user_settings.appearance.font_size;
        let grid_gap = page.user_settings.appearance.grid_gap;
        let theme = self.workspace.theme();

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(settings_section_header("Theme", theme, true))
            .child(setting_row(
                "Color theme",
                "Switch between dark and light themes",
                theme,
                self.render_dropdown_control(
                    "dd-theme",
                    &["dark", "light"],
                    &theme_name,
                    cx,
                    |this, val, _, _| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.user_settings.appearance.theme = val.clone();
                            page.user_save_pending = true;
                        }
                        let new_theme = if val == "light" {
                            crate::theme::CodirigentTheme::light()
                        } else {
                            crate::theme::CodirigentTheme::dark()
                        };
                        // Preserve user settings across theme switch
                        let (gap, ui_size, term_size) = this
                            .settings_page
                            .as_ref()
                            .map(|p| {
                                (
                                    p.user_settings.appearance.grid_gap,
                                    p.user_settings.appearance.font_size,
                                    p.user_settings.terminal.font_size,
                                )
                            })
                            .unwrap_or((4, 13, 13));
                        this.workspace.set_theme(new_theme);
                        let t = this.workspace.theme_mut();
                        t.grid_gap = gap as f32;
                        t.font_size_base = ui_size as f32;
                        t.font_size_small = (ui_size as f32 - 2.0).max(8.0);
                        t.font_size_large = ui_size as f32 + 2.0;
                        t.terminal_font_size = term_size as f32;
                    },
                ),
            ))
            .child(settings_section_header("Interface", theme, false))
            .child(setting_row(
                "UI font size",
                "Font size for interface elements (10-24)",
                theme,
                self.number_stepper(
                    "num-font-size",
                    &font_size.to_string(),
                    cx,
                    |this, _, cx| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.user_settings.appearance.font_size = page
                                .user_settings
                                .appearance
                                .font_size
                                .saturating_sub(1)
                                .max(10);
                            page.user_save_pending = true;
                            let size = page.user_settings.appearance.font_size as f32;
                            this.apply_ui_font_size(size);
                        }
                        cx.notify();
                    },
                    |this, _, cx| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.user_settings.appearance.font_size =
                                (page.user_settings.appearance.font_size + 1).min(24);
                            page.user_save_pending = true;
                            let size = page.user_settings.appearance.font_size as f32;
                            this.apply_ui_font_size(size);
                        }
                        cx.notify();
                    },
                ),
            ))
            .child(setting_row(
                "Grid gap",
                "Spacing between session panes in pixels (0-16)",
                theme,
                self.number_stepper(
                    "num-grid-gap",
                    &grid_gap.to_string(),
                    cx,
                    |this, _, cx| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.user_settings.appearance.grid_gap =
                                page.user_settings.appearance.grid_gap.saturating_sub(1);
                            page.user_save_pending = true;
                            this.workspace.theme_mut().grid_gap =
                                page.user_settings.appearance.grid_gap as f32;
                        }
                        cx.notify();
                    },
                    |this, _, cx| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.user_settings.appearance.grid_gap =
                                (page.user_settings.appearance.grid_gap + 1).min(16);
                            page.user_save_pending = true;
                            this.workspace.theme_mut().grid_gap =
                                page.user_settings.appearance.grid_gap as f32;
                        }
                        cx.notify();
                    },
                ),
            ))
            .into_any_element()
    }

    // ── Terminal ─────────────────────────────────────────────────────

    fn render_terminal_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let page = self.settings_page.as_ref().unwrap();
        let font_family = page.user_settings.terminal.font_family.clone();
        let font_size = page.user_settings.terminal.font_size;
        let cursor_style = page.user_settings.terminal.cursor_style.clone();
        let line_height = page.user_settings.terminal.line_height;
        let font_options: Vec<&str> = page.detected_fonts.iter().map(|s| s.as_str()).collect();
        let theme = self.workspace.theme();

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(settings_section_header("Font", theme, true))
            .child(setting_row(
                "Font family",
                "Monospace font for terminal rendering",
                theme,
                self.render_dropdown_control(
                    "dd-font-family",
                    &font_options,
                    &font_family,
                    cx,
                    |this, val, _, _| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.user_settings.terminal.font_family = val;
                            page.user_save_pending = true;
                        }
                    },
                ),
            ))
            .child(setting_row(
                "Font size",
                "Terminal font size in points (8-24)",
                theme,
                self.number_stepper(
                    "num-term-font",
                    &font_size.to_string(),
                    cx,
                    |this, window, cx| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.user_settings.terminal.font_size = page
                                .user_settings
                                .terminal
                                .font_size
                                .saturating_sub(1)
                                .max(8);
                            page.user_save_pending = true;
                            let size = page.user_settings.terminal.font_size as f32;
                            this.apply_terminal_font_size(window, size);
                        }
                        cx.notify();
                    },
                    |this, window, cx| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.user_settings.terminal.font_size =
                                (page.user_settings.terminal.font_size + 1).min(24);
                            page.user_save_pending = true;
                            let size = page.user_settings.terminal.font_size as f32;
                            this.apply_terminal_font_size(window, size);
                        }
                        cx.notify();
                    },
                ),
            ))
            .child(settings_section_header("Cursor", theme, false))
            .child(setting_row(
                "Cursor style",
                "Shape of the terminal cursor",
                theme,
                self.render_dropdown_control(
                    "dd-cursor",
                    &["block", "underline", "bar"],
                    &cursor_style,
                    cx,
                    |this, val, _, _| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.user_settings.terminal.cursor_style = val.clone();
                            page.user_save_pending = true;
                        }
                        // Apply cursor style to all terminals
                        let shape: CursorShape = match val.as_str() {
                            "underline" => CursorShape::Underline,
                            "bar" | "beam" => CursorShape::Beam,
                            _ => CursorShape::Block,
                        };
                        for tv in this.terminals_mut().values_mut() {
                            tv.set_cursor_shape(shape);
                        }
                    },
                ),
            ))
            .child(settings_section_header("Layout", theme, false))
            .child(setting_row(
                "Line height",
                "Line height multiplier (1.0-2.5)",
                theme,
                self.number_stepper(
                    "num-line-height",
                    &format!("{:.1}", line_height),
                    cx,
                    |this, _, cx| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.user_settings.terminal.line_height =
                                (page.user_settings.terminal.line_height - 0.1).max(1.0);
                            page.user_settings.terminal.line_height =
                                (page.user_settings.terminal.line_height * 10.0).round() / 10.0;
                            page.user_save_pending = true;
                        }
                        cx.notify();
                    },
                    |this, _, cx| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.user_settings.terminal.line_height =
                                (page.user_settings.terminal.line_height + 0.1).min(2.5);
                            page.user_settings.terminal.line_height =
                                (page.user_settings.terminal.line_height * 10.0).round() / 10.0;
                            page.user_save_pending = true;
                        }
                        cx.notify();
                    },
                ),
            ))
            .into_any_element()
    }

    // ── Keyboard Shortcuts ───────────────────────────────────────────

    fn render_shortcuts_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let page = self.settings_page.as_ref().unwrap();
        let theme = self.workspace.theme();
        let fg: Hsla = theme.foreground.into();
        let muted: Hsla = theme.muted.into();
        let accent: Hsla = theme.primary.into();
        let panel_bg: Hsla = theme.panel_background.into();
        let border: Hsla = theme.border.into();

        let mut sorted: Vec<_> = page.user_settings.keybindings.iter().collect();
        sorted.sort_by_key(|(k, _)| (*k).clone());

        let recording = page.recording_shortcut.clone();

        let mut container = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(settings_section_header("Keyboard Shortcuts", theme, true));

        // Table header
        container = container.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .px_2()
                .py_1()
                .child(
                    div()
                        .flex_1()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(muted)
                        .child("ACTION"),
                )
                .child(
                    div()
                        .w(px(160.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(muted)
                        .child("BINDING"),
                ),
        );

        for (action, binding) in sorted {
            let action_name = action.clone();
            let is_recording = recording.as_deref() == Some(action.as_str());
            let display = if is_recording {
                "Press a key...".to_string()
            } else {
                binding.clone()
            };
            let binding_color = if is_recording { accent } else { fg };
            let label = humanize_action_name(action);

            container = container.child(
                div()
                    .id(SharedString::from(format!("shortcut-{}", action)))
                    .flex()
                    .flex_row()
                    .items_center()
                    .px_2()
                    .py(px(6.0))
                    .rounded_md()
                    .hover(|s| s.bg(Hsla { a: 0.05, ..fg }))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            if let Some(page) = this.settings_page.as_mut() {
                                if page.recording_shortcut.as_deref() == Some(&action_name) {
                                    page.recording_shortcut = None;
                                } else {
                                    page.recording_shortcut = Some(action_name.clone());
                                }
                            }
                            cx.notify();
                        }),
                    )
                    .child(div().flex_1().text_color(fg).child(label))
                    .child(
                        div()
                            .w(px(160.0))
                            .text_color(binding_color)
                            .bg(panel_bg)
                            .border_1()
                            .border_color(if is_recording { accent } else { border })
                            .rounded_md()
                            .px_2()
                            .py_1()
                            .child(display),
                    ),
            );
        }

        container.into_any_element()
    }

    // ── Sessions ─────────────────────────────────────────────────────

    fn render_sessions_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let page = self.settings_page.as_ref().unwrap();
        let max_concurrent = page.project_config.sessions.max_concurrent;
        let default_cli = page.project_config.sessions.default_cli.clone();
        let auto_cleanup = page.project_config.sessions.auto_cleanup;
        let theme = self.workspace.theme();

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(settings_section_header("Session Limits", theme, true))
            .child(setting_row(
                "Max concurrent sessions",
                "Maximum number of sessions running simultaneously (1-16)",
                theme,
                self.number_stepper(
                    "num-max-sessions",
                    &max_concurrent.to_string(),
                    cx,
                    |this, _, cx| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.project_config.sessions.max_concurrent = page
                                .project_config
                                .sessions
                                .max_concurrent
                                .saturating_sub(1)
                                .max(1);
                            page.project_save_pending = true;
                        }
                        cx.notify();
                    },
                    |this, _, cx| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.project_config.sessions.max_concurrent =
                                (page.project_config.sessions.max_concurrent + 1).min(16);
                            page.project_save_pending = true;
                        }
                        cx.notify();
                    },
                ),
            ))
            .child(setting_row(
                "Default CLI",
                "CLI tool used for new sessions",
                theme,
                self.render_dropdown_control(
                    "dd-cli",
                    &["claude", "codex", "gemini"],
                    &default_cli,
                    cx,
                    |this, val, _, _| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.project_config.sessions.default_cli = val;
                            page.project_save_pending = true;
                        }
                    },
                ),
            ))
            .child(settings_section_header("Cleanup", theme, false))
            .child(setting_row(
                "Auto-cleanup idle sessions",
                "Automatically close sessions that have been idle for a long time",
                theme,
                div()
                    .id("toggle-cleanup")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if let Some(page) = this.settings_page.as_mut() {
                                page.project_config.sessions.auto_cleanup =
                                    !page.project_config.sessions.auto_cleanup;
                                page.project_save_pending = true;
                            }
                            cx.notify();
                        }),
                    )
                    .child(setting_toggle(auto_cleanup, theme)),
            ))
            .into_any_element()
    }

    // ── Advanced ─────────────────────────────────────────────────────

    fn render_advanced_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let page = self.settings_page.as_ref().unwrap();
        let scheduler_mode = format!("{:?}", page.project_config.scheduler.mode);
        let auto_assign = page.project_config.scheduler.auto_assign;
        let ver_enabled = page.project_config.verification.enabled;
        let ver_auto = page.project_config.verification.auto_detect;
        let max_retries = page.project_config.verification.max_retries;
        let use_worktrees = page.project_config.git.use_worktrees;
        let auto_commit = page.project_config.git.auto_commit;
        let theme = self.workspace.theme();

        div()
            .flex()
            .flex_col()
            .gap_1()
            // Scheduler
            .child(settings_section_header("Scheduler", theme, true))
            .child(setting_row(
                "Scheduler mode",
                "Task scheduling strategy",
                theme,
                self.render_dropdown_control(
                    "dd-scheduler",
                    &["Fifo", "Priority", "Dependency", "Smart"],
                    &scheduler_mode,
                    cx,
                    |this, val, _, _| {
                        if let Some(page) = this.settings_page.as_mut() {
                            use codirigent_core::config::SchedulerMode;
                            page.project_config.scheduler.mode = match val.as_str() {
                                "Priority" => SchedulerMode::Priority,
                                "Dependency" => SchedulerMode::Dependency,
                                "Smart" => SchedulerMode::Smart,
                                _ => SchedulerMode::Fifo,
                            };
                            page.project_save_pending = true;
                        }
                    },
                ),
            ))
            .child(setting_row(
                "Auto-assign tasks",
                "Automatically assign tasks to idle sessions",
                theme,
                div()
                    .id("toggle-auto-assign")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if let Some(page) = this.settings_page.as_mut() {
                                page.project_config.scheduler.auto_assign =
                                    !page.project_config.scheduler.auto_assign;
                                page.project_save_pending = true;
                            }
                            cx.notify();
                        }),
                    )
                    .child(setting_toggle(auto_assign, theme)),
            ))
            // Verification
            .child(settings_section_header("Verification", theme, false))
            .child(setting_row(
                "Enable verification",
                "Run verification after task completion",
                theme,
                div()
                    .id("toggle-ver-enabled")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if let Some(page) = this.settings_page.as_mut() {
                                page.project_config.verification.enabled =
                                    !page.project_config.verification.enabled;
                                page.project_save_pending = true;
                            }
                            cx.notify();
                        }),
                    )
                    .child(setting_toggle(ver_enabled, theme)),
            ))
            .child(setting_row(
                "Auto-detect commands",
                "Auto-detect test/lint commands based on project type",
                theme,
                div()
                    .id("toggle-ver-auto")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if let Some(page) = this.settings_page.as_mut() {
                                page.project_config.verification.auto_detect =
                                    !page.project_config.verification.auto_detect;
                                page.project_save_pending = true;
                            }
                            cx.notify();
                        }),
                    )
                    .child(setting_toggle(ver_auto, theme)),
            ))
            .child(setting_row(
                "Max retries",
                "Maximum retry attempts before blocking (1-10)",
                theme,
                self.number_stepper(
                    "num-retries",
                    &max_retries.to_string(),
                    cx,
                    |this, _, cx| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.project_config.verification.max_retries = page
                                .project_config
                                .verification
                                .max_retries
                                .saturating_sub(1)
                                .max(1);
                            page.project_save_pending = true;
                        }
                        cx.notify();
                    },
                    |this, _, cx| {
                        if let Some(page) = this.settings_page.as_mut() {
                            page.project_config.verification.max_retries =
                                (page.project_config.verification.max_retries + 1).min(10);
                            page.project_save_pending = true;
                        }
                        cx.notify();
                    },
                ),
            ))
            // Git
            .child(settings_section_header("Git", theme, false))
            .child(setting_row(
                "Use worktrees",
                "Isolate sessions in separate git worktrees",
                theme,
                div()
                    .id("toggle-worktrees")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if let Some(page) = this.settings_page.as_mut() {
                                page.project_config.git.use_worktrees =
                                    !page.project_config.git.use_worktrees;
                                page.project_save_pending = true;
                            }
                            cx.notify();
                        }),
                    )
                    .child(setting_toggle(use_worktrees, theme)),
            ))
            .child(setting_row(
                "Auto-commit",
                "Automatically commit changes after task completion",
                theme,
                div()
                    .id("toggle-auto-commit")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if let Some(page) = this.settings_page.as_mut() {
                                page.project_config.git.auto_commit =
                                    !page.project_config.git.auto_commit;
                                page.project_save_pending = true;
                            }
                            cx.notify();
                        }),
                    )
                    .child(setting_toggle(auto_commit, theme)),
            ))
            .into_any_element()
    }

    // ── Helpers ──────────────────────────────────────────────────────

    /// Build a path picker with Browse button that opens native file dialog.
    fn render_path_picker(
        &self,
        id: &str,
        current_path: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.workspace.theme();
        let fg: Hsla = theme.foreground.into();
        let muted: Hsla = theme.muted.into();
        let panel_bg: Hsla = theme.panel_background.into();
        let border: Hsla = theme.border.into();
        let accent: Hsla = theme.primary.into();
        let display = if current_path.is_empty() {
            "(not set)".to_string()
        } else {
            current_path.to_string()
        };
        let text_color = if current_path.is_empty() { muted } else { fg };

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .child(
                div()
                    .flex_1()
                    .min_w(px(120.0))
                    .text_color(text_color)
                    .bg(panel_bg)
                    .border_1()
                    .border_color(border)
                    .rounded_md()
                    .px_2()
                    .py_1()
                    .overflow_hidden()
                    .child(display),
            )
            .child(
                div()
                    .id(SharedString::from(format!("{}-browse", id)))
                    .text_color(accent)
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(Hsla { a: 0.1, ..accent }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _window, cx| {
                            // Open native directory picker via App (Context derefs to App)
                            let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
                                files: false,
                                directories: true,
                                multiple: false,
                                prompt: None,
                            });
                            cx.spawn(async move |this, cx| {
                                if let Ok(Ok(Some(paths))) = receiver.await {
                                    if let Some(dir) = paths.into_iter().next() {
                                        let dir_str: String = dir.display().to_string();
                                        let _ = this.update(cx, |this, cx| {
                                            if let Some(page) = this.settings_page.as_mut() {
                                                page.user_settings.general.default_working_dir =
                                                    Some(dir_str);
                                                page.user_save_pending = true;
                                            }
                                            cx.notify();
                                        });
                                    }
                                }
                            })
                            .detach();
                        }),
                    )
                    .child("Browse"),
            )
    }

    /// Build an interactive number stepper with - and + buttons.
    fn number_stepper(
        &self,
        id_prefix: &str,
        value: &str,
        cx: &mut Context<Self>,
        on_dec: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        on_inc: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let theme = self.workspace.theme();
        let fg: Hsla = theme.foreground.into();
        let panel_bg: Hsla = theme.panel_background.into();
        let border: Hsla = theme.border.into();
        let value = value.to_string();

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .child(
                div()
                    .id(SharedString::from(format!("{}-dec", id_prefix)))
                    .text_size(px(14.0))
                    .text_color(fg)
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(Hsla { a: 0.1, ..fg }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            on_dec(this, window, cx);
                        }),
                    )
                    .child("\u{2212}"),
            )
            .child(
                div()
                    .min_w(px(48.0))
                    .text_color(fg)
                    .bg(panel_bg)
                    .border_1()
                    .border_color(border)
                    .rounded_md()
                    .px_2()
                    .py_1()
                    .child(value),
            )
            .child(
                div()
                    .id(SharedString::from(format!("{}-inc", id_prefix)))
                    .text_size(px(14.0))
                    .text_color(fg)
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(Hsla { a: 0.1, ..fg }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            on_inc(this, window, cx);
                        }),
                    )
                    .child("+"),
            )
    }
}

/// Convert snake_case action name to human-readable label.
fn humanize_action_name(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
