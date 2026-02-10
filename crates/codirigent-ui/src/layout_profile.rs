//! Layout profile management for Codirigent.
//!
//! This module provides a manager for saved layout profiles, enabling:
//! - Multiple saved layout configurations
//! - Quick switching between layouts
//! - Custom session arrangements
//! - Keyboard shortcuts for layouts
//!
//! # Example
//!
//! ```
//! use codirigent_ui::layout_profile::{LayoutProfileManager, SavedLayoutProfile};
//! use codirigent_core::LayoutMode;
//!
//! let mut manager = LayoutProfileManager::with_defaults();
//! assert_eq!(manager.active().unwrap().id, "2x2");
//!
//! manager.next_profile();
//! assert_eq!(manager.active().unwrap().id, "1x4");
//! ```

use codirigent_core::{GridPosition, LayoutMode, SessionId};
use serde::{Deserialize, Serialize};

/// A saved layout profile.
///
/// Represents a named layout configuration that can be saved and restored.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedLayoutProfile {
    /// Profile identifier (unique).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Layout mode configuration.
    pub layout: LayoutMode,
    /// Optional fixed session positions.
    pub session_positions: Option<Vec<(SessionId, GridPosition)>>,
    /// Keyboard shortcut to activate this profile.
    pub shortcut: Option<String>,
}

impl SavedLayoutProfile {
    /// Create a new profile with just ID, name, and layout.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier
    /// * `name` - Display name
    /// * `layout` - Layout mode
    pub fn new(id: impl Into<String>, name: impl Into<String>, layout: LayoutMode) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            layout,
            session_positions: None,
            shortcut: None,
        }
    }

    /// Add a keyboard shortcut to this profile.
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Add session positions to this profile.
    pub fn with_positions(mut self, positions: Vec<(SessionId, GridPosition)>) -> Self {
        self.session_positions = Some(positions);
        self
    }

    /// Convert to a SavedLayout for persistence (drops optional fields).
    pub fn to_saved_layout(&self) -> codirigent_core::SavedLayout {
        codirigent_core::SavedLayout {
            id: self.id.clone(),
            name: self.name.clone(),
            layout: self.layout.clone(),
        }
    }

    /// Create from a SavedLayout (with no optional fields).
    pub fn from_saved_layout(saved: codirigent_core::SavedLayout) -> Self {
        Self {
            id: saved.id,
            name: saved.name,
            layout: saved.layout,
            session_positions: None,
            shortcut: None,
        }
    }
}

/// Layout profile manager.
///
/// Manages a collection of saved layout profiles and tracks the active profile.
#[derive(Debug, Clone)]
pub struct LayoutProfileManager {
    /// All saved profiles.
    profiles: Vec<SavedLayoutProfile>,
    /// Currently active profile ID.
    active_profile: Option<String>,
}

impl LayoutProfileManager {
    /// Create an empty manager.
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
            active_profile: None,
        }
    }

    /// Create with built-in default profiles.
    ///
    /// Includes standard grid layouts:
    /// - 2x2 Grid (default)
    /// - 1x4 Horizontal
    /// - 2x3 Grid
    /// - 3x3 Grid
    /// - Single Session
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::layout_profile::LayoutProfileManager;
    ///
    /// let manager = LayoutProfileManager::with_defaults();
    /// assert_eq!(manager.list_profiles().len(), 5);
    /// ```
    pub fn with_defaults() -> Self {
        let profiles = vec![
            SavedLayoutProfile::new("2x2", "2x2 Grid", LayoutMode::Grid { rows: 2, cols: 2 })
                .with_shortcut("Cmd+1"),
            SavedLayoutProfile::new(
                "1x4",
                "1x4 Horizontal",
                LayoutMode::Grid { rows: 1, cols: 4 },
            )
            .with_shortcut("Cmd+2"),
            SavedLayoutProfile::new("2x3", "2x3 Grid", LayoutMode::Grid { rows: 2, cols: 3 })
                .with_shortcut("Cmd+3"),
            SavedLayoutProfile::new("3x3", "3x3 Grid", LayoutMode::Grid { rows: 3, cols: 3 })
                .with_shortcut("Cmd+4"),
            SavedLayoutProfile::new("single", "Single Session", LayoutMode::Single)
                .with_shortcut("Cmd+0"),
        ];

        Self {
            profiles,
            active_profile: Some("2x2".to_string()),
        }
    }

    /// Add a new profile.
    ///
    /// If a profile with the same ID exists, it will be replaced.
    ///
    /// # Arguments
    ///
    /// * `profile` - The profile to add
    pub fn add_profile(&mut self, profile: SavedLayoutProfile) {
        // Remove existing profile with same ID
        self.profiles.retain(|p| p.id != profile.id);
        self.profiles.push(profile);
    }

    /// Remove a profile by ID.
    ///
    /// If the removed profile was active, the first remaining profile
    /// becomes active.
    ///
    /// # Arguments
    ///
    /// * `id` - Profile ID to remove
    pub fn remove_profile(&mut self, id: &str) {
        self.profiles.retain(|p| p.id != id);
        if self.active_profile.as_deref() == Some(id) {
            self.active_profile = self.profiles.first().map(|p| p.id.clone());
        }
    }

    /// Get a profile by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Profile ID to look up
    ///
    /// # Returns
    ///
    /// Reference to the profile, or None if not found.
    pub fn get_profile(&self, id: &str) -> Option<&SavedLayoutProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    /// Get a mutable reference to a profile by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Profile ID to look up
    pub fn get_profile_mut(&mut self, id: &str) -> Option<&mut SavedLayoutProfile> {
        self.profiles.iter_mut().find(|p| p.id == id)
    }

    /// List all profiles.
    ///
    /// # Returns
    ///
    /// Slice of all saved profiles.
    pub fn list_profiles(&self) -> &[SavedLayoutProfile] {
        &self.profiles
    }

    /// Get the number of profiles.
    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    /// Check if there are no profiles.
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// Set the active profile by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Profile ID to activate
    ///
    /// # Returns
    ///
    /// `true` if the profile exists and was activated.
    pub fn set_active(&mut self, id: &str) -> bool {
        if self.profiles.iter().any(|p| p.id == id) {
            self.active_profile = Some(id.to_string());
            true
        } else {
            false
        }
    }

    /// Get the currently active profile.
    ///
    /// # Returns
    ///
    /// Reference to the active profile, or None if no profile is active.
    pub fn active(&self) -> Option<&SavedLayoutProfile> {
        self.active_profile
            .as_ref()
            .and_then(|id| self.get_profile(id))
    }

    /// Get the ID of the active profile.
    pub fn active_id(&self) -> Option<&str> {
        self.active_profile.as_deref()
    }

    /// Quick switch to the next profile.
    ///
    /// Cycles through profiles in order, wrapping around at the end.
    ///
    /// # Returns
    ///
    /// Reference to the newly active profile.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::layout_profile::LayoutProfileManager;
    ///
    /// let mut manager = LayoutProfileManager::with_defaults();
    /// let first = manager.active().unwrap().id.clone();
    ///
    /// manager.next_profile();
    /// let second = manager.active().unwrap().id.clone();
    /// assert_ne!(first, second);
    /// ```
    pub fn next_profile(&mut self) -> Option<&SavedLayoutProfile> {
        if self.profiles.is_empty() {
            return None;
        }

        let current_idx = self
            .active_profile
            .as_ref()
            .and_then(|id| self.profiles.iter().position(|p| &p.id == id))
            .unwrap_or(0);

        let next_idx = (current_idx + 1) % self.profiles.len();
        self.active_profile = Some(self.profiles[next_idx].id.clone());
        self.active()
    }

    /// Quick switch to the previous profile.
    ///
    /// Cycles through profiles in reverse order, wrapping at the beginning.
    ///
    /// # Returns
    ///
    /// Reference to the newly active profile.
    pub fn previous_profile(&mut self) -> Option<&SavedLayoutProfile> {
        if self.profiles.is_empty() {
            return None;
        }

        let current_idx = self
            .active_profile
            .as_ref()
            .and_then(|id| self.profiles.iter().position(|p| &p.id == id))
            .unwrap_or(0);

        let prev_idx = if current_idx == 0 {
            self.profiles.len() - 1
        } else {
            current_idx - 1
        };

        self.active_profile = Some(self.profiles[prev_idx].id.clone());
        self.active()
    }

    /// Find a profile by its keyboard shortcut.
    ///
    /// # Arguments
    ///
    /// * `shortcut` - The keyboard shortcut string
    ///
    /// # Returns
    ///
    /// Reference to the profile with this shortcut, if any.
    pub fn find_by_shortcut(&self, shortcut: &str) -> Option<&SavedLayoutProfile> {
        self.profiles
            .iter()
            .find(|p| p.shortcut.as_deref() == Some(shortcut))
    }

    /// Activate a profile by its keyboard shortcut.
    ///
    /// # Arguments
    ///
    /// * `shortcut` - The keyboard shortcut string
    ///
    /// # Returns
    ///
    /// `true` if a profile with this shortcut was found and activated.
    pub fn activate_by_shortcut(&mut self, shortcut: &str) -> bool {
        if let Some(profile) = self.find_by_shortcut(shortcut) {
            let id = profile.id.clone();
            self.active_profile = Some(id);
            true
        } else {
            false
        }
    }

    /// Get the layout mode of the active profile.
    ///
    /// # Returns
    ///
    /// The active profile's layout mode, or default 2x2 grid if none active.
    pub fn active_layout(&self) -> LayoutMode {
        self.active()
            .map(|p| p.layout.clone())
            .unwrap_or_else(|| LayoutMode::Grid { rows: 2, cols: 2 })
    }
}

impl Default for LayoutProfileManager {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saved_layout_profile_new() {
        let profile = SavedLayoutProfile::new(
            "test",
            "Test Profile",
            LayoutMode::Grid { rows: 2, cols: 2 },
        );
        assert_eq!(profile.id, "test");
        assert_eq!(profile.name, "Test Profile");
        assert!(profile.shortcut.is_none());
        assert!(profile.session_positions.is_none());
    }

    #[test]
    fn test_saved_layout_profile_with_shortcut() {
        let profile =
            SavedLayoutProfile::new("test", "Test", LayoutMode::Single).with_shortcut("Cmd+1");
        assert_eq!(profile.shortcut, Some("Cmd+1".to_string()));
    }

    #[test]
    fn test_saved_layout_profile_with_positions() {
        let positions = vec![(SessionId(1), GridPosition { row: 0, col: 0 })];
        let profile = SavedLayoutProfile::new("test", "Test", LayoutMode::Single)
            .with_positions(positions.clone());
        assert_eq!(profile.session_positions, Some(positions));
    }

    #[test]
    fn test_saved_layout_profile_serialization() {
        let profile =
            SavedLayoutProfile::new("test", "Test", LayoutMode::Grid { rows: 2, cols: 2 })
                .with_shortcut("Cmd+1");
        let json = serde_json::to_string(&profile).unwrap();
        let parsed: SavedLayoutProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(profile, parsed);
    }

    #[test]
    fn test_manager_new() {
        let manager = LayoutProfileManager::new();
        assert!(manager.is_empty());
        assert!(manager.active().is_none());
    }

    #[test]
    fn test_manager_with_defaults() {
        let manager = LayoutProfileManager::with_defaults();
        assert_eq!(manager.len(), 5);
        assert!(manager.active().is_some());
        assert_eq!(manager.active().unwrap().id, "2x2");
    }

    #[test]
    fn test_default_profiles() {
        let manager = LayoutProfileManager::with_defaults();
        assert_eq!(manager.list_profiles().len(), 5);
        assert!(manager.get_profile("2x2").is_some());
        assert!(manager.get_profile("1x4").is_some());
        assert!(manager.get_profile("2x3").is_some());
        assert!(manager.get_profile("3x3").is_some());
        assert!(manager.get_profile("single").is_some());
    }

    #[test]
    fn test_add_profile() {
        let mut manager = LayoutProfileManager::new();
        let profile = SavedLayoutProfile::new("custom", "Custom", LayoutMode::Single);
        manager.add_profile(profile);
        assert_eq!(manager.len(), 1);
        assert!(manager.get_profile("custom").is_some());
    }

    #[test]
    fn test_add_profile_replaces() {
        let mut manager = LayoutProfileManager::new();
        manager.add_profile(SavedLayoutProfile::new("test", "Old", LayoutMode::Single));
        manager.add_profile(SavedLayoutProfile::new(
            "test",
            "New",
            LayoutMode::Grid { rows: 2, cols: 2 },
        ));
        assert_eq!(manager.len(), 1);
        assert_eq!(manager.get_profile("test").unwrap().name, "New");
    }

    #[test]
    fn test_remove_profile() {
        let mut manager = LayoutProfileManager::with_defaults();
        manager.remove_profile("2x2");
        assert!(manager.get_profile("2x2").is_none());
        assert_eq!(manager.len(), 4);
    }

    #[test]
    fn test_remove_active_profile() {
        let mut manager = LayoutProfileManager::with_defaults();
        assert_eq!(manager.active_id(), Some("2x2"));
        manager.remove_profile("2x2");
        // First remaining profile should be active
        assert!(manager.active().is_some());
        assert_eq!(manager.active().unwrap().id, "1x4");
    }

    #[test]
    fn test_get_profile() {
        let manager = LayoutProfileManager::with_defaults();
        let profile = manager.get_profile("2x2").unwrap();
        assert_eq!(profile.name, "2x2 Grid");
    }

    #[test]
    fn test_get_profile_not_found() {
        let manager = LayoutProfileManager::with_defaults();
        assert!(manager.get_profile("nonexistent").is_none());
    }

    #[test]
    fn test_get_profile_mut() {
        let mut manager = LayoutProfileManager::with_defaults();
        if let Some(profile) = manager.get_profile_mut("2x2") {
            profile.name = "Modified".to_string();
        }
        assert_eq!(manager.get_profile("2x2").unwrap().name, "Modified");
    }

    #[test]
    fn test_set_active() {
        let mut manager = LayoutProfileManager::with_defaults();
        assert!(manager.set_active("3x3"));
        assert_eq!(manager.active_id(), Some("3x3"));
    }

    #[test]
    fn test_set_active_nonexistent() {
        let mut manager = LayoutProfileManager::with_defaults();
        assert!(!manager.set_active("nonexistent"));
        assert_eq!(manager.active_id(), Some("2x2")); // Unchanged
    }

    #[test]
    fn test_active_profile() {
        let manager = LayoutProfileManager::with_defaults();
        assert_eq!(manager.active().unwrap().id, "2x2");
    }

    #[test]
    fn test_active_id() {
        let manager = LayoutProfileManager::with_defaults();
        assert_eq!(manager.active_id(), Some("2x2"));
    }

    #[test]
    fn test_next_profile() {
        let mut manager = LayoutProfileManager::with_defaults();
        let first = manager.active().unwrap().id.clone();

        manager.next_profile();
        let second = manager.active().unwrap().id.clone();
        assert_ne!(first, second);
        assert_eq!(second, "1x4");
    }

    #[test]
    fn test_next_profile_wraps() {
        let mut manager = LayoutProfileManager::with_defaults();
        // Go through all profiles
        for _ in 0..5 {
            manager.next_profile();
        }
        // Should wrap back to first
        assert_eq!(manager.active().unwrap().id, "2x2");
    }

    #[test]
    fn test_previous_profile() {
        let mut manager = LayoutProfileManager::with_defaults();
        manager.previous_profile();
        assert_eq!(manager.active().unwrap().id, "single"); // Last profile
    }

    #[test]
    fn test_previous_profile_wraps() {
        let mut manager = LayoutProfileManager::with_defaults();
        manager.set_active("single");
        manager.next_profile();
        assert_eq!(manager.active().unwrap().id, "2x2");
    }

    #[test]
    fn test_next_profile_empty() {
        let mut manager = LayoutProfileManager::new();
        assert!(manager.next_profile().is_none());
    }

    #[test]
    fn test_previous_profile_empty() {
        let mut manager = LayoutProfileManager::new();
        assert!(manager.previous_profile().is_none());
    }

    #[test]
    fn test_find_by_shortcut() {
        let manager = LayoutProfileManager::with_defaults();
        let profile = manager.find_by_shortcut("Cmd+1").unwrap();
        assert_eq!(profile.id, "2x2");
    }

    #[test]
    fn test_find_by_shortcut_not_found() {
        let manager = LayoutProfileManager::with_defaults();
        assert!(manager.find_by_shortcut("Unknown").is_none());
    }

    #[test]
    fn test_activate_by_shortcut() {
        let mut manager = LayoutProfileManager::with_defaults();
        assert!(manager.activate_by_shortcut("Cmd+3"));
        assert_eq!(manager.active_id(), Some("2x3"));
    }

    #[test]
    fn test_activate_by_shortcut_not_found() {
        let mut manager = LayoutProfileManager::with_defaults();
        assert!(!manager.activate_by_shortcut("Unknown"));
        assert_eq!(manager.active_id(), Some("2x2")); // Unchanged
    }

    #[test]
    fn test_active_layout() {
        let manager = LayoutProfileManager::with_defaults();
        let layout = manager.active_layout();
        assert!(matches!(layout, LayoutMode::Grid { rows: 2, cols: 2 }));
    }

    #[test]
    fn test_active_layout_no_active() {
        let manager = LayoutProfileManager::new();
        let layout = manager.active_layout();
        // Default fallback
        assert!(matches!(layout, LayoutMode::Grid { rows: 2, cols: 2 }));
    }

    #[test]
    fn test_manager_default() {
        let manager = LayoutProfileManager::default();
        assert_eq!(manager.len(), 5);
        assert!(manager.active().is_some());
    }

    #[test]
    fn test_manager_clone() {
        let manager = LayoutProfileManager::with_defaults();
        let cloned = manager.clone();
        assert_eq!(manager.len(), cloned.len());
        assert_eq!(manager.active_id(), cloned.active_id());
    }

    #[test]
    fn test_manager_debug() {
        let manager = LayoutProfileManager::with_defaults();
        let debug_str = format!("{:?}", manager);
        assert!(debug_str.contains("LayoutProfileManager"));
    }

    #[test]
    fn test_list_profiles() {
        let manager = LayoutProfileManager::with_defaults();
        let profiles = manager.list_profiles();
        assert_eq!(profiles.len(), 5);
        assert_eq!(profiles[0].id, "2x2");
    }

    #[test]
    fn test_len_and_is_empty() {
        let mut manager = LayoutProfileManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);

        manager.add_profile(SavedLayoutProfile::new("test", "Test", LayoutMode::Single));
        assert!(!manager.is_empty());
        assert_eq!(manager.len(), 1);
    }
}
