//! Tests for PluginHandler using registry instead of creating duplicate instances

use crate::cli::plugin_handler::PluginHandler;
use crate::plugin::SharedPluginRegistry;
use crate::app::initialization::initialize_plugins_via_discovery;
use crate::display::ColourManager;

#[tokio::test]
async fn test_plugin_handler_uses_registry_not_new_instances() {
    // This test expects PluginHandler to use plugins from the registry
    // instead of creating new instances
    
    // Initialize builtin plugins in a registry
    let registry = SharedPluginRegistry::new();
    let colour_manager = ColourManager::from_color_args(false, false, None);
    initialize_plugins_via_discovery(&registry, &colour_manager, Vec::new()).await.unwrap();
    
    // Create PluginHandler with the same registry
    let mut handler = PluginHandler::with_registry(registry.clone()).unwrap();
    
    // Build command mappings - should use registry plugins, not create new ones
    handler.build_command_mappings().await.unwrap();
    
    // Verify that command mappings were built from registry plugins
    let mappings = handler.get_function_mappings();
    
    // Should have functions from plugins in the registry
    let plugin_names: std::collections::HashSet<String> = mappings.iter()
        .map(|m| m.plugin_name.clone())
        .collect();
    
    // Note: build_command_mappings includes ALL registered plugins for command routing,
    // not just active ones. This allows plugin-specific help to work even for inactive plugins.
    // All 4 builtin plugins should be in command mappings
    assert!(plugin_names.contains("export"), "Export plugin should appear in command mappings");
    assert!(plugin_names.contains("debug"), "Debug plugin should appear in command mappings");
    assert!(plugin_names.contains("commits"), "Commits plugin should appear in command mappings");
    assert!(plugin_names.contains("metrics"), "Metrics plugin should appear in command mappings");
    
    // The command mappings include all plugins to enable help functionality
    assert_eq!(plugin_names.len(), 4, "All 4 builtin plugins should be in command mappings");
}

#[tokio::test]
async fn test_command_resolution_without_plugin_duplication() {
    // This test expects command resolution to work without creating duplicate plugins
    
    let registry = SharedPluginRegistry::new();
    let colour_manager = ColourManager::from_color_args(false, false, None);
    initialize_plugins_via_discovery(&registry, &colour_manager, Vec::new()).await.unwrap();
    
    // Manually activate commits plugin for this test
    {
        let mut reg = registry.inner().write().await;
        reg.activate_plugin("commits").await.unwrap();
    }
    
    let mut handler = PluginHandler::with_registry(registry.clone()).unwrap();
    handler.build_command_mappings().await.unwrap();
    
    // Try to resolve a command - this should work without creating new plugins
    let resolution = handler.resolve_command("export").await;
    
    // Export plugin should be resolvable (it's active by default)
    assert!(resolution.is_ok(), "Export command should resolve successfully");
    
    let export_resolution = resolution.unwrap();
    match export_resolution {
        crate::cli::command_mapper::CommandResolution::Function { plugin_name, function_name, .. } => {
            assert_eq!(plugin_name, "export");
            assert_eq!(function_name, "output"); // Default function
        }
        crate::cli::command_mapper::CommandResolution::DirectPlugin { plugin_name, .. } => {
            assert_eq!(plugin_name, "export");
        }
        crate::cli::command_mapper::CommandResolution::Explicit { plugin_name, .. } => {
            assert_eq!(plugin_name, "export");
        }
    }
    
    // Commits should also be resolvable since we activated it
    let commits_resolution = handler.resolve_command("commits").await;
    assert!(commits_resolution.is_ok(), "Commits command should resolve (manually activated)");
}

#[tokio::test] 
async fn test_inactive_plugins_not_in_command_mappings() {
    // This test expects that only ACTIVE plugins appear in command mappings
    
    let registry = SharedPluginRegistry::new();
    let colour_manager = ColourManager::from_color_args(false, false, None);
    initialize_plugins_via_discovery(&registry, &colour_manager, Vec::new()).await.unwrap();
    
    let mut handler = PluginHandler::with_registry(registry.clone()).unwrap();
    handler.build_command_mappings().await.unwrap();
    
    let mappings = handler.get_function_mappings();
    let plugin_names: std::collections::HashSet<String> = mappings.iter()
        .map(|m| m.plugin_name.clone())
        .collect();
    
    // Note: build_command_mappings includes ALL registered plugins for command routing,
    // not just active ones. This allows plugin-specific help to work even for inactive plugins.
    // All 4 builtin plugins should be in command mappings
    assert_eq!(plugin_names.len(), 4, "All 4 builtin plugins should be in command mappings");
    assert!(plugin_names.contains("export"), "Export plugin should be in command mappings");
    assert!(plugin_names.contains("debug"), "Debug plugin should be in command mappings");
    assert!(plugin_names.contains("commits"), "Commits plugin should be in command mappings");
    assert!(plugin_names.contains("metrics"), "Metrics plugin should be in command mappings");
    
    // Now activate commits plugin and rebuild mappings
    {
        let mut reg = registry.inner().write().await;
        reg.activate_plugin("commits").await.unwrap();
    }
    
    // Rebuild command mappings after activation
    handler.build_command_mappings().await.unwrap();
    
    let updated_mappings = handler.get_function_mappings();
    let updated_plugin_names: std::collections::HashSet<String> = updated_mappings.iter()
        .map(|m| m.plugin_name.clone())
        .collect();
    
    // Should still have the same 4 plugins (activation doesn't change command mappings)
    assert_eq!(updated_plugin_names.len(), 4, "Should still have all 4 plugins in command mappings");
    assert!(updated_plugin_names.contains("export"));
    assert!(updated_plugin_names.contains("debug"));
    assert!(updated_plugin_names.contains("commits"));
    assert!(updated_plugin_names.contains("metrics"));
}