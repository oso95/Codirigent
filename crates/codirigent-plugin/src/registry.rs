//! Plugin registry for storing and looking up plugins.

use anyhow::{anyhow, Result};
use codirigent_core::plugin::{Plugin, PluginMetadata};
use std::collections::HashMap;

/// Registry for storing and looking up plugins.
///
/// The registry manages plugin storage using a HashMap keyed by plugin ID.
/// It provides methods for registering, unregistering, and querying plugins.
///
/// # Example
///
/// ```ignore
/// use codirigent_plugin::PluginRegistry;
///
/// let mut registry = PluginRegistry::new();
/// registry.register(Box::new(my_plugin))?;
/// let plugin = registry.get("my-plugin");
/// ```
pub struct PluginRegistry {
    plugins: HashMap<String, Box<dyn Plugin>>,
}

impl PluginRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Register a plugin.
    ///
    /// # Arguments
    ///
    /// * `plugin` - The plugin to register
    ///
    /// # Errors
    ///
    /// Returns an error if a plugin with the same ID is already registered.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> Result<()> {
        let id = plugin.metadata().id.clone();
        if self.plugins.contains_key(&id) {
            return Err(anyhow!("Plugin '{}' already registered", id));
        }
        self.plugins.insert(id, plugin);
        Ok(())
    }

    /// Unregister a plugin by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The plugin ID to unregister
    ///
    /// # Returns
    ///
    /// The unregistered plugin, or an error if not found.
    pub fn unregister(&mut self, id: &str) -> Result<Box<dyn Plugin>> {
        self.plugins
            .remove(id)
            .ok_or_else(|| anyhow!("Plugin '{}' not found", id))
    }

    /// Get a reference to a plugin by ID.
    pub fn get(&self, id: &str) -> Option<&dyn Plugin> {
        self.plugins.get(id).map(|p| p.as_ref())
    }

    /// Get a mutable reference to a plugin by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut (dyn Plugin + '_)> {
        match self.plugins.get_mut(id) {
            Some(boxed) => Some(&mut **boxed),
            None => None,
        }
    }

    /// List all registered plugins.
    pub fn list(&self) -> Vec<&dyn Plugin> {
        self.plugins.values().map(|p| p.as_ref()).collect()
    }

    /// Get mutable iterator over plugins.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn Plugin>> {
        self.plugins.values_mut()
    }

    /// Get metadata for all plugins.
    pub fn list_metadata(&self) -> Vec<&PluginMetadata> {
        self.plugins.values().map(|p| p.metadata()).collect()
    }

    /// Check if a plugin is registered.
    pub fn contains(&self, id: &str) -> bool {
        self.plugins.contains_key(id)
    }

    /// Get the number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codirigent_core::events::CodirigentEvent;
    use codirigent_core::plugin::{PluginCapabilities, PluginState};
    use codirigent_core::traits::EventBus;
    use std::any::Any;

    /// Mock plugin for testing the registry.
    #[derive(Debug)]
    struct MockPlugin {
        metadata: PluginMetadata,
        state: PluginState,
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
                state: PluginState::Unloaded,
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
            self.state = PluginState::Active;
            Ok(())
        }

        fn shutdown(&mut self) -> Result<()> {
            self.state = PluginState::Unloaded;
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
    fn test_registry_new() {
        let registry = PluginRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_default() {
        let registry = PluginRegistry::default();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_register() {
        let mut registry = PluginRegistry::new();
        let plugin = MockPlugin::new("test");

        assert!(registry.register(Box::new(plugin)).is_ok());
        assert_eq!(registry.len(), 1);
        assert!(registry.contains("test"));
    }

    #[test]
    fn test_registry_register_duplicate() {
        let mut registry = PluginRegistry::new();
        let plugin1 = MockPlugin::new("test");
        let plugin2 = MockPlugin::new("test");

        assert!(registry.register(Box::new(plugin1)).is_ok());
        let result = registry.register(Box::new(plugin2));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("already registered"));
    }

    #[test]
    fn test_registry_unregister() {
        let mut registry = PluginRegistry::new();
        let plugin = MockPlugin::new("test");

        registry.register(Box::new(plugin)).unwrap();
        assert!(registry.contains("test"));

        let unregistered = registry.unregister("test").unwrap();
        assert_eq!(unregistered.metadata().id, "test");
        assert!(!registry.contains("test"));
    }

    #[test]
    fn test_registry_unregister_not_found() {
        let mut registry = PluginRegistry::new();
        let result = registry.unregister("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_registry_get() {
        let mut registry = PluginRegistry::new();
        let plugin = MockPlugin::new("test");
        registry.register(Box::new(plugin)).unwrap();

        let got = registry.get("test");
        assert!(got.is_some());
        assert_eq!(got.unwrap().metadata().id, "test");

        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_get_mut() {
        let mut registry = PluginRegistry::new();
        let plugin = MockPlugin::new("test");
        registry.register(Box::new(plugin)).unwrap();

        let got = registry.get_mut("test");
        assert!(got.is_some());
        assert_eq!(got.unwrap().metadata().id, "test");

        assert!(registry.get_mut("nonexistent").is_none());
    }

    #[test]
    fn test_registry_list() {
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(MockPlugin::new("plugin1")))
            .unwrap();
        registry
            .register(Box::new(MockPlugin::new("plugin2")))
            .unwrap();

        let list = registry.list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_registry_list_metadata() {
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(MockPlugin::new("plugin1")))
            .unwrap();
        registry
            .register(Box::new(MockPlugin::new("plugin2")))
            .unwrap();

        let metadata_list = registry.list_metadata();
        assert_eq!(metadata_list.len(), 2);

        let ids: Vec<&str> = metadata_list.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"plugin1"));
        assert!(ids.contains(&"plugin2"));
    }

    #[test]
    fn test_registry_iter_mut() {
        let mut registry = PluginRegistry::new();
        registry
            .register(Box::new(MockPlugin::new("plugin1")))
            .unwrap();
        registry
            .register(Box::new(MockPlugin::new("plugin2")))
            .unwrap();

        let count = registry.iter_mut().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_registry_contains() {
        let mut registry = PluginRegistry::new();
        assert!(!registry.contains("test"));

        registry
            .register(Box::new(MockPlugin::new("test")))
            .unwrap();
        assert!(registry.contains("test"));
    }

    #[test]
    fn test_registry_len() {
        let mut registry = PluginRegistry::new();
        assert_eq!(registry.len(), 0);

        registry
            .register(Box::new(MockPlugin::new("plugin1")))
            .unwrap();
        assert_eq!(registry.len(), 1);

        registry
            .register(Box::new(MockPlugin::new("plugin2")))
            .unwrap();
        assert_eq!(registry.len(), 2);

        registry.unregister("plugin1").unwrap();
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_registry_is_empty() {
        let mut registry = PluginRegistry::new();
        assert!(registry.is_empty());

        registry
            .register(Box::new(MockPlugin::new("test")))
            .unwrap();
        assert!(!registry.is_empty());

        registry.unregister("test").unwrap();
        assert!(registry.is_empty());
    }
}
