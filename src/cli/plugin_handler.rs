//! CLI Plugin Handler
//! 
//! Handles plugin discovery, listing, and validation for CLI operations.

use crate::plugin::{
    registry::SharedPluginRegistry, 
    discovery::{PluginDiscovery, FileBasedDiscovery, MultiDirectoryDiscovery},
    traits::{PluginDescriptor, PluginType, PluginFunction},
    error::{PluginError, PluginResult}
};
use crate::cli::converter::PluginConfig;
use crate::cli::command_mapper::{CommandMapper, CommandResolution};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use anyhow::{Result, Context};
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
        let plugin_path = plugin_dir.as_ref();
        
        // Create plugin directory if it doesn't exist
        if !plugin_path.exists() {
            debug!("Creating plugin directory: {}", plugin_path.display());
            std::fs::create_dir_all(&plugin_path)
                .map_err(|e| PluginError::discovery_error(format!(
                    "Failed to create plugin directory {}: {}", 
                    plugin_path.display(), e
                )))?;
        }
        
        let discovery = Box::new(FileBasedDiscovery::with_caching(plugin_path, true)?);
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
        // Convert directories to PathBuf
        let directories: Vec<PathBuf> = config.directories
            .iter()
            .map(PathBuf::from)
            .collect();
        
        // Create directories if they don't exist
        for directory in &directories {
            if !directory.exists() {
                debug!("Creating plugin directory: {}", directory.display());
                std::fs::create_dir_all(directory)
                    .map_err(|e| PluginError::discovery_error(format!(
                        "Failed to create plugin directory {}: {}", 
                        directory.display(), e
                    )))?;
            }
        }
        
        let discovery = Box::new(MultiDirectoryDiscovery::new(
            directories,
            config.plugin_load.clone(),
            config.plugin_exclude.clone(),
        ));
        
        let registry = SharedPluginRegistry::new();
        let command_mapper = CommandMapper::new();
        
        Ok(Self {
            _registry: registry,
            discovery,
            command_mapper,
            plugin_config: Some(config),
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
    
    /// Validate plugin names and check if they exist
    pub async fn validate_plugin_names(&self, plugin_names: &[String]) -> Result<Vec<String>> {
        if plugin_names.is_empty() {
            return Ok(vec!["commits".to_string()]); // Default plugin
        }
        
        debug!("Validating {} plugin names", plugin_names.len());
        
        let available_plugins = self.discover_plugins().await
            .context("Failed to discover available plugins")?;
        
        let available_names: HashMap<String, &PluginDescriptor> = available_plugins.iter()
            .map(|desc| (desc.info.name.clone(), desc))
            .collect();
        
        let mut validated = Vec::new();
        let mut errors = Vec::new();
        
        for plugin_name in plugin_names {
            let trimmed = plugin_name.trim();
            
            if trimmed.is_empty() {
                errors.push(format!("Empty plugin name"));
                continue;
            }
            
            // Basic name validation
            if !trimmed.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                errors.push(format!(
                    "Invalid plugin name '{}': must contain only letters, numbers, underscores, and hyphens", 
                    trimmed
                ));
                continue;
            }
            
            // Check if plugin exists
            if let Some(descriptor) = available_names.get(trimmed) {
                debug!("Validated plugin: {} v{}", descriptor.info.name, descriptor.info.version);
                validated.push(trimmed.to_string());
            } else {
                // Check for fuzzy matches
                let suggestions = find_similar_plugin_names(trimmed, available_names.keys());
                let suggestion_text = if suggestions.is_empty() {
                    String::new()
                } else {
                    format!(". Did you mean: {}", suggestions.join(", "))
                };
                
                errors.push(format!("Plugin '{}' not found{}", trimmed, suggestion_text));
            }
        }
        
        if !errors.is_empty() {
            return Err(anyhow::anyhow!(
                "Plugin validation failed:\n  {}",
                errors.join("\n  ")
            ));
        }
        
        debug!("Validated {} plugins successfully", validated.len());
        Ok(validated)
    }
    
    /// Get plugin information by name
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
    
    /// Check plugin compatibility with current API
    pub async fn check_plugin_compatibility(&self, plugin_name: &str) -> PluginResult<CompatibilityReport> {
        let current_api_version = crate::scanner::version::get_api_version()
            .try_into()
            .map_err(|_| PluginError::generic("Invalid API version format"))?;
        
        if let Some(_plugin_info) = self.get_plugin_info(plugin_name).await? {
            let descriptor = self.discovery.discover_plugins().await?
                .into_iter()
                .find(|desc| desc.info.name == plugin_name)
                .ok_or_else(|| PluginError::plugin_not_found(plugin_name))?;
            
            let is_compatible = descriptor.info.is_compatible_with_api(current_api_version);
            let plugin_major = descriptor.info.api_version / 10000;
            let current_major = current_api_version / 10000;
            
            Ok(CompatibilityReport {
                plugin_name: plugin_name.to_string(),
                plugin_version: descriptor.info.version.clone(),
                plugin_api_version: descriptor.info.api_version,
                current_api_version,
                is_compatible,
                compatibility_message: if is_compatible {
                    "Plugin is compatible with current API".to_string()
                } else {
                    format!(
                        "Plugin API version {} (major {}) is incompatible with current API version {} (major {})",
                        descriptor.info.api_version, plugin_major,
                        current_api_version, current_major
                    )
                },
            })
        } else {
            Err(PluginError::plugin_not_found(plugin_name))
        }
    }
    
    /// Build command mappings from discovered plugins and built-in plugins
    pub async fn build_command_mappings(&mut self) -> PluginResult<()> {
        // Clear existing mappings
        self.command_mapper = CommandMapper::new();
        
        // Register built-in plugins
        self.register_builtin_plugins()?;
        
        // Discover and register external plugins
        let descriptors = self.discover_plugins().await?;
        for descriptor in descriptors {
            // For now, register external plugins with basic capability mapping
            // This will be enhanced when external plugins implement function advertisement
            let functions = self.extract_functions_from_descriptor(&descriptor);
            self.command_mapper.register_plugin(&descriptor.info.name, functions);
        }
        
        debug!("Built command mappings for {} plugins", self.command_mapper.plugin_count());
        Ok(())
    }
    
    /// Register built-in plugins with their advertised functions
    fn register_builtin_plugins(&mut self) -> PluginResult<()> {
        use crate::plugin::builtin::{CommitsPlugin, MetricsPlugin, ExportPlugin};
        use crate::plugin::Plugin;
        
        let mut registered = Vec::new();
        
        // Get exclusion list from configuration
        let excluded = if let Some(ref config) = self.plugin_config {
            &config.plugin_exclude
        } else {
            // If no config, don't exclude any built-in plugins
            &Vec::new()
        };
        
        // Register CommitsPlugin functions
        if !excluded.contains(&"commits".to_string()) {
            let commits_plugin = CommitsPlugin::new();
            let commits_functions = commits_plugin.advertised_functions();
            self.command_mapper.register_plugin("commits", commits_functions);
            registered.push("commits");
        } else {
            debug!("Excluded built-in plugin: commits");
        }
        
        // Register MetricsPlugin functions  
        if !excluded.contains(&"metrics".to_string()) {
            let metrics_plugin = MetricsPlugin::new();
            let metrics_functions = metrics_plugin.advertised_functions();
            self.command_mapper.register_plugin("metrics", metrics_functions);
            registered.push("metrics");
        } else {
            debug!("Excluded built-in plugin: metrics");
        }
        
        // Register ExportPlugin functions
        if !excluded.contains(&"export".to_string()) {
            let export_plugin = ExportPlugin::new();
            let export_functions = export_plugin.advertised_functions();
            self.command_mapper.register_plugin("export", export_functions);
            registered.push("export");
        } else {
            debug!("Excluded built-in plugin: export");
        }
        
        debug!("Registered built-in plugins: {}", registered.join(", "));
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
}

/// Plugin information for CLI display
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub plugin_type: PluginType,
    pub description: String,
    pub author: String,
    pub file_path: Option<PathBuf>,
    pub capabilities: Vec<String>,
}

/// Plugin compatibility report
#[derive(Debug)]
pub struct CompatibilityReport {
    pub plugin_name: String,
    pub plugin_version: String,
    pub plugin_api_version: u32,
    pub current_api_version: u32,
    pub is_compatible: bool,
    pub compatibility_message: String,
}

/// Function mapping information for plugin-help display
#[derive(Debug, Clone)]
pub struct FunctionMapping {
    pub function_name: String,
    pub aliases: Vec<String>,
    pub plugin_name: String,
    pub description: String,
    pub is_default: bool,
}

/// Find similar plugin names using basic string distance
fn find_similar_plugin_names<'a>(
    target: &str,
    available: impl Iterator<Item = &'a String>
) -> Vec<String> {
    let mut suggestions = Vec::new();
    let target_lower = target.to_lowercase();
    
    for name in available {
        let name_lower = name.to_lowercase();
        
        // Check for substring matches
        if name_lower.contains(&target_lower) || target_lower.contains(&name_lower) {
            suggestions.push(name.clone());
            continue;
        }
        
        // Check for simple edit distance (basic implementation)
        if simple_edit_distance(&target_lower, &name_lower) <= 2 && name.len() > 2 {
            suggestions.push(name.clone());
        }
    }
    
    // Limit to 3 suggestions
    suggestions.truncate(3);
    suggestions
}

/// Simple edit distance calculation (Levenshtein distance)
fn simple_edit_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();
    
    if len1 == 0 { return len2; }
    if len2 == 0 { return len1; }
    
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];
    
    // Initialize first row and column
    for i in 0..=len1 { matrix[i][0] = i; }
    for j in 0..=len2 { matrix[0][j] = j; }
    
    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();
    
    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if s1_chars[i - 1] == s2_chars[j - 1] { 0 } else { 1 };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }
    
    matrix[len1][len2]
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
    async fn test_plugin_handler_creation() {
        let temp_dir = tempdir().unwrap();
        let handler = PluginHandler::with_plugin_directory(temp_dir.path()).unwrap();
        
        assert_eq!(handler.discovery.plugin_directory(), temp_dir.path());
    }

    #[tokio::test]
    async fn test_discover_plugins() {
        let temp_dir = tempdir().unwrap();
        
        // Create test plugin descriptors
        create_test_plugin_descriptor(temp_dir.path(), "test-scanner", "1.0.0", PluginType::Scanner).await.unwrap();
        create_test_plugin_descriptor(temp_dir.path(), "test-notification", "2.0.0", PluginType::Notification).await.unwrap();
        
        let handler = PluginHandler::with_plugin_directory(temp_dir.path()).unwrap();
        let plugins = handler.discover_plugins().await.unwrap();
        
        assert_eq!(plugins.len(), 2);
        
        let names: Vec<String> = plugins.iter().map(|p| p.info.name.clone()).collect();
        assert!(names.contains(&"test-scanner".to_string()));
        assert!(names.contains(&"test-notification".to_string()));
    }

    #[tokio::test]
    async fn test_list_plugins() {
        let temp_dir = tempdir().unwrap();
        
        create_test_plugin_descriptor(temp_dir.path(), "alpha-plugin", "1.0.0", PluginType::Scanner).await.unwrap();
        create_test_plugin_descriptor(temp_dir.path(), "beta-plugin", "2.0.0", PluginType::Processing).await.unwrap();
        
        let handler = PluginHandler::with_plugin_directory(temp_dir.path()).unwrap();
        let plugins = handler.list_plugins().await.unwrap();
        
        assert_eq!(plugins.len(), 2);
        
        // Should be sorted by name
        assert_eq!(plugins[0].name, "alpha-plugin");
        assert_eq!(plugins[1].name, "beta-plugin");
    }

    #[tokio::test]
    async fn test_validate_plugin_names_valid() {
        let temp_dir = tempdir().unwrap();
        
        create_test_plugin_descriptor(temp_dir.path(), "valid-plugin", "1.0.0", PluginType::Scanner).await.unwrap();
        
        let handler = PluginHandler::with_plugin_directory(temp_dir.path()).unwrap();
        let result = handler.validate_plugin_names(&["valid-plugin".to_string()]).await.unwrap();
        
        assert_eq!(result, vec!["valid-plugin"]);
    }

    #[tokio::test]
    async fn test_validate_plugin_names_invalid() {
        let temp_dir = tempdir().unwrap();
        
        let handler = PluginHandler::with_plugin_directory(temp_dir.path()).unwrap();
        let result = handler.validate_plugin_names(&["nonexistent-plugin".to_string()]).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Plugin 'nonexistent-plugin' not found"));
    }

    #[tokio::test]
    async fn test_validate_plugin_names_empty_defaults_to_commits() {
        let temp_dir = tempdir().unwrap();
        
        let handler = PluginHandler::with_plugin_directory(temp_dir.path()).unwrap();
        let result = handler.validate_plugin_names(&[]).await.unwrap();
        
        assert_eq!(result, vec!["commits"]);
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
        
        create_test_plugin_descriptor(temp_dir.path(), "scanner1", "1.0.0", PluginType::Scanner).await.unwrap();
        create_test_plugin_descriptor(temp_dir.path(), "scanner2", "2.0.0", PluginType::Scanner).await.unwrap();
        create_test_plugin_descriptor(temp_dir.path(), "output1", "1.0.0", PluginType::Output).await.unwrap();
        
        let handler = PluginHandler::with_plugin_directory(temp_dir.path()).unwrap();
        let scanners = handler.get_plugins_by_type(PluginType::Scanner).await.unwrap();
        
        assert_eq!(scanners.len(), 2);
        for plugin in &scanners {
            assert_eq!(plugin.plugin_type, PluginType::Scanner);
        }
    }

    #[test]
    fn test_simple_edit_distance() {
        assert_eq!(simple_edit_distance("", ""), 0);
        assert_eq!(simple_edit_distance("a", ""), 1);
        assert_eq!(simple_edit_distance("", "a"), 1);
        assert_eq!(simple_edit_distance("abc", "abc"), 0);
        assert_eq!(simple_edit_distance("abc", "abd"), 1);
        assert_eq!(simple_edit_distance("commits", "commit"), 1);
        assert_eq!(simple_edit_distance("metrics", "metric"), 1);
    }

    #[test]
    fn test_find_similar_plugin_names() {
        let available = vec![
            "commits".to_string(),
            "metrics".to_string(),
            "history".to_string(),
            "files".to_string(),
        ];
        
        let suggestions = find_similar_plugin_names("commit", available.iter());
        assert!(suggestions.contains(&"commits".to_string()));
        
        let suggestions = find_similar_plugin_names("metric", available.iter());
        assert!(suggestions.contains(&"metrics".to_string()));
        
        let suggestions = find_similar_plugin_names("xyz", available.iter());
        assert!(suggestions.is_empty());
    }

    #[tokio::test]
    async fn test_build_command_mappings_includes_external_plugins() {
        let temp_dir = tempdir().unwrap();
        
        // Create an external plugin descriptor
        create_test_plugin_descriptor(temp_dir.path(), "external-test", "1.0.0", PluginType::Scanner).await.unwrap();
        
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
        create_test_plugin_descriptor(temp_dir1.path(), "plugin1", "1.0.0", PluginType::Scanner).await.unwrap();
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
        
        assert_eq!(plugins.len(), 2);
        let plugin_names: Vec<&str> = plugins.iter().map(|p| p.info.name.as_str()).collect();
        assert!(plugin_names.contains(&"plugin1"));
        assert!(plugin_names.contains(&"plugin2"));
    }

    #[tokio::test]
    async fn test_plugin_handler_with_explicit_loading() {
        let temp_dir = tempdir().unwrap();
        
        // Create two plugins
        create_test_plugin_descriptor(temp_dir.path(), "wanted", "1.0.0", PluginType::Scanner).await.unwrap();
        create_test_plugin_descriptor(temp_dir.path(), "unwanted", "2.0.0", PluginType::Output).await.unwrap();
        
        let config = PluginConfig {
            directories: vec![temp_dir.path().to_string_lossy().to_string()],
            plugin_load: vec!["wanted".to_string()], // Only load 'wanted'
            plugin_exclude: Vec::new(),
        };
        
        let handler = PluginHandler::with_plugin_config(config).unwrap();
        let plugins = handler.discover_plugins().await.unwrap();
        
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].info.name, "wanted");
    }

    #[tokio::test]
    async fn test_plugin_handler_with_exclusion() {
        let temp_dir = tempdir().unwrap();
        
        // Create two plugins
        create_test_plugin_descriptor(temp_dir.path(), "wanted", "1.0.0", PluginType::Scanner).await.unwrap();
        create_test_plugin_descriptor(temp_dir.path(), "unwanted", "2.0.0", PluginType::Output).await.unwrap();
        
        let config = PluginConfig {
            directories: vec![temp_dir.path().to_string_lossy().to_string()],
            plugin_load: Vec::new(),
            plugin_exclude: vec!["unwanted".to_string()], // Exclude 'unwanted'
        };
        
        let handler = PluginHandler::with_plugin_config(config).unwrap();
        let plugins = handler.discover_plugins().await.unwrap();
        
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].info.name, "wanted");
    }

    #[tokio::test]
    async fn test_plugin_handler_with_builtin_exclusion() {
        let temp_dir = tempdir().unwrap();
        
        let config = PluginConfig {
            directories: vec![temp_dir.path().to_string_lossy().to_string()],
            plugin_load: Vec::new(),
            plugin_exclude: vec!["commits".to_string(), "export".to_string()], // Exclude built-in plugins
        };
        
        let mut handler = PluginHandler::with_plugin_config(config).unwrap();
        handler.build_command_mappings().await.unwrap();
        
        let mappings = handler.get_function_mappings();
        
        // Should only have metrics plugin functions, not commits or export
        let plugin_names: Vec<String> = mappings.iter()
            .map(|m| m.plugin_name.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        
        assert!(plugin_names.contains(&"metrics".to_string()));
        assert!(!plugin_names.contains(&"commits".to_string()));
        assert!(!plugin_names.contains(&"export".to_string()));
    }

}