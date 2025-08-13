//! Plugin Registry
//! 
//! Manages plugin registration, lifecycle, and lookups with notification subscription support.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::plugin::traits::{Plugin, PluginType};
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::context::PluginContext;
use crate::plugin::subscriber::PluginSubscriber;
use crate::plugin::priority_queue::PriorityQueue;
use crate::notifications::{AsyncNotificationManager, ScanEvent};
use crate::notifications::traits::{NotificationManager, Subscriber};

/// Registry for managing plugin instances with notification support
pub struct PluginRegistry {
    /// Registered plugins by name
    plugins: HashMap<String, Box<dyn Plugin>>,
    
    /// Plugin initialization status
    initialized: HashMap<String, bool>,
    
    /// Plugin activation status
    active: HashMap<String, bool>,
    
    /// Plugin subscribers for notification management
    subscribers: HashMap<String, Arc<PluginSubscriber>>,
    
    /// Optional notification manager for automatic subscription
    notification_manager: Option<AsyncNotificationManager<ScanEvent>>,
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            initialized: HashMap::new(),
            active: HashMap::new(),
            subscribers: HashMap::new(),
            notification_manager: None,
        }
    }
    
    /// Create a new plugin registry with notification manager
    pub fn with_notification_manager(
        notification_manager: Arc<AsyncNotificationManager<ScanEvent>>
    ) -> Self {
        Self {
            plugins: HashMap::new(),
            initialized: HashMap::new(),
            active: HashMap::new(),
            subscribers: HashMap::new(),
            notification_manager: Some((*notification_manager).clone()),
        }
    }
    
    /// Set the notification manager for automatic subscription
    pub fn set_notification_manager(
        &mut self, 
        notification_manager: Arc<AsyncNotificationManager<ScanEvent>>
    ) {
        self.notification_manager = Some((*notification_manager).clone());
    }
    
    /// Register a plugin with automatic notification subscription
    pub async fn register_plugin(&mut self, plugin: Box<dyn Plugin>) -> PluginResult<()> {
        let name = plugin.plugin_info().name.clone();
        
        if self.plugins.contains_key(&name) {
            return Err(PluginError::plugin_already_registered(&name));
        }
        
        // Store plugin first
        self.plugins.insert(name.clone(), plugin);
        self.initialized.insert(name.clone(), false);
        self.active.insert(name.clone(), true); // Original register_plugin activates by default
        
        // Create plugin subscriber for notification handling
        // We'll create a simple subscriber that references the plugin by name
        let subscriber = Arc::new(PluginSubscriber::new_with_name(name.clone()));
        
        // Subscribe to notifications if manager is available
        if let Some(ref mut notification_manager) = self.notification_manager {
            notification_manager.subscribe(subscriber.clone()).await
                .map_err(|e| PluginError::NotificationFailed { 
                    message: format!("Failed to subscribe plugin '{}' to notifications: {}", name, e) 
                })?;
            
            log::debug!("Plugin '{}' subscribed to scanner events", name);
        }
        
        // Store subscriber
        self.subscribers.insert(name, subscriber);
        
        Ok(())
    }
    
    /// Register a plugin without automatic notification subscription
    pub async fn register_plugin_without_notifications(&mut self, plugin: Box<dyn Plugin>) -> PluginResult<()> {
        let name = plugin.plugin_info().name.clone();
        
        if self.plugins.contains_key(&name) {
            return Err(PluginError::plugin_already_registered(&name));
        }
        
        self.plugins.insert(name.clone(), plugin);
        self.initialized.insert(name.clone(), false);
        self.active.insert(name, false); // Register as inactive by default
        
        Ok(())
    }
    
    /// Register a plugin as inactive (loaded but not processing events)
    pub async fn register_plugin_inactive(&mut self, plugin: Box<dyn Plugin>) -> PluginResult<()> {
        let name = plugin.plugin_info().name.clone();
        
        if self.plugins.contains_key(&name) {
            return Err(PluginError::plugin_already_registered(&name));
        }
        
        // Store plugin first
        self.plugins.insert(name.clone(), plugin);
        self.initialized.insert(name.clone(), false);
        self.active.insert(name.clone(), false); // Mark as inactive
        
        // Create plugin subscriber for notification handling
        let subscriber = Arc::new(PluginSubscriber::new_with_name(name.clone()));
        
        // Subscribe to notifications if manager is available
        if let Some(ref mut notification_manager) = self.notification_manager {
            notification_manager.subscribe(subscriber.clone()).await
                .map_err(|e| PluginError::NotificationFailed { 
                    message: format!("Failed to subscribe plugin '{}' to notifications: {}", name, e) 
                })?;
            
            log::debug!("Plugin '{}' registered as inactive and subscribed to scanner events", name);
        }
        
        // Store subscriber
        self.subscribers.insert(name, subscriber);
        
        Ok(())
    }
    
    /// Activate a plugin for processing events
    pub async fn activate_plugin(&mut self, name: &str) -> PluginResult<()> {
        if !self.plugins.contains_key(name) {
            return Err(PluginError::plugin_not_found(name));
        }
        
        self.active.insert(name.to_string(), true);
        log::debug!("Plugin '{}' activated", name);
        
        Ok(())
    }
    
    /// Deactivate a plugin (stop processing events)
    pub async fn deactivate_plugin(&mut self, name: &str) -> PluginResult<()> {
        if !self.plugins.contains_key(name) {
            return Err(PluginError::plugin_not_found(name));
        }
        
        self.active.insert(name.to_string(), false);
        log::debug!("Plugin '{}' deactivated", name);
        
        Ok(())
    }
    
    /// Check if a plugin is active
    pub fn is_plugin_active(&self, name: &str) -> bool {
        self.active.get(name).copied().unwrap_or(false)
    }
    
    /// Get list of active plugin names
    pub fn get_active_plugins(&self) -> Vec<String> {
        self.active.iter()
            .filter_map(|(name, &is_active)| {
                if is_active { Some(name.clone()) } else { None }
            })
            .collect()
    }
    
    /// Auto-activate plugins marked with load_by_default = true
    pub async fn auto_activate_default_plugins(&mut self) -> PluginResult<()> {
        let plugins_to_activate: Vec<String> = self.plugins.iter()
            .filter_map(|(name, plugin)| {
                if plugin.plugin_info().load_by_default {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();
        
        for plugin_name in plugins_to_activate {
            self.activate_plugin(&plugin_name).await?;
            log::debug!("Auto-activated plugin '{}' (load_by_default = true)", plugin_name);
        }
        
        Ok(())
    }
    
    /// Check if a plugin exists in the registry  
    pub fn has_plugin(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }
    
    /// Unregister a plugin with notification cleanup
    pub async fn unregister_plugin(&mut self, name: &str) -> PluginResult<()> {
        if !self.plugins.contains_key(name) {
            return Err(PluginError::plugin_not_found(name));
        }
        
        // Unsubscribe from notifications if manager is available
        if let Some(ref mut notification_manager) = self.notification_manager {
            if let Some(subscriber) = self.subscribers.get(name) {
                notification_manager.unsubscribe(subscriber.subscriber_id()).await
                    .map_err(|e| PluginError::NotificationFailed { 
                        message: format!("Failed to unsubscribe plugin '{}' from notifications: {}", name, e) 
                    })?;
                
                log::debug!("Plugin '{}' unsubscribed from scanner events", name);
            }
        }
        
        // Cleanup plugin before removing
        if let Some(mut plugin) = self.plugins.remove(name) {
            plugin.cleanup().await?;
        }
        
        self.subscribers.remove(name);
        self.initialized.remove(name);
        self.active.remove(name);
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
    
    /// Get plugins by type, ordered by priority (highest to lowest)
    pub fn get_plugins_by_type(&self, plugin_type: PluginType) -> Vec<String> {
        let mut plugin_queue = PriorityQueue::new();
        
        // Add matching plugins to priority queue
        for (name, plugin) in &self.plugins {
            if plugin.plugin_info().plugin_type == plugin_type {
                let priority = plugin.plugin_info().priority;
                plugin_queue.push(priority, name.clone());
            }
        }
        
        // Extract names in priority order
        let mut ordered_plugins = Vec::new();
        while let Some((_, name)) = plugin_queue.pop() {
            ordered_plugins.push(name);
        }
        
        ordered_plugins
    }
    
    /// Get plugins with a specific capability, ordered by priority (highest to lowest)
    pub fn get_plugins_with_capability(&self, capability: &str) -> Vec<String> {
        let mut plugin_queue = PriorityQueue::new();
        
        // Add matching plugins to priority queue
        for (name, plugin) in &self.plugins {
            if plugin.supports_capability(capability) {
                let priority = plugin.plugin_info().priority;
                plugin_queue.push(priority, name.clone());
            }
        }
        
        // Extract names in priority order
        let mut ordered_plugins = Vec::new();
        while let Some((_, name)) = plugin_queue.pop() {
            ordered_plugins.push(name);
        }
        
        ordered_plugins
    }
    
    /// Get a plugin subscriber by name
    pub fn get_subscriber(&self, name: &str) -> Option<&Arc<PluginSubscriber>> {
        self.subscribers.get(name)
    }
    
    /// List all plugin subscribers
    pub fn list_subscribers(&self) -> Vec<Arc<PluginSubscriber>> {
        self.subscribers.values().cloned().collect()
    }
    
    /// Check if notifications are enabled
    pub fn has_notification_manager(&self) -> bool {
        self.notification_manager.is_some()
    }
    
    /// Get the number of subscribed plugins
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }
    
    /// Subscribe all existing plugins to notifications (if manager is set)
    pub async fn subscribe_all_plugins(&mut self) -> PluginResult<()> {
        if let Some(ref mut notification_manager) = self.notification_manager {
            for (name, _plugin) in &self.plugins {
                if !self.subscribers.contains_key(name) {
                    // Create subscriber for plugins that don't have one
                    let subscriber = Arc::new(PluginSubscriber::new_with_name(name.clone()));
                    
                    notification_manager.subscribe(subscriber.clone()).await
                        .map_err(|e| PluginError::NotificationFailed { 
                            message: format!("Failed to subscribe plugin '{}' to notifications: {}", name, e) 
                        })?;
                    
                    self.subscribers.insert(name.clone(), subscriber);
                    log::debug!("Plugin '{}' subscribed to scanner events", name);
                }
            }
        }
        Ok(())
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
    
    /// Wait for the registry to become empty (all plugins unregistered)
    /// Returns true if registry becomes empty within timeout, false if timeout occurs
    pub async fn wait_for_empty_registry(&self, timeout: std::time::Duration) -> bool {
        use tokio::time::{sleep, Duration, Instant};
        
        let start = Instant::now();
        let poll_interval = Duration::from_millis(10);
        
        while start.elapsed() < timeout {
            if self.plugin_count() == 0 {
                return true;
            }
            
            sleep(poll_interval).await;
        }
        
        false
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
    
    /// Create a new shared plugin registry with notification manager
    pub fn with_notification_manager(
        notification_manager: Arc<AsyncNotificationManager<ScanEvent>>
    ) -> Self {
        Self {
            inner: Arc::new(RwLock::new(PluginRegistry::with_notification_manager(notification_manager))),
        }
    }
    
    /// Set the notification manager for automatic subscription
    pub async fn set_notification_manager(
        &self, 
        notification_manager: Arc<AsyncNotificationManager<ScanEvent>>
    ) {
        let mut registry = self.inner.write().await;
        registry.set_notification_manager(notification_manager);
    }
    
    /// Register a plugin with automatic notification subscription
    pub async fn register_plugin(&self, plugin: Box<dyn Plugin>) -> PluginResult<()> {
        let mut registry = self.inner.write().await;
        registry.register_plugin(plugin).await
    }
    
    /// Register a plugin without automatic notification subscription
    pub async fn register_plugin_without_notifications(&self, plugin: Box<dyn Plugin>) -> PluginResult<()> {
        let mut registry = self.inner.write().await;
        registry.register_plugin_without_notifications(plugin).await
    }
    
    /// Subscribe all existing plugins to notifications
    pub async fn subscribe_all_plugins(&self) -> PluginResult<()> {
        let mut registry = self.inner.write().await;
        registry.subscribe_all_plugins().await
    }
    
    /// Unregister a plugin with notification cleanup
    pub async fn unregister_plugin(&self, name: &str) -> PluginResult<()> {
        let mut registry = self.inner.write().await;
        registry.unregister_plugin(name).await
    }
    
    /// Get the count of registered plugins
    pub async fn get_plugin_count(&self) -> usize {
        let registry = self.inner.read().await;
        registry.plugin_count()
    }
    
    /// Wait for the registry to become empty (all plugins unregistered)
    /// Returns true if registry becomes empty within timeout, false if timeout occurs
    pub async fn wait_for_empty_registry(&self, timeout: std::time::Duration) -> bool {
        use tokio::time::{sleep, Duration, Instant};
        
        let start = Instant::now();
        let poll_interval = Duration::from_millis(10);
        
        while start.elapsed() < timeout {
            {
                let registry = self.inner.read().await;
                if registry.plugin_count() == 0 {
                    return true;
                }
            }
            
            sleep(poll_interval).await;
        }
        
        false
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
    
    #[tokio::test]
    async fn test_registry_notification_subscription() {
        use crate::notifications::AsyncNotificationManager;
        use std::sync::Arc;
        
        // Create notification manager
        let notification_manager = Arc::new(AsyncNotificationManager::new());
        
        // Create registry with notification manager
        let mut registry = PluginRegistry::with_notification_manager(notification_manager.clone());
        
        // Create mock plugin
        let mock_plugin = Box::new(MockPlugin::new("test-plugin", false));
        
        // Register plugin - should automatically subscribe to notifications
        let result = registry.register_plugin(mock_plugin).await;
        assert!(result.is_ok());
        
        // Verify plugin is registered
        assert!(registry.get_plugin("test-plugin").is_some());
        
        // Verify subscriber is created
        assert!(registry.get_subscriber("test-plugin").is_some());
        assert_eq!(registry.subscriber_count(), 1);
        
        // Verify notification manager has the subscriber
        assert_eq!(notification_manager.subscriber_count().await, 1);
        assert!(notification_manager.has_subscriber("plugin_test-plugin").await);
        
        // Test unregistration - should unsubscribe from notifications
        let result = registry.unregister_plugin("test-plugin").await;
        assert!(result.is_ok());
        
        // Verify plugin is unregistered
        assert!(registry.get_plugin("test-plugin").is_none());
        
        // Verify subscriber is removed
        assert!(registry.get_subscriber("test-plugin").is_none());
        assert_eq!(registry.subscriber_count(), 0);
        
        // Verify notification manager no longer has the subscriber
        assert_eq!(notification_manager.subscriber_count().await, 0);
        assert!(!notification_manager.has_subscriber("plugin_test-plugin").await);
    }
    
    #[tokio::test]
    async fn test_registry_subscribe_all_plugins() {
        use crate::notifications::AsyncNotificationManager;
        use std::sync::Arc;
        
        // Create registry without notification manager
        let mut registry = PluginRegistry::new();
        
        // Register plugins without notifications
        let mock_plugin1 = Box::new(MockPlugin::new("plugin1", false));
        let mock_plugin2 = Box::new(MockPlugin::new("plugin2", false));
        
        let result1 = registry.register_plugin_without_notifications(mock_plugin1).await;
        let result2 = registry.register_plugin_without_notifications(mock_plugin2).await;
        assert!(result1.is_ok());
        assert!(result2.is_ok());
        
        // Verify no subscribers initially
        assert_eq!(registry.subscriber_count(), 0);
        
        // Set notification manager
        let notification_manager = Arc::new(AsyncNotificationManager::new());
        registry.set_notification_manager(notification_manager.clone());
        
        // Subscribe all existing plugins
        let result = registry.subscribe_all_plugins().await;
        assert!(result.is_ok());
        
        // Verify all plugins are now subscribed
        assert_eq!(registry.subscriber_count(), 2);
        assert!(registry.get_subscriber("plugin1").is_some());
        assert!(registry.get_subscriber("plugin2").is_some());
        
        // Verify notification manager has all subscribers
        assert_eq!(notification_manager.subscriber_count().await, 2);
        assert!(notification_manager.has_subscriber("plugin_plugin1").await);
        assert!(notification_manager.has_subscriber("plugin_plugin2").await);
    }
    
    #[tokio::test]
    async fn test_shared_registry_notification_subscription() {
        use crate::notifications::AsyncNotificationManager;
        use std::sync::Arc;
        
        // Create notification manager
        let notification_manager = Arc::new(AsyncNotificationManager::new());
        
        // Create shared registry with notification manager
        let shared_registry = SharedPluginRegistry::with_notification_manager(notification_manager.clone());
        
        // Create mock plugin
        let mock_plugin = Box::new(MockPlugin::new("shared-test-plugin", false));
        
        // Register plugin - should automatically subscribe to notifications
        let result = shared_registry.register_plugin(mock_plugin).await;
        assert!(result.is_ok());
        
        // Verify notification manager has the subscriber
        assert_eq!(notification_manager.subscriber_count().await, 1);
        assert!(notification_manager.has_subscriber("plugin_shared-test-plugin").await);
        
        // Test subscribe all plugins functionality
        let result = shared_registry.subscribe_all_plugins().await;
        assert!(result.is_ok());
        
        // Should still have the same subscriber (no duplicates)
        assert_eq!(notification_manager.subscriber_count().await, 1);
    }
    
    #[tokio::test]
    async fn test_registry_wait_for_empty_registry() {
        use std::time::Duration;
        use tokio::time::timeout;
        
        let mut registry = PluginRegistry::new();
        
        // Test empty registry - should return immediately
        let start = std::time::Instant::now();
        let result = registry.wait_for_empty_registry(Duration::from_secs(1)).await;
        let elapsed = start.elapsed();
        
        assert!(result); // Should succeed immediately
        assert!(elapsed < Duration::from_millis(100)); // Should be very fast
        
        // Register a plugin
        let plugin = Box::new(MockPlugin::new("test", false));
        registry.register_plugin(plugin).await.unwrap();
        assert_eq!(registry.plugin_count(), 1);
        
        // Test timeout when registry is not empty
        let start = std::time::Instant::now();
        let result = registry.wait_for_empty_registry(Duration::from_millis(100)).await;
        let elapsed = start.elapsed();
        
        assert!(!result); // Should timeout
        assert!(elapsed >= Duration::from_millis(90)); // Should wait for timeout
        assert!(elapsed < Duration::from_millis(200)); // But not too long
        
        // Test successful wait when plugin is unregistered concurrently
        let mut registry = PluginRegistry::new();
        let plugin = Box::new(MockPlugin::new("test", false));
        registry.register_plugin(plugin).await.unwrap();
        
        let registry_arc = Arc::new(tokio::sync::Mutex::new(registry));
        let registry_for_task = Arc::clone(&registry_arc);
        
        // Start waiting for empty registry
        let wait_task = tokio::spawn(async move {
            // We need to clone the registry state for the wait operation
            // Since wait_for_empty_registry needs to poll the state
            let start = std::time::Instant::now();
            let timeout_duration = Duration::from_secs(2);
            
            while start.elapsed() < timeout_duration {
                {
                    let registry = registry_for_task.lock().await;
                    if registry.plugin_count() == 0 {
                        return true;
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            false
        });
        
        // Unregister plugin after a short delay
        tokio::time::sleep(Duration::from_millis(50)).await;
        {
            let mut registry = registry_arc.lock().await;
            registry.unregister_plugin("test").await.unwrap();
        }
        
        // Wait should complete successfully
        let result = timeout(Duration::from_secs(3), wait_task).await.unwrap().unwrap();
        assert!(result);
    }
    
    #[tokio::test]
    async fn test_shared_registry_wait_for_empty_registry() {
        use std::time::Duration;
        
        let shared_registry = SharedPluginRegistry::new();
        
        // Test empty registry
        let result = shared_registry.wait_for_empty_registry(Duration::from_secs(1)).await;
        assert!(result);
        
        // Register a plugin
        let plugin = Box::new(MockPlugin::new("test", false));
        shared_registry.register_plugin(plugin).await.unwrap();
        
        // Test timeout
        let result = shared_registry.wait_for_empty_registry(Duration::from_millis(100)).await;
        assert!(!result);
        
        // Unregister plugin
        shared_registry.unregister_plugin("test").await.unwrap();
        
        // Should now be empty
        let result = shared_registry.wait_for_empty_registry(Duration::from_secs(1)).await;
        assert!(result);
    }
}