//! CLI Plugin Handler
//! 
//! Handles plugin discovery, listing, and validation for CLI operations.

use crate::plugin::{
    registry::SharedPluginRegistry, 
    discovery::{PluginDiscovery, UnifiedPluginDiscovery},
    traits::{PluginDescriptor, PluginType, PluginFunction},
    error::{PluginError, PluginResult}
};
use crate::cli::converter::PluginConfig;
use crate::cli::command_mapper::{CommandMapper, CommandResolution};
use std::path::{Path, PathBuf};
use anyhow::Result;
use log::debug;

/// CLI Plugin Handler for managing plugin operations
pub struct PluginHandler {
    _registry: SharedPluginRegistry,
    discovery: Box<dyn PluginDiscovery>,
    command_mapper: CommandMapper,
    plugin_config: Option<PluginConfig>,
}

impl PluginHandler {
    /// Create a new plugin handler with default plugin directory
    pub fn new() -> PluginResult<Self> {
        Self::with_plugin_directory("plugins")
    }
    
    /// Create a new plugin handler with specified plugin directory
    pub fn with_plugin_directory<P: AsRef<Path>>(plugin_dir: P) -> PluginResult<Self> {
        let plugin_path = plugin_dir.as_ref().to_path_buf();
        
        // Create plugin directory if it doesn't exist
        if !plugin_path.exists() {
            debug!("Creating plugin directory: {}", plugin_path.display());
            std::fs::create_dir_all(&plugin_path)
                .map_err(|e| PluginError::discovery_error(format!(
                    "Failed to create plugin directory {}: {}", 
                    plugin_path.display(), e
                )))?;
        }
        
        let discovery = Box::new(UnifiedPluginDiscovery::new(Some(plugin_path), Vec::new())?);
        let registry = SharedPluginRegistry::new();
        let command_mapper = CommandMapper::new();
        
        Ok(Self {
            _registry: registry,
            discovery,
            command_mapper,
            plugin_config: None,
        })
    }
    
    /// Create a new plugin handler with enhanced configuration
    pub fn with_plugin_config(config: PluginConfig) -> PluginResult<Self> {
        // Use the first directory, log about others being ignored (simplified approach)
        let plugin_directory = if config.directories.is_empty() {
            None
        } else {
            let first_dir = PathBuf::from(&config.directories[0]);
            
            // Log warning if multiple directories were specified (no longer supported)
            if config.directories.len() > 1 {
                log::warn!("Multiple plugin directories specified, only using first: {}", first_dir.display());
                for (i, dir) in config.directories.iter().enumerate().skip(1) {
                    log::warn!("  Ignoring directory {}: {}", i + 1, dir);
                }
            }
            
            // Create directory if it doesn't exist
            if !first_dir.exists() {
                debug!("Creating plugin directory: {}", first_dir.display());
                std::fs::create_dir_all(&first_dir)
                    .map_err(|e| PluginError::discovery_error(format!(
                        "Failed to create plugin directory {}: {}", 
                        first_dir.display(), e
                    )))?;
            }
            
            Some(first_dir)
        };
        
        // Log warning if explicit plugin loading was specified (no longer supported)
        if !config.plugin_load.is_empty() {
            log::warn!("Explicit plugin loading (plugin_load) is no longer supported, ignoring {} plugins", config.plugin_load.len());
            for plugin in &config.plugin_load {
                log::debug!("  Ignoring explicit load: {}", plugin);
            }
        }
        
        let discovery = Box::new(UnifiedPluginDiscovery::new(
            plugin_directory,
            config.plugin_exclude.clone(),
        )?);
        
        let registry = SharedPluginRegistry::new();
        let command_mapper = CommandMapper::new();
        
        Ok(Self {
            _registry: registry,
            discovery,
            command_mapper,
            plugin_config: Some(config),
        })
    }
    
    /// Create a new plugin handler with an existing registry
    /// This allows the handler to use plugins from a pre-populated registry
    /// instead of creating duplicate instances
    #[allow(dead_code)]
    pub fn with_registry(registry: SharedPluginRegistry) -> PluginResult<Self> {
        let discovery = Box::new(UnifiedPluginDiscovery::new(Some("plugins".into()), Vec::new())?);
        let command_mapper = CommandMapper::new();
        
        Ok(Self {
            _registry: registry,
            discovery,
            command_mapper,
            plugin_config: None,
        })
    }
    
    /// Discover and register all available plugins
    pub async fn discover_plugins(&self) -> PluginResult<Vec<PluginDescriptor>> {
        debug!("Discovering plugins in directory: {}", self.discovery.plugin_directory().display());
        
        let descriptors = self.discovery.discover_plugins().await?;
        debug!("Discovered {} plugins", descriptors.len());
        
        for descriptor in &descriptors {
            debug!("Found plugin: {} v{} ({:?})", 
                descriptor.info.name, 
                descriptor.info.version, 
                descriptor.info.plugin_type
            );
        }
        
        Ok(descriptors)
    }
    
    /// List available plugins with details
    pub async fn list_plugins(&self) -> PluginResult<Vec<PluginInfo>> {
        let descriptors = self.discover_plugins().await?;
        
        let mut plugins: Vec<PluginInfo> = descriptors.into_iter()
            .map(|desc| PluginInfo {
                name: desc.info.name.clone(),
                version: desc.info.version.clone(),
                plugin_type: desc.info.plugin_type.clone(),
                description: desc.info.description.clone(),
                author: desc.info.author.clone(),
                file_path: desc.file_path.clone(),
                capabilities: desc.info.capabilities.iter()
                    .map(|cap| cap.name.clone())
                    .collect(),
            })
            .collect();
        
        // Sort by name for consistent output
        plugins.sort_by(|a, b| a.name.cmp(&b.name));
        
        Ok(plugins)
    }
    
    
    /// Get plugin information by name
    #[allow(dead_code)]
    pub async fn get_plugin_info(&self, plugin_name: &str) -> PluginResult<Option<PluginInfo>> {
        let descriptors = self.discover_plugins().await?;
        
        for descriptor in descriptors {
            if descriptor.info.name == plugin_name {
                return Ok(Some(PluginInfo {
                    name: descriptor.info.name.clone(),
                    version: descriptor.info.version.clone(),
                    plugin_type: descriptor.info.plugin_type.clone(),
                    description: descriptor.info.description.clone(),
                    author: descriptor.info.author.clone(),
                    file_path: descriptor.file_path.clone(),
                    capabilities: descriptor.info.capabilities.iter()
                        .map(|cap| cap.name.clone())
                        .collect(),
                }));
            }
        }
        
        Ok(None)
    }
    
    /// Filter plugins by type
    #[allow(dead_code)]
    pub async fn get_plugins_by_type(&self, plugin_type: PluginType) -> PluginResult<Vec<PluginInfo>> {
        let descriptors = self.discovery.discover_plugins_by_type(plugin_type).await?;
        
        let plugins = descriptors.into_iter()
            .map(|desc| PluginInfo {
                name: desc.info.name.clone(),
                version: desc.info.version.clone(),
                plugin_type: desc.info.plugin_type.clone(),
                description: desc.info.description.clone(),
                author: desc.info.author.clone(),
                file_path: desc.file_path.clone(),
                capabilities: desc.info.capabilities.iter()
                    .map(|cap| cap.name.clone())
                    .collect(),
            })
            .collect();
        
        Ok(plugins)
    }
    
    /// Build command mappings from discovered plugins and built-in plugins
    pub async fn build_command_mappings(&mut self) -> PluginResult<()> {
        // Clear existing mappings
        self.command_mapper = CommandMapper::new();
        
        // Discover and register all plugins (builtin and external)
        // UnifiedPluginDiscovery handles both builtin and external plugins
        let descriptors = self.discover_plugins().await?;
        for descriptor in descriptors {
            // For builtin plugins, we need to instantiate them to get their advertised functions
            // For external plugins, we extract functions from the descriptor
            let functions = if crate::plugin::builtin::get_builtin_plugins().contains(&descriptor.info.name.as_str()) {
                // This is a builtin plugin - instantiate it to get advertised functions
                if let Some(plugin) = crate::plugin::builtin::create_builtin_plugin(&descriptor.info.name).await {
                    plugin.advertised_functions()
                } else {
                    // Fallback to descriptor-based functions
                    self.extract_functions_from_descriptor(&descriptor)
                }
            } else {
                // External plugin - use descriptor-based functions
                self.extract_functions_from_descriptor(&descriptor)
            };
            
            self.command_mapper.register_plugin(&descriptor.info.name, functions);
            debug!("Registered plugin '{}' from discovery", descriptor.info.name);
        }
        
        debug!("Built command mappings for {} plugins", self.command_mapper.plugin_count());
        Ok(())
    }
    
    /// Register built-in plugins with their advertised functions  
    /// For CLI help purposes, we need to instantiate plugins to get their functions
    async fn register_builtin_plugins(&mut self) -> PluginResult<()> {
        // Get exclusion list from configuration
        let excluded = if let Some(ref config) = self.plugin_config {
            &config.plugin_exclude
        } else {
            &Vec::new()
        };
        
        let mut registered: Vec<String> = Vec::new();
        
        // Get builtin plugin names from discovery
        let builtin_names = crate::plugin::builtin::get_builtin_plugins();
        
        for name in builtin_names {
            // Skip if plugin is excluded by configuration
            if excluded.contains(&name.to_string()) {
                debug!("Excluded built-in plugin: {}", name);
                continue;
            }
            
            // Create the plugin instance to get its advertised functions
            if let Some(plugin) = crate::plugin::builtin::create_builtin_plugin(name).await {
                let functions = plugin.advertised_functions();
                self.command_mapper.register_plugin(name, functions);
                registered.push(name.to_string());
                debug!("Registered builtin plugin for command mapping: {}", name);
            }
        }
        
        debug!("Registered built-in plugins for command mapping: {}", registered.join(", "));
        Ok(())
    }
    
    /// Extract functions from plugin descriptor (for external plugins)
    fn extract_functions_from_descriptor(&self, descriptor: &PluginDescriptor) -> Vec<PluginFunction> {
        // For now, create a basic function from plugin name
        // This will be enhanced when external plugins implement function advertisement
        vec![PluginFunction {
            name: descriptor.info.name.clone(),
            aliases: vec![],
            description: descriptor.info.description.clone(),
            is_default: true,
        }]
    }
    
    /// Resolve a command to a plugin and function
    #[allow(dead_code)]
    pub async fn resolve_command(&self, command: &str) -> Result<CommandResolution, String> {
        self.command_mapper.resolve_command(command)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn resolve_command_with_colors(&self, command: &str, no_color: bool, color: bool) -> Result<CommandResolution, String> {
        self.command_mapper.resolve_command_with_colors(command, no_color, color)
            .await
            .map_err(|e| e.to_string())
    }
    
    /// Get all available function mappings for help display
    pub fn get_function_mappings(&self) -> Vec<FunctionMapping> {
        let mut mappings = Vec::new();
        
        for (plugin_name, functions) in self.command_mapper.get_all_mappings() {
            for function in functions {
                mappings.push(FunctionMapping {
                    function_name: function.name.clone(),
                    aliases: function.aliases.clone(),
                    plugin_name: plugin_name.clone(),
                    description: function.description.clone(),
                    is_default: function.is_default,
                });
            }
        }
        
        // Sort by function name for consistent display
        mappings.sort_by(|a, b| a.function_name.cmp(&b.function_name));
        mappings
    }
    
    /// Get ambiguity reports for debugging
    pub fn get_ambiguity_reports(&self) -> Vec<String> {
        self.command_mapper.detect_ambiguities()
            .into_iter()
            .map(|report| report.to_string())
            .collect()
    }
    
    /// Get argument schema for a specific plugin using proper discovery
    /// 
    /// This method provides access to the argument definitions for any discovered plugin.
    /// It uses the unified plugin discovery system to maintain SOLID principles and
    /// consistent plugin handling between builtin and external plugins.
    pub async fn get_plugin_arg_schema(&self, plugin_name: &str) -> PluginResult<Vec<crate::plugin::traits::PluginArgDefinition>> {
        // First, discover all plugins using the unified discovery system
        let discovered_plugins = self.discovery.discover_plugins().await?;
        
        // Find the plugin by name in discovered plugins
        for descriptor in &discovered_plugins {
            if descriptor.info.name == plugin_name {
                // Use the builtin plugin creation system to get argument schema
                // This approach respects the existing architecture while accessing schemas
                return self.get_plugin_schema_for_discovered_plugin(&descriptor.info.name).await;
            }
        }
        
        Err(PluginError::PluginNotFound {
            plugin_name: plugin_name.to_string(),
        })
    }
    
    /// Helper method to get plugin argument schema for a discovered plugin
    /// 
    /// This method creates a temporary plugin instance to access its argument schema.
    /// It maintains the principle of using the unified plugin creation system.
    async fn get_plugin_schema_for_discovered_plugin(&self, plugin_name: &str) -> PluginResult<Vec<crate::plugin::traits::PluginArgDefinition>> {
        // Use the proper plugin creation mechanism and access schema through Plugin trait
        if let Some(plugin) = crate::plugin::builtin::create_builtin_plugin(plugin_name).await {
            // Now we can use the unified Plugin trait method
            Ok(plugin.get_arg_schema())
        } else {
            // External plugins - for future implementation
            Ok(vec![])
        }
    }
    
    /// Get argument schemas for all discovered plugins
    /// 
    /// Returns a mapping of plugin names to their argument definitions.
    /// This uses the unified plugin discovery system to ensure consistency.
    pub async fn get_all_plugin_arg_schemas(&self) -> PluginResult<std::collections::HashMap<String, Vec<crate::plugin::traits::PluginArgDefinition>>> {
        use std::collections::HashMap;
        
        let mut schemas = HashMap::new();
        
        // Discover all plugins using the unified discovery system
        let discovered_plugins = self.discovery.discover_plugins().await?;
        
        for descriptor in &discovered_plugins {
            let plugin_name = &descriptor.info.name;
            
            // Try to get the argument schema for this plugin
            if let Ok(schema) = self.get_plugin_arg_schema(plugin_name).await {
                schemas.insert(plugin_name.clone(), schema);
            }
        }
        
        Ok(schemas)
    }
    
}

/// Plugin information for CLI display
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub plugin_type: PluginType,
    pub description: String,
    pub author: String,
    pub file_path: Option<PathBuf>,
    pub capabilities: Vec<String>,
}


/// Function mapping information for plugin-help display
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FunctionMapping {
    pub function_name: String,
    pub aliases: Vec<String>,
    pub plugin_name: String,
    pub description: String,
    pub is_default: bool,
}



#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::fs;
    use crate::plugin::traits::PluginInfo as TraitPluginInfo;
    use std::collections::HashMap;

    async fn create_test_plugin_descriptor(
        dir: &Path,
        name: &str,
        version: &str,
        plugin_type: PluginType,
    ) -> Result<(), std::io::Error> {
        let info = TraitPluginInfo::new(
            name.to_string(),
            version.to_string(),
            20250727,
            format!("Test plugin: {}", name),
            "Test Author".to_string(),
            plugin_type,
        );
        
        let descriptor = PluginDescriptor {
            info,
            file_path: None,
            entry_point: "main".to_string(),
            config: HashMap::new(),
        };
        
        let yaml_content = serde_yaml::to_string(&descriptor).unwrap();
        let file_path = dir.join(format!("{}.yaml", name));
        fs::write(&file_path, yaml_content).await
    }

    #[tokio::test]
    async fn test_plugin_arg_schema_access() {
        let handler = PluginHandler::new().unwrap();
        
        // Test debug plugin schema
        let debug_schema = handler.get_plugin_arg_schema("debug").await.unwrap();
        assert!(!debug_schema.is_empty(), "Debug plugin should have argument schema");
        
        // Test export plugin schema
        let export_schema = handler.get_plugin_arg_schema("export").await.unwrap();
        assert!(!export_schema.is_empty(), "Export plugin should have argument schema");
        
        // Test commits plugin (should return empty for now)
        let commits_schema = handler.get_plugin_arg_schema("commits").await.unwrap();
        assert!(commits_schema.is_empty(), "Commits plugin should return empty schema for now");
        
        // Test unknown plugin
        let unknown_result = handler.get_plugin_arg_schema("unknown").await;
        assert!(unknown_result.is_err(), "Unknown plugin should return error");
    }
    
    #[tokio::test]
    async fn test_all_plugin_arg_schemas() {
        let handler = PluginHandler::new().unwrap();
        
        let all_schemas = handler.get_all_plugin_arg_schemas().await.unwrap();
        
        // Should contain entries for all discovered plugins
        assert!(all_schemas.contains_key("debug"), "Should contain debug plugin schema");
        assert!(all_schemas.contains_key("export"), "Should contain export plugin schema");
        assert!(all_schemas.contains_key("commits"), "Should contain commits plugin schema");
        assert!(all_schemas.contains_key("metrics"), "Should contain metrics plugin schema");
        
        // Debug and export should have non-empty schemas
        assert!(!all_schemas["debug"].is_empty(), "Debug schema should not be empty");
        assert!(!all_schemas["export"].is_empty(), "Export schema should not be empty");
    }

    #[tokio::test]
    async fn test_plugin_handler_creation() {
        let temp_dir = tempdir().unwrap();
        let handler = PluginHandler::with_plugin_directory(temp_dir.path()).unwrap();
        
        assert_eq!(handler.discovery.plugin_directory(), temp_dir.path());
    }

    #[tokio::test]
    async fn test_discover_plugins() {
        let temp_dir = tempdir().unwrap();
        
        // Create test plugin descriptors
        create_test_plugin_descriptor(temp_dir.path(), "test-scanner", "1.0.0", PluginType::Processing).await.unwrap();
        create_test_plugin_descriptor(temp_dir.path(), "test-notification", "2.0.0", PluginType::Notification).await.unwrap();
        
        let handler = PluginHandler::with_plugin_directory(temp_dir.path()).unwrap();
        let plugins = handler.discover_plugins().await.unwrap();
        
        assert_eq!(plugins.len(), 6); // 4 builtin + 2 external
        
        let names: Vec<String> = plugins.iter().map(|p| p.info.name.clone()).collect();
        assert!(names.contains(&"test-scanner".to_string()));
        assert!(names.contains(&"test-notification".to_string()));
    }

    #[tokio::test]
    async fn test_list_plugins() {
        let temp_dir = tempdir().unwrap();
        
        create_test_plugin_descriptor(temp_dir.path(), "alpha-plugin", "1.0.0", PluginType::Processing).await.unwrap();
        create_test_plugin_descriptor(temp_dir.path(), "beta-plugin", "2.0.0", PluginType::Processing).await.unwrap();
        
        let handler = PluginHandler::with_plugin_directory(temp_dir.path()).unwrap();
        let plugins = handler.list_plugins().await.unwrap();
        
        assert_eq!(plugins.len(), 6); // 4 builtin + 2 external
        
        // Should be sorted by name - check that external plugins are in the mix
        let plugin_names: Vec<&str> = plugins.iter().map(|p| p.name.as_str()).collect();
        assert!(plugin_names.contains(&"alpha-plugin"));
        assert!(plugin_names.contains(&"beta-plugin"));
        // Also should include builtin plugins
        assert!(plugin_names.contains(&"commits"));
        assert!(plugin_names.contains(&"metrics"));
        assert!(plugin_names.contains(&"export"));
    }


    #[tokio::test]
    async fn test_get_plugin_info() {
        let temp_dir = tempdir().unwrap();
        
        create_test_plugin_descriptor(temp_dir.path(), "info-test", "1.2.3", PluginType::Output).await.unwrap();
        
        let handler = PluginHandler::with_plugin_directory(temp_dir.path()).unwrap();
        let info = handler.get_plugin_info("info-test").await.unwrap();
        
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.name, "info-test");
        assert_eq!(info.version, "1.2.3");
        assert_eq!(info.plugin_type, PluginType::Output);
    }

    #[tokio::test]
    async fn test_get_plugins_by_type() {
        let temp_dir = tempdir().unwrap();
        
        create_test_plugin_descriptor(temp_dir.path(), "scanner1", "1.0.0", PluginType::Processing).await.unwrap();
        create_test_plugin_descriptor(temp_dir.path(), "scanner2", "2.0.0", PluginType::Processing).await.unwrap();
        create_test_plugin_descriptor(temp_dir.path(), "output1", "1.0.0", PluginType::Output).await.unwrap();
        
        let handler = PluginHandler::with_plugin_directory(temp_dir.path()).unwrap();
        let scanners = handler.get_plugins_by_type(PluginType::Processing).await.unwrap();
        
        assert_eq!(scanners.len(), 5); // 2 external + 3 builtin Processing (debug, commits, metrics)
        for plugin in &scanners {
            assert_eq!(plugin.plugin_type, PluginType::Processing);
        }
    }



    #[tokio::test]
    async fn test_build_command_mappings_includes_external_plugins() {
        let temp_dir = tempdir().unwrap();
        
        // Create an external plugin descriptor
        create_test_plugin_descriptor(temp_dir.path(), "external-test", "1.0.0", PluginType::Processing).await.unwrap();
        
        let mut handler = PluginHandler::with_plugin_directory(temp_dir.path()).unwrap();
        handler.build_command_mappings().await.unwrap();
        
        // Get the command mappings
        let mappings = handler.get_function_mappings();
        
        // Check that external plugin is included as a function mapping
        let external_function_found = mappings.iter().any(|mapping| mapping.plugin_name == "external-test");
        assert!(external_function_found, "External plugin 'external-test' should be included in command mappings");
        
        // Verify the external plugin function details
        let external_mapping = mappings.iter().find(|m| m.plugin_name == "external-test").unwrap();
        assert_eq!(external_mapping.function_name, "external-test", "External plugin function name should match plugin name");
        assert!(external_mapping.is_default, "External plugin function should be marked as default");
    }

    #[tokio::test]
    async fn test_plugin_handler_with_enhanced_config() {
        let temp_dir1 = tempdir().unwrap();
        let temp_dir2 = tempdir().unwrap();
        
        // Create test plugins in different directories
        create_test_plugin_descriptor(temp_dir1.path(), "plugin1", "1.0.0", PluginType::Processing).await.unwrap();
        create_test_plugin_descriptor(temp_dir2.path(), "plugin2", "2.0.0", PluginType::Output).await.unwrap();
        
        let config = PluginConfig {
            directories: vec![
                temp_dir1.path().to_string_lossy().to_string(),
                temp_dir2.path().to_string_lossy().to_string(),
            ],
            plugin_load: Vec::new(),
            plugin_exclude: Vec::new(),
        };
        
        let handler = PluginHandler::with_plugin_config(config).unwrap();
        let plugins = handler.discover_plugins().await.unwrap();
        
        assert_eq!(plugins.len(), 5); // 4 builtin + 1 external (only first directory used)
        let plugin_names: Vec<&str> = plugins.iter().map(|p| p.info.name.as_str()).collect();
        assert!(plugin_names.contains(&"plugin1")); // From first directory
        // plugin2 should not be found since only first directory is used now
        // assert!(plugin_names.contains(&"plugin2"));
        // But builtin plugins should be there
        assert!(plugin_names.contains(&"commits"));
        assert!(plugin_names.contains(&"metrics"));
        assert!(plugin_names.contains(&"export"));
    }

    #[tokio::test]
    async fn test_plugin_handler_with_explicit_loading() {
        let temp_dir = tempdir().unwrap();
        
        // Create two plugins
        create_test_plugin_descriptor(temp_dir.path(), "wanted", "1.0.0", PluginType::Processing).await.unwrap();
        create_test_plugin_descriptor(temp_dir.path(), "unwanted", "2.0.0", PluginType::Output).await.unwrap();
        
        let config = PluginConfig {
            directories: vec![temp_dir.path().to_string_lossy().to_string()],
            plugin_load: vec!["wanted".to_string()], // Only load 'wanted' - but this is no longer supported
            plugin_exclude: Vec::new(),
        };
        
        let handler = PluginHandler::with_plugin_config(config).unwrap();
        let plugins = handler.discover_plugins().await.unwrap();
        
        // Note: explicit loading (plugin_load) is no longer supported, so this finds all plugins
        assert_eq!(plugins.len(), 6); // 4 builtin + 2 external
        let plugin_names: Vec<&str> = plugins.iter().map(|p| p.info.name.as_str()).collect();
        assert!(plugin_names.contains(&"wanted"));
        assert!(plugin_names.contains(&"unwanted")); // Not filtered out anymore
        assert!(plugin_names.contains(&"commits"));
        assert!(plugin_names.contains(&"metrics"));
        assert!(plugin_names.contains(&"export"));
    }

    #[tokio::test]
    async fn test_plugin_handler_with_exclusion() {
        let temp_dir = tempdir().unwrap();
        
        // Create two plugins
        create_test_plugin_descriptor(temp_dir.path(), "wanted", "1.0.0", PluginType::Processing).await.unwrap();
        create_test_plugin_descriptor(temp_dir.path(), "unwanted", "2.0.0", PluginType::Output).await.unwrap();
        
        let config = PluginConfig {
            directories: vec![temp_dir.path().to_string_lossy().to_string()],
            plugin_load: Vec::new(),
            plugin_exclude: vec!["unwanted".to_string()], // Exclude 'unwanted'
        };
        
        let handler = PluginHandler::with_plugin_config(config).unwrap();
        let plugins = handler.discover_plugins().await.unwrap();
        
        assert_eq!(plugins.len(), 5); // 4 builtin + 1 external (unwanted excluded)
        let plugin_names: Vec<&str> = plugins.iter().map(|p| p.info.name.as_str()).collect();
        assert!(plugin_names.contains(&"wanted"));
        assert!(!plugin_names.contains(&"unwanted")); // Should be excluded
        assert!(plugin_names.contains(&"commits"));
        assert!(plugin_names.contains(&"metrics"));
        assert!(plugin_names.contains(&"export"));
    }

    #[tokio::test]
    async fn test_plugin_handler_with_builtin_exclusion() {
        let temp_dir = tempdir().unwrap();
        
        let config = PluginConfig {
            directories: vec![temp_dir.path().to_string_lossy().to_string()],
            plugin_load: Vec::new(),
            plugin_exclude: vec!["commits".to_string(), "export".to_string()], // Exclude built-in plugins
        };
        
        let handler = PluginHandler::with_plugin_config(config).unwrap();
        let plugins = handler.discover_plugins().await.unwrap();
        
        // Should only have metrics plugin, not commits or export (which are excluded)
        let plugin_names: Vec<String> = plugins.iter()
            .map(|p| p.info.name.clone())
            .collect();
        
        println!("DEBUG: Found plugin names: {:?}", plugin_names);
        println!("DEBUG: Total plugins: {}", plugins.len());
        for plugin in &plugins {
            println!("DEBUG: Plugin - name: {}, type: {:?}", 
                plugin.info.name, plugin.info.plugin_type);
        }
        
        assert!(plugin_names.contains(&"metrics".to_string()), "metrics plugin should be discovered");
        assert!(!plugin_names.contains(&"commits".to_string()), "commits plugin should be excluded");
        assert!(!plugin_names.contains(&"export".to_string()), "export plugin should be excluded");
    }

}