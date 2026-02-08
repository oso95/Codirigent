//! Configuration service for loading and saving configuration files.
//!
//! This module provides the [`ConfigService`] trait and [`DefaultConfigService`]
//! implementation for managing project-level and user-level configuration.
//!
//! ## File Locations
//!
//! - Project config: `.codirigent/config.json` (in project directory)
//! - User settings: `~/.config/dirigent/settings.json` (platform-specific)
//!
//! ## Example
//!
//! ```no_run
//! use codirigent_core::config_service::{ConfigService, DefaultConfigService};
//! use std::path::Path;
//!
//! let service = DefaultConfigService::new().unwrap();
//! let project_dir = Path::new("/path/to/project");
//!
//! // Load effective configuration (merged project + user settings)
//! let effective = service.get_effective_config(project_dir).unwrap();
//! println!("Theme: {}", effective.user.appearance.theme);
//! println!("Max sessions: {}", effective.project.sessions.max_concurrent);
//! ```

use crate::config::{ProjectConfig, UserSettings};
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Service for managing configuration files.
///
/// This trait defines the contract for loading and saving configuration
/// from project-level and user-level locations.
///
/// # Implementors
///
/// The primary implementation is [`DefaultConfigService`], which reads
/// and writes JSON files to the filesystem.
pub trait ConfigService: Send + Sync {
    /// Load project configuration from `.codirigent/config.json`.
    ///
    /// If the file doesn't exist, returns default configuration.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Path to the project directory
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be parsed.
    fn load_project_config(&self, project_dir: &Path) -> Result<ProjectConfig>;

    /// Save project configuration to `.codirigent/config.json`.
    ///
    /// Creates the `.codirigent` directory if it doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Path to the project directory
    /// * `config` - Configuration to save
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    fn save_project_config(&self, project_dir: &Path, config: &ProjectConfig) -> Result<()>;

    /// Load global user settings from the user config directory.
    ///
    /// If the file doesn't exist, returns default settings.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be parsed.
    fn load_user_settings(&self) -> Result<UserSettings>;

    /// Save global user settings to the user config directory.
    ///
    /// Creates the config directory if it doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `settings` - Settings to save
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    fn save_user_settings(&self, settings: &UserSettings) -> Result<()>;

    /// Get the merged configuration (user settings + project config).
    ///
    /// Returns both configurations in an [`EffectiveConfig`] struct.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Path to the project directory
    ///
    /// # Errors
    ///
    /// Returns an error if either configuration cannot be loaded.
    fn get_effective_config(&self, project_dir: &Path) -> Result<EffectiveConfig>;

    /// Watch for configuration changes.
    ///
    /// Returns a receiver that will emit events when configuration files change.
    /// Currently a placeholder for future hot-reload support.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Path to the project directory to watch
    ///
    /// # Errors
    ///
    /// Returns an error if the watcher cannot be set up.
    fn watch_config(&self, project_dir: &Path) -> Result<mpsc::Receiver<ConfigChange>>;

    /// Get the user config directory path.
    ///
    /// Returns the path where user settings are stored.
    fn user_config_dir(&self) -> &Path;
}

/// Merged effective configuration.
///
/// Contains both project-level and user-level configuration
/// loaded from their respective files.
///
/// # Example
///
/// ```
/// use codirigent_core::config::{ProjectConfig, UserSettings};
/// use codirigent_core::config_service::EffectiveConfig;
///
/// let config = EffectiveConfig {
///     project: ProjectConfig::default(),
///     user: UserSettings::default(),
/// };
/// assert_eq!(config.project.version, "1.0");
/// assert_eq!(config.user.appearance.theme, "dark");
/// ```
#[derive(Debug, Clone)]
pub struct EffectiveConfig {
    /// Project-level configuration.
    pub project: ProjectConfig,
    /// User-level settings.
    pub user: UserSettings,
}

/// Configuration change event.
///
/// Emitted when a configuration file is modified and reloaded.
#[derive(Debug, Clone)]
pub enum ConfigChange {
    /// Project configuration was changed.
    ProjectConfigChanged(ProjectConfig),
    /// User settings were changed.
    UserSettingsChanged(UserSettings),
}

/// Default implementation of [`ConfigService`].
///
/// Reads and writes configuration files as JSON to the filesystem.
/// Project config is stored in `.codirigent/config.json` relative to
/// the project directory, while user settings are stored in the
/// platform-specific config directory.
///
/// # Example
///
/// ```no_run
/// use codirigent_core::config_service::{ConfigService, DefaultConfigService};
/// use std::path::Path;
///
/// let service = DefaultConfigService::new().unwrap();
/// let project = Path::new("/my/project");
///
/// // Load project config (or defaults if not found)
/// let config = service.load_project_config(project).unwrap();
/// println!("Scheduler mode: {:?}", config.scheduler.mode);
/// ```
pub struct DefaultConfigService {
    user_config_dir: PathBuf,
}

impl DefaultConfigService {
    /// Create a new config service with the default user config directory.
    ///
    /// The user config directory is determined by the platform:
    /// - Linux: `~/.config/dirigent`
    /// - macOS: `~/Library/Application Support/dirigent`
    /// - Windows: `%APPDATA%\dirigent`
    ///
    /// # Errors
    ///
    /// Returns an error if the config directory cannot be determined.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_core::config_service::DefaultConfigService;
    ///
    /// let service = DefaultConfigService::new().unwrap();
    /// ```
    pub fn new() -> Result<Self> {
        let user_config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
            .join("codirigent");
        Ok(Self { user_config_dir })
    }

    /// Create a config service with a custom config directory.
    ///
    /// This is primarily useful for testing.
    ///
    /// # Arguments
    ///
    /// * `config_dir` - Custom path for user configuration
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::config_service::DefaultConfigService;
    /// use std::path::PathBuf;
    ///
    /// let service = DefaultConfigService::with_config_dir(PathBuf::from("/tmp/test-config"));
    /// ```
    pub fn with_config_dir(config_dir: PathBuf) -> Self {
        Self {
            user_config_dir: config_dir,
        }
    }

    /// Get the path to the project config file.
    fn project_config_path(project_dir: &Path) -> PathBuf {
        project_dir.join(".codirigent").join("config.json")
    }

    /// Get the path to the user settings file.
    fn user_settings_path(&self) -> PathBuf {
        self.user_config_dir.join("settings.json")
    }
}

impl ConfigService for DefaultConfigService {
    fn load_project_config(&self, project_dir: &Path) -> Result<ProjectConfig> {
        let path = Self::project_config_path(project_dir);
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let config: ProjectConfig = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            Ok(ProjectConfig::default())
        }
    }

    fn save_project_config(&self, project_dir: &Path, config: &ProjectConfig) -> Result<()> {
        let path = Self::project_config_path(project_dir);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(config)?;
        fs::write(path, json)?;
        Ok(())
    }

    fn load_user_settings(&self) -> Result<UserSettings> {
        let path = self.user_settings_path();
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let settings: UserSettings = serde_json::from_str(&content)?;
            Ok(settings)
        } else {
            Ok(UserSettings::default())
        }
    }

    fn save_user_settings(&self, settings: &UserSettings) -> Result<()> {
        let path = self.user_settings_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(settings)?;
        fs::write(path, json)?;
        Ok(())
    }

    fn get_effective_config(&self, project_dir: &Path) -> Result<EffectiveConfig> {
        Ok(EffectiveConfig {
            project: self.load_project_config(project_dir)?,
            user: self.load_user_settings()?,
        })
    }

    fn watch_config(&self, _project_dir: &Path) -> Result<mpsc::Receiver<ConfigChange>> {
        // Hot-reload placeholder using tokio mpsc channel
        // In the future, this will use the notify crate to watch for file changes
        let (_tx, rx) = mpsc::channel(16);
        Ok(rx)
    }

    fn user_config_dir(&self) -> &Path {
        &self.user_config_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // EffectiveConfig tests

    #[test]
    fn test_effective_config_creation() {
        let config = EffectiveConfig {
            project: ProjectConfig::default(),
            user: UserSettings::default(),
        };
        assert_eq!(config.project.version, "1.0");
        assert_eq!(config.user.appearance.theme, "dark");
    }

    #[test]
    fn test_effective_config_clone() {
        let config = EffectiveConfig {
            project: ProjectConfig::default(),
            user: UserSettings::default(),
        };
        let cloned = config.clone();
        assert_eq!(cloned.project.version, config.project.version);
        assert_eq!(cloned.user.appearance.theme, config.user.appearance.theme);
    }

    #[test]
    fn test_effective_config_debug() {
        let config = EffectiveConfig {
            project: ProjectConfig::default(),
            user: UserSettings::default(),
        };
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("EffectiveConfig"));
        assert!(debug_str.contains("project"));
        assert!(debug_str.contains("user"));
    }

    // ConfigChange tests

    #[test]
    fn test_config_change_project() {
        let change = ConfigChange::ProjectConfigChanged(ProjectConfig::default());
        assert!(matches!(change, ConfigChange::ProjectConfigChanged(_)));
    }

    #[test]
    fn test_config_change_user() {
        let change = ConfigChange::UserSettingsChanged(UserSettings::default());
        assert!(matches!(change, ConfigChange::UserSettingsChanged(_)));
    }

    #[test]
    fn test_config_change_clone() {
        let change = ConfigChange::ProjectConfigChanged(ProjectConfig::default());
        let cloned = change.clone();
        assert!(matches!(cloned, ConfigChange::ProjectConfigChanged(_)));
    }

    #[test]
    fn test_config_change_debug() {
        let change = ConfigChange::ProjectConfigChanged(ProjectConfig::default());
        let debug_str = format!("{:?}", change);
        assert!(debug_str.contains("ProjectConfigChanged"));
    }

    // DefaultConfigService tests

    #[test]
    fn test_with_config_dir() {
        let path = PathBuf::from("/tmp/test-config");
        let service = DefaultConfigService::with_config_dir(path.clone());
        assert_eq!(service.user_config_dir(), path);
    }

    #[test]
    fn test_project_config_path() {
        let project_dir = PathBuf::from("/my/project");
        let path = DefaultConfigService::project_config_path(&project_dir);
        assert_eq!(path, PathBuf::from("/my/project/.codirigent/config.json"));
    }

    #[test]
    fn test_user_settings_path() {
        let service =
            DefaultConfigService::with_config_dir(PathBuf::from("/home/user/.config/dirigent"));
        let path = service.user_settings_path();
        assert_eq!(
            path,
            PathBuf::from("/home/user/.config/dirigent/settings.json")
        );
    }

    #[test]
    fn test_load_default_project_config() {
        let temp = tempdir().unwrap();
        let service = DefaultConfigService::with_config_dir(temp.path().to_path_buf());
        let config = service.load_project_config(temp.path()).unwrap();
        assert_eq!(config.version, "1.0");
        assert!(config.scheduler.auto_assign);
    }

    #[test]
    fn test_save_and_load_project_config() {
        let temp = tempdir().unwrap();
        let service = DefaultConfigService::with_config_dir(temp.path().to_path_buf());

        let mut config = ProjectConfig::default();
        config.sessions.max_concurrent = 10;
        config.scheduler.auto_assign = false;

        service.save_project_config(temp.path(), &config).unwrap();

        // Verify file was created
        let config_path = temp.path().join(".codirigent").join("config.json");
        assert!(config_path.exists());

        let loaded = service.load_project_config(temp.path()).unwrap();
        assert_eq!(loaded.sessions.max_concurrent, 10);
        assert!(!loaded.scheduler.auto_assign);
    }

    #[test]
    fn test_load_default_user_settings() {
        let temp = tempdir().unwrap();
        let service = DefaultConfigService::with_config_dir(temp.path().to_path_buf());
        let settings = service.load_user_settings().unwrap();
        assert_eq!(settings.appearance.theme, "dark");
        assert!(settings.notifications.desktop);
    }

    #[test]
    fn test_save_and_load_user_settings() {
        let temp = tempdir().unwrap();
        let service = DefaultConfigService::with_config_dir(temp.path().to_path_buf());

        let mut settings = UserSettings::default();
        settings.appearance.theme = "light".to_string();
        settings.appearance.font_size = 16;

        service.save_user_settings(&settings).unwrap();

        // Verify file was created
        let settings_path = temp.path().join("settings.json");
        assert!(settings_path.exists());

        let loaded = service.load_user_settings().unwrap();
        assert_eq!(loaded.appearance.theme, "light");
        assert_eq!(loaded.appearance.font_size, 16);
    }

    #[test]
    fn test_effective_config() {
        let temp = tempdir().unwrap();
        let service = DefaultConfigService::with_config_dir(temp.path().to_path_buf());

        // Save custom project config
        let mut project_config = ProjectConfig::default();
        project_config.sessions.max_concurrent = 12;
        service
            .save_project_config(temp.path(), &project_config)
            .unwrap();

        // Save custom user settings
        let mut user_settings = UserSettings::default();
        user_settings.appearance.theme = "light".to_string();
        service.save_user_settings(&user_settings).unwrap();

        // Get effective config
        let effective = service.get_effective_config(temp.path()).unwrap();
        assert_eq!(effective.project.sessions.max_concurrent, 12);
        assert_eq!(effective.user.appearance.theme, "light");
    }

    #[test]
    fn test_effective_config_with_defaults() {
        let temp = tempdir().unwrap();
        let service = DefaultConfigService::with_config_dir(temp.path().to_path_buf());

        // Get effective config without any saved files
        let effective = service.get_effective_config(temp.path()).unwrap();
        assert_eq!(effective.project.version, "1.0");
        assert_eq!(effective.user.appearance.theme, "dark");
    }

    #[test]
    fn test_watch_config_returns_receiver() {
        let temp = tempdir().unwrap();
        let service = DefaultConfigService::with_config_dir(temp.path().to_path_buf());

        let rx = service.watch_config(temp.path()).unwrap();
        // For now, just verify we get a receiver
        drop(rx);
    }

    #[test]
    fn test_creates_directories() {
        let temp = tempdir().unwrap();
        let project_dir = temp.path().join("nested").join("project");
        fs::create_dir_all(&project_dir).unwrap();

        let service = DefaultConfigService::with_config_dir(temp.path().join("config"));

        // Save should create .codirigent directory
        let config = ProjectConfig::default();
        service.save_project_config(&project_dir, &config).unwrap();
        assert!(project_dir.join(".codirigent").exists());

        // Save should create user config directory
        let settings = UserSettings::default();
        service.save_user_settings(&settings).unwrap();
        assert!(temp.path().join("config").exists());
    }

    #[test]
    fn test_load_invalid_json_project_config() {
        let temp = tempdir().unwrap();
        let service = DefaultConfigService::with_config_dir(temp.path().to_path_buf());

        // Write invalid JSON
        let config_dir = temp.path().join(".codirigent");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("config.json"), "not valid json").unwrap();

        let result = service.load_project_config(temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_invalid_json_user_settings() {
        let temp = tempdir().unwrap();
        let service = DefaultConfigService::with_config_dir(temp.path().to_path_buf());

        // Write invalid JSON
        fs::write(temp.path().join("settings.json"), "{ invalid }").unwrap();

        let result = service.load_user_settings();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_service_trait_object_safe() {
        fn _takes_config_service(_: &dyn ConfigService) {}
    }

    #[test]
    fn test_user_config_dir_accessor() {
        let path = PathBuf::from("/custom/config/dir");
        let service = DefaultConfigService::with_config_dir(path.clone());
        assert_eq!(service.user_config_dir(), path.as_path());
    }

    #[test]
    fn test_overwrite_existing_config() {
        let temp = tempdir().unwrap();
        let service = DefaultConfigService::with_config_dir(temp.path().to_path_buf());

        // Save initial config
        let mut config1 = ProjectConfig::default();
        config1.sessions.max_concurrent = 5;
        service.save_project_config(temp.path(), &config1).unwrap();

        // Save updated config
        let mut config2 = ProjectConfig::default();
        config2.sessions.max_concurrent = 15;
        service.save_project_config(temp.path(), &config2).unwrap();

        // Verify updated config is loaded
        let loaded = service.load_project_config(temp.path()).unwrap();
        assert_eq!(loaded.sessions.max_concurrent, 15);
    }
}
