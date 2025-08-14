//! Plugin Activation Tests

// Plugin trait imports removed - functionality moved to integration tests
use crate::plugin::registry::PluginRegistry;
use crate::plugin::tests::mock_plugins::MockPlugin;

#[tokio::test]
async fn test_register_plugin_inactive() {
    let mut registry = PluginRegistry::new();
    
    // Create a plugin that should be loaded but inactive
    let plugin = MockPlugin::new("test-plugin", false);
    
    // Register plugin as inactive
    registry.register_plugin_inactive(Box::new(plugin)).await.unwrap();
    
    // Plugin should be registered but not active
    assert!(registry.has_plugin("test-plugin"));
    assert!(!registry.is_plugin_active("test-plugin"));
    
    // Should not be in active plugins list
    let active_plugins = registry.get_active_plugins();
    assert_eq!(active_plugins.len(), 0);
}

#[tokio::test]
async fn test_activate_plugin() {
    let mut registry = PluginRegistry::new();
    
    // Register plugin as inactive
    let plugin = MockPlugin::new("test-plugin", false);
    registry.register_plugin_inactive(Box::new(plugin)).await.unwrap();
    
    // Activate the plugin
    registry.activate_plugin("test-plugin").await.unwrap();
    
    // Plugin should now be active
    assert!(registry.is_plugin_active("test-plugin"));
    
    // Should be in active plugins list
    let active_plugins = registry.get_active_plugins();
    assert_eq!(active_plugins.len(), 1);
    assert_eq!(active_plugins[0], "test-plugin");
}

#[tokio::test]
async fn test_deactivate_plugin() {
    let mut registry = PluginRegistry::new();
    
    // Register and activate plugin
    let plugin = MockPlugin::new("test-plugin", false);
    registry.register_plugin_inactive(Box::new(plugin)).await.unwrap();
    registry.activate_plugin("test-plugin").await.unwrap();
    
    // Verify it's active
    assert!(registry.is_plugin_active("test-plugin"));
    
    // Deactivate the plugin
    registry.deactivate_plugin("test-plugin").await.unwrap();
    
    // Plugin should no longer be active
    assert!(!registry.is_plugin_active("test-plugin"));
    
    // Should not be in active plugins list
    let active_plugins = registry.get_active_plugins();
    assert_eq!(active_plugins.len(), 0);
}

#[tokio::test]
async fn test_auto_activate_load_by_default_plugins() {
    let mut registry = PluginRegistry::new();
    
    // Create export plugin with load_by_default = true
    let export_plugin = MockPlugin::new("export", false)
        .with_load_by_default(true);
    
    // Create commits plugin with load_by_default = false
    let commits_plugin = MockPlugin::new("commits", false)
        .with_load_by_default(false);
    
    // Register plugins as inactive
    registry.register_plugin_inactive(Box::new(export_plugin)).await.unwrap();
    registry.register_plugin_inactive(Box::new(commits_plugin)).await.unwrap();
    
    // Auto-activate plugins marked with load_by_default = true
    registry.auto_activate_default_plugins().await.unwrap();
    
    // Export plugin should be active, commits should not
    assert!(registry.is_plugin_active("export"));
    assert!(!registry.is_plugin_active("commits"));
    
    // Only export plugin should be in active list
    let active_plugins = registry.get_active_plugins();
    assert_eq!(active_plugins.len(), 1);
    assert_eq!(active_plugins[0], "export");
}

#[tokio::test]
async fn test_activate_nonexistent_plugin() {
    let mut registry = PluginRegistry::new();
    
    // Try to activate a plugin that doesn't exist
    let result = registry.activate_plugin("nonexistent").await;
    
    // Should return an error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_multiple_plugin_activation() {
    let mut registry = PluginRegistry::new();
    
    // Register multiple plugins as inactive
    let plugin1 = MockPlugin::new("plugin1", false);
    let plugin2 = MockPlugin::new("plugin2", false);
    let plugin3 = MockPlugin::new("plugin3", false);
    
    registry.register_plugin_inactive(Box::new(plugin1)).await.unwrap();
    registry.register_plugin_inactive(Box::new(plugin2)).await.unwrap();
    registry.register_plugin_inactive(Box::new(plugin3)).await.unwrap();
    
    // Activate specific plugins
    registry.activate_plugin("plugin1").await.unwrap();
    registry.activate_plugin("plugin3").await.unwrap();
    
    // Check activation status
    assert!(registry.is_plugin_active("plugin1"));
    assert!(!registry.is_plugin_active("plugin2"));
    assert!(registry.is_plugin_active("plugin3"));
    
    // Check active plugins list
    let mut active_plugins = registry.get_active_plugins();
    active_plugins.sort(); // Sort for consistent testing
    assert_eq!(active_plugins, vec!["plugin1", "plugin3"]);
}