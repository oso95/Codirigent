//! Plugin system types and traits.
//!
//! This module defines the core abstractions for the Codirigent plugin system:
//!
//! - [`PluginMetadata`]: Plugin identity and information
//! - [`PluginCapabilities`]: What the plugin can access
//! - [`PluginState`]: Plugin lifecycle state
//! - [`Plugin`]: Core plugin trait
//! - [`PluginManager`]: Plugin management trait

use crate::events::CodirigentEvent;
use crate::traits::EventBus;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::path::Path;

/// Plugin metadata describing a plugin's identity and capabilities.
///
/// This struct contains all the information needed to identify and
/// describe a plugin to users and other plugins.
///
/// # Example
///
/// ```
/// use codirigent_core::plugin::PluginMetadata;
///
/// let metadata = PluginMetadata {
///     id: "context-tracker".to_string(),
///     name: "Context Tracker".to_string(),
///     version: "1.0.0".to_string(),
///     description: "Tracks context usage in AI sessions".to_string(),
///     dependencies: vec![],
///     size_bytes: 1024,
/// };
///
/// assert_eq!(metadata.id, "context-tracker");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Unique plugin identifier (e.g., "context-tracker").
    ///
    /// This ID must be unique across all plugins and is used
    /// for plugin lookup and dependency resolution.
    pub id: String,

    /// Human-readable name.
    ///
    /// Displayed in the UI and plugin listings.
    pub name: String,

    /// Plugin version (semver format).
    ///
    /// Used for dependency resolution and update checks.
    pub version: String,

    /// Brief description of the plugin's functionality.
    pub description: String,

    /// Dependencies on other plugins by ID.
    ///
    /// The plugin manager ensures these plugins are loaded
    /// before this plugin is initialized.
    pub dependencies: Vec<String>,

    /// Size in bytes (for display purposes).
    pub size_bytes: u64,
}

impl PluginMetadata {
    /// Create new plugin metadata with required fields.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique plugin identifier
    /// * `name` - Human-readable name
    /// * `version` - Semver version string
    /// * `description` - Brief description
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            description: description.into(),
            dependencies: Vec::new(),
            size_bytes: 0,
        }
    }

    /// Add a dependency on another plugin.
    pub fn with_dependency(mut self, dependency: impl Into<String>) -> Self {
        self.dependencies.push(dependency.into());
        self
    }

    /// Set the size in bytes.
    pub fn with_size(mut self, size_bytes: u64) -> Self {
        self.size_bytes = size_bytes;
        self
    }
}

/// Plugin capability flags.
///
/// These flags indicate what resources and functionality a plugin
/// requires access to. The plugin manager can use these to enforce
/// security policies and display permission requests to users.
///
/// # Example
///
/// ```
/// use codirigent_core::plugin::PluginCapabilities;
///
/// let caps = PluginCapabilities::default()
///     .with_session_events(true)
///     .with_pty_output(true);
///
/// assert!(caps.session_events);
/// assert!(caps.pty_output);
/// assert!(!caps.network);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginCapabilities {
    /// Plugin needs access to session events.
    pub session_events: bool,

    /// Plugin needs access to PTY output.
    pub pty_output: bool,

    /// Plugin provides UI components.
    pub ui_components: bool,

    /// Plugin needs file system access.
    pub filesystem: bool,

    /// Plugin needs network access.
    pub network: bool,
}

impl PluginCapabilities {
    /// Create new capabilities with all flags set to false.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the session_events capability.
    pub fn with_session_events(mut self, value: bool) -> Self {
        self.session_events = value;
        self
    }

    /// Set the pty_output capability.
    pub fn with_pty_output(mut self, value: bool) -> Self {
        self.pty_output = value;
        self
    }

    /// Set the ui_components capability.
    pub fn with_ui_components(mut self, value: bool) -> Self {
        self.ui_components = value;
        self
    }

    /// Set the filesystem capability.
    pub fn with_filesystem(mut self, value: bool) -> Self {
        self.filesystem = value;
        self
    }

    /// Set the network capability.
    pub fn with_network(mut self, value: bool) -> Self {
        self.network = value;
        self
    }

    /// Check if any capability is enabled.
    pub fn has_any(&self) -> bool {
        self.session_events
            || self.pty_output
            || self.ui_components
            || self.filesystem
            || self.network
    }

    /// Count the number of enabled capabilities.
    pub fn count(&self) -> usize {
        [
            self.session_events,
            self.pty_output,
            self.ui_components,
            self.filesystem,
            self.network,
        ]
        .iter()
        .filter(|&&v| v)
        .count()
    }
}

/// Plugin lifecycle state.
///
/// Tracks the current state of a plugin through its lifecycle.
///
/// # State Transitions
///
/// ```text
/// Unloaded -> Loaded -> Active
///                |        |
///                v        v
///              Error   Disabled
///                        |
///                        v
///                     Active (re-enable)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PluginState {
    /// Not yet loaded.
    #[default]
    Unloaded,

    /// Loaded and ready to initialize.
    Loaded,

    /// Active and running.
    Active,

    /// Temporarily disabled by user.
    Disabled,

    /// Error state (initialization or runtime failure).
    Error,
}

impl PluginState {
    /// Check if the plugin is in a runnable state.
    pub fn is_runnable(&self) -> bool {
        matches!(self, PluginState::Active)
    }

    /// Check if the plugin can be enabled.
    pub fn can_enable(&self) -> bool {
        matches!(self, PluginState::Loaded | PluginState::Disabled)
    }

    /// Check if the plugin can be disabled.
    pub fn can_disable(&self) -> bool {
        matches!(self, PluginState::Active)
    }

    /// Get a human-readable description of the state.
    pub fn description(&self) -> &'static str {
        match self {
            PluginState::Unloaded => "Not loaded",
            PluginState::Loaded => "Loaded, ready to activate",
            PluginState::Active => "Active and running",
            PluginState::Disabled => "Disabled by user",
            PluginState::Error => "Error occurred",
        }
    }
}

/// Core plugin trait that all plugins must implement.
///
/// This trait defines the contract for plugins in the Codirigent system.
/// Plugins can subscribe to events, provide functionality, and integrate
/// with the session management system.
///
/// # Thread Safety
///
/// Plugins must be `Send + Sync` to allow the plugin manager to
/// store and access them from multiple threads.
///
/// # Example Implementation
///
/// ```ignore
/// use codirigent_core::plugin::{Plugin, PluginMetadata, PluginCapabilities, PluginState};
/// use codirigent_core::events::CodirigentEvent;
/// use codirigent_core::traits::EventBus;
/// use anyhow::Result;
/// use std::any::Any;
///
/// struct MyPlugin {
///     metadata: PluginMetadata,
///     state: PluginState,
/// }
///
/// impl Plugin for MyPlugin {
///     fn metadata(&self) -> &PluginMetadata { &self.metadata }
///     fn capabilities(&self) -> PluginCapabilities { PluginCapabilities::default() }
///     fn initialize(&mut self, _: &dyn EventBus) -> Result<()> {
///         self.state = PluginState::Active;
///         Ok(())
///     }
///     fn shutdown(&mut self) -> Result<()> {
///         self.state = PluginState::Unloaded;
///         Ok(())
///     }
///     fn handle_event(&mut self, _: &CodirigentEvent) {}
///     fn state(&self) -> PluginState { self.state }
///     fn as_any(&self) -> &dyn Any { self }
///     fn as_any_mut(&mut self) -> &mut dyn Any { self }
/// }
/// ```
pub trait Plugin: Send + Sync + std::fmt::Debug {
    /// Get plugin metadata.
    fn metadata(&self) -> &PluginMetadata;

    /// Get plugin capabilities.
    fn capabilities(&self) -> PluginCapabilities;

    /// Initialize the plugin with the event bus.
    ///
    /// Called when the plugin is enabled. The plugin should use this
    /// opportunity to set up any resources and subscribe to events.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails. The plugin will be
    /// put into the Error state.
    fn initialize(&mut self, event_bus: &dyn EventBus) -> Result<()>;

    /// Shutdown the plugin cleanly.
    ///
    /// Called when the plugin is disabled or unloaded. The plugin
    /// should release all resources and unsubscribe from events.
    ///
    /// # Errors
    ///
    /// Returns an error if shutdown fails.
    fn shutdown(&mut self) -> Result<()>;

    /// Handle an event from the event bus.
    ///
    /// Called for each event that the plugin has subscribed to.
    /// The plugin should handle the event quickly and not block.
    fn handle_event(&mut self, event: &CodirigentEvent);

    /// Get the plugin's current state.
    fn state(&self) -> PluginState;

    /// Cast to Any for downcasting to concrete type.
    ///
    /// This enables accessing plugin-specific methods when needed.
    fn as_any(&self) -> &dyn Any;

    /// Cast to mutable Any for downcasting to concrete type.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Plugin manager for loading, unloading, and managing plugins.
///
/// This trait defines the contract for plugin management. The manager
/// is responsible for:
/// - Loading plugins from disk
/// - Managing plugin lifecycle (enable/disable)
/// - Providing access to loaded plugins
///
/// # Thread Safety
///
/// Plugin managers must be `Send + Sync` to allow access from
/// multiple threads.
pub trait PluginManager: Send + Sync {
    /// Load a plugin from the specified path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the plugin file (e.g., .so, .dll, .dylib)
    ///
    /// # Returns
    ///
    /// The plugin ID on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin cannot be loaded.
    fn load_plugin(&mut self, path: &Path) -> Result<String>;

    /// Unload a plugin by ID.
    ///
    /// This shuts down the plugin if active and removes it from
    /// the manager.
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin doesn't exist or shutdown fails.
    fn unload_plugin(&mut self, id: &str) -> Result<()>;

    /// Enable a loaded plugin.
    ///
    /// This initializes the plugin and moves it to the Active state.
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin doesn't exist or initialization fails.
    fn enable_plugin(&mut self, id: &str) -> Result<()>;

    /// Disable a plugin without unloading.
    ///
    /// This shuts down the plugin and moves it to the Disabled state.
    /// The plugin can be re-enabled later.
    ///
    /// # Errors
    ///
    /// Returns an error if the plugin doesn't exist or shutdown fails.
    fn disable_plugin(&mut self, id: &str) -> Result<()>;

    /// Get a reference to a plugin by ID.
    fn get_plugin(&self, id: &str) -> Option<&(dyn Plugin + '_)>;

    /// Get a mutable reference to a plugin by ID.
    fn get_plugin_mut(&mut self, id: &str) -> Option<&mut (dyn Plugin + '_)>;

    /// List all loaded plugins.
    fn list_plugins(&self) -> Vec<&dyn Plugin>;

    /// Get plugins directory path.
    fn plugins_dir(&self) -> &Path;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    // === PluginMetadata Tests ===

    #[test]
    fn test_plugin_metadata_creation() {
        let meta = PluginMetadata {
            id: "test-plugin".to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "A test plugin".to_string(),
            dependencies: vec![],
            size_bytes: 1024,
        };
        assert_eq!(meta.id, "test-plugin");
        assert_eq!(meta.name, "Test Plugin");
        assert_eq!(meta.version, "1.0.0");
        assert_eq!(meta.description, "A test plugin");
        assert!(meta.dependencies.is_empty());
        assert_eq!(meta.size_bytes, 1024);
    }

    #[test]
    fn test_plugin_metadata_new() {
        let meta = PluginMetadata::new("my-plugin", "My Plugin", "2.0.0", "Description here");
        assert_eq!(meta.id, "my-plugin");
        assert_eq!(meta.name, "My Plugin");
        assert_eq!(meta.version, "2.0.0");
        assert_eq!(meta.description, "Description here");
        assert!(meta.dependencies.is_empty());
        assert_eq!(meta.size_bytes, 0);
    }

    #[test]
    fn test_plugin_metadata_with_dependency() {
        let meta = PluginMetadata::new("my-plugin", "My Plugin", "1.0.0", "Desc")
            .with_dependency("core-plugin")
            .with_dependency("ui-plugin");

        assert_eq!(meta.dependencies.len(), 2);
        assert_eq!(meta.dependencies[0], "core-plugin");
        assert_eq!(meta.dependencies[1], "ui-plugin");
    }

    #[test]
    fn test_plugin_metadata_with_size() {
        let meta = PluginMetadata::new("my-plugin", "My Plugin", "1.0.0", "Desc").with_size(4096);
        assert_eq!(meta.size_bytes, 4096);
    }

    #[test]
    fn test_plugin_metadata_clone() {
        let meta = PluginMetadata::new("test", "Test", "1.0.0", "Desc");
        let cloned = meta.clone();
        assert_eq!(meta.id, cloned.id);
    }

    #[test]
    fn test_plugin_metadata_debug() {
        let meta = PluginMetadata::new("test", "Test", "1.0.0", "Desc");
        let debug_str = format!("{:?}", meta);
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_plugin_metadata_serialize() {
        let meta = PluginMetadata::new("test", "Test", "1.0.0", "Desc")
            .with_dependency("dep1")
            .with_size(1024);

        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("\"id\":\"test\""));
        assert!(json.contains("\"dep1\""));

        let deserialized: PluginMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "test");
        assert_eq!(deserialized.dependencies.len(), 1);
    }

    // === PluginCapabilities Tests ===

    #[test]
    fn test_plugin_capabilities_default() {
        let caps = PluginCapabilities::default();
        assert!(!caps.session_events);
        assert!(!caps.pty_output);
        assert!(!caps.ui_components);
        assert!(!caps.filesystem);
        assert!(!caps.network);
    }

    #[test]
    fn test_plugin_capabilities_new() {
        let caps = PluginCapabilities::new();
        assert!(!caps.session_events);
        assert!(!caps.network);
    }

    #[test]
    fn test_plugin_capabilities_with_session_events() {
        let caps = PluginCapabilities::default().with_session_events(true);
        assert!(caps.session_events);
        assert!(!caps.pty_output);
    }

    #[test]
    fn test_plugin_capabilities_with_pty_output() {
        let caps = PluginCapabilities::default().with_pty_output(true);
        assert!(caps.pty_output);
        assert!(!caps.session_events);
    }

    #[test]
    fn test_plugin_capabilities_with_ui_components() {
        let caps = PluginCapabilities::default().with_ui_components(true);
        assert!(caps.ui_components);
    }

    #[test]
    fn test_plugin_capabilities_with_filesystem() {
        let caps = PluginCapabilities::default().with_filesystem(true);
        assert!(caps.filesystem);
    }

    #[test]
    fn test_plugin_capabilities_with_network() {
        let caps = PluginCapabilities::default().with_network(true);
        assert!(caps.network);
    }

    #[test]
    fn test_plugin_capabilities_chaining() {
        let caps = PluginCapabilities::default()
            .with_session_events(true)
            .with_pty_output(true)
            .with_network(true);

        assert!(caps.session_events);
        assert!(caps.pty_output);
        assert!(!caps.ui_components);
        assert!(!caps.filesystem);
        assert!(caps.network);
    }

    #[test]
    fn test_plugin_capabilities_has_any() {
        let empty = PluginCapabilities::default();
        assert!(!empty.has_any());

        let with_one = PluginCapabilities::default().with_network(true);
        assert!(with_one.has_any());
    }

    #[test]
    fn test_plugin_capabilities_count() {
        let empty = PluginCapabilities::default();
        assert_eq!(empty.count(), 0);

        let with_two = PluginCapabilities::default()
            .with_session_events(true)
            .with_network(true);
        assert_eq!(with_two.count(), 2);

        let all = PluginCapabilities {
            session_events: true,
            pty_output: true,
            ui_components: true,
            filesystem: true,
            network: true,
        };
        assert_eq!(all.count(), 5);
    }

    #[test]
    fn test_plugin_capabilities_equality() {
        let caps1 = PluginCapabilities::default().with_network(true);
        let caps2 = PluginCapabilities::default().with_network(true);
        let caps3 = PluginCapabilities::default().with_filesystem(true);

        assert_eq!(caps1, caps2);
        assert_ne!(caps1, caps3);
    }

    #[test]
    fn test_plugin_capabilities_clone() {
        let caps = PluginCapabilities::default().with_network(true);
        let cloned = caps;
        assert_eq!(caps, cloned);
    }

    #[test]
    fn test_plugin_capabilities_serialize() {
        let caps = PluginCapabilities::default()
            .with_session_events(true)
            .with_network(true);

        let json = serde_json::to_string(&caps).unwrap();
        assert!(json.contains("\"session_events\":true"));
        assert!(json.contains("\"network\":true"));
        assert!(json.contains("\"filesystem\":false"));

        let deserialized: PluginCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, caps);
    }

    // === PluginState Tests ===

    #[test]
    fn test_plugin_state_default() {
        assert_eq!(PluginState::default(), PluginState::Unloaded);
    }

    #[test]
    fn test_plugin_state_is_runnable() {
        assert!(!PluginState::Unloaded.is_runnable());
        assert!(!PluginState::Loaded.is_runnable());
        assert!(PluginState::Active.is_runnable());
        assert!(!PluginState::Disabled.is_runnable());
        assert!(!PluginState::Error.is_runnable());
    }

    #[test]
    fn test_plugin_state_can_enable() {
        assert!(!PluginState::Unloaded.can_enable());
        assert!(PluginState::Loaded.can_enable());
        assert!(!PluginState::Active.can_enable());
        assert!(PluginState::Disabled.can_enable());
        assert!(!PluginState::Error.can_enable());
    }

    #[test]
    fn test_plugin_state_can_disable() {
        assert!(!PluginState::Unloaded.can_disable());
        assert!(!PluginState::Loaded.can_disable());
        assert!(PluginState::Active.can_disable());
        assert!(!PluginState::Disabled.can_disable());
        assert!(!PluginState::Error.can_disable());
    }

    #[test]
    fn test_plugin_state_description() {
        assert_eq!(PluginState::Unloaded.description(), "Not loaded");
        assert_eq!(
            PluginState::Loaded.description(),
            "Loaded, ready to activate"
        );
        assert_eq!(PluginState::Active.description(), "Active and running");
        assert_eq!(PluginState::Disabled.description(), "Disabled by user");
        assert_eq!(PluginState::Error.description(), "Error occurred");
    }

    #[test]
    fn test_plugin_state_equality() {
        assert_eq!(PluginState::Active, PluginState::Active);
        assert_ne!(PluginState::Active, PluginState::Disabled);
    }

    #[test]
    fn test_plugin_state_clone() {
        let state = PluginState::Active;
        let cloned = state;
        assert_eq!(state, cloned);
    }

    #[test]
    fn test_plugin_state_debug() {
        let debug_str = format!("{:?}", PluginState::Active);
        assert_eq!(debug_str, "Active");
    }

    #[test]
    fn test_plugin_state_serialize() {
        let state = PluginState::Active;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"Active\"");

        let deserialized: PluginState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, PluginState::Active);
    }

    // === Plugin Trait Tests ===

    /// Mock event bus for testing.
    struct MockEventBus {
        sender: broadcast::Sender<CodirigentEvent>,
    }

    impl MockEventBus {
        fn new() -> Self {
            let (sender, _) = broadcast::channel(16);
            Self { sender }
        }
    }

    impl EventBus for MockEventBus {
        fn subscribe(&self) -> broadcast::Receiver<CodirigentEvent> {
            self.sender.subscribe()
        }

        fn publish(&self, event: CodirigentEvent) {
            let _ = self.sender.send(event);
        }
    }

    /// Mock plugin for testing trait implementations.
    #[derive(Debug)]
    struct MockPlugin {
        metadata: PluginMetadata,
        state: PluginState,
        event_count: u32,
    }

    impl MockPlugin {
        fn new(id: &str) -> Self {
            Self {
                metadata: PluginMetadata::new(id, format!("{} Plugin", id), "0.1.0", "Mock plugin"),
                state: PluginState::Unloaded,
                event_count: 0,
            }
        }
    }

    impl Plugin for MockPlugin {
        fn metadata(&self) -> &PluginMetadata {
            &self.metadata
        }

        fn capabilities(&self) -> PluginCapabilities {
            PluginCapabilities::default().with_session_events(true)
        }

        fn initialize(&mut self, _event_bus: &dyn EventBus) -> Result<()> {
            self.state = PluginState::Active;
            Ok(())
        }

        fn shutdown(&mut self) -> Result<()> {
            self.state = PluginState::Unloaded;
            Ok(())
        }

        fn handle_event(&mut self, _event: &CodirigentEvent) {
            self.event_count += 1;
        }

        fn state(&self) -> PluginState {
            self.state
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn test_mock_plugin_lifecycle() {
        let mut plugin = MockPlugin::new("test");
        assert_eq!(plugin.state(), PluginState::Unloaded);
        assert_eq!(plugin.metadata().id, "test");

        let bus = MockEventBus::new();
        plugin.initialize(&bus).unwrap();
        assert_eq!(plugin.state(), PluginState::Active);

        plugin.shutdown().unwrap();
        assert_eq!(plugin.state(), PluginState::Unloaded);
    }

    #[test]
    fn test_mock_plugin_capabilities() {
        let plugin = MockPlugin::new("test");
        let caps = plugin.capabilities();
        assert!(caps.session_events);
        assert!(!caps.network);
    }

    #[test]
    fn test_mock_plugin_handle_event() {
        use crate::types::SessionId;

        let mut plugin = MockPlugin::new("test");
        assert_eq!(plugin.event_count, 0);

        plugin.handle_event(&CodirigentEvent::SessionCreated { id: SessionId(1) });
        assert_eq!(plugin.event_count, 1);

        plugin.handle_event(&CodirigentEvent::SessionClosed { id: SessionId(1) });
        assert_eq!(plugin.event_count, 2);
    }

    #[test]
    fn test_mock_plugin_as_any() {
        let plugin = MockPlugin::new("test");
        let any = plugin.as_any();
        let downcasted = any.downcast_ref::<MockPlugin>();
        assert!(downcasted.is_some());
        assert_eq!(downcasted.unwrap().metadata.id, "test");
    }

    #[test]
    fn test_mock_plugin_as_any_mut() {
        let mut plugin = MockPlugin::new("test");
        let any = plugin.as_any_mut();
        let downcasted = any.downcast_mut::<MockPlugin>();
        assert!(downcasted.is_some());
        downcasted.unwrap().event_count = 42;
        assert_eq!(plugin.event_count, 42);
    }

    // === Plugin Trait Object Safety Tests ===

    #[test]
    fn test_plugin_trait_is_object_safe() {
        fn _takes_plugin(_: &dyn Plugin) {}
    }

    #[test]
    fn test_plugin_manager_trait_is_object_safe() {
        fn _takes_manager(_: &dyn PluginManager) {}
    }

    #[test]
    fn test_plugin_can_be_boxed() {
        let plugin: Box<dyn Plugin> = Box::new(MockPlugin::new("test"));
        assert_eq!(plugin.metadata().id, "test");
    }

    #[test]
    fn test_plugin_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockPlugin>();
    }
}
