//! Configuration types for the Codirigent application.
//!
//! This module contains all configuration types used for project-level settings
//! and global user preferences. Configuration is stored in JSON files:
//!
//! - Project config: `.codirigent/config.json`
//! - User settings: `~/.config/dirigent/settings.json`
//!
//! ## Project Configuration
//!
//! Project-level configuration is stored per-project and controls scheduler,
//! verification, session, and git settings.
//!
//! ## User Settings
//!
//! Global user settings control appearance, notifications, keyboard shortcuts,
//! and module-specific preferences.

use crate::scheduler::SchedulerConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Project Configuration
// ============================================================================

/// Project-level configuration stored in `.codirigent/config.json`.
///
/// This configuration is specific to a project and controls how Codirigent
/// behaves when working in that project directory.
///
/// # Example
///
/// ```
/// use codirigent_core::config::ProjectConfig;
///
/// let config = ProjectConfig::default();
/// assert_eq!(config.version, "1.0");
/// assert!(config.scheduler.auto_assign);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectConfig {
    /// Configuration version for migration support.
    pub version: String,
    /// Scheduler settings (reuses SchedulerConfig from scheduler module).
    pub scheduler: SchedulerConfig,
    /// Verification settings.
    pub verification: VerificationSettings,
    /// Session notes settings.
    pub session_notes: SessionNotesConfig,
    /// Session settings.
    pub sessions: SessionsConfig,
    /// Git settings.
    pub git: GitConfig,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            scheduler: SchedulerConfig::default(),
            verification: VerificationSettings::default(),
            session_notes: SessionNotesConfig::default(),
            sessions: SessionsConfig::default(),
            git: GitConfig::default(),
        }
    }
}

// Re-export SchedulerMode from scheduler module for convenience
pub use crate::scheduler::SchedulerMode;

/// Verification configuration for the project.
///
/// Controls automatic verification behavior and commands.
///
/// # Example
///
/// ```
/// use codirigent_core::config::VerificationSettings;
///
/// let config = VerificationSettings::default();
/// assert!(config.enabled);
/// assert!(config.auto_detect);
/// assert_eq!(config.max_retries, 3);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerificationSettings {
    /// Enable automatic verification.
    pub enabled: bool,
    /// Auto-detect test commands based on project type.
    pub auto_detect: bool,
    /// Maximum retry attempts before blocking.
    pub max_retries: u32,
    /// Custom verification commands by category.
    pub commands: HashMap<String, String>,
}

impl Default for VerificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_detect: true,
            max_retries: 3,
            commands: HashMap::new(),
        }
    }
}

/// Session notes configuration.
///
/// Controls the session notes feature for tracking work history.
///
/// # Example
///
/// ```
/// use codirigent_core::config::{SessionNotesConfig, SummaryMode};
///
/// let config = SessionNotesConfig::default();
/// assert!(config.enabled);
/// assert_eq!(config.summary_mode, SummaryMode::Manual);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionNotesConfig {
    /// Enable session notes.
    pub enabled: bool,
    /// Summary generation mode: auto, manual, none.
    pub summary_mode: SummaryMode,
    /// Only record structured data (no AI summary).
    pub structured_data_only: bool,
}

impl Default for SessionNotesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            summary_mode: SummaryMode::Manual,
            structured_data_only: false,
        }
    }
}

/// Summary generation mode for session notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SummaryMode {
    /// AI-generated summaries (uses tokens).
    Auto,
    /// Manual summaries only.
    #[default]
    Manual,
    /// No summaries.
    None,
}

impl std::fmt::Display for SummaryMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SummaryMode::Auto => write!(f, "Auto"),
            SummaryMode::Manual => write!(f, "Manual"),
            SummaryMode::None => write!(f, "None"),
        }
    }
}

/// Session configuration.
///
/// Controls session behavior and limits.
///
/// # Example
///
/// ```
/// use codirigent_core::config::SessionsConfig;
///
/// let config = SessionsConfig::default();
/// assert_eq!(config.max_concurrent, 9);
/// assert_eq!(config.default_cli, "claude");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionsConfig {
    /// Maximum concurrent sessions.
    pub max_concurrent: u32,
    /// Default CLI tool to use.
    pub default_cli: String,
    /// Automatically clean up idle sessions.
    #[serde(default)]
    pub auto_cleanup: bool,
}

impl Default for SessionsConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 9,
            default_cli: "claude".to_string(),
            auto_cleanup: false,
        }
    }
}

/// Git configuration.
///
/// Controls git worktree and commit behavior.
///
/// # Example
///
/// ```
/// use codirigent_core::config::GitConfig;
///
/// let config = GitConfig::default();
/// assert!(config.use_worktrees);
/// assert!(!config.auto_commit);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitConfig {
    /// Use git worktrees for session isolation.
    pub use_worktrees: bool,
    /// Automatically commit changes.
    pub auto_commit: bool,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            use_worktrees: true,
            auto_commit: false,
        }
    }
}

// ============================================================================
// General Settings
// ============================================================================

/// General user preferences (editor, shell, startup).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeneralSettings {
    /// Default external editor command.
    pub editor_command: String,
    /// Default shell for new sessions.
    pub default_shell: String,
    /// Default working directory for new sessions.
    pub default_working_dir: Option<String>,
    /// Show splash screen on startup.
    pub show_splash: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            editor_command: "code".to_string(),
            default_shell: if cfg!(windows) {
                "powershell".to_string()
            } else {
                "bash".to_string()
            },
            default_working_dir: None,
            show_splash: true,
        }
    }
}

// ============================================================================
// Terminal Settings
// ============================================================================

/// Terminal rendering preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerminalSettings {
    /// Font family for terminal rendering.
    pub font_family: String,
    /// Font size in points.
    pub font_size: u32,
    /// Cursor style (block, underline, bar).
    pub cursor_style: String,
    /// Line height multiplier.
    pub line_height: f32,
    /// Color scheme name.
    pub color_scheme: String,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            font_family: "JetBrains Mono".to_string(),
            font_size: 13,
            cursor_style: "block".to_string(),
            line_height: 1.0,
            color_scheme: "default".to_string(),
        }
    }
}

// ============================================================================
// User Settings
// ============================================================================

/// Global user settings stored in `~/.config/dirigent/settings.json`.
///
/// These settings apply across all projects and control user preferences.
///
/// # Example
///
/// ```
/// use codirigent_core::config::UserSettings;
///
/// let settings = UserSettings::default();
/// assert_eq!(settings.appearance.theme, "dark");
/// assert!(settings.notifications.desktop);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserSettings {
    /// General preferences.
    #[serde(default)]
    pub general: GeneralSettings,
    /// Appearance settings.
    pub appearance: AppearanceSettings,
    /// Terminal settings.
    #[serde(default)]
    pub terminal: TerminalSettings,
    /// Notification settings.
    pub notifications: NotificationSettings,
    /// Module-specific settings.
    pub modules: ModuleSettings,
    /// Keyboard shortcuts.
    #[serde(default = "UserSettings::default_keybindings")]
    pub keybindings: HashMap<String, String>,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            general: GeneralSettings::default(),
            appearance: AppearanceSettings::default(),
            terminal: TerminalSettings::default(),
            notifications: NotificationSettings::default(),
            modules: ModuleSettings::default(),
            keybindings: Self::default_keybindings(),
        }
    }
}

impl UserSettings {
    /// Get the default keyboard bindings.
    ///
    /// Returns a map of action names to key combinations.
    pub fn default_keybindings() -> HashMap<String, String> {
        let mut bindings = HashMap::new();
        bindings.insert("switch_session_1".to_string(), "Cmd+1".to_string());
        bindings.insert("switch_session_2".to_string(), "Cmd+2".to_string());
        bindings.insert("switch_session_3".to_string(), "Cmd+3".to_string());
        bindings.insert("switch_session_4".to_string(), "Cmd+4".to_string());
        bindings.insert("new_session".to_string(), "Cmd+N".to_string());
        bindings.insert("close_session".to_string(), "Cmd+W".to_string());
        bindings.insert("quick_switch".to_string(), "Cmd+K".to_string());
        bindings.insert("toggle_layout".to_string(), "Cmd+\\".to_string());
        bindings.insert("toggle_task_board".to_string(), "Cmd+B".to_string());
        bindings
    }
}

/// Appearance settings.
///
/// Controls the visual appearance of the application.
///
/// # Example
///
/// ```
/// use codirigent_core::config::AppearanceSettings;
///
/// let settings = AppearanceSettings::default();
/// assert_eq!(settings.theme, "dark");
/// assert_eq!(settings.font_size, 14);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppearanceSettings {
    /// Theme name: dark, light, or custom.
    pub theme: String,
    /// Font family for terminals.
    pub font_family: String,
    /// Font size in points.
    pub font_size: u32,
    /// Grid gap in pixels.
    pub grid_gap: u32,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            font_family: "JetBrains Mono".to_string(),
            font_size: 14,
            grid_gap: 4,
        }
    }
}

/// Notification settings.
///
/// Controls when and how notifications are displayed.
///
/// # Example
///
/// ```
/// use codirigent_core::config::NotificationSettings;
///
/// let settings = NotificationSettings::default();
/// assert!(settings.desktop);
/// assert!(!settings.sound);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotificationSettings {
    /// Enable desktop notifications.
    pub desktop: bool,
    /// Enable sound notifications.
    pub sound: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            desktop: true,
            sound: false,
        }
    }
}

/// Module-specific settings.
///
/// Contains settings for individual Codirigent modules.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ModuleSettings {
    /// Input detector settings.
    pub input_detector: InputDetectorSettings,
    /// Context tracker settings.
    pub context_tracker: ContextTrackerSettings,
}

/// Input detector settings.
///
/// Controls custom patterns for detecting input prompts.
///
/// # Example
///
/// ```
/// use codirigent_core::config::InputDetectorSettings;
///
/// let settings = InputDetectorSettings::default();
/// assert!(settings.custom_patterns.is_empty());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct InputDetectorSettings {
    /// Custom patterns to detect as input prompts.
    pub custom_patterns: Vec<String>,
}

/// Context tracker settings.
///
/// Controls thresholds for context window tracking.
///
/// # Example
///
/// ```
/// use codirigent_core::config::ContextTrackerSettings;
///
/// let settings = ContextTrackerSettings::default();
/// assert_eq!(settings.warning_threshold, 0.75);
/// assert_eq!(settings.critical_threshold, 0.90);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextTrackerSettings {
    /// Warning threshold (0.0-1.0).
    pub warning_threshold: f32,
    /// Critical threshold (0.0-1.0).
    pub critical_threshold: f32,
}

impl Default for ContextTrackerSettings {
    fn default() -> Self {
        Self {
            warning_threshold: 0.75,
            critical_threshold: 0.90,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ProjectConfig tests

    #[test]
    fn test_project_config_default() {
        let config = ProjectConfig::default();
        assert_eq!(config.version, "1.0");
        assert!(config.scheduler.auto_assign);
        assert_eq!(config.scheduler.mode, SchedulerMode::Smart);
        assert!(config.verification.enabled);
        assert!(config.session_notes.enabled);
        assert_eq!(config.sessions.max_concurrent, 9);
        assert!(config.git.use_worktrees);
    }

    #[test]
    fn test_project_config_serialization() {
        let config = ProjectConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: ProjectConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, config.version);
        assert_eq!(parsed.scheduler.mode, config.scheduler.mode);
    }

    #[test]
    fn test_project_config_custom_values() {
        let config = ProjectConfig {
            version: "2.0".to_string(),
            scheduler: SchedulerConfig {
                auto_assign: false,
                ..Default::default()
            },
            sessions: SessionsConfig {
                max_concurrent: 12,
                ..Default::default()
            },
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: ProjectConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.version, "2.0");
        assert!(!parsed.scheduler.auto_assign);
        assert_eq!(parsed.sessions.max_concurrent, 12);
    }

    #[test]
    fn test_project_config_equality() {
        let config1 = ProjectConfig::default();
        let config2 = ProjectConfig::default();
        let config3 = ProjectConfig {
            version: "2.0".to_string(),
            ..Default::default()
        };

        assert_eq!(config1, config2);
        assert_ne!(config1, config3);
    }

    #[test]
    fn test_project_config_clone() {
        let config = ProjectConfig::default();
        let cloned = config.clone();
        assert_eq!(config, cloned);
    }

    // SchedulerConfig tests (uses scheduler module types)

    #[test]
    fn test_scheduler_config_default() {
        let config = SchedulerConfig::default();
        assert_eq!(config.mode, SchedulerMode::Smart);
        assert!(config.auto_assign);
        assert!(!config.confirm_before_assign);
        assert_eq!(config.idle_threshold_seconds, 5);
    }

    #[test]
    fn test_scheduler_config_in_project() {
        let project_config = ProjectConfig::default();
        assert_eq!(project_config.scheduler.mode, SchedulerMode::Smart);
        assert!(project_config.scheduler.auto_assign);
    }

    // SchedulerMode tests (re-exported from scheduler module)

    #[test]
    fn test_scheduler_mode_default() {
        assert_eq!(SchedulerMode::default(), SchedulerMode::Smart);
    }

    #[test]
    fn test_scheduler_mode_serialization() {
        let mode = SchedulerMode::Smart;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"Smart\"");
    }

    #[test]
    fn test_scheduler_mode_all_variants() {
        let variants = [
            SchedulerMode::Fifo,
            SchedulerMode::Priority,
            SchedulerMode::Dependency,
            SchedulerMode::Smart,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: SchedulerMode = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    // VerificationSettings tests

    #[test]
    fn test_verification_settings_default() {
        let config = VerificationSettings::default();
        assert!(config.enabled);
        assert!(config.auto_detect);
        assert_eq!(config.max_retries, 3);
        assert!(config.commands.is_empty());
    }

    #[test]
    fn test_verification_settings_with_commands() {
        let mut config = VerificationSettings::default();
        config
            .commands
            .insert("unit".to_string(), "cargo test".to_string());
        config
            .commands
            .insert("lint".to_string(), "cargo clippy".to_string());

        let json = serde_json::to_string(&config).unwrap();
        let parsed: VerificationSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.commands.len(), 2);
        assert_eq!(parsed.commands.get("unit"), Some(&"cargo test".to_string()));
    }

    // SessionNotesConfig tests

    #[test]
    fn test_session_notes_config_default() {
        let config = SessionNotesConfig::default();
        assert!(config.enabled);
        assert_eq!(config.summary_mode, SummaryMode::Manual);
        assert!(!config.structured_data_only);
    }

    #[test]
    fn test_session_notes_config_serialization() {
        let config = SessionNotesConfig {
            enabled: false,
            summary_mode: SummaryMode::Auto,
            structured_data_only: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: SessionNotesConfig = serde_json::from_str(&json).unwrap();
        assert!(!parsed.enabled);
        assert_eq!(parsed.summary_mode, SummaryMode::Auto);
        assert!(parsed.structured_data_only);
    }

    // SummaryMode tests

    #[test]
    fn test_summary_mode_default() {
        assert_eq!(SummaryMode::default(), SummaryMode::Manual);
    }

    #[test]
    fn test_summary_mode_all_variants() {
        let variants = [SummaryMode::Auto, SummaryMode::Manual, SummaryMode::None];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: SummaryMode = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_summary_mode_display() {
        assert_eq!(format!("{}", SummaryMode::Auto), "Auto");
        assert_eq!(format!("{}", SummaryMode::Manual), "Manual");
        assert_eq!(format!("{}", SummaryMode::None), "None");
    }

    // SessionsConfig tests

    #[test]
    fn test_sessions_config_default() {
        let config = SessionsConfig::default();
        assert_eq!(config.max_concurrent, 9);
        assert_eq!(config.default_cli, "claude");
        assert!(!config.auto_cleanup);
    }

    #[test]
    fn test_sessions_config_serialization() {
        let config = SessionsConfig {
            max_concurrent: 12,
            default_cli: "codex".to_string(),
            auto_cleanup: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: SessionsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_concurrent, 12);
        assert_eq!(parsed.default_cli, "codex");
        assert!(parsed.auto_cleanup);
    }

    // GitConfig tests

    #[test]
    fn test_git_config_default() {
        let config = GitConfig::default();
        assert!(config.use_worktrees);
        assert!(!config.auto_commit);
    }

    #[test]
    fn test_git_config_serialization() {
        let config = GitConfig {
            use_worktrees: false,
            auto_commit: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: GitConfig = serde_json::from_str(&json).unwrap();
        assert!(!parsed.use_worktrees);
        assert!(parsed.auto_commit);
    }

    // UserSettings tests

    #[test]
    fn test_user_settings_default() {
        let settings = UserSettings::default();
        assert_eq!(settings.appearance.theme, "dark");
        assert!(settings.notifications.desktop);
        assert!(!settings.keybindings.is_empty());
    }

    #[test]
    fn test_user_settings_serialization() {
        let settings = UserSettings::default();
        let json = serde_json::to_string_pretty(&settings).unwrap();
        let parsed: UserSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.appearance.theme, settings.appearance.theme);
        assert_eq!(parsed.keybindings.len(), settings.keybindings.len());
    }

    #[test]
    fn test_default_keybindings() {
        let bindings = UserSettings::default_keybindings();
        assert_eq!(bindings.get("new_session"), Some(&"Cmd+N".to_string()));
        assert_eq!(bindings.get("close_session"), Some(&"Cmd+W".to_string()));
        assert_eq!(bindings.get("quick_switch"), Some(&"Cmd+K".to_string()));
        assert!(bindings.contains_key("toggle_task_board"));
    }

    #[test]
    fn test_user_settings_equality() {
        let settings1 = UserSettings::default();
        let settings2 = UserSettings::default();
        let mut settings3 = UserSettings::default();
        settings3.appearance.theme = "light".to_string();

        assert_eq!(settings1, settings2);
        assert_ne!(settings1, settings3);
    }

    // AppearanceSettings tests

    #[test]
    fn test_appearance_settings_default() {
        let settings = AppearanceSettings::default();
        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.font_family, "JetBrains Mono");
        assert_eq!(settings.font_size, 14);
        assert_eq!(settings.grid_gap, 4);
    }

    #[test]
    fn test_appearance_settings_serialization() {
        let settings = AppearanceSettings {
            theme: "light".to_string(),
            font_family: "Fira Code".to_string(),
            font_size: 16,
            grid_gap: 8,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: AppearanceSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.theme, "light");
        assert_eq!(parsed.font_size, 16);
    }

    // NotificationSettings tests

    #[test]
    fn test_notification_settings_default() {
        let settings = NotificationSettings::default();
        assert!(settings.desktop);
        assert!(!settings.sound);
    }

    #[test]
    fn test_notification_settings_serialization() {
        let settings = NotificationSettings {
            desktop: false,
            sound: true,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: NotificationSettings = serde_json::from_str(&json).unwrap();
        assert!(!parsed.desktop);
        assert!(parsed.sound);
    }

    // ModuleSettings tests

    #[test]
    fn test_module_settings_default() {
        let settings = ModuleSettings::default();
        assert!(settings.input_detector.custom_patterns.is_empty());
        assert_eq!(settings.context_tracker.warning_threshold, 0.75);
    }

    #[test]
    fn test_module_settings_serialization() {
        let mut settings = ModuleSettings::default();
        settings.input_detector.custom_patterns = vec!["[y/n]".to_string()];
        settings.context_tracker.warning_threshold = 0.80;

        let json = serde_json::to_string(&settings).unwrap();
        let parsed: ModuleSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.input_detector.custom_patterns.len(), 1);
        assert_eq!(parsed.context_tracker.warning_threshold, 0.80);
    }

    // InputDetectorSettings tests

    #[test]
    fn test_input_detector_settings_default() {
        let settings = InputDetectorSettings::default();
        assert!(settings.custom_patterns.is_empty());
    }

    #[test]
    fn test_input_detector_settings_with_patterns() {
        let settings = InputDetectorSettings {
            custom_patterns: vec!["[Y/n]".to_string(), "Password:".to_string()],
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: InputDetectorSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.custom_patterns.len(), 2);
    }

    // ContextTrackerSettings tests

    #[test]
    fn test_context_tracker_settings_default() {
        let settings = ContextTrackerSettings::default();
        assert_eq!(settings.warning_threshold, 0.75);
        assert_eq!(settings.critical_threshold, 0.90);
    }

    #[test]
    fn test_context_tracker_settings_serialization() {
        let settings = ContextTrackerSettings {
            warning_threshold: 0.70,
            critical_threshold: 0.85,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: ContextTrackerSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.warning_threshold, 0.70);
        assert_eq!(parsed.critical_threshold, 0.85);
    }

    #[test]
    fn test_context_tracker_settings_equality() {
        let settings1 = ContextTrackerSettings::default();
        let settings2 = ContextTrackerSettings::default();
        assert_eq!(settings1, settings2);
    }
}
