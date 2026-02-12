//! Settings management for WorkspaceView.

use super::editor_detection::detect_installed_editors;
use super::gpui::WorkspaceView;
use crate::app::OpenSettings;
use crate::settings::SettingsPage;
use codirigent_core::config_service::ConfigService;
use gpui::{Context, Window};
use tracing::warn;

impl WorkspaceView {
    pub(super) fn open_settings(&mut self) {
        if self.settings_page.is_none() {
            let mut user_settings = self
                .config_service
                .as_ref()
                .and_then(|cs| cs.load_user_settings().ok())
                .unwrap_or_default();

            let project_config = self
                .config_service
                .as_ref()
                .and_then(|cs| {
                    std::env::current_dir()
                        .ok()
                        .and_then(|cwd| cs.load_project_config(&cwd).ok())
                })
                .unwrap_or_default();

            let default_keys = codirigent_core::config::UserSettings::default_keybindings();
            user_settings
                .keybindings
                .retain(|k, _| default_keys.contains_key(k));
            for (k, v) in &default_keys {
                user_settings
                    .keybindings
                    .entry(k.clone())
                    .or_insert_with(|| v.clone());
            }

            let bg: gpui::Hsla = self.workspace.theme().background.into();
            user_settings.appearance.theme = if bg.l > 0.5 {
                "light".to_string()
            } else {
                "dark".to_string()
            };

            let theme = self.workspace.theme();
            user_settings.appearance.font_size = theme.font_size_base as u32;
            user_settings.appearance.grid_gap = theme.grid_gap as u32;
            user_settings.terminal.font_size = theme.terminal_font_size as u32;

            let mut detected = detect_installed_editors();
            let current = &user_settings.general.editor_command;
            if !detected.iter().any(|e| e == current) {
                detected.insert(0, current.clone());
            }
            if detected.is_empty() {
                detected.push("code".to_string());
            }

            let mut detected_shells = codirigent_session::detect_available_shells();
            let current_shell = &user_settings.general.default_shell;
            if !current_shell.is_empty() && !detected_shells.iter().any(|s| s == current_shell) {
                detected_shells.insert(0, current_shell.clone());
            }
            detected_shells.insert(0, String::new());

            let mut detected_fonts = self.cached_monospace_fonts.clone().unwrap_or_default();
            let current_font = &user_settings.terminal.font_family;
            if !current_font.is_empty() && !detected_fonts.iter().any(|f| f == current_font) {
                detected_fonts.insert(0, current_font.clone());
            }

            self.settings_page = Some(SettingsPage::new(
                user_settings,
                project_config,
                detected,
                detected_shells,
                detected_fonts,
            ));
        }
        self.settings_open = true;
    }

    pub(super) fn close_settings(&mut self) {
        self.flush_settings();
        if let Some(ref mut page) = self.settings_page {
            page.open_dropdown = None;
        }
        self.settings_open = false;
    }

    pub(super) fn flush_settings(&mut self) {
        if let (Some(page), Some(cs)) = (self.settings_page.as_mut(), self.config_service.as_ref())
        {
            if page.user_save_pending {
                if let Err(e) = cs.save_user_settings(&page.user_settings) {
                    warn!("Failed to save user settings: {}", e);
                } else {
                    page.mark_user_saved();
                }
            }
            if page.project_save_pending {
                if let Ok(cwd) = std::env::current_dir() {
                    if let Err(e) = cs.save_project_config(&cwd, &page.project_config) {
                        warn!("Failed to save project config: {}", e);
                    } else {
                        page.mark_project_saved();
                    }
                }
            }
        }
    }

    pub(super) fn handle_open_settings(
        &mut self,
        _action: &OpenSettings,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings();
        cx.notify();
    }
}
