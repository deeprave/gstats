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
    
    /// Plugin coordination states (GS-65)
    states: HashMap<String, crate::plugin::traits::PluginState>,
    
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
            states: HashMap::new(),
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
            states: HashMap::new(),
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

    /// Get list of active plugins that are still processing
    pub fn get_active_processing_plugins(&self) -> Vec<String> {
        self.get_active_plugins()
            .into_iter()
            .filter(|name| {
                let state = self.states.get(name)
                    .unwrap_or(&crate::plugin::traits::PluginState::Initialized);
                matches!(
                    state,
                    crate::plugin::traits::PluginState::Processing | 
                    crate::plugin::traits::PluginState::Running
                )
            })
            .collect()
    }

    /// Transition a plugin to a new state (GS-65 coordination)
    /// 
    /// This method updates a plugin's coordination state for lifecycle management.
    /// It's primarily used during plugin execution to signal when plugins are
    /// actively processing work versus idle and ready for shutdown.
    /// 
    /// # Arguments
    /// * `name` - The name of the plugin to transition
    /// * `new_state` - The new state to transition to
    /// 
    /// # States
    /// * `Processing` - Plugin is actively working on tasks
    /// * `Initialized` - Plugin is idle and ready for shutdown
    /// * `Error(msg)` - Plugin encountered an error (treated as idle for shutdown)
    /// 
    /// # Examples
    /// ```ignore
    /// // Signal that plugin is starting work
    /// registry.transition_plugin_state("my-plugin", PluginState::Processing).await?;
    /// 
    /// // Signal that plugin finished work
    /// registry.transition_plugin_state("my-plugin", PluginState::Initialized).await?;
    /// ```
    pub async fn transition_plugin_state(&mut self, name: &str, new_state: crate::plugin::traits::PluginState) -> PluginResult<()> {
        if !self.plugins.contains_key(name) {
            return Err(PluginError::plugin_not_found(name));
        }
        
        log::debug!("Plugin '{}' state transition: {:?} -> {:?}", 
            name, 
            self.states.get(name).unwrap_or(&crate::plugin::traits::PluginState::Unloaded),
            new_state
        );
        
        self.states.insert(name.to_string(), new_state);
        Ok(())
    }

    /// Check if all active plugins are idle (not processing work)
    /// 
    /// Returns `true` if all currently active plugins are in states that don't
    /// block system shutdown. This includes plugins in `Initialized`, `Loaded`,
    /// `Unloaded`, `ShuttingDown`, and `Error` states. Only `Processing` and
    /// `Running` states are considered non-idle.
    /// 
    /// Inactive plugins are ignored completely, as they don't participate in
    /// work processing and shouldn't block shutdown.
    /// 
    /// # Returns
    /// * `true` - All active plugins are idle, safe to shutdown
    /// * `false` - One or more active plugins are still processing work
    /// 
    /// # Examples
    /// ```ignore
    /// if registry.are_all_active_plugins_idle() {
    ///     println!("Safe to shutdown - no plugins are processing");
    /// } else {
    ///     println!("Still waiting for plugins to finish work");
    /// }
    /// ```
    pub fn are_all_active_plugins_idle(&self) -> bool {
        let active_plugins = self.get_active_plugins();
        
        for plugin_name in active_plugins {
            let state = self.states.get(&plugin_name)
                .unwrap_or(&crate::plugin::traits::PluginState::Initialized);
            
            match state {
                crate::plugin::traits::PluginState::Processing => {
                    log::debug!("Plugin '{}' is still processing - not idle", plugin_name);
                    return false;
                }
                crate::plugin::traits::PluginState::Running => {
                    log::debug!("Plugin '{}' is still running - not idle", plugin_name);
                    return false;
                }
                // Error state is considered "done" - doesn't block shutdown
                // Initialized, Loaded, Unloaded, ShuttingDown are all considered idle
                _ => continue,
            }
        }
        
        log::debug!("All active plugins are idle");
        true
    }

    /// Wait for all active plugins to become idle with timeout
    /// 
    /// This method polls the plugin states until all active plugins transition
    /// to idle states or the timeout is reached. It's designed for coordinated
    /// system shutdown where the system needs to wait for plugins to complete
    /// their current work before exiting.
    /// 
    /// The method polls every 10ms and provides detailed error information
    /// including which specific plugins are still processing when timeouts occur.
    /// 
    /// # Arguments
    /// * `timeout` - Maximum duration to wait for plugins to become idle
    /// 
    /// # Returns
    /// * `Ok(())` - All active plugins became idle within the timeout
    /// * `Err(PluginError::Timeout)` - Timeout reached with plugins still processing
    /// 
    /// # Examples
    /// ```ignore
    /// use std::time::Duration;
    /// 
    /// // Wait up to 5 seconds for plugins to finish
    /// match registry.wait_for_all_plugins_idle(Duration::from_secs(5)).await {
    ///     Ok(()) => println!("All plugins are idle, safe to exit"),
    ///     Err(e) => eprintln!("Timeout waiting for plugins: {}", e),
    /// }
    /// ```
    pub async fn wait_for_all_plugins_idle(&self, timeout: std::time::Duration) -> PluginResult<()> {
        use tokio::time::{sleep, Instant};
        
        let start = Instant::now();
        let poll_interval = std::time::Duration::from_millis(10);
        
        loop {
            if self.are_all_active_plugins_idle() {
                log::debug!("All active plugins are idle - coordination complete");
                return Ok(());
            }
            
            if start.elapsed() >= timeout {
                let active_processing: Vec<String> = self.get_active_plugins()
                    .into_iter()
                    .filter(|name| {
                        matches!(
                            self.states.get(name).unwrap_or(&crate::plugin::traits::PluginState::Initialized),
                            crate::plugin::traits::PluginState::Processing | crate::plugin::traits::PluginState::Running
                        )
                    })
                    .collect();
                
                return Err(PluginError::timeout(format!(
                    "Timed out waiting for plugins to become idle. Still processing: {:?}", 
                    active_processing
                )));
            }
            
            sleep(poll_interval).await;
        }
    }
    
    /// Auto-activate plugins marked with active_by_default = true
    pub async fn auto_activate_default_plugins(&mut self) -> PluginResult<()> {
        let plugins_to_activate: Vec<String> = self.plugins.iter()
            .filter_map(|(name, plugin)| {
                if plugin.plugin_info().active_by_default {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();
        
        for plugin_name in plugins_to_activate {
            self.activate_plugin(&plugin_name).await?;
            log::debug!("Auto-activated plugin '{}' (active_by_default = true)", plugin_name);
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
    
    /// Discover and load all available plugins using the unified discovery system
    /// This is the ONLY correct way to populate a plugin registry from external code
    pub fn discover_and_load_plugins(
        &self,
        context: &crate::plugin::PluginContext,
        excluded_plugins: Vec<String>,
    ) -> PluginResult<()> {
        use crate::plugin::discovery::{PluginDiscovery, UnifiedPluginDiscovery};
        
        log::debug!("SharedPluginRegistry: Starting plugin discovery with {} exclusions", excluded_plugins.len());
        
        // Create unified discovery (no external plugin directory for default usage)
        let discovery = UnifiedPluginDiscovery::new_with_notification_manager(
            None, 
            excluded_plugins, 
            crate::plugin::PluginSettings::default(),
            Some(context.get_notification_manager())
        ).map_err(|e| PluginError::InitializationFailed { 
            message: format!("Failed to create plugin discovery: {}", e) 
        })?;
        
        // Discover and instantiate all available plugins
        let plugins = discovery.discover_and_instantiate_plugins()?;
        let plugin_count = plugins.len();
        log::debug!("SharedPluginRegistry: Instantiated {} plugins", plugin_count);
        
        // Create a runtime for async plugin registration
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| PluginError::InitializationFailed { 
                message: format!("Failed to create runtime for plugin registration: {}", e) 
            })?;
        
        // Register and initialize each plugin
        rt.block_on(async {
            let mut registry = self.inner.write().await;
            
            for mut plugin in plugins {
                let plugin_name = plugin.plugin_info().name.clone();
                let active_by_default = plugin.plugin_info().active_by_default;
                
                // Initialize plugin with context
                plugin.initialize(context).await?;
                
                // Register plugin as inactive first
                registry.register_plugin_inactive(plugin).await?;
                
                // Activate if marked as active_by_default
                if active_by_default {
                    registry.activate_plugin(&plugin_name).await?;
                    log::debug!("SharedPluginRegistry: Activated plugin '{}' (active_by_default = true)", plugin_name);
                } else {
                    log::debug!("SharedPluginRegistry: Registered plugin '{}' as inactive (active_by_default = false)", plugin_name);
                }
            }
            
            Result::<_, PluginError>::Ok(())
        })?;
        
        log::info!("SharedPluginRegistry: Successfully loaded {} plugins via discovery system", plugin_count);
        Ok(())
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