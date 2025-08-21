//! Plugin Encapsulation Tests
//! 
//! These tests ensure that the plugin system maintains proper architectural boundaries:
//! 1. No hardcoded plugin names outside of plugin discovery
//! 2. Plugin discovery works dynamically without external dependencies
//! 3. Plugin registration follows proper encapsulation principles

use std::collections::HashSet;
use crate::plugin::{Plugin, PluginRegistry};

/// Test that plugin discovery works without hardcoded names
#[tokio::test]
async fn test_plugin_discovery_without_hardcoded_names() {
    let mut registry = PluginRegistry::new();
    
    // Create builtin plugins dynamically without hardcoding names
    let builtin_plugins = create_all_builtin_plugins();
    
    // Register all discovered plugins
    for plugin in builtin_plugins {
        let plugin_name = plugin.plugin_info().name.clone();
        registry.register_plugin_inactive(plugin).await
            .expect(&format!("Failed to register discovered plugin: {}", plugin_name));
    }
    
    // Verify that we have plugins registered
    let registered_plugins = registry.list_plugins();
    assert!(!registered_plugins.is_empty(), "Plugin discovery should find builtin plugins");
    
    // Verify each plugin has proper function advertisement
    for plugin_name in &registered_plugins {
        if let Some(plugin) = registry.get_plugin(plugin_name) {
            let functions = plugin.advertised_functions();
            assert!(!functions.is_empty(), 
                   "Plugin '{}' should advertise at least one function", plugin_name);
            
            // Each function should have a name and description
            for function in &functions {
                assert!(!function.name.is_empty(), 
                       "Function in plugin '{}' should have a name", plugin_name);
                assert!(!function.description.is_empty(), 
                       "Function '{}' in plugin '{}' should have a description", 
                       function.name, plugin_name);
            }
        }
    }
}

/// Test that plugin activation works based on plugin metadata, not hardcoded names
#[tokio::test]
async fn test_dynamic_plugin_activation() {
    let mut registry = PluginRegistry::new();
    
    // Register all builtin plugins
    let builtin_plugins = create_all_builtin_plugins();
    for plugin in builtin_plugins {
        registry.register_plugin_inactive(plugin).await.unwrap();
    }
    
    // Activate plugins based on their metadata, not hardcoded names
    let plugin_names = registry.list_plugins();
    let mut activated_count = 0;
    
    for plugin_name in plugin_names {
        if let Some(plugin) = registry.get_plugin(&plugin_name) {
            if plugin.plugin_info().active_by_default {
                registry.activate_plugin(&plugin_name).await
                    .expect(&format!("Failed to activate plugin: {}", plugin_name));
                activated_count += 1;
            }
        }
    }
    
    // At least one plugin should be activated by default (export)
    assert!(activated_count > 0, "At least one builtin plugin should have load_by_default=true");
    
    // Verify activated plugins are actually active
    let active_plugins = registry.get_active_plugins();
    assert_eq!(active_plugins.len(), activated_count, 
              "Number of active plugins should match activated count");
}

/// Test that command mapping works without hardcoded plugin knowledge
#[tokio::test]
async fn test_command_mapping_encapsulation() {
    let mut registry = PluginRegistry::new();
    
    // Initialize with all builtin plugins
    let builtin_plugins = create_all_builtin_plugins();
    for plugin in builtin_plugins {
        registry.register_plugin_inactive(plugin).await.unwrap();
    }
    
    // Activate plugins based on metadata
    let plugin_names = registry.list_plugins();
    for plugin_name in plugin_names {
        if let Some(plugin) = registry.get_plugin(&plugin_name) {
            if plugin.plugin_info().active_by_default {
                registry.activate_plugin(&plugin_name).await.unwrap();
            }
        }
    }
    
    // Verify that plugins are registered and have advertised functions
    let registered_plugins = registry.list_plugins();
    assert!(!registered_plugins.is_empty(), "Plugins should be registered");
    
    // Every plugin should have advertised functions that can be mapped
    for plugin_name in &registered_plugins {
        if let Some(plugin) = registry.get_plugin(plugin_name) {
            let functions = plugin.advertised_functions();
            assert!(!functions.is_empty(), "Plugin '{}' should advertise functions", plugin_name);
            
            for function in &functions {
                assert!(!function.name.is_empty(), "Function should have a name");
                assert!(!function.description.is_empty(), "Function should have a description");
            }
        }
    }
}

/// Test that help system works without plugin name assumptions
#[tokio::test]
async fn test_help_system_encapsulation() {
    let mut registry = PluginRegistry::new();
    
    // Setup plugins dynamically
    let builtin_plugins = create_all_builtin_plugins();
    for plugin in builtin_plugins {
        registry.register_plugin_inactive(plugin).await.unwrap();
    }
    
    // Test that help can be generated for discovered plugins
    let plugin_names = registry.list_plugins();
    for plugin_name in &plugin_names {
        if let Some(plugin) = registry.get_plugin(plugin_name) {
            // Test if plugin implements clap help (PluginClapParser)
            if let Some(help) = plugin.get_plugin_help() {
                assert!(!help.is_empty(), 
                       "Plugin '{}' should generate non-empty help", plugin_name);
                assert!(help.contains(plugin_name) || help.contains("Usage"),
                       "Help for plugin '{}' should contain relevant content", plugin_name);
            }
            
            // Test if plugin can build clap command
            if let Some(command) = plugin.build_clap_command() {
                let name = command.get_name();
                assert!(!name.is_empty(), 
                       "Plugin '{}' clap command should have a name", plugin_name);
            }
        }
    }
}

/// Test that no hardcoded plugin names exist in non-plugin-discovery code
#[test]
fn test_no_hardcoded_builtin_plugin_names() {
    use std::fs;
    
    // Known builtin plugin names that should NOT appear in non-discovery code
    let builtin_names = ["debug", "export", "commits", "metrics"];
    
    // Files that are allowed to reference plugin names (discovery and tests)
    let _allowed_files = [
        "src/plugin/builtin/mod.rs",
        "src/plugin/builtin/debug/mod.rs", 
        "src/plugin/builtin/export/mod.rs",
        "src/plugin/builtin/commits/mod.rs",
        "src/plugin/builtin/metrics/mod.rs",
        "src/plugin/tests/",  // All test files
    ];
    
    // Check main application files for violations
    let violation_prone_files = [
        "src/main.rs",
        "src/app/initialization.rs",
        "src/app/execution.rs", 
        "src/cli/plugin_handler.rs",
        "src/cli/command_segmenter.rs",
    ];
    
    for file_path in &violation_prone_files {
        if let Ok(content) = fs::read_to_string(file_path) {
            for plugin_name in &builtin_names {
                // Look for hardcoded plugin names in strings
                let patterns = [
                    &format!("\"{}\"", plugin_name),
                    &format!("'{}'", plugin_name),
                    &format!("activate_plugin(\"{}\")", plugin_name),
                    &format!("register_plugin.*{}", plugin_name),
                ];
                
                for pattern in &patterns {
                    if content.contains(*pattern) {
                        // Allow if it's in a comment or test function
                        let lines: Vec<&str> = content.lines().collect();
                        let mut in_test_function = false;
                        
                        for (line_num, line) in lines.iter().enumerate() {
                            let trimmed = line.trim();
                            
                            // Track if we're in a test function
                            if trimmed.starts_with("#[tokio::test]") || trimmed.starts_with("#[test]") {
                                in_test_function = true;
                                continue;
                            }
                            
                            // Exit test function when we hit another function or impl block
                            if (trimmed.starts_with("fn ") || trimmed.starts_with("async fn ") || 
                                trimmed.starts_with("impl ") || trimmed.starts_with("pub fn ")) 
                                && !trimmed.contains("test_") {
                                in_test_function = false;
                            }
                            
                            if line.contains(*pattern) && 
                               !line.trim_start().starts_with("//") && 
                               !in_test_function {
                                panic!(
                                    "VIOLATION: Hardcoded plugin name '{}' found in {}:{}\n\
                                     Line: {}\n\
                                     Plugin names should only be discovered dynamically!",
                                    plugin_name, file_path, line_num + 1, line.trim()
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Test helper to ensure all builtin plugins have consistent metadata
#[tokio::test]
async fn test_builtin_plugin_metadata_consistency() {
    let builtin_plugins = create_all_builtin_plugins();
    
    let mut plugin_names = HashSet::new();
    let mut function_names = HashSet::new();
    
    for plugin in builtin_plugins {
        let info = plugin.plugin_info();
        
        // Plugin names must be unique
        assert!(!plugin_names.contains(&info.name), 
               "Duplicate plugin name: {}", info.name);
        plugin_names.insert(info.name.clone());
        
        // Plugin must have required metadata
        assert!(!info.name.is_empty(), "Plugin must have a name");
        assert!(!info.description.is_empty(), "Plugin must have a description");
        assert!(!info.version.is_empty(), "Plugin must have a version");
        
        // Plugin must advertise functions
        let functions = plugin.advertised_functions();
        assert!(!functions.is_empty(), "Plugin '{}' must advertise functions", info.name);
        
        // Function names must be unique across all plugins
        for function in &functions {
            assert!(!function_names.contains(&function.name),
                   "Duplicate function name '{}' in plugin '{}'", function.name, info.name);
            function_names.insert(function.name.clone());
            
            // Function must have metadata
            assert!(!function.name.is_empty(), "Function must have a name");
            assert!(!function.description.is_empty(), "Function must have a description");
        }
        
        // Exactly one function should be default per plugin
        let default_functions: Vec<_> = functions.iter().filter(|f| f.is_default).collect();
        assert_eq!(default_functions.len(), 1, 
                  "Plugin '{}' must have exactly one default function", info.name);
    }
}

/// Centralized helper function to create all builtin plugins
/// This is the ONLY place where builtin plugin names should be hardcoded
fn create_all_builtin_plugins() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(crate::plugin::builtin::debug::DebugPlugin::new()),
        Box::new(crate::plugin::builtin::export::ExportPlugin::new()),
        Box::new(crate::plugin::builtin::commits::CommitsPlugin::new()),
        Box::new(crate::plugin::builtin::metrics::MetricsPlugin::new()),
    ]
}

/// Test that the centralized helper function is the only source of builtin plugins
#[test]
fn test_centralized_builtin_creation_is_only_source() {
    // This test documents that create_all_builtin_plugins() should be the
    // single source of truth for builtin plugin instantiation
    let plugins = create_all_builtin_plugins();
    
    // Verify we have the expected number of builtin plugins
    assert!(plugins.len() >= 4, "Should have at least 4 builtin plugins");
    
    // Verify each plugin is properly initialized
    for plugin in plugins {
        let info = plugin.plugin_info();
        assert!(!info.name.is_empty(), "Plugin should have a name");
        assert!(!info.description.is_empty(), "Plugin should have a description");
        assert!(!info.version.is_empty(), "Plugin should have a version");
    }
}