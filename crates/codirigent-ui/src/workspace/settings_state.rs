//! Settings state management for WorkspaceView.

use crate::settings::SettingsPage;
use codirigent_core::config_service::DefaultConfigService;

/// Groups all settings-related state for the workspace.
pub(super) struct SettingsState {
    /// Currently active settings page (None if settings closed).
    pub(super) page: Option<SettingsPage>,
    /// Whether the settings panel is open.
    pub(super) open: bool,
    /// Configuration service for reading/writing settings.
    pub(super) config_service: Option<DefaultConfigService>,
}

impl SettingsState {
    pub(super) fn new() -> Self {
        Self {
            page: None,
            open: false,
            config_service: DefaultConfigService::new().ok(),
        }
    }
}
