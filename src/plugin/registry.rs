//! Plugin Registry
//! 
//! Manages plugin registration, lifecycle, and lookups.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::plugin::traits::{Plugin, PluginType};
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::context::PluginContext;

/// Registry for managing plugin instances
pub struct PluginRegistry {
    /// Registered plugins by name
    plugins: HashMap<String, Box<dyn Plugin>>,
    
    /// Plugin initialization status
    initialized: HashMap<String, bool>,
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            initialized: HashMap::new(),
        }
    }
    
    /// Register a plugin
    pub async fn register_plugin(&mut self, plugin: Box<dyn Plugin>) -> PluginResult<()> {
        let name = plugin.plugin_info().name.clone();
        
        if self.plugins.contains_key(&name) {
            return Err(PluginError::plugin_already_registered(&name));
        }
        
        self.plugins.insert(name.clone(), plugin);
        self.initialized.insert(name, false);
        
        Ok(())
    }
    
    /// Unregister a plugin
    pub async fn unregister_plugin(&mut self, name: &str) -> PluginResult<()> {
        if !self.plugins.contains_key(name) {
            return Err(PluginError::plugin_not_found(name));
        }
        
        // Cleanup plugin before removing
        if let Some(mut plugin) = self.plugins.remove(name) {
            plugin.cleanup().await?;
        }
        
        self.initialized.remove(name);
        Ok(())
    }
    
    /// Get a plugin by name (immutable)
    pub fn get_plugin(&self, name: &str) -> Option<&dyn Plugin> {
        self.plugins.get(name).map(|p| p.as_ref())
    }
    
    /// Get a plugin by name (mutable)
    pub fn get_plugin_mut(&mut self, name: &str) -> Option<&mut Box<dyn Plugin>> {
        self.plugins.get_mut(name)
    }
    
    /// List all registered plugin names
    pub fn list_plugins(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }
    
    /// Get plugins by type
    pub fn get_plugins_by_type(&self, plugin_type: PluginType) -> Vec<String> {
        self.plugins
            .iter()
            .filter(|(_, plugin)| plugin.plugin_info().plugin_type == plugin_type)
            .map(|(name, _)| name.clone())
            .collect()
    }
    
    /// Get plugins with a specific capability
    pub fn get_plugins_with_capability(&self, capability: &str) -> Vec<String> {
        self.plugins
            .iter()
            .filter(|(_, plugin)| plugin.supports_capability(capability))
            .map(|(name, _)| name.clone())
            .collect()
    }
    
    /// Initialize all plugins
    pub async fn initialize_all(&mut self, context: &PluginContext) -> HashMap<String, PluginResult<()>> {
        let mut results = HashMap::new();
        
        // Collect plugin names to avoid borrow issues
        let plugin_names: Vec<String> = self.plugins.keys().cloned().collect();
        
        for name in plugin_names {
            let result = if let Some(plugin) = self.plugins.get_mut(&name) {
                match plugin.initialize(context).await {
                    Ok(()) => {
                        self.initialized.insert(name.clone(), true);
                        Ok(())
                    }
                    Err(e) => {
                        self.initialized.insert(name.clone(), false);
                        Err(e)
                    }
                }
            } else {
                Err(PluginError::plugin_not_found(&name))
            };
            
            results.insert(name, result);
        }
        
        results
    }
    
    /// Cleanup all plugins
    pub async fn cleanup_all(&mut self) -> HashMap<String, PluginResult<()>> {
        let mut results = HashMap::new();
        
        // Collect plugin names to avoid borrow issues
        let plugin_names: Vec<String> = self.plugins.keys().cloned().collect();
        
        for name in plugin_names {
            let result = if let Some(plugin) = self.plugins.get_mut(&name) {
                match plugin.cleanup().await {
                    Ok(()) => {
                        self.initialized.insert(name.clone(), false);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            } else {
                Err(PluginError::plugin_not_found(&name))
            };
            
            results.insert(name, result);
        }
        
        results
    }
    
    /// Check if a plugin is initialized
    pub fn is_initialized(&self, name: &str) -> bool {
        self.initialized.get(name).copied().unwrap_or(false)
    }
    
    /// Get the count of registered plugins
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
    
    /// Get the count of initialized plugins
    pub fn initialized_count(&self) -> usize {
        self.initialized.values().filter(|&&v| v).count()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe plugin registry wrapper
pub struct SharedPluginRegistry {
    inner: Arc<RwLock<PluginRegistry>>,
}

impl SharedPluginRegistry {
    /// Create a new shared plugin registry
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(PluginRegistry::new())),
        }
    }
    
    /// Get the inner registry for direct access
    pub fn inner(&self) -> &Arc<RwLock<PluginRegistry>> {
        &self.inner
    }
    
    /// Clone the Arc for sharing
    pub fn clone_inner(&self) -> Arc<RwLock<PluginRegistry>> {
        Arc::clone(&self.inner)
    }
}

impl Clone for SharedPluginRegistry {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Default for SharedPluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::tests::mock_plugins::*;
    
    #[tokio::test]
    async fn test_registry_basic_operations() {
        let mut registry = PluginRegistry::new();
        
        // Test empty registry
        assert_eq!(registry.plugin_count(), 0);
        assert_eq!(registry.list_plugins().len(), 0);
        
        // Register plugin
        let plugin = Box::new(MockPlugin::new("test", false));
        registry.register_plugin(plugin).await.unwrap();
        
        assert_eq!(registry.plugin_count(), 1);
        assert!(registry.list_plugins().contains(&"test".to_string()));
        
        // Get plugin
        assert!(registry.get_plugin("test").is_some());
        assert!(registry.get_plugin("missing").is_none());
        
        // Unregister plugin
        registry.unregister_plugin("test").await.unwrap();
        assert_eq!(registry.plugin_count(), 0);
    }
    
    #[tokio::test]
    async fn test_registry_duplicate_registration() {
        let mut registry = PluginRegistry::new();
        
        let plugin1 = Box::new(MockPlugin::new("test", false));
        registry.register_plugin(plugin1).await.unwrap();
        
        let plugin2 = Box::new(MockPlugin::new("test", false));
        let result = registry.register_plugin(plugin2).await;
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PluginError::PluginAlreadyRegistered { .. }));
    }
    
    #[tokio::test]
    async fn test_registry_initialization_tracking() {
        let mut registry = PluginRegistry::new();
        let context = create_test_context();
        
        let plugin = Box::new(MockPlugin::new("test", false));
        registry.register_plugin(plugin).await.unwrap();
        
        // Not initialized yet
        assert!(!registry.is_initialized("test"));
        assert_eq!(registry.initialized_count(), 0);
        
        // Initialize
        let results = registry.initialize_all(&context).await;
        assert!(results.get("test").unwrap().is_ok());
        assert!(registry.is_initialized("test"));
        assert_eq!(registry.initialized_count(), 1);
        
        // Cleanup
        let cleanup_results = registry.cleanup_all().await;
        assert!(cleanup_results.get("test").unwrap().is_ok());
        assert!(!registry.is_initialized("test"));
        assert_eq!(registry.initialized_count(), 0);
    }
    
    #[tokio::test]
    async fn test_shared_registry() {
        let shared = SharedPluginRegistry::new();
        let registry = shared.clone_inner();
        
        // Register plugin through shared registry
        {
            let mut reg = registry.write().await;
            let plugin = Box::new(MockPlugin::new("shared-test", false));
            reg.register_plugin(plugin).await.unwrap();
        }
        
        // Read plugin through shared registry
        {
            let reg = registry.read().await;
            assert!(reg.get_plugin("shared-test").is_some());
        }
    }
}