//! Interactive settings panels with real dropdown menus and live setting wiring.
//!
//! Each category panel reads current values from `SettingsPage`, builds
//! controls with `cx.listener()` handlers, and applies changes to the
//! running app (theme, terminal cursor, grid gap, etc.) immediately.

use gpui::*;
use std::collections::HashSet;
use std::sync::Arc;

use crate::settings::controls::{setting_row, setting_toggle, settings_section_header};
use crate::settings::SettingsCategory;
use crate::settings::TerminalStyleField;
use crate::terminal_view::CursorShape;

use super::settings_theme_picker::{build_theme_picker_sections, theme_picker_display_label};
use super::types::DROPDOWN_TRIGGER_HEIGHT;

const SETTINGS_DROPDOWN_MAX_HEIGHT: f32 = 280.0;
const TERMINAL_COLOR_PICKER_FALLBACK_SWATCHES: &[&str] = &[
    "#0f172a", "#1e293b", "#334155", "#475569", "#64748b", "#94a3b8", "#cbd5e1", "#f8fafc",
    "#7f1d1d", "#b91c1c", "#dc2626", "#f97316", "#f59e0b", "#eab308", "#65a30d", "#16a34a",
    "#059669", "#0891b2", "#0284c7", "#2563eb", "#4f46e5", "#7c3aed", "#c026d3", "#db2777",
];

#[derive(Clone)]
enum DropdownEntry {
    Option { value: String, label: String },
    Section { label: String },
    Separator,
}

fn build_theme_dropdown_entries(
    theme_manager: &crate::theme_manager::ThemeManager,
) -> Vec<DropdownEntry> {
    build_theme_picker_sections(theme_manager)
        .into_iter()
        .flat_map(|section| section.options)
        .map(|option| DropdownEntry::Option {
            value: option.id,
            label: option.label,
        })
        .collect()
}

fn terminal_style_field_label(field: TerminalStyleField) -> &'static str {
    match field {
        TerminalStyleField::Background => "Background",
        TerminalStyleField::Foreground => "Foreground",
        TerminalStyleField::Cursor => "Cursor color",
        TerminalStyleField::SelectionBackground => "Selection background",
        TerminalStyleField::SelectionForeground => "Selection foreground",
        TerminalStyleField::Black => "Black",
        TerminalStyleField::Red => "Red",
        TerminalStyleField::Green => "Green",
        TerminalStyleField::Yellow => "Yellow",
        TerminalStyleField::Blue => "Blue",
        TerminalStyleField::Magenta => "Magenta",
        TerminalStyleField::Cyan => "Cyan",
        TerminalStyleField::White => "White",
        TerminalStyleField::BrightBlack => "Bright black",
        TerminalStyleField::BrightRed => "Bright red",
        TerminalStyleField::BrightGreen => "Bright green",
        TerminalStyleField::BrightYellow => "Bright yellow",
        TerminalStyleField::BrightBlue => "Bright blue",
        TerminalStyleField::BrightMagenta => "Bright magenta",
        TerminalStyleField::BrightCyan => "Bright cyan",
        TerminalStyleField::BrightWhite => "Bright white",
    }
}

fn terminal_style_field_description(field: TerminalStyleField) -> &'static str {
    match field {
        TerminalStyleField::Background => {
            "Terminal background color. Hex override (#RGB, #RGBA, #RRGGBB, #RRGGBBAA). Empty = theme."
        }
        TerminalStyleField::Foreground => {
            "Default terminal text color for plain output that does not request a specific ANSI color."
        }
        TerminalStyleField::Cursor => {
            "Terminal cursor color. Empty = theme."
        }
        TerminalStyleField::SelectionBackground => {
            "Background color used when text is selected in the terminal."
        }
        TerminalStyleField::SelectionForeground => {
            "Text color used for selected terminal text."
        }
        TerminalStyleField::Black => {
            "Base black used for dark text, borders, and some dim CLI output."
        }
        TerminalStyleField::Red => {
            "ANSI red. Commonly used for errors, failed commands, and git deletions."
        }
        TerminalStyleField::Green => {
            "ANSI green. Commonly used for success, completed steps, and git additions."
        }
        TerminalStyleField::Yellow => {
            "ANSI yellow. Commonly used for warnings, prompts, and caution states."
        }
        TerminalStyleField::Blue => {
            "ANSI blue. Commonly used for links, paths, headings, and command hints."
        }
        TerminalStyleField::Magenta => {
            "ANSI magenta. Often used for accents, categories, or secondary highlights."
        }
        TerminalStyleField::Cyan => {
            "ANSI cyan. Often used for info messages, metadata, or highlighted values."
        }
        TerminalStyleField::White => {
            "Base light text used by tools that explicitly request ANSI white."
        }
        TerminalStyleField::BrightBlack => {
            "Bright black, usually used for dim text, comments, and subtle separators."
        }
        TerminalStyleField::BrightRed => {
            "Bright red for stronger error emphasis and urgent highlights."
        }
        TerminalStyleField::BrightGreen => {
            "Bright green for stronger success emphasis and positive status output."
        }
        TerminalStyleField::BrightYellow => {
            "Bright yellow for stronger warnings and attention-grabbing prompts."
        }
        TerminalStyleField::BrightBlue => {
            "Bright blue for stronger links, headings, and highlighted commands."
        }
        TerminalStyleField::BrightMagenta => {
            "Bright magenta for vivid accents and standout labels."
        }
        TerminalStyleField::BrightCyan => {
            "Bright cyan for vivid info text and highlighted values."
        }
        TerminalStyleField::BrightWhite => {
            "Bright white for the brightest text and high-contrast highlights."
        }
    }
}

fn rgba_to_hex(color: crate::theme::Rgba) -> String {
    if color.a == u8::MAX {
        format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            color.r, color.g, color.b, color.a
        )
    }
}

fn terminal_color_picker_swatches(theme: &crate::theme::CodirigentTheme) -> Vec<String> {
    let mut swatches = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |value: String| {
        if seen.insert(value.clone()) {
            swatches.push(value);
        }
    };

    for field in TerminalStyleField::ALL {
        push(rgba_to_hex(field.theme_color(theme)));
    }

    for value in TERMINAL_COLOR_PICKER_FALLBACK_SWATCHES {
        push((*value).to_string());
    }

    swatches
}

fn terminal_style_picker_dropdown_id(field: TerminalStyleField) -> String {
    format!("term-style-picker-{}", field.id())
}

impl super::gpui::WorkspaceView {
    /// Render the full settings overlay (sidebar + content area).
    pub(super) fn render_settings_overlay(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(page) = self.settings.page.as_ref() else {
            return div()
                .w_full()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(if self.settings.load_task.is_some() {
                    "Loading settings..."
                } else {
                    "Settings not available"
                })
                .into_any_element();
        };
        let theme = self.workspace.theme();
        let bg: Hsla = theme.background.into();
        let panel_bg: Hsla = theme.panel_background.into();
        let fg: Hsla = theme.foreground.into();
        let muted: Hsla = theme.muted.into();
        let primary: Hsla = theme.primary.into();
        let border: Hsla = theme.border.into();
        let active_cat = page.active_category();
        let base_font_size = theme.font_size_base;
        let version_label = format!("v{}", env!("CARGO_PKG_VERSION"));

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
                    this.close_settings(cx);
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
                        if let Some(ref mut page) = this.settings.page {
                            page.set_category(cat);
                            page.open_dropdown = None;
                            page.focused_shortcut_row = None;
                            page.focused_terminal_style_field = None;
                        }
                        cx.notify();
                    }))
                    .child(div().text_color(text_color).child(label)),
            );
        }

        sidebar = sidebar.child(div().flex_1()).child(
            div()
                .px_3()
                .pt_2()
                .pb_1()
                .text_size(px(base_font_size - 1.0))
                .text_color(muted)
                .child(version_label),
        );

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
            .into_any_element()
    }

    /// Dispatch to the active category's render method.
    fn render_settings_content(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let cat = self
            .settings
            .page
            .as_ref()
            .map(|p| p.active_category())
            .unwrap_or(SettingsCategory::Appearance);
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
        let entries = options
            .iter()
            .map(|option| DropdownEntry::Option {
                value: (*option).to_string(),
                label: (*option).to_string(),
            })
            .collect::<Vec<_>>();
        self.render_dropdown_control_with_entries(
            dropdown_id,
            &entries,
            selected,
            selected,
            cx,
            on_select,
        )
    }

    fn render_dropdown_control_with_entries(
        &self,
        dropdown_id: &str,
        entries: &[DropdownEntry],
        selected_value: &str,
        selected_display: &str,
        cx: &mut Context<Self>,
        on_select: impl Fn(&mut Self, String, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let theme = self.workspace.theme();
        let fg: Hsla = theme.foreground.into();
        let muted: Hsla = theme.muted.into();
        let panel_bg: Hsla = theme.panel_background.into();
        let border: Hsla = theme.border.into();
        let hover_bg: Hsla = theme.hover.into();
        let accent: Hsla = theme.primary.into();

        let is_open = self
            .settings
            .page
            .as_ref()
            .and_then(|p| p.open_dropdown.as_deref())
            == Some(dropdown_id);

        let dd_id = dropdown_id.to_string();
        let selected_display = selected_display.to_string();

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
                        if let Some(page) = this.settings.page.as_mut() {
                            page.focused_terminal_style_field = None;
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
                .settings
                .page
                .as_ref()
                .map(|p| p.dropdown_click_pos)
                .unwrap_or((0.0, 0.0));

            let mut options_body = div().flex().flex_col();

            for entry in entries {
                match entry {
                    DropdownEntry::Option { value, label } => {
                        let opt_value = value.clone();
                        let opt_label = label.clone();
                        let is_selected = value == selected_value;
                        let cb = on_select.clone();

                        options_body = options_body.child(
                            div()
                                .id(SharedString::from(format!("{}-opt-{}", dd_id, value)))
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
                                        this.clear_terminal_style_field_focus();
                                        cb(this, opt_value.clone(), window, cx);
                                        if let Some(page) = this.settings.page.as_mut() {
                                            page.open_dropdown = None;
                                        }
                                        cx.notify();
                                    }),
                                )
                                .child(opt_label),
                        );
                    }
                    DropdownEntry::Section { label } => {
                        options_body = options_body.child(
                            div()
                                .px_2()
                                .pt(px(8.0))
                                .pb(px(4.0))
                                .text_xs()
                                .text_color(muted.opacity(0.7))
                                .child(label.clone()),
                        );
                    }
                    DropdownEntry::Separator => {
                        options_body =
                            options_body.child(div().h(px(1.0)).mx_2().my_1().bg(border));
                    }
                }
            }

            let options_list = div()
                .id(SharedString::from(format!("{}-scroll", dd_id)))
                .flex()
                .flex_col()
                .overflow_y_scroll()
                .max_h(px(SETTINGS_DROPDOWN_MAX_HEIGHT))
                .child(options_body);

            let options_panel = div()
                .min_w(px(140.0))
                .bg(panel_bg)
                .border_1()
                .border_color(border)
                .rounded_md()
                .shadow_md()
                .py_1()
                .flex()
                .flex_col()
                .overflow_hidden()
                .child(options_list);

            // Click-away backdrop (closes dropdown when clicking outside)
            let backdrop = div()
                .id(SharedString::from(format!("{}-backdrop", dd_id)))
                .occlude()
                .absolute()
                .inset_0()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.clear_terminal_style_field_focus();
                        if let Some(page) = this.settings.page.as_mut() {
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
                    .child(div().occlude().child(options_panel)),
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
        let page = self
            .settings
            .page
            .as_ref()
            .expect("BUG: settings page should exist when rendering settings");
        let editor = page.user_settings.general.editor_command.clone();
        let shell = page.user_settings.general.default_shell.clone();
        let working_dir = page.user_settings.general.default_working_dir.clone();
        let show_splash = page.user_settings.general.show_splash;
        let restore_cli_on_startup = page.user_settings.general.restore_cli_on_startup;
        let notif = page.user_settings.notifications.clone();
        let theme = self.workspace.theme();

        // Show the current project root when no custom path is configured.
        let display_dir = working_dir
            .clone()
            .or_else(|| {
                self.project
                    .project_root
                    .as_ref()
                    .map(|path| path.display().to_string())
            })
            .unwrap_or_default();

        let editor_options: Vec<&str> = page.detected_editors.iter().map(|s| s.as_str()).collect();

        let shell_sections = self.shell_picker_sections(&page.detected_shells);
        let mut shell_entries = Vec::new();
        for (section_index, section) in shell_sections.iter().enumerate() {
            if section_index > 0 {
                shell_entries.push(DropdownEntry::Separator);
            }
            if let Some(title) = section.title {
                shell_entries.push(DropdownEntry::Section {
                    label: title.to_string(),
                });
            }
            for option in &section.options {
                shell_entries.push(DropdownEntry::Option {
                    value: option.raw_value.clone(),
                    label: option.label.clone(),
                });
            }
        }
        let shell_display = Self::shell_picker_display_label(&shell);

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
                        if let Some(page) = this.settings.page.as_mut() {
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
                self.render_dropdown_control_with_entries(
                    "dd-shell",
                    &shell_entries,
                    &shell,
                    &shell_display,
                    cx,
                    |this, val, _, _| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.general.default_shell = val;
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
                self.render_toggle_control("toggle-splash", show_splash, cx, |this, _, cx| {
                    if let Some(page) = this.settings.page.as_mut() {
                        page.user_settings.general.show_splash =
                            !page.user_settings.general.show_splash;
                        page.user_save_pending = true;
                    }
                    cx.notify();
                }),
            ))
            .child(setting_row(
                "Restore AI sessions",
                "Resume previous Claude/Codex/Gemini sessions on startup",
                theme,
                self.render_toggle_control(
                    "toggle-restore-cli",
                    restore_cli_on_startup,
                    cx,
                    |this, _, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.general.restore_cli_on_startup =
                                !page.user_settings.general.restore_cli_on_startup;
                            page.user_save_pending = true;
                        }
                        cx.notify();
                    },
                ),
            ))
            .child(settings_section_header("Notifications", theme, false))
            .child(setting_row(
                "Desktop notifications",
                "Send OS notifications when an agent responds or needs your input",
                theme,
                self.render_toggle_control(
                    "toggle-notif-desktop",
                    notif.desktop,
                    cx,
                    |this, _, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.notifications.desktop =
                                !page.user_settings.notifications.desktop;
                            page.user_save_pending = true;
                        }
                        cx.notify();
                    },
                ),
            ))
            .child(setting_row(
                "Sound",
                "Play a sound with each notification",
                theme,
                self.render_toggle_control("toggle-notif-sound", notif.sound, cx, |this, _, cx| {
                    if let Some(page) = this.settings.page.as_mut() {
                        page.user_settings.notifications.sound =
                            !page.user_settings.notifications.sound;
                        page.user_save_pending = true;
                    }
                    cx.notify();
                }),
            ))
            .child(settings_section_header("Notification types", theme, false))
            .child(setting_row(
                "Input required",
                "Notify when an agent is waiting for your response",
                theme,
                self.render_toggle_control(
                    "toggle-notif-input-required",
                    notif.input_required,
                    cx,
                    |this, _, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.notifications.input_required =
                                !page.user_settings.notifications.input_required;
                            page.user_save_pending = true;
                        }
                        cx.notify();
                    },
                ),
            ))
            .child(setting_row(
                "Task completed",
                "Notify when a task finishes successfully",
                theme,
                self.render_toggle_control(
                    "toggle-notif-task-completed",
                    notif.task_completed,
                    cx,
                    |this, _, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.notifications.task_completed =
                                !page.user_settings.notifications.task_completed;
                            page.user_save_pending = true;
                        }
                        cx.notify();
                    },
                ),
            ))
            .child(setting_row(
                "Task failed",
                "Notify when a task fails or errors out",
                theme,
                self.render_toggle_control(
                    "toggle-notif-task-failed",
                    notif.task_failed,
                    cx,
                    |this, _, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.notifications.task_failed =
                                !page.user_settings.notifications.task_failed;
                            page.user_save_pending = true;
                        }
                        cx.notify();
                    },
                ),
            ))
            .child(setting_row(
                "Permission prompt",
                "Notify when an agent needs permission to use a tool",
                theme,
                self.render_toggle_control(
                    "toggle-notif-permission",
                    notif.permission_prompt,
                    cx,
                    |this, _, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.notifications.permission_prompt =
                                !page.user_settings.notifications.permission_prompt;
                            page.user_save_pending = true;
                        }
                        cx.notify();
                    },
                ),
            ))
            .child(setting_row(
                "Response ready",
                "Notify when an agent finishes responding in a background session",
                theme,
                self.render_toggle_control(
                    "toggle-notif-response-ready",
                    notif.response_ready,
                    cx,
                    |this, _, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.notifications.response_ready =
                                !page.user_settings.notifications.response_ready;
                            page.user_save_pending = true;
                        }
                        cx.notify();
                    },
                ),
            ))
            .child(setting_row(
                "Error",
                "Notify on unexpected errors",
                theme,
                self.render_toggle_control("toggle-notif-error", notif.error, cx, |this, _, cx| {
                    if let Some(page) = this.settings.page.as_mut() {
                        page.user_settings.notifications.error =
                            !page.user_settings.notifications.error;
                        page.user_save_pending = true;
                    }
                    cx.notify();
                }),
            ))
            .child(settings_section_header("Cooldown", theme, false))
            .child(setting_row(
                "Cooldown per session",
                "Suppress repeated notifications for the same session (0 = no cooldown, max 300s)",
                theme,
                self.number_stepper(
                    "num-notif-cooldown",
                    &format!("{}s", notif.cooldown_seconds),
                    cx,
                    |this, _, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.notifications.cooldown_seconds = page
                                .user_settings
                                .notifications
                                .cooldown_seconds
                                .saturating_sub(5);
                            page.user_save_pending = true;
                        }
                        cx.notify();
                    },
                    |this, _, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.notifications.cooldown_seconds =
                                (page.user_settings.notifications.cooldown_seconds + 5).min(300);
                            page.user_save_pending = true;
                        }
                        cx.notify();
                    },
                ),
            ))
            .into_any_element()
    }

    // ── Appearance ───────────────────────────────────────────────────

    fn render_appearance_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let page = self
            .settings
            .page
            .as_ref()
            .expect("BUG: settings page should exist when rendering settings");
        let theme_id = page.user_settings.appearance.theme.clone();
        let font_size = page.user_settings.appearance.font_size;
        let grid_gap = page.user_settings.appearance.grid_gap;
        let tab_status_style = page.user_settings.appearance.tab_status_style.clone();
        let theme = self.workspace.theme();
        let theme_entries = build_theme_dropdown_entries(&self.settings.theme_manager);
        let selected_theme_label =
            theme_picker_display_label(&self.settings.theme_manager, &theme_id);

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(settings_section_header("Theme", theme, true))
            .child(setting_row(
                "Color theme",
                "Select the active UI and terminal theme",
                theme,
                self.render_dropdown_control_with_entries(
                    "dd-theme",
                    &theme_entries,
                    &theme_id,
                    &selected_theme_label,
                    cx,
                    |this, val, _, _| {
                        let mut user_settings = None;
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.appearance.theme = val.clone();
                            page.user_save_pending = true;
                            user_settings = Some(page.user_settings.clone());
                        }
                        if let Some(user_settings) = user_settings {
                            let resolved_theme_id =
                                this.resolve_and_apply_theme_id(&val, &user_settings);
                            if let Some(page) = this.settings.page.as_mut() {
                                page.user_settings.appearance.theme = resolved_theme_id;
                            }
                        }
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
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.appearance.font_size =
                                (page.user_settings.appearance.font_size - 1.0).max(10.0);
                            page.user_save_pending = true;
                        }
                        let size = this
                            .settings
                            .page
                            .as_ref()
                            .map(|p| p.user_settings.appearance.font_size)
                            .unwrap_or(13.0);
                        this.apply_ui_font_size(size);
                        cx.notify();
                    },
                    |this, _, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.appearance.font_size =
                                (page.user_settings.appearance.font_size + 1.0).min(24.0);
                            page.user_save_pending = true;
                        }
                        let size = this
                            .settings
                            .page
                            .as_ref()
                            .map(|p| p.user_settings.appearance.font_size)
                            .unwrap_or(13.0);
                        this.apply_ui_font_size(size);
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
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.appearance.grid_gap =
                                page.user_settings.appearance.grid_gap.saturating_sub(1);
                            page.user_save_pending = true;
                            this.workspace.theme_mut().grid_gap =
                                page.user_settings.appearance.grid_gap as f32;
                        }
                        cx.notify();
                    },
                    |this, _, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
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
            .child(settings_section_header("Status", theme, false))
            .child(setting_row(
                "Tab status style",
                "How session status is shown on tabs (dot, badge, or glow)",
                theme,
                self.render_dropdown_control(
                    "dd-tab-status-style",
                    &["dot", "badge", "glow"],
                    &tab_status_style,
                    cx,
                    |this, val, _, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.appearance.tab_status_style = val;
                            page.user_save_pending = true;
                        }
                        cx.notify();
                    },
                ),
            ))
            .into_any_element()
    }

    // ── Terminal ─────────────────────────────────────────────────────

    fn render_terminal_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let page = self
            .settings
            .page
            .as_ref()
            .expect("BUG: settings page should exist when rendering settings");
        let font_family = page.user_settings.terminal.font_family.clone();
        let font_size = page.user_settings.terminal.font_size;
        let cursor_style = page.user_settings.terminal.cursor_style.clone();
        let line_height = page.user_settings.terminal.line_height;
        let font_options: Vec<&str> = page.detected_fonts.iter().map(|s| s.as_str()).collect();
        let theme_name = theme_picker_display_label(
            &self.settings.theme_manager,
            &page.user_settings.appearance.theme,
        );
        let theme = self.workspace.theme();
        let mut container = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(settings_section_header("Theme Editor", theme, true))
            .child(
                div()
                    .px_2()
                    .pb_2()
                    .text_sm()
                    .text_color(theme.muted)
                    .child(format!(
                        "Editing terminal colors updates a custom theme derived from {}.",
                        theme_name
                    )),
            )
            .child(setting_row(
                "Preview",
                "Live mock terminal preview using the current theme and terminal color edits.",
                theme,
                self.render_terminal_theme_preview(),
            ))
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
                    |this, val, window, _| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.terminal.font_family = val.clone();
                            page.user_save_pending = true;
                        }
                        this.apply_terminal_font_family(window, val);
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
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.terminal.font_size =
                                (page.user_settings.terminal.font_size - 1.0).max(8.0);
                            page.user_save_pending = true;
                        }
                        let size = this
                            .settings
                            .page
                            .as_ref()
                            .map(|p| p.user_settings.terminal.font_size)
                            .unwrap_or(13.0);
                        this.apply_terminal_font_size(window, size);
                        cx.notify();
                    },
                    |this, window, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.terminal.font_size =
                                (page.user_settings.terminal.font_size + 1.0).min(24.0);
                            page.user_save_pending = true;
                        }
                        let size = this
                            .settings
                            .page
                            .as_ref()
                            .map(|p| p.user_settings.terminal.font_size)
                            .unwrap_or(13.0);
                        this.apply_terminal_font_size(window, size);
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
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.terminal.cursor_style = val.clone();
                            page.user_save_pending = true;
                        }
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
                    |this, window, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.terminal.line_height =
                                (page.user_settings.terminal.line_height - 0.1).max(1.0);
                            page.user_settings.terminal.line_height =
                                (page.user_settings.terminal.line_height * 10.0).round() / 10.0;
                            page.user_save_pending = true;
                            let lh = page.user_settings.terminal.line_height;
                            this.apply_terminal_line_height(window, lh);
                        }
                        cx.notify();
                    },
                    |this, window, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.user_settings.terminal.line_height =
                                (page.user_settings.terminal.line_height + 0.1).min(2.5);
                            page.user_settings.terminal.line_height =
                                (page.user_settings.terminal.line_height * 10.0).round() / 10.0;
                            page.user_save_pending = true;
                            let lh = page.user_settings.terminal.line_height;
                            this.apply_terminal_line_height(window, lh);
                        }
                        cx.notify();
                    },
                ),
            ))
            .child(settings_section_header("Advanced Colors", theme, false));

        for field in TerminalStyleField::BASE {
            container = container.child(setting_row(
                terminal_style_field_label(field),
                terminal_style_field_description(field),
                theme,
                self.render_terminal_style_input_control(field, cx),
            ));
        }

        container = container.child(settings_section_header(
            "Advanced ANSI Palette",
            theme,
            false,
        ));
        for field in TerminalStyleField::ANSI {
            container = container.child(setting_row(
                terminal_style_field_label(field),
                terminal_style_field_description(field),
                theme,
                self.render_terminal_style_input_control(field, cx),
            ));
        }

        container.into_any_element()
    }

    fn render_terminal_theme_preview(&self) -> impl IntoElement {
        let theme = self.workspace.theme();
        let fg: Hsla = theme.terminal_foreground.into();
        let bg: Hsla = theme.terminal_background.into();
        let header_bg: Hsla = theme.header_background.into();
        let selection_bg: Hsla = theme.terminal_selection_bg.into();
        let selection_fg: Hsla = theme.terminal_selection_fg.into();
        let cursor: Hsla = theme.terminal_cursor.into();
        let border: Hsla = theme.border.into();
        let muted: Hsla = theme.muted.into();
        let ansi = theme.ansi.colors;

        let ansi_hsla = |index: usize| -> Hsla { ansi[index].into() };

        div()
            .min_w(px(320.0))
            .bg(bg)
            .border_1()
            .border_color(border)
            .rounded_md()
            .overflow_hidden()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .bg(header_bg)
                    .text_xs()
                    .text_color(muted)
                    .child("codirigent preview terminal"),
            )
            .child(
                div()
                    .px_3()
                    .py_3()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .font_family(theme.terminal_font_family.clone())
                    .text_size(px((theme.terminal_font_size - 1.0).max(11.0)))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_0()
                            .child("$ ")
                            .child(div().text_color(ansi_hsla(10)).child("cargo test"))
                            .child(" ")
                            .child(div().text_color(cursor).child("▉")),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_0()
                            .text_color(ansi_hsla(12))
                            .child("Compiling ")
                            .child(div().text_color(fg).child("codirigent-ui"))
                            .child(format!(" v{}", env!("CARGO_PKG_VERSION"))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_0()
                            .child(div().text_color(ansi_hsla(9)).child("error"))
                            .child(div().text_color(fg).child(": preview mismatch on line 42")),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_0()
                            .child(div().text_color(ansi_hsla(11)).child("warning"))
                            .child(div().text_color(fg).child(": using fallback palette")),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_1()
                            .text_color(fg)
                            .child("Selected ")
                            .child(
                                div()
                                    .bg(selection_bg)
                                    .text_color(selection_fg)
                                    .px_1()
                                    .child("terminal text"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_1()
                            .children((0..16).map(|index| {
                                div()
                                    .w(px(10.0))
                                    .h(px(10.0))
                                    .rounded_sm()
                                    .bg(ansi_hsla(index))
                            })),
                    ),
            )
    }

    // ── Keyboard Shortcuts ───────────────────────────────────────────

    fn render_shortcuts_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let page = self
            .settings
            .page
            .as_ref()
            .expect("BUG: settings page should exist when rendering settings");
        let theme = self.workspace.theme();
        let fg: Hsla = theme.foreground.into();
        let muted: Hsla = theme.muted.into();
        let accent: Hsla = theme.primary.into();
        let panel_bg: Hsla = theme.panel_background.into();
        let border: Hsla = theme.border.into();

        let sorted: Vec<_> = page
            .sorted_shortcut_keys
            .iter()
            .filter_map(|k| page.user_settings.keybindings.get(k).map(|v| (k, v)))
            .collect();

        let recording = page.recording_shortcut.clone();
        let focused_row = page.focused_shortcut_row.clone();

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
            let is_focused = focused_row.as_deref() == Some(action.as_str());
            let display = if is_recording {
                "Press a shortcut...".to_string()
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
                    .bg(if is_focused {
                        Hsla { a: 0.08, ..fg }
                    } else {
                        Hsla {
                            a: 0.0,
                            ..Default::default()
                        }
                    })
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.clear_terminal_style_field_focus();
                            if let Some(page) = this.settings.page.as_mut() {
                                if page.recording_shortcut.as_deref() == Some(&action_name) {
                                    page.recording_shortcut = None;
                                    page.focused_shortcut_row = None;
                                } else {
                                    page.recording_shortcut = Some(action_name.clone());
                                    page.focused_shortcut_row = Some(action_name.clone());
                                }
                            }
                            window.focus(&this.focus_handle(cx));
                            cx.stop_propagation();
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
        let page = self
            .settings
            .page
            .as_ref()
            .expect("BUG: settings page should exist when rendering settings");
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
                        if let Some(page) = this.settings.page.as_mut() {
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
                        if let Some(page) = this.settings.page.as_mut() {
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
                        if let Some(page) = this.settings.page.as_mut() {
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
                            if let Some(page) = this.settings.page.as_mut() {
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
        let page = self
            .settings
            .page
            .as_ref()
            .expect("BUG: settings page should exist when rendering settings");
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
                        if let Some(page) = this.settings.page.as_mut() {
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
                            if let Some(page) = this.settings.page.as_mut() {
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
                            if let Some(page) = this.settings.page.as_mut() {
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
                            if let Some(page) = this.settings.page.as_mut() {
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
                        if let Some(page) = this.settings.page.as_mut() {
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
                        if let Some(page) = this.settings.page.as_mut() {
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
                            if let Some(page) = this.settings.page.as_mut() {
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
                            if let Some(page) = this.settings.page.as_mut() {
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
                        cx.listener(|this, _, _window, cx| {
                            this.clear_terminal_style_field_focus();
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
                                            if let Some(page) = this.settings.page.as_mut() {
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

    fn render_terminal_style_input_control(
        &self,
        field: TerminalStyleField,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.workspace.theme();
        let fg: Hsla = theme.foreground.into();
        let muted: Hsla = theme.muted.into();
        let panel_bg: Hsla = theme.panel_background.into();
        let border: Hsla = theme.border.into();
        let accent: Hsla = theme.primary.into();
        let error: Hsla = crate::sidebar::Color::from_hex("#ef4444").into();
        let (value, base_value, is_focused, is_picker_open, click_pos) = self
            .settings
            .page
            .as_ref()
            .map(|page| {
                let picker_id = terminal_style_picker_dropdown_id(field);
                (
                    page.terminal_style_draft_value(field).to_string(),
                    page.terminal_style_base_value(field).to_string(),
                    page.focused_terminal_style_field == Some(field),
                    page.open_dropdown.as_deref() == Some(picker_id.as_str()),
                    page.dropdown_click_pos,
                )
            })
            .unwrap_or_else(|| (String::new(), String::new(), false, false, (0.0, 0.0)));

        let has_error = !value.trim().is_empty()
            && super::impl_settings::parse_terminal_override_rgba(&value).is_none();
        let preview = super::impl_settings::parse_terminal_override_rgba(&value)
            .unwrap_or_else(|| field.theme_color(theme));
        let preview_hsla: Hsla = preview.into();
        let picker_id = terminal_style_picker_dropdown_id(field);
        let picker_swatches = terminal_color_picker_swatches(theme);
        let display = if value.trim().is_empty() {
            "#RRGGBB".to_string()
        } else {
            value.clone()
        };

        let mut container = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                div()
                    .id(SharedString::from(format!(
                        "term-style-{}-swatch",
                        field.id()
                    )))
                    .w(px(14.0))
                    .h(px(14.0))
                    .rounded_sm()
                    .bg(preview_hsla)
                    .border_1()
                    .border_color(if is_picker_open { accent } else { border })
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let picker_id = picker_id.clone();
                            move |this, event: &MouseDownEvent, window, cx| {
                                this.set_terminal_style_field_focus(field);
                                if let Some(page) = this.settings.page.as_mut() {
                                    page.open_dropdown = Some(picker_id.clone());
                                    page.dropdown_click_pos =
                                        (event.position.x / px(1.0), event.position.y / px(1.0));
                                }
                                window.focus(&this.focus_handle(cx));
                                cx.stop_propagation();
                                cx.notify();
                            }
                        }),
                    ),
            )
            .child(
                div()
                    .id(SharedString::from(format!("term-style-{}", field.id())))
                    .min_w(px(190.0))
                    .h(px(DROPDOWN_TRIGGER_HEIGHT))
                    .px_2()
                    .bg(panel_bg)
                    .border_1()
                    .border_color(if is_focused {
                        accent
                    } else if has_error {
                        error
                    } else {
                        border
                    })
                    .rounded_md()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let picker_id = picker_id.clone();
                            move |this, event: &MouseDownEvent, window, cx| {
                                this.set_terminal_style_field_focus(field);
                                if let Some(page) = this.settings.page.as_mut() {
                                    page.open_dropdown = Some(picker_id.clone());
                                    page.dropdown_click_pos =
                                        (event.position.x / px(1.0), event.position.y / px(1.0));
                                }
                                window.focus(&this.focus_handle(cx));
                                cx.stop_propagation();
                                cx.notify();
                            }
                        }),
                    )
                    .child(
                        div()
                            .text_color(if value.trim().is_empty() || has_error {
                                muted
                            } else {
                                fg
                            })
                            .child(display),
                    ),
            )
            .child(
                div()
                    .id(SharedString::from(format!(
                        "term-style-{}-reset",
                        field.id()
                    )))
                    .text_color(accent)
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|style| style.bg(Hsla { a: 0.08, ..accent }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.set_terminal_style_field_focus(field);
                            this.reset_terminal_style_field(field, cx);
                            if let Some(page) = this.settings.page.as_mut() {
                                page.open_dropdown = None;
                            }
                            window.focus(&this.focus_handle(cx));
                            cx.stop_propagation();
                        }),
                    )
                    .child("Use theme"),
            );

        if is_picker_open {
            let mut swatch_grid = div().flex().flex_wrap().gap_1();
            for swatch in picker_swatches {
                let swatch_value = swatch.clone();
                let swatch_color =
                    super::impl_settings::parse_terminal_override_rgba(&swatch_value)
                        .unwrap_or(preview);
                let swatch_hsla: Hsla = swatch_color.into();
                swatch_grid = swatch_grid.child(
                    div()
                        .id(SharedString::from(format!(
                            "term-style-{}-swatch-{}",
                            field.id(),
                            swatch_value.replace('#', "")
                        )))
                        .w(px(18.0))
                        .h(px(18.0))
                        .rounded_sm()
                        .bg(swatch_hsla)
                        .border_1()
                        .border_color(border)
                        .cursor_pointer()
                        .hover(|style| style.border_color(accent))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.set_terminal_style_field_focus(field);
                                this.update_terminal_style_field_value(
                                    field,
                                    swatch_value.clone(),
                                    cx,
                                );
                                if let Some(page) = this.settings.page.as_mut() {
                                    page.open_dropdown = None;
                                }
                                window.focus(&this.focus_handle(cx));
                                cx.stop_propagation();
                            }),
                        ),
                );
            }

            let picker_panel = div()
                .min_w(px(240.0))
                .bg(panel_bg)
                .border_1()
                .border_color(border)
                .rounded_md()
                .shadow_md()
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(fg)
                        .child(terminal_style_field_label(field)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child("Pick a swatch or type a hex value in the field."),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w(px(28.0))
                                .h(px(28.0))
                                .rounded_md()
                                .bg(preview_hsla)
                                .border_1()
                                .border_color(border),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(div().text_color(fg).child(value.clone()))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child(format!("Theme value: {}", base_value)),
                                ),
                        ),
                )
                .child(swatch_grid);

            let backdrop = div()
                .id(SharedString::from(format!("{}-backdrop", picker_id)))
                .occlude()
                .absolute()
                .inset_0()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        if let Some(page) = this.settings.page.as_mut() {
                            page.open_dropdown = None;
                        }
                        cx.notify();
                    }),
                );

            let overlay = deferred(
                anchored()
                    .anchor(Corner::TopLeft)
                    .position(point(
                        px(click_pos.0),
                        px(click_pos.1 + DROPDOWN_TRIGGER_HEIGHT),
                    ))
                    .snap_to_window_with_margin(px(8.0))
                    .child(div().occlude().child(picker_panel)),
            )
            .with_priority(1);

            container = container
                .child(deferred(backdrop).with_priority(0))
                .child(overlay);
        }

        container
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
                            this.clear_terminal_style_field_focus();
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
                            this.clear_terminal_style_field_focus();
                            on_inc(this, window, cx);
                        }),
                    )
                    .child("+"),
            )
    }

    /// Build an interactive toggle control — a `setting_toggle` wired to a click handler.
    fn render_toggle_control(
        &self,
        id: &str,
        current: bool,
        cx: &mut Context<Self>,
        on_toggle: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let theme = self.workspace.theme();
        div()
            .id(SharedString::from(id.to_string()))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.clear_terminal_style_field_focus();
                    on_toggle(this, window, cx);
                }),
            )
            .child(setting_toggle(current, theme))
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
