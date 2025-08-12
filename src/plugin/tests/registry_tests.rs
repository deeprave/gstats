//! Tests for Plugin Registry
//! 
//! Tests plugin registration, lifecycle management, and state handling.

use super::mock_plugins::*;
use crate::plugin::registry::PluginRegistry;
use crate::plugin::error::PluginError;
use crate::plugin::traits::{Plugin, PluginState, PluginType};
use std::sync::Arc;

#[tokio::test]
async fn test_plugin_registry_registration() {
    let mut registry = PluginRegistry::new();
    let plugin = Box::new(MockPlugin::new("test-plugin", false));
    
    // Test successful registration
    let result = registry.register_plugin(plugin).await;
    assert!(result.is_ok());
    
    // Test duplicate registration
    let duplicate = Box::new(MockPlugin::new("test-plugin", false));
    let result = registry.register_plugin(duplicate).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), PluginError::PluginAlreadyRegistered { .. }));
}

#[tokio::test]
async fn test_plugin_registry_get_plugin() {
    let mut registry = PluginRegistry::new();
    let plugin = Box::new(MockPlugin::new("test-plugin", false));
    
    registry.register_plugin(plugin).await.unwrap();
    
    // Test getting existing plugin
    let retrieved = registry.get_plugin("test-plugin");
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().plugin_info().name, "test-plugin");
    
    // Test getting non-existent plugin
    let missing = registry.get_plugin("missing-plugin");
    assert!(missing.is_none());
}

#[tokio::test]
async fn test_plugin_registry_unregister() {
    let mut registry = PluginRegistry::new();
    let plugin = Box::new(MockPlugin::new("test-plugin", false));
    
    registry.register_plugin(plugin).await.unwrap();
    
    // Test successful unregistration
    let result = registry.unregister_plugin("test-plugin").await;
    assert!(result.is_ok());
    
    // Verify plugin is removed
    let retrieved = registry.get_plugin("test-plugin");
    assert!(retrieved.is_none());
    
    // Test unregistering non-existent plugin
    let result = registry.unregister_plugin("missing-plugin").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), PluginError::PluginNotFound { .. }));
}

#[tokio::test]
async fn test_plugin_registry_list_plugins() {
    let mut registry = PluginRegistry::new();
    
    // Empty registry
    assert_eq!(registry.list_plugins().len(), 0);
    
    // Add plugins
    registry.register_plugin(Box::new(MockPlugin::new("plugin1", false))).await.unwrap();
    registry.register_plugin(Box::new(MockPlugin::new("plugin2", false))).await.unwrap();
    registry.register_plugin(Box::new(MockScannerPlugin::new("scanner1", ScanMode::FILES, false))).await.unwrap();
    
    // Check list
    let plugins = registry.list_plugins();
    assert_eq!(plugins.len(), 3);
    assert!(plugins.contains(&"plugin1".to_string()));
    assert!(plugins.contains(&"plugin2".to_string()));
    assert!(plugins.contains(&"scanner1".to_string()));
}

#[tokio::test]
async fn test_plugin_registry_get_plugins_by_type() {
    let mut registry = PluginRegistry::new();
    
    // Add different types of plugins
    registry.register_plugin(Box::new(MockPlugin::new("plugin1", false))).await.unwrap();
    registry.register_plugin(Box::new(MockScannerPlugin::new("scanner1", ScanMode::FILES, false))).await.unwrap();
    registry.register_plugin(Box::new(MockScannerPlugin::new("scanner2", ScanMode::HISTORY, false))).await.unwrap();
    registry.register_plugin(Box::new(MockNotificationPlugin::new("notifier1", false))).await.unwrap();
    
    // Get scanner plugins
    let scanners = registry.get_plugins_by_type(PluginType::Scanner);
    assert_eq!(scanners.len(), 3); // MockPlugin also reports as Scanner type
    
    // Get notification plugins
    let notifiers = registry.get_plugins_by_type(PluginType::Notification);
    assert_eq!(notifiers.len(), 1);
}

#[tokio::test]
async fn test_plugin_registry_initialization() {
    let mut registry = PluginRegistry::new();
    let context = create_test_context();
    
    // Register plugins
    registry.register_plugin(Box::new(MockPlugin::new("plugin1", false))).await.unwrap();
    registry.register_plugin(Box::new(MockPlugin::new("plugin2", true))).await.unwrap(); // Will fail
    
    // Initialize all plugins
    let results = registry.initialize_all(&context).await;
    
    // Check results
    assert_eq!(results.len(), 2);
    assert!(results.get("plugin1").unwrap().is_ok());
    assert!(results.get("plugin2").unwrap().is_err());
    
    // Verify plugin states
    let plugin1 = registry.get_plugin("plugin1").unwrap();
    assert_eq!(plugin1.plugin_state(), PluginState::Initialized);
}

#[tokio::test]
async fn test_plugin_registry_cleanup() {
    let mut registry = PluginRegistry::new();
    let context = create_test_context();
    
    // Register and initialize plugins
    let mut plugin1 = MockPlugin::new("plugin1", false);
    let mut plugin2 = MockPlugin::new("plugin2", false);
    
    // Initialize plugins before registering
    plugin1.initialize(&context).await.unwrap();
    plugin2.initialize(&context).await.unwrap();
    
    registry.register_plugin(Box::new(plugin1)).await.unwrap();
    registry.register_plugin(Box::new(plugin2)).await.unwrap();
    
    // Cleanup all plugins
    let results = registry.cleanup_all().await;
    
    // Check results
    assert_eq!(results.len(), 2);
    assert!(results.get("plugin1").unwrap().is_ok());
    assert!(results.get("plugin2").unwrap().is_ok());
}

#[tokio::test]
async fn test_plugin_registry_concurrent_access() {
    let registry = Arc::new(tokio::sync::RwLock::new(PluginRegistry::new()));
    
    // Register plugin
    {
        let mut reg = registry.write().await;
        reg.register_plugin(Box::new(MockPlugin::new("shared-plugin", false))).await.unwrap();
    }
    
    // Concurrent reads
    let mut handles = Vec::new();
    for i in 0..10 {
        let registry_clone = Arc::clone(&registry);
        let handle = tokio::spawn(async move {
            let reg = registry_clone.read().await;
            let plugin = reg.get_plugin("shared-plugin");
            assert!(plugin.is_some());
            format!("Reader {} found plugin", i)
        });
        handles.push(handle);
    }
    
    // Wait for all reads
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.starts_with("Reader"));
    }
}

#[tokio::test]
async fn test_plugin_registry_state_transitions() {
    let mut registry = PluginRegistry::new();
    let context = create_test_context();
    
    // Register plugin
    let plugin = Box::new(MockPlugin::new("state-test", false));
    registry.register_plugin(plugin).await.unwrap();
    
    // Check initial state
    {
        let plugin = registry.get_plugin("state-test").unwrap();
        assert_eq!(plugin.plugin_state(), PluginState::Unloaded);
    }
    
    // Initialize
    {
        let plugin = registry.get_plugin_mut("state-test").unwrap();
        plugin.initialize(&context).await.unwrap();
    }
    {
        let plugin = registry.get_plugin("state-test").unwrap();
        assert_eq!(plugin.plugin_state(), PluginState::Initialized);
    }
    
    // Execute (changes to Running and back)
    {
        let plugin = registry.get_plugin("state-test").unwrap();
        let request = create_test_request(ScanMode::FILES);
        plugin.execute(request).await.unwrap();
    }
    
    // Cleanup
    {
        let plugin = registry.get_plugin_mut("state-test").unwrap();
        plugin.cleanup().await.unwrap();
    }
    {
        let plugin = registry.get_plugin("state-test").unwrap();
        assert_eq!(plugin.plugin_state(), PluginState::Unloaded);
    }
}

#[tokio::test]
async fn test_plugin_registry_error_handling() {
    let mut registry = PluginRegistry::new();
    let context = create_test_context();
    
    // Register failing plugin
    registry.register_plugin(Box::new(MockPlugin::new("failing-plugin", true))).await.unwrap();
    
    // Try to initialize - should fail but not panic
    let results = registry.initialize_all(&context).await;
    assert!(results.get("failing-plugin").unwrap().is_err());
    
    // Registry should still be functional
    registry.register_plugin(Box::new(MockPlugin::new("good-plugin", false))).await.unwrap();
    let good_results = registry.initialize_all(&context).await;
    assert!(good_results.get("good-plugin").unwrap().is_ok());
}

#[tokio::test]
async fn test_plugin_registry_capability_search() {
    let mut registry = PluginRegistry::new();
    
    // Add plugins with different capabilities
    let mut plugin1 = MockPlugin::new("plugin1", false);
    plugin1.add_capability(
        "async".to_string(),
        "Async support".to_string(),
        "1.0.0".to_string(),
    );
    
    let mut plugin2 = MockPlugin::new("plugin2", false);
    plugin2.add_capability(
        "streaming".to_string(),
        "Streaming support".to_string(),
        "1.0.0".to_string(),
    );
    
    registry.register_plugin(Box::new(plugin1)).await.unwrap();
    registry.register_plugin(Box::new(plugin2)).await.unwrap();
    
    // Search by capability
    let async_plugins = registry.get_plugins_with_capability("async");
    assert_eq!(async_plugins.len(), 1);
    assert_eq!(async_plugins[0], "plugin1");
    
    let streaming_plugins = registry.get_plugins_with_capability("streaming");
    assert_eq!(streaming_plugins.len(), 1);
    assert_eq!(streaming_plugins[0], "plugin2");
    
    let missing = registry.get_plugins_with_capability("missing");
    assert_eq!(missing.len(), 0);
}