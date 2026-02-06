//! Settings page state management.
//!
//! Contains the `SettingsPage` struct that holds the working copy of
//! user and project settings, tracks the active category, and manages
//! dirty detection and reset.

use codirigent_core::config::{ProjectConfig, UserSettings};

/// Settings category for the sidebar navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    /// General preferences (editor, shell, startup).
    General,
    /// Appearance (theme, UI font size, grid gap).
    Appearance,
    /// Terminal (font, cursor, line height).
    Terminal,
    /// Keyboard shortcuts table.
    KeyboardShortcuts,
    /// Session limits and defaults (project-level).
    Sessions,
    /// Advanced (scheduler, verification, git) (project-level).
    Advanced,
}

impl SettingsCategory {
    /// All categories in display order.
    pub const ALL: [SettingsCategory; 6] = [
        SettingsCategory::General,
        SettingsCategory::Appearance,
        SettingsCategory::Terminal,
        SettingsCategory::KeyboardShortcuts,
        SettingsCategory::Sessions,
        SettingsCategory::Advanced,
    ];

    /// Human-readable label for the category.
    pub fn label(&self) -> &'static str {
        match self {
            SettingsCategory::General => "General",
            SettingsCategory::Appearance => "Appearance",
            SettingsCategory::Terminal => "Terminal",
            SettingsCategory::KeyboardShortcuts => "Keyboard Shortcuts",
            SettingsCategory::Sessions => "Sessions",
            SettingsCategory::Advanced => "Advanced",
        }
    }
}

/// Settings page state.
///
/// Holds working copies of user and project settings that are edited
/// in-place. Changes are applied immediately to the running app and
/// persisted via debounced save.
pub struct SettingsPage {
    /// Currently selected category.
    active_category: SettingsCategory,
    /// Working copy of user settings (edited live).
    pub user_settings: UserSettings,
    /// Working copy of project config (edited live).
    pub project_config: ProjectConfig,
    /// Original user settings snapshot for dirty detection and reset.
    original_user: UserSettings,
    /// Original project config snapshot for dirty detection and reset.
    original_project: ProjectConfig,
    /// Which keybinding is currently being recorded (action name).
    pub recording_shortcut: Option<String>,
    /// Which dropdown is currently open (by ID string).
    pub open_dropdown: Option<String>,
    /// Click position (window coordinates) where the dropdown was triggered.
    pub dropdown_click_pos: (f32, f32),
    /// Whether a debounced save for user settings is pending.
    pub user_save_pending: bool,
    /// Whether a debounced save for project config is pending.
    pub project_save_pending: bool,
}

impl SettingsPage {
    /// Create a new settings page with the given configuration.
    pub fn new(user_settings: UserSettings, project_config: ProjectConfig) -> Self {
        Self {
            active_category: SettingsCategory::General,
            original_user: user_settings.clone(),
            original_project: project_config.clone(),
            user_settings,
            project_config,
            recording_shortcut: None,
            open_dropdown: None,
            dropdown_click_pos: (0.0, 0.0),
            user_save_pending: false,
            project_save_pending: false,
        }
    }

    /// Get the active category.
    pub fn active_category(&self) -> SettingsCategory {
        self.active_category
    }

    /// Set the active category.
    pub fn set_category(&mut self, category: SettingsCategory) {
        self.active_category = category;
    }

    /// Check if user settings have been modified.
    pub fn is_user_dirty(&self) -> bool {
        self.user_settings != self.original_user
    }

    /// Check if project config has been modified.
    pub fn is_project_dirty(&self) -> bool {
        self.project_config != self.original_project
    }

    /// Reset user settings for the current category to defaults.
    pub fn reset_user_category(&mut self) {
        let defaults = UserSettings::default();
        match self.active_category {
            SettingsCategory::General => {
                self.user_settings.general = defaults.general;
            }
            SettingsCategory::Appearance => {
                self.user_settings.appearance = defaults.appearance;
            }
            SettingsCategory::Terminal => {
                self.user_settings.terminal = defaults.terminal;
            }
            SettingsCategory::KeyboardShortcuts => {
                self.user_settings.keybindings = defaults.keybindings;
            }
            _ => {}
        }
        self.user_save_pending = true;
    }

    /// Reset project config for the current category to defaults.
    pub fn reset_project_category(&mut self) {
        let defaults = ProjectConfig::default();
        match self.active_category {
            SettingsCategory::Sessions => {
                self.project_config.sessions = defaults.sessions;
            }
            SettingsCategory::Advanced => {
                self.project_config.scheduler = defaults.scheduler;
                self.project_config.verification = defaults.verification;
                self.project_config.git = defaults.git;
            }
            _ => {}
        }
        self.project_save_pending = true;
    }

    /// Mark user settings as saved (snapshot updated).
    pub fn mark_user_saved(&mut self) {
        self.original_user = self.user_settings.clone();
        self.user_save_pending = false;
    }

    /// Mark project config as saved (snapshot updated).
    pub fn mark_project_saved(&mut self) {
        self.original_project = self.project_config.clone();
        self.project_save_pending = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_category_all() {
        assert_eq!(SettingsCategory::ALL.len(), 6);
        assert_eq!(SettingsCategory::ALL[0], SettingsCategory::General);
        assert_eq!(SettingsCategory::ALL[5], SettingsCategory::Advanced);
    }

    #[test]
    fn test_settings_category_label() {
        assert_eq!(SettingsCategory::General.label(), "General");
        assert_eq!(SettingsCategory::Appearance.label(), "Appearance");
        assert_eq!(SettingsCategory::Terminal.label(), "Terminal");
        assert_eq!(SettingsCategory::KeyboardShortcuts.label(), "Keyboard Shortcuts");
        assert_eq!(SettingsCategory::Sessions.label(), "Sessions");
        assert_eq!(SettingsCategory::Advanced.label(), "Advanced");
    }

    #[test]
    fn test_settings_page_new() {
        let page = SettingsPage::new(UserSettings::default(), ProjectConfig::default());
        assert_eq!(page.active_category(), SettingsCategory::General);
        assert!(!page.is_user_dirty());
        assert!(!page.is_project_dirty());
        assert!(page.recording_shortcut.is_none());
    }

    #[test]
    fn test_set_category() {
        let mut page = SettingsPage::new(UserSettings::default(), ProjectConfig::default());
        page.set_category(SettingsCategory::Terminal);
        assert_eq!(page.active_category(), SettingsCategory::Terminal);
    }

    #[test]
    fn test_dirty_detection() {
        let mut page = SettingsPage::new(UserSettings::default(), ProjectConfig::default());
        assert!(!page.is_user_dirty());
        page.user_settings.general.editor_command = "vim".to_string();
        assert!(page.is_user_dirty());
    }

    #[test]
    fn test_project_dirty_detection() {
        let mut page = SettingsPage::new(UserSettings::default(), ProjectConfig::default());
        assert!(!page.is_project_dirty());
        page.project_config.sessions.max_concurrent = 16;
        assert!(page.is_project_dirty());
    }

    #[test]
    fn test_reset_user_category() {
        let mut page = SettingsPage::new(UserSettings::default(), ProjectConfig::default());
        page.user_settings.general.editor_command = "vim".to_string();
        assert!(page.is_user_dirty());
        page.set_category(SettingsCategory::General);
        page.reset_user_category();
        assert_eq!(page.user_settings.general.editor_command, "code");
        assert!(page.user_save_pending);
    }

    #[test]
    fn test_reset_project_category() {
        let mut page = SettingsPage::new(UserSettings::default(), ProjectConfig::default());
        page.project_config.sessions.max_concurrent = 16;
        page.set_category(SettingsCategory::Sessions);
        page.reset_project_category();
        assert_eq!(page.project_config.sessions.max_concurrent, 9);
        assert!(page.project_save_pending);
    }

    #[test]
    fn test_mark_saved() {
        let mut page = SettingsPage::new(UserSettings::default(), ProjectConfig::default());
        page.user_settings.general.editor_command = "vim".to_string();
        page.user_save_pending = true;
        page.mark_user_saved();
        assert!(!page.is_user_dirty());
        assert!(!page.user_save_pending);
    }
}
