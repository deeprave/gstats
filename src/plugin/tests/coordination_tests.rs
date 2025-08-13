//! Plugin Coordination Tests
//! 
//! Tests for plugin lifecycle coordination needed for safe scanner shutdown.

use crate::plugin::{
    registry::PluginRegistry,
    traits::PluginState,
};
use crate::plugin::tests::mock_plugins::MockPlugin;

/// Test that plugins can transition to Processing state when handling work
#[tokio::test]
async fn test_plugin_processing_state_transition() {
    let mut registry = PluginRegistry::new();
    
    // Create a mock plugin (should_fail: false for success case)
    let plugin = MockPlugin::new("test-plugin", false);
    
    // Register plugin
    registry.register_plugin(Box::new(plugin)).await.unwrap();
    
    // Transition to Processing state when starting work
    registry.transition_plugin_state("test-plugin", PluginState::Processing).await.unwrap();
    
    // Transition back to Initialized when work complete
    registry.transition_plugin_state("test-plugin", PluginState::Initialized).await.unwrap();
}

/// Test that plugins can transition to Error state on failures
#[tokio::test]
async fn test_plugin_error_state_transition() {
    let mut registry = PluginRegistry::new();
    
    // Create a mock plugin
    let plugin = MockPlugin::new("test-plugin", false);
    
    // Register plugin
    registry.register_plugin(Box::new(plugin)).await.unwrap();
    
    // Transition to Error state on failure
    let error_msg = "Plugin execution failed";
    registry.transition_plugin_state("test-plugin", PluginState::Error(error_msg.to_string())).await.unwrap();
}

/// Test coordination method to check if all active plugins are idle (not processing)
#[tokio::test]
async fn test_are_all_active_plugins_idle() {
    let mut registry = PluginRegistry::new();
    
    // Create multiple mock plugins
    let plugin1 = MockPlugin::new("plugin1", false);
    let plugin2 = MockPlugin::new("plugin2", false);
    
    // Register plugins
    registry.register_plugin(Box::new(plugin1)).await.unwrap();
    registry.register_plugin(Box::new(plugin2)).await.unwrap();
    
    // Activate both plugins
    registry.activate_plugin("plugin1").await.unwrap();
    registry.activate_plugin("plugin2").await.unwrap();
    
    // Initially all active plugins should be idle (default Initialized state)
    assert!(registry.are_all_active_plugins_idle(), "All active plugins should be idle initially");
    
    // Put one plugin into Processing state
    registry.transition_plugin_state("plugin1", PluginState::Processing).await.unwrap();
    
    // Now not all active plugins are idle
    assert!(!registry.are_all_active_plugins_idle(), "Not all active plugins should be idle when one is processing");
    
    // Return plugin to idle state
    registry.transition_plugin_state("plugin1", PluginState::Initialized).await.unwrap();
    
    // Now all active plugins are idle again
    assert!(registry.are_all_active_plugins_idle(), "All active plugins should be idle when processing is done");
}

/// Test coordination method with error states
#[tokio::test]
async fn test_idle_coordination_with_error_states() {
    let mut registry = PluginRegistry::new();
    
    // Create multiple mock plugins
    let plugin1 = MockPlugin::new("plugin1", false);
    let plugin2 = MockPlugin::new("plugin2", false);
    
    // Register and activate plugins
    registry.register_plugin(Box::new(plugin1)).await.unwrap();
    registry.register_plugin(Box::new(plugin2)).await.unwrap();
    registry.activate_plugin("plugin1").await.unwrap();
    registry.activate_plugin("plugin2").await.unwrap();
    
    // Put one plugin into Error state
    registry.transition_plugin_state("plugin1", PluginState::Error("Failed".to_string())).await.unwrap();
    
    // Error plugins should be considered "done" (not blocking shutdown)
    assert!(registry.are_all_active_plugins_idle(), "Error plugins should be considered idle/done");
    
    // Put other plugin into Processing state
    registry.transition_plugin_state("plugin2", PluginState::Processing).await.unwrap();
    
    // Now we have one error + one processing, so not idle
    assert!(!registry.are_all_active_plugins_idle(), "Processing plugin should block idle state");
}

/// Test coordination with mixed active/inactive plugins
#[tokio::test] 
async fn test_idle_coordination_with_activation() {
    let mut registry = PluginRegistry::new();
    
    // Create multiple mock plugins
    let plugin1 = MockPlugin::new("plugin1", false);
    let plugin2 = MockPlugin::new("plugin2", false);
    
    // Register plugins - plugin1 active, plugin2 inactive
    registry.register_plugin(Box::new(plugin1)).await.unwrap();
    registry.register_plugin_inactive(Box::new(plugin2)).await.unwrap();
    
    // plugin1 is already active from register_plugin, plugin2 is inactive from register_plugin_inactive
    
    // Put inactive plugin2 into Processing state
    registry.transition_plugin_state("plugin2", PluginState::Processing).await.unwrap();
    
    // Inactive plugins shouldn't block shutdown even if processing
    assert!(registry.are_all_active_plugins_idle(), "Inactive plugins shouldn't block shutdown");
    
    // Put active plugin1 into Processing state  
    registry.transition_plugin_state("plugin1", PluginState::Processing).await.unwrap();
    
    // Active processing plugins should block shutdown
    assert!(!registry.are_all_active_plugins_idle(), "Active processing plugins should block shutdown");
}

/// Test async wait for all plugins to become idle
#[tokio::test]
async fn test_wait_for_all_plugins_idle() {
    use std::time::Duration;
    
    let mut registry = PluginRegistry::new();
    
    // Create mock plugin
    let plugin = MockPlugin::new("test-plugin", false);
    registry.register_plugin(Box::new(plugin)).await.unwrap();
    registry.activate_plugin("test-plugin").await.unwrap();
    
    // Put plugin into processing state
    registry.transition_plugin_state("test-plugin", PluginState::Processing).await.unwrap();
    
    // Test that plugin is not idle initially
    assert!(!registry.are_all_active_plugins_idle(), "Plugin should not be idle while processing");
    
    // Return plugin to idle state
    registry.transition_plugin_state("test-plugin", PluginState::Initialized).await.unwrap();
    
    // Wait for plugins to become idle (should succeed immediately)
    let result = registry.wait_for_all_plugins_idle(Duration::from_millis(100)).await;
    assert!(result.is_ok(), "Should successfully wait for plugins to become idle");
}

/// Test timeout when waiting for plugins to become idle
#[tokio::test]
async fn test_wait_for_plugins_idle_timeout() {
    use std::time::Duration;
    
    let mut registry = PluginRegistry::new();
    
    // Create mock plugin
    let plugin = MockPlugin::new("test-plugin", false);
    registry.register_plugin(Box::new(plugin)).await.unwrap();
    registry.activate_plugin("test-plugin").await.unwrap();
    
    // Put plugin into processing state and leave it there
    registry.transition_plugin_state("test-plugin", PluginState::Processing).await.unwrap();
    
    // Wait should timeout since plugin never returns to idle
    let result = registry.wait_for_all_plugins_idle(Duration::from_millis(50)).await;
    assert!(result.is_err(), "Should timeout when plugins don't become idle");
}