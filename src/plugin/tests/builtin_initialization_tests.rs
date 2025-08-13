//! Tests for builtin plugin initialization with activation control

use crate::plugin::registry::PluginRegistry;
use crate::app::initialization::initialize_builtin_plugins;
use crate::plugin::SharedPluginRegistry;

#[tokio::test]
async fn test_builtin_plugins_loaded_as_inactive() {
    // This test expects builtin plugins to be loaded as inactive by default
    // (except for those with load_by_default = true)
    
    let shared_registry = SharedPluginRegistry::new();
    
    // Initialize builtin plugins
    initialize_builtin_plugins(&shared_registry).await.unwrap();
    
    // Access the inner registry to check plugin states
    let registry = shared_registry.inner().read().await;
    
    // Verify plugins are registered
    assert!(registry.has_plugin("commits"));
    assert!(registry.has_plugin("metrics"));
    assert!(registry.has_plugin("export"));
    
    // Expected behavior: Only export plugin should be active (load_by_default = true)
    // Other plugins should be inactive by default
    assert!(!registry.is_plugin_active("commits"), "Commits plugin should be inactive by default");
    assert!(!registry.is_plugin_active("metrics"), "Metrics plugin should be inactive by default");
    assert!(registry.is_plugin_active("export"), "Export plugin should be active by default (load_by_default = true)");
    
    // Verify active plugins list contains only export
    let active_plugins = registry.get_active_plugins();
    assert_eq!(active_plugins.len(), 1, "Only export plugin should be active");
    assert_eq!(active_plugins[0], "export");
}

#[tokio::test]
async fn test_auto_activation_of_load_by_default_plugins() {
    // This test verifies that plugins with load_by_default = true are auto-activated
    
    let shared_registry = SharedPluginRegistry::new();
    
    // Initialize builtin plugins
    initialize_builtin_plugins(&shared_registry).await.unwrap();
    
    // Access the inner registry
    let registry = shared_registry.inner().read().await;
    
    // Get plugin infos to check load_by_default settings
    let export_plugin = registry.get_plugin("export").unwrap();
    let commits_plugin = registry.get_plugin("commits").unwrap();
    let metrics_plugin = registry.get_plugin("metrics").unwrap();
    
    // Expected load_by_default settings:
    assert!(export_plugin.plugin_info().load_by_default, "Export plugin should have load_by_default = true");
    assert!(!commits_plugin.plugin_info().load_by_default, "Commits plugin should have load_by_default = false");
    assert!(!metrics_plugin.plugin_info().load_by_default, "Metrics plugin should have load_by_default = false");
    
    // Verify activation matches load_by_default settings
    assert!(registry.is_plugin_active("export"), "Export plugin should be auto-activated");
    assert!(!registry.is_plugin_active("commits"), "Commits plugin should not be auto-activated");
    assert!(!registry.is_plugin_active("metrics"), "Metrics plugin should not be auto-activated");
}

#[tokio::test]
async fn test_manual_plugin_activation_after_initialization() {
    // This test verifies that inactive plugins can be manually activated
    
    let shared_registry = SharedPluginRegistry::new();
    
    // Initialize builtin plugins
    initialize_builtin_plugins(&shared_registry).await.unwrap();
    
    // Access the inner registry for activation
    {
        let mut registry = shared_registry.inner().write().await;
        
        // Manually activate commits plugin
        registry.activate_plugin("commits").await.unwrap();
    }
    
    // Verify activation worked
    {
        let registry = shared_registry.inner().read().await;
        assert!(registry.is_plugin_active("commits"), "Commits plugin should be active after manual activation");
        
        // Should now have 2 active plugins (export + commits)
        let active_plugins = registry.get_active_plugins();
        assert_eq!(active_plugins.len(), 2, "Should have 2 active plugins after manual activation");
        assert!(active_plugins.contains(&"export".to_string()));
        assert!(active_plugins.contains(&"commits".to_string()));
    }
}