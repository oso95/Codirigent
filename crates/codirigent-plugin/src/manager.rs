//! Default plugin manager implementation.

use crate::registry::PluginRegistry;
use anyhow::{anyhow, Result};
use codirigent_core::plugin::{Plugin, PluginManager, PluginState};
use codirigent_core::traits::EventBus;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Default implementation of the PluginManager trait.
///
/// This manager handles plugin lifecycle including:
/// - Registering built-in plugins
/// - Loading external plugins from disk (future)
/// - Enabling/disabling plugins
/// - Initializing and shutting down plugins
///
/// # Example
///
/// ```ignore
/// use codirigent_plugin::DefaultPluginManager;
/// use std::path::PathBuf;
/// use std::sync::Arc;
///
/// let event_bus = Arc::new(DefaultEventBus::new(16));
/// let mut manager = DefaultPluginManager::new(
///     PathBuf::from("~/.codirigent/plugins"),
///     event_bus,
/// );
///
/// // Register a built-in plugin
/// manager.register_builtin(Box::new(MyPlugin::new()))?;
///
/// // Initialize all loaded plugins
/// manager.initialize_all()?;
/// ```
pub struct DefaultPluginManager {
    registry: PluginRegistry,
    plugins_dir: PathBuf,
    event_bus: Arc<dyn EventBus>,
}

impl DefaultPluginManager {
    /// Create a new plugin manager.
    ///
    /// # Arguments
    ///
    /// * `plugins_dir` - Directory for external plugins
    /// * `event_bus` - Event bus for plugin event handling
    pub fn new(plugins_dir: PathBuf, event_bus: Arc<dyn EventBus>) -> Self {
        Self {
            registry: PluginRegistry::new(),
            plugins_dir,
            event_bus,
        }
    }

    /// Register a built-in plugin directly.
    ///
    /// This bypasses the file-based loading mechanism and registers
    /// a plugin that's compiled into the application.
    ///
    /// # Arguments
    ///
    /// * `plugin` - The plugin to register
    ///
    /// # Errors
    ///
    /// Returns an error if a plugin with the same ID already exists.
    pub fn register_builtin(&mut self, plugin: Box<dyn Plugin>) -> Result<()> {
        self.registry.register(plugin)
    }

    /// Initialize all loaded plugins.
    ///
    /// Plugins in the `Loaded` state will be initialized and moved to `Active`.
    ///
    /// # Errors
    ///
    /// Returns an error if any plugin fails to initialize.
    pub fn initialize_all(&mut self) -> Result<()> {
        for plugin in self.registry.iter_mut() {
            if plugin.state() == PluginState::Loaded {
                plugin.initialize(self.event_bus.as_ref())?;
            }
        }
        Ok(())
    }

    /// Shutdown all active plugins.
    ///
    /// Plugins in the `Active` state will be shut down.
    ///
    /// # Errors
    ///
    /// Returns an error if any plugin fails to shut down.
    pub fn shutdown_all(&mut self) -> Result<()> {
        for plugin in self.registry.iter_mut() {
            if plugin.state() == PluginState::Active {
                plugin.shutdown()?;
            }
        }
        Ok(())
    }

    /// Get the underlying registry for direct access.
    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    /// Get mutable access to the underlying registry.
    pub fn registry_mut(&mut self) -> &mut PluginRegistry {
        &mut self.registry
    }
}

impl PluginManager for DefaultPluginManager {
    fn load_plugin(&mut self, path: &Path) -> Result<String> {
        // Phase 5: External plugin loading via dynamic library
        // For now, return error - built-in plugins use register_builtin
        Err(anyhow!(
            "External plugin loading not yet implemented: {:?}",
            path
        ))
    }

    fn unload_plugin(&mut self, id: &str) -> Result<()> {
        let mut plugin = self.registry.unregister(id)?;
        if plugin.state() == PluginState::Active {
            plugin.shutdown()?;
        }
        Ok(())
    }

    fn enable_plugin(&mut self, id: &str) -> Result<()> {
        let plugin = self
            .registry
            .get_mut(id)
            .ok_or_else(|| anyhow!("Plugin '{}' not found", id))?;
        if plugin.state() == PluginState::Disabled || plugin.state() == PluginState::Loaded {
            plugin.initialize(self.event_bus.as_ref())?;
        }
        Ok(())
    }

    fn disable_plugin(&mut self, id: &str) -> Result<()> {
        let plugin = self
            .registry
            .get_mut(id)
            .ok_or_else(|| anyhow!("Plugin '{}' not found", id))?;
        if plugin.state() == PluginState::Active {
            plugin.shutdown()?;
        }
        Ok(())
    }

    fn get_plugin(&self, id: &str) -> Option<&(dyn Plugin + '_)> {
        self.registry.get(id)
    }

    fn get_plugin_mut(&mut self, id: &str) -> Option<&mut (dyn Plugin + '_)> {
        self.registry.get_mut(id)
    }

    fn list_plugins(&self) -> Vec<&dyn Plugin> {
        self.registry.list()
    }

    fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codirigent_core::events::CodirigentEvent;
    use codirigent_core::plugin::{PluginCapabilities, PluginMetadata};
    use std::any::Any;
    use tokio::sync::broadcast;

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

    /// Mock plugin for testing the manager.
    #[derive(Debug)]
    struct MockPlugin {
        metadata: PluginMetadata,
        state: PluginState,
        #[allow(dead_code)]
        init_count: u32,
        #[allow(dead_code)]
        shutdown_count: u32,
    }

    impl MockPlugin {
        fn new(id: &str) -> Self {
            Self {
                metadata: PluginMetadata {
                    id: id.to_string(),
                    name: format!("{} Plugin", id),
                    version: "0.1.0".to_string(),
                    description: "Mock plugin for testing".to_string(),
                    dependencies: vec![],
                    size_bytes: 0,
                },
                state: PluginState::Loaded,
                init_count: 0,
                shutdown_count: 0,
            }
        }
    }

    impl Plugin for MockPlugin {
        fn metadata(&self) -> &PluginMetadata {
            &self.metadata
        }

        fn capabilities(&self) -> PluginCapabilities {
            PluginCapabilities::default()
        }

        fn initialize(&mut self, _event_bus: &dyn EventBus) -> Result<()> {
            self.init_count += 1;
            self.state = PluginState::Active;
            Ok(())
        }

        fn shutdown(&mut self) -> Result<()> {
            self.shutdown_count += 1;
            self.state = PluginState::Disabled;
            Ok(())
        }

        fn handle_event(&mut self, _event: &CodirigentEvent) {}

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
    fn test_manager_new() {
        let event_bus = Arc::new(MockEventBus::new());
        let manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        assert!(manager.list_plugins().is_empty());
        assert_eq!(manager.plugins_dir(), Path::new("/tmp/plugins"));
    }

    #[test]
    fn test_manager_register_builtin() {
        let event_bus = Arc::new(MockEventBus::new());
        let mut manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        let plugin = MockPlugin::new("test");
        assert!(manager.register_builtin(Box::new(plugin)).is_ok());
        assert!(manager.get_plugin("test").is_some());
    }

    #[test]
    fn test_manager_register_builtin_duplicate() {
        let event_bus = Arc::new(MockEventBus::new());
        let mut manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        let plugin1 = MockPlugin::new("test");
        let plugin2 = MockPlugin::new("test");

        assert!(manager.register_builtin(Box::new(plugin1)).is_ok());
        let result = manager.register_builtin(Box::new(plugin2));
        assert!(result.is_err());
    }

    #[test]
    fn test_manager_initialize_all() {
        let event_bus = Arc::new(MockEventBus::new());
        let mut manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        manager
            .register_builtin(Box::new(MockPlugin::new("plugin1")))
            .unwrap();
        manager
            .register_builtin(Box::new(MockPlugin::new("plugin2")))
            .unwrap();

        assert!(manager.initialize_all().is_ok());

        // Verify all plugins are now active
        for plugin in manager.list_plugins() {
            assert_eq!(plugin.state(), PluginState::Active);
        }
    }

    #[test]
    fn test_manager_shutdown_all() {
        let event_bus = Arc::new(MockEventBus::new());
        let mut manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        manager
            .register_builtin(Box::new(MockPlugin::new("plugin1")))
            .unwrap();
        manager
            .register_builtin(Box::new(MockPlugin::new("plugin2")))
            .unwrap();
        manager.initialize_all().unwrap();

        assert!(manager.shutdown_all().is_ok());

        // Verify all plugins are now disabled
        for plugin in manager.list_plugins() {
            assert_eq!(plugin.state(), PluginState::Disabled);
        }
    }

    #[test]
    fn test_manager_load_plugin_not_implemented() {
        let event_bus = Arc::new(MockEventBus::new());
        let mut manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        let result = manager.load_plugin(Path::new("/path/to/plugin.so"));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not yet implemented"));
    }

    #[test]
    fn test_manager_unload_plugin() {
        let event_bus = Arc::new(MockEventBus::new());
        let mut manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        manager
            .register_builtin(Box::new(MockPlugin::new("test")))
            .unwrap();
        manager.initialize_all().unwrap();

        assert!(manager.unload_plugin("test").is_ok());
        assert!(manager.get_plugin("test").is_none());
    }

    #[test]
    fn test_manager_unload_plugin_not_found() {
        let event_bus = Arc::new(MockEventBus::new());
        let mut manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        let result = manager.unload_plugin("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_manager_enable_plugin() {
        let event_bus = Arc::new(MockEventBus::new());
        let mut manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        manager
            .register_builtin(Box::new(MockPlugin::new("test")))
            .unwrap();

        assert!(manager.enable_plugin("test").is_ok());
        assert_eq!(
            manager.get_plugin("test").unwrap().state(),
            PluginState::Active
        );
    }

    #[test]
    fn test_manager_enable_plugin_not_found() {
        let event_bus = Arc::new(MockEventBus::new());
        let mut manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        let result = manager.enable_plugin("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_manager_disable_plugin() {
        let event_bus = Arc::new(MockEventBus::new());
        let mut manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        manager
            .register_builtin(Box::new(MockPlugin::new("test")))
            .unwrap();
        manager.enable_plugin("test").unwrap();

        assert!(manager.disable_plugin("test").is_ok());
        assert_eq!(
            manager.get_plugin("test").unwrap().state(),
            PluginState::Disabled
        );
    }

    #[test]
    fn test_manager_disable_plugin_not_found() {
        let event_bus = Arc::new(MockEventBus::new());
        let mut manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        let result = manager.disable_plugin("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_manager_get_plugin() {
        let event_bus = Arc::new(MockEventBus::new());
        let mut manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        manager
            .register_builtin(Box::new(MockPlugin::new("test")))
            .unwrap();

        assert!(manager.get_plugin("test").is_some());
        assert!(manager.get_plugin("nonexistent").is_none());
    }

    #[test]
    fn test_manager_get_plugin_mut() {
        let event_bus = Arc::new(MockEventBus::new());
        let mut manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        manager
            .register_builtin(Box::new(MockPlugin::new("test")))
            .unwrap();

        assert!(manager.get_plugin_mut("test").is_some());
        assert!(manager.get_plugin_mut("nonexistent").is_none());
    }

    #[test]
    fn test_manager_list_plugins() {
        let event_bus = Arc::new(MockEventBus::new());
        let mut manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        assert!(manager.list_plugins().is_empty());

        manager
            .register_builtin(Box::new(MockPlugin::new("plugin1")))
            .unwrap();
        manager
            .register_builtin(Box::new(MockPlugin::new("plugin2")))
            .unwrap();

        let plugins = manager.list_plugins();
        assert_eq!(plugins.len(), 2);
    }

    #[test]
    fn test_manager_plugins_dir() {
        let event_bus = Arc::new(MockEventBus::new());
        let manager =
            DefaultPluginManager::new(PathBuf::from("/home/user/.codirigent/plugins"), event_bus);

        assert_eq!(
            manager.plugins_dir(),
            Path::new("/home/user/.codirigent/plugins")
        );
    }

    #[test]
    fn test_manager_registry_access() {
        let event_bus = Arc::new(MockEventBus::new());
        let mut manager = DefaultPluginManager::new(PathBuf::from("/tmp/plugins"), event_bus);

        manager
            .register_builtin(Box::new(MockPlugin::new("test")))
            .unwrap();

        assert!(manager.registry().contains("test"));
        assert!(manager.registry_mut().contains("test"));
    }
}
