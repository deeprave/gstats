//! Priority-based Plugin Registration Tests

use crate::plugin::traits::{PluginInfo, PluginType};
use crate::plugin::registry::PluginRegistry;
use crate::plugin::tests::mock_plugins::MockPlugin;

#[tokio::test]
async fn test_plugins_ordered_by_priority() {
    let mut registry = PluginRegistry::new();
    
    // Create plugins with different priorities using builder pattern
    let plugin_low = MockPlugin::new("low-priority-plugin", false).with_priority(1);
    let plugin_high = MockPlugin::new("high-priority-plugin", false).with_priority(10);
    let plugin_medium = MockPlugin::new("medium-priority-plugin", false).with_priority(5);
    let plugin_default = MockPlugin::new("default-priority-plugin", false).with_priority(0); // Override to priority 0
    
    // Register plugins in random order
    registry.register_plugin(Box::new(plugin_medium)).await.unwrap();
    registry.register_plugin(Box::new(plugin_low)).await.unwrap();
    registry.register_plugin(Box::new(plugin_default)).await.unwrap();
    registry.register_plugin(Box::new(plugin_high)).await.unwrap();
    
    // Get plugins by type - should be ordered by priority (high to low)
    let plugin_names = registry.get_plugins_by_type(PluginType::Processing);
    
    // Should be in priority order: 10, 5, 1, 0
    assert_eq!(plugin_names.len(), 4);
    assert_eq!(plugin_names[0], "high-priority-plugin");
    assert_eq!(plugin_names[1], "medium-priority-plugin");
    assert_eq!(plugin_names[2], "low-priority-plugin");
    assert_eq!(plugin_names[3], "default-priority-plugin");
}

#[tokio::test] 
async fn test_plugins_with_equal_priority() {
    let mut registry = PluginRegistry::new();
    
    // Create multiple plugins with the same priority
    let plugin_a = MockPlugin::new("plugin-a", false).with_priority(5);
    let plugin_b = MockPlugin::new("plugin-b", false).with_priority(5);
    let plugin_c = MockPlugin::new("plugin-c", false).with_priority(5);
    
    // Register plugins in specific order
    registry.register_plugin(Box::new(plugin_b)).await.unwrap();
    registry.register_plugin(Box::new(plugin_a)).await.unwrap();
    registry.register_plugin(Box::new(plugin_c)).await.unwrap();
    
    // Get plugins by type
    let plugin_names = registry.get_plugins_by_type(PluginType::Processing);
    
    // All should have same priority, but order should be consistent
    assert_eq!(plugin_names.len(), 3);
    
    // All plugins should be present
    assert!(plugin_names.contains(&"plugin-a".to_string()));
    assert!(plugin_names.contains(&"plugin-b".to_string()));
    assert!(plugin_names.contains(&"plugin-c".to_string()));
}

#[tokio::test]
async fn test_capability_based_ordering() {
    let mut registry = PluginRegistry::new();
    
    // Create plugins with different priorities and capabilities
    let plugin_high = MockPlugin::new("high-priority-scanner", false)
        .with_priority(10)
        .with_capability("advanced-analysis", "Advanced analysis capability");
    
    let plugin_low = MockPlugin::new("low-priority-scanner", false)
        .with_priority(1)
        .with_capability("advanced-analysis", "Advanced analysis capability");
    
    // Register plugins
    registry.register_plugin(Box::new(plugin_low)).await.unwrap();
    registry.register_plugin(Box::new(plugin_high)).await.unwrap();
    
    // Get plugins with capability - should be ordered by priority
    let plugin_names = registry.get_plugins_with_capability("advanced-analysis");
    
    assert_eq!(plugin_names.len(), 2);
    assert_eq!(plugin_names[0], "high-priority-scanner");
    assert_eq!(plugin_names[1], "low-priority-scanner");
}

#[test]
fn test_plugin_info_priority_builder() {
    let info = PluginInfo::new(
        "test-plugin".to_string(),
        "1.0.0".to_string(),
        1,
        "Test plugin".to_string(),
        "Test Author".to_string(),
        PluginType::Processing,
    ).with_priority(15);
    
    assert_eq!(info.priority, 15);
    assert_eq!(info.name, "test-plugin");
}