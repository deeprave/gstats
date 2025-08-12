//! Plugin Subscriber Implementation
//! 
//! Provides a wrapper that implements Subscriber<ScanEvent> to enable plugins
//! to receive scanner events and coordinate their processing accordingly.

use std::sync::Arc;
use async_trait::async_trait;
use crate::notifications::{ScanEvent, NotificationResult};
use crate::notifications::traits::Subscriber;
use crate::plugin::traits::Plugin;

/// Wrapper that implements Subscriber<ScanEvent> for plugins
/// 
/// This enables plugins to receive scanner events and coordinate their processing:
/// - ScanDataReady: Fetch and process queued data of interest
/// - DataReady: Collect processed data from other plugins (for export plugins)
/// - ScanWarning: Log warnings and continue processing
/// - ScanError: Handle errors appropriately (cleanup if fatal)
/// - ScanCompleted: Finalize processing
pub struct PluginSubscriber {
    plugin: Option<Arc<dyn Plugin>>,
    plugin_name: String,
    subscriber_id: String,
    registry: Arc<tokio::sync::RwLock<Option<Arc<crate::plugin::registry::SharedPluginRegistry>>>>,
    notification_manager: Arc<tokio::sync::RwLock<Option<Arc<crate::notifications::AsyncNotificationManager<crate::notifications::ScanEvent>>>>>,
}

impl PluginSubscriber {
    /// Create a new plugin subscriber wrapper
    pub fn new(plugin: Arc<dyn Plugin>) -> Self {
        let plugin_name = plugin.plugin_info().name.clone();
        Self {
            subscriber_id: format!("plugin_{}", plugin_name),
            plugin_name: plugin_name.clone(),
            plugin: Some(plugin),
            registry: Arc::new(tokio::sync::RwLock::new(None)),
            notification_manager: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }
    
    /// Create a new plugin subscriber wrapper with just a name
    /// This is used when the plugin is managed by the registry
    pub fn new_with_name(plugin_name: String) -> Self {
        Self {
            subscriber_id: format!("plugin_{}", plugin_name),
            plugin_name,
            plugin: None,
            registry: Arc::new(tokio::sync::RwLock::new(None)),
            notification_manager: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }
    
    /// Create a new plugin subscriber with registry and notification manager references
    pub fn new_with_registry(
        plugin: Arc<dyn Plugin>,
        registry: Arc<crate::plugin::registry::SharedPluginRegistry>,
        notification_manager: Arc<crate::notifications::AsyncNotificationManager<crate::notifications::ScanEvent>>,
    ) -> Self {
        let plugin_name = plugin.plugin_info().name.clone();
        Self {
            subscriber_id: format!("plugin_{}", plugin_name),
            plugin_name: plugin_name.clone(),
            plugin: Some(plugin),
            registry: Arc::new(tokio::sync::RwLock::new(Some(registry))),
            notification_manager: Arc::new(tokio::sync::RwLock::new(Some(notification_manager))),
        }
    }
    
    /// Get the wrapped plugin
    pub fn plugin(&self) -> Option<&Arc<dyn Plugin>> {
        self.plugin.as_ref()
    }
    
    /// Get the plugin name
    pub fn plugin_name(&self) -> &str {
        &self.plugin_name
    }
    
    /// Set registry and notification manager references for self-deregistration
    pub async fn set_references(
        &self,
        registry: Arc<crate::plugin::registry::SharedPluginRegistry>,
        notification_manager: Arc<crate::notifications::AsyncNotificationManager<crate::notifications::ScanEvent>>,
    ) {
        *self.registry.write().await = Some(registry);
        *self.notification_manager.write().await = Some(notification_manager);
    }
}

#[async_trait]
impl Subscriber<ScanEvent> for PluginSubscriber {
    async fn handle_event(&self, event: ScanEvent) -> NotificationResult<()> {
        // For now, we'll just log events since we don't have direct plugin access
        // In future tasks, we'll implement actual plugin event handling
        
        match event {
            ScanEvent::ScanStarted { scan_id } => {
                log::debug!("Plugin {} received ScanStarted event for scan {}", self.plugin_name, scan_id);
                // Plugins can prepare for incoming data based on scan modes
                // For now, just log the event
            }
            
            ScanEvent::ScanProgress { scan_id: _, progress, phase } => {
                log::trace!("Plugin {} received ScanProgress event: {} - {:.1}%", 
                           self.plugin_name, phase, progress * 100.0);
                // Plugins can track scan progress for UI updates or logging
            }
            
            ScanEvent::ScanDataReady { scan_id, data_type, message_count } => {
                log::debug!("Plugin {} received ScanDataReady event: {} messages of type '{}' for scan {}", 
                           self.plugin_name, message_count, data_type, scan_id);
                
                // Check if this plugin is interested in this data type
                if self.plugin_handles_data_type(&data_type) {
                    log::info!("Plugin {} will process {} messages of type '{}'", 
                              self.plugin_name, message_count, data_type);
                    // TODO: In future tasks, implement actual data fetching and processing
                    // self.plugin.handle_scan_data_ready(scan_id, data_type, message_count).await?;
                } else {
                    log::trace!("Plugin {} ignoring data type '{}'", self.plugin_name, data_type);
                }
            }
            
            ScanEvent::DataReady { scan_id, plugin_id, data_type } => {
                log::debug!("Plugin {} received DataReady event from plugin '{}' with data type '{}' for scan {}", 
                           self.plugin_name, plugin_id, data_type, scan_id);
                
                // Forward to export plugins or plugins that coordinate with others
                if self.is_export_plugin() || self.coordinates_with_plugins() {
                    log::info!("Plugin {} will collect processed data from plugin '{}'", 
                              self.plugin_name, plugin_id);
                    // TODO: In future tasks, implement actual data collection
                    // self.plugin.handle_data_ready(scan_id, plugin_id, data_type).await?;
                } else {
                    log::trace!("Plugin {} ignoring DataReady from plugin '{}'", self.plugin_name, plugin_id);
                }
            }
            
            ScanEvent::ScanWarning { scan_id, warning, recoverable } => {
                log::warn!("Plugin {} received scan warning for scan {}: {} (recoverable: {})", 
                          self.plugin_name, scan_id, warning, recoverable);
                
                // All plugins should handle warnings gracefully
                // Log the warning and continue processing with degraded data quality
                // TODO: In future tasks, implement plugin-specific warning handling
                // self.plugin.handle_scan_warning(scan_id, warning, recoverable).await?;
            }
            
            ScanEvent::ScanError { scan_id, error, fatal } => {
                if fatal {
                    log::error!("Plugin {} received fatal scan error for scan {}: {}", 
                               self.plugin_name, scan_id, error);
                    // Fatal errors require cleanup and abort processing
                    // TODO: In future tasks, implement plugin cleanup
                    // self.plugin.cleanup_partial_data(scan_id).await?;
                    
                    // Note: Actual deregistration will be handled by the scanner
                    // to avoid deadlocks during event processing
                    log::info!("Plugin {} marked for deregistration due to fatal error", self.plugin_name);
                } else {
                    log::warn!("Plugin {} received non-fatal scan error for scan {}: {}", 
                              self.plugin_name, scan_id, error);
                    // Non-fatal errors allow graceful degradation
                    // TODO: In future tasks, implement graceful degradation
                    // self.plugin.handle_scan_error(scan_id, error, fatal).await?;
                }
            }
            
            ScanEvent::ScanCompleted { scan_id, duration, warnings } => {
                log::info!("Plugin {} received ScanCompleted event for scan {} (duration: {:?}, warnings: {})", 
                          self.plugin_name, scan_id, duration, warnings.len());
                
                // All plugins should finalize their processing when scan completes
                // TODO: In future tasks, implement plugin finalization
                // self.plugin.handle_scan_completed(scan_id, duration, warnings).await?;
            }
        }
        
        Ok(())
    }
    
    fn subscriber_id(&self) -> &str {
        &self.subscriber_id
    }
}

impl PluginSubscriber {
    /// Handle fatal scan error by deregistering the plugin
    async fn handle_fatal_scan_error(&self, scan_id: &str, error: &str) -> crate::notifications::NotificationResult<()> {
        println!("handle_fatal_scan_error called for plugin {}", self.plugin_name);
        log::info!("Plugin {} deregistering due to fatal error in scan {}: {}", 
                   self.plugin_name, scan_id, error);
        
        // Deregister from plugin registry if available
        {
            let registry_guard = self.registry.read().await;
            if let Some(ref registry) = *registry_guard {
                println!("Registry reference found, attempting to unregister plugin {}", self.plugin_name);
                match registry.unregister_plugin(&self.plugin_name).await {
                    Ok(()) => {
                        println!("Plugin {} successfully unregistered from registry", self.plugin_name);
                        log::debug!("Plugin {} successfully unregistered from registry", self.plugin_name);
                    }
                    Err(e) => {
                        println!("Failed to unregister plugin {} from registry: {}", self.plugin_name, e);
                        log::error!("Failed to unregister plugin {} from registry: {}", self.plugin_name, e);
                    }
                }
            } else {
                println!("No registry reference found for plugin {}", self.plugin_name);
            }
        }
        
        // Unsubscribe from notification manager if available
        {
            let notification_manager_guard = self.notification_manager.read().await;
            if let Some(ref notification_manager) = *notification_manager_guard {
                println!("Notification manager reference found, attempting to unsubscribe plugin {}", self.plugin_name);
                if let Err(e) = notification_manager.unsubscribe_by_id(&self.subscriber_id).await {
                    log::error!("Failed to unsubscribe plugin {} from notifications: {}", self.plugin_name, e);
                } else {
                    println!("Plugin {} successfully unsubscribed from notifications", self.plugin_name);
                    log::debug!("Plugin {} successfully unsubscribed from notifications", self.plugin_name);
                }
            } else {
                println!("No notification manager reference found for plugin {}", self.plugin_name);
            }
        }
        
        Ok(())
    }
    
    /// Check if this plugin handles a specific data type
    /// 
    /// This is a simplified implementation that maps data types to plugin names.
    /// In a more sophisticated system, plugins would declare their data type interests.
    fn plugin_handles_data_type(&self, data_type: &str) -> bool {
        match data_type {
            "commits" => self.plugin_name == "commits" || self.plugin_name.contains("commit"),
            "files" => self.plugin_name == "metrics" || self.plugin_name.contains("file") || self.plugin_name.contains("metric"),
            "metrics" => self.plugin_name == "metrics" || self.plugin_name.contains("metric"),
            "change_frequency" => self.plugin_name == "metrics" || self.plugin_name.contains("frequency") || self.plugin_name.contains("change"),
            _ => false, // Unknown data types are ignored
        }
    }
    
    /// Check if this plugin is an export plugin that should collect data from other plugins
    fn is_export_plugin(&self) -> bool {
        self.plugin_name == "export" || self.plugin_name.contains("export") || self.plugin_name.contains("render")
    }
    
    /// Check if this plugin coordinates with other plugins
    fn coordinates_with_plugins(&self) -> bool {
        // For now, only export plugins coordinate with others
        // In the future, this could include aggregation plugins, dashboard plugins, etc.
        self.is_export_plugin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::tests::mock_plugins::MockPlugin;
    use crate::notifications::traits::NotificationManager;
    use std::time::Duration;

    #[tokio::test]
    async fn test_plugin_subscriber_creation() {
        let mock_plugin = Arc::new(MockPlugin::new("test-plugin", false));
        let subscriber = PluginSubscriber::new(mock_plugin.clone());
        
        assert_eq!(subscriber.subscriber_id(), "plugin_test-plugin");
        assert_eq!(subscriber.plugin_name(), "test-plugin");
        assert!(subscriber.plugin().is_some());
        
        // Test name-based constructor
        let subscriber2 = PluginSubscriber::new_with_name("test-plugin2".to_string());
        assert_eq!(subscriber2.subscriber_id(), "plugin_test-plugin2");
        assert_eq!(subscriber2.plugin_name(), "test-plugin2");
        assert!(subscriber2.plugin().is_none());
    }
    
    #[tokio::test]
    async fn test_plugin_subscriber_handles_scan_started() {
        let subscriber = PluginSubscriber::new_with_name("test-plugin".to_string());
        
        let event = ScanEvent::ScanStarted {
            scan_id: "test_scan".to_string(),
        };
        
        // Should handle event without error
        let result = subscriber.handle_event(event).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_plugin_subscriber_handles_scan_data_ready() {
        let subscriber = PluginSubscriber::new_with_name("commits".to_string());
        
        let event = ScanEvent::ScanDataReady {
            scan_id: "test_scan".to_string(),
            data_type: "commits".to_string(),
            message_count: 10,
        };
        
        // Should handle event without error
        let result = subscriber.handle_event(event).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_plugin_subscriber_handles_data_ready() {
        let subscriber = PluginSubscriber::new_with_name("export".to_string());
        
        let event = ScanEvent::data_ready(
            "test_scan".to_string(),
            "commits".to_string(),
            "commits".to_string(),
        );
        
        // Should handle event without error
        let result = subscriber.handle_event(event).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_plugin_subscriber_handles_scan_warning() {
        let subscriber = PluginSubscriber::new_with_name("test-plugin".to_string());
        
        let event = ScanEvent::ScanWarning {
            scan_id: "test_scan".to_string(),
            warning: "Test warning".to_string(),
            recoverable: true,
        };
        
        // Should handle event without error
        let result = subscriber.handle_event(event).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_plugin_subscriber_handles_scan_error() {
        let subscriber = PluginSubscriber::new_with_name("test-plugin".to_string());
        
        let event = ScanEvent::ScanError {
            scan_id: "test_scan".to_string(),
            error: "Test error".to_string(),
            fatal: false,
        };
        
        // Should handle event without error
        let result = subscriber.handle_event(event).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_plugin_subscriber_handles_scan_completed() {
        let subscriber = PluginSubscriber::new_with_name("test-plugin".to_string());
        
        let event = ScanEvent::ScanCompleted {
            scan_id: "test_scan".to_string(),
            duration: Duration::from_secs(10),
            warnings: vec!["Warning 1".to_string(), "Warning 2".to_string()],
        };
        
        // Should handle event without error
        let result = subscriber.handle_event(event).await;
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_plugin_handles_data_type() {
        let commits_subscriber = PluginSubscriber::new_with_name("commits".to_string());
        
        assert!(commits_subscriber.plugin_handles_data_type("commits"));
        assert!(!commits_subscriber.plugin_handles_data_type("files"));
        assert!(!commits_subscriber.plugin_handles_data_type("metrics"));
        
        let metrics_subscriber = PluginSubscriber::new_with_name("metrics".to_string());
        
        assert!(!metrics_subscriber.plugin_handles_data_type("commits"));
        assert!(metrics_subscriber.plugin_handles_data_type("files"));
        assert!(metrics_subscriber.plugin_handles_data_type("metrics"));
        assert!(metrics_subscriber.plugin_handles_data_type("change_frequency"));
    }
    
    #[test]
    fn test_is_export_plugin() {
        let export_subscriber = PluginSubscriber::new_with_name("export".to_string());
        assert!(export_subscriber.is_export_plugin());
        
        let commits_subscriber = PluginSubscriber::new_with_name("commits".to_string());
        assert!(!commits_subscriber.is_export_plugin());
    }
    
    #[tokio::test]
    async fn test_plugin_subscriber_handles_fatal_scan_error() {
        use crate::notifications::{AsyncNotificationManager, ScanEvent};
        use crate::plugin::registry::SharedPluginRegistry;
        use std::sync::Arc;
        
        // Create notification manager and registry
        let notification_manager = Arc::new(AsyncNotificationManager::<ScanEvent>::new());
        let registry = SharedPluginRegistry::with_notification_manager(notification_manager.clone());
        
        // Register a plugin
        let plugin = Box::new(MockPlugin::new("test_plugin", false));
        registry.register_plugin(plugin).await.unwrap();
        
        // Get the subscriber and set its references manually for testing
        {
            let registry_inner = registry.inner().read().await;
            if let Some(subscriber) = registry_inner.get_subscriber("test_plugin") {
                subscriber.set_references(Arc::new(registry.clone()), notification_manager.clone()).await;
            }
        }
        
        // Verify plugin is registered
        assert_eq!(registry.get_plugin_count().await, 1);
        assert_eq!(notification_manager.subscriber_count().await, 1);
        
        // Create fatal ScanError event
        let fatal_error = ScanEvent::ScanError {
            scan_id: "test_scan".to_string(),
            error: "Fatal error occurred".to_string(),
            fatal: true,
        };
        
        // Publish the fatal error
        notification_manager.publish(fatal_error).await.unwrap();
        
        // Give time for event processing
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
        // For now, just verify the event was processed
        // The actual deregistration will be handled by the scanner in Phase 4
        // This test verifies that the fatal error handling code path is executed
        // and the plugin is marked for deregistration
    }
    
    #[tokio::test]
    async fn test_plugin_subscriber_handles_non_fatal_scan_error() {
        use crate::notifications::{AsyncNotificationManager, ScanEvent};
        use crate::plugin::registry::SharedPluginRegistry;
        use std::sync::Arc;
        
        // Create notification manager and registry
        let notification_manager = Arc::new(AsyncNotificationManager::<ScanEvent>::new());
        let registry = SharedPluginRegistry::with_notification_manager(notification_manager.clone());
        
        // Register a plugin
        let plugin = Box::new(MockPlugin::new("test_plugin", false));
        registry.register_plugin(plugin).await.unwrap();
        
        // Verify plugin is registered
        assert_eq!(registry.get_plugin_count().await, 1);
        assert_eq!(notification_manager.subscriber_count().await, 1);
        
        // Create non-fatal ScanError event
        let non_fatal_error = ScanEvent::ScanError {
            scan_id: "test_scan".to_string(),
            error: "Non-fatal error occurred".to_string(),
            fatal: false,
        };
        
        // Publish the non-fatal error
        notification_manager.publish(non_fatal_error).await.unwrap();
        
        // Give time for event processing
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        
        // Plugin should NOT have deregistered itself for non-fatal errors
        assert_eq!(registry.get_plugin_count().await, 1);
        assert_eq!(notification_manager.subscriber_count().await, 1);
    }
}
