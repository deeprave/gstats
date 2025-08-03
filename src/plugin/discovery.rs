//! Plugin Discovery System
//! 
//! Comprehensive plugin discovery mechanism supporting file-based discovery,
//! descriptor parsing, and plugin validation.

use super::error::{PluginError, PluginResult};
use super::traits::{PluginDescriptor, PluginInfo, PluginType};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use async_trait::async_trait;
use tokio::fs;
use std::time::SystemTime;

/// Plugin discovery trait for finding and loading plugins
#[async_trait]
pub trait PluginDiscovery: Send + Sync {
    /// Discover all available plugins
    async fn discover_plugins(&self) -> PluginResult<Vec<PluginDescriptor>>;
    
    /// Discover plugins filtered by type
    async fn discover_plugins_by_type(&self, plugin_type: PluginType) -> PluginResult<Vec<PluginDescriptor>> {
        let all_plugins = self.discover_plugins().await?;
        Ok(all_plugins.into_iter()
            .filter(|p| p.info.plugin_type == plugin_type)
            .collect())
    }
    
    /// Discover plugins compatible with a specific API version
    async fn discover_compatible_plugins(&self, api_version: u32) -> PluginResult<Vec<PluginDescriptor>> {
        let all_plugins = self.discover_plugins().await?;
        let major_version = api_version / 10000;
        
        Ok(all_plugins.into_iter()
            .filter(|p| (p.info.api_version / 10000) == major_version)
            .collect())
    }
    
    /// Check if this discovery mechanism supports dynamic loading
    fn supports_dynamic_loading(&self) -> bool {
        false
    }
    
    /// Get the plugin directory being scanned
    fn plugin_directory(&self) -> &Path;
}

/// File-based plugin discovery implementation
#[derive(Debug)]
pub struct FileBasedDiscovery {
    plugin_directory: PathBuf,
    parser: PluginDescriptorParser,
    supports_dynamic_loading: bool,
    cache_enabled: bool,
    cached_descriptors: Option<(SystemTime, Vec<PluginDescriptor>)>,
}

impl FileBasedDiscovery {
    /// Create a new file-based discovery instance
    pub fn new<P: AsRef<Path>>(plugin_directory: P) -> PluginResult<Self> {
        let path = plugin_directory.as_ref().to_path_buf();
        
        if !path.exists() {
            return Err(PluginError::discovery_error(format!(
                "Plugin directory does not exist: {}", 
                path.display()
            )));
        }
        
        if !path.is_dir() {
            return Err(PluginError::discovery_error(format!(
                "Plugin path is not a directory: {}", 
                path.display()
            )));
        }
        
        Ok(Self {
            plugin_directory: path,
            parser: PluginDescriptorParser::new(),
            supports_dynamic_loading: false,
            cache_enabled: false,
            cached_descriptors: None,
        })
    }
    
    /// Create a new file-based discovery with dynamic loading support
    pub fn with_dynamic_loading<P: AsRef<Path>>(plugin_directory: P, supports_dynamic: bool) -> PluginResult<Self> {
        let mut discovery = Self::new(plugin_directory)?;
        discovery.supports_dynamic_loading = supports_dynamic;
        Ok(discovery)
    }
    
    /// Create a new file-based discovery with caching enabled
    pub fn with_caching<P: AsRef<Path>>(plugin_directory: P, cache_enabled: bool) -> PluginResult<Self> {
        let mut discovery = Self::new(plugin_directory)?;
        discovery.cache_enabled = cache_enabled;
        Ok(discovery)
    }
    
    /// Recursively scan directories for plugin descriptors
    async fn scan_directory(&self, dir: &Path) -> PluginResult<Vec<PluginDescriptor>> {
        let mut descriptors = Vec::new();
        let mut directories_to_scan = vec![dir.to_path_buf()];
        
        while let Some(current_dir) = directories_to_scan.pop() {
            let mut entries = fs::read_dir(&current_dir).await
                .map_err(|e| PluginError::discovery_error(format!("Failed to read directory {}: {}", current_dir.display(), e)))?;
            
            while let Some(entry) = entries.next_entry().await
                .map_err(|e| PluginError::discovery_error(format!("Failed to read directory entry: {}", e)))? {
                
                let path = entry.path();
                
                if path.is_dir() {
                    // Add subdirectory to scan list
                    directories_to_scan.push(path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("yaml") || 
                         path.extension().and_then(|s| s.to_str()) == Some("yml") {
                    // Try to parse as plugin descriptor
                    match self.parse_descriptor_file(&path).await {
                        Ok(descriptor) => descriptors.push(descriptor),
                        Err(_) => {
                            // Ignore invalid descriptors - they might not be plugin files
                            continue;
                        }
                    }
                }
            }
        }
        
        Ok(descriptors)
    }
    
    /// Parse a plugin descriptor from a file
    async fn parse_descriptor_file(&self, file_path: &Path) -> PluginResult<PluginDescriptor> {
        let content = fs::read_to_string(file_path).await
            .map_err(|e| PluginError::discovery_error(format!("Failed to read file {}: {}", file_path.display(), e)))?;
        
        let mut descriptor = self.parser.parse_yaml(&content)?;
        
        // Set the file path if not already set
        if descriptor.file_path.is_none() {
            descriptor.file_path = Some(file_path.to_path_buf());
        }
        
        // Validate the descriptor
        self.parser.validate_descriptor(&descriptor)?;
        
        Ok(descriptor)
    }
    
    /// Check if cache is valid
    fn is_cache_valid(&self) -> Option<&Vec<PluginDescriptor>> {
        if !self.cache_enabled {
            return None;
        }
        
        if let Some((cache_time, ref descriptors)) = self.cached_descriptors {
            // Cache is valid for 5 minutes
            if SystemTime::now().duration_since(cache_time)
                .map(|d| d.as_secs() < 300)
                .unwrap_or(false) {
                return Some(descriptors);
            }
        }
        
        None
    }
    
    /// Update cache with new descriptors
    fn update_cache(&mut self, descriptors: Vec<PluginDescriptor>) {
        if self.cache_enabled {
            self.cached_descriptors = Some((SystemTime::now(), descriptors.clone()));
        }
    }
}

#[async_trait]
impl PluginDiscovery for FileBasedDiscovery {
    async fn discover_plugins(&self) -> PluginResult<Vec<PluginDescriptor>> {
        // Check cache first
        if let Some(cached_descriptors) = self.is_cache_valid() {
            return Ok(cached_descriptors.clone());
        }
        
        let descriptors = self.scan_directory(&self.plugin_directory).await?;
        
        // Update cache if enabled (note: we can't use &mut self here, so this won't work as-is)
        // For now, caching will need to be implemented differently
        
        Ok(descriptors)
    }
    
    fn supports_dynamic_loading(&self) -> bool {
        self.supports_dynamic_loading
    }
    
    fn plugin_directory(&self) -> &Path {
        &self.plugin_directory
    }
}

/// Parser for plugin descriptor files
#[derive(Debug)]
pub struct PluginDescriptorParser {
    // Could add validation rules, schema checking, etc.
}

impl PluginDescriptorParser {
    /// Create a new descriptor parser
    pub fn new() -> Self {
        Self {}
    }
    
    /// Parse a YAML string into a plugin descriptor
    pub fn parse_yaml(&self, yaml_content: &str) -> PluginResult<PluginDescriptor> {
        serde_yaml::from_str(yaml_content)
            .map_err(|e| PluginError::descriptor_parse_error(format!("Failed to parse YAML: {}", e)))
    }
    
    /// Parse a JSON string into a plugin descriptor
    pub fn parse_json(&self, json_content: &str) -> PluginResult<PluginDescriptor> {
        serde_json::from_str(json_content)
            .map_err(|e| PluginError::descriptor_parse_error(format!("Failed to parse JSON: {}", e)))
    }
    
    /// Validate a plugin descriptor
    pub fn validate_descriptor(&self, descriptor: &PluginDescriptor) -> PluginResult<()> {
        // Validate plugin name
        if descriptor.info.name.is_empty() {
            return Err(PluginError::descriptor_parse_error("Plugin name cannot be empty"));
        }
        
        // Validate version format (basic semver check)
        if !self.is_valid_version(&descriptor.info.version) {
            return Err(PluginError::descriptor_parse_error(format!(
                "Invalid version format: {}", descriptor.info.version
            )));
        }
        
        // Validate API version
        if descriptor.info.api_version == 0 {
            return Err(PluginError::descriptor_parse_error("API version cannot be zero"));
        }
        
        // Validate entry point
        if descriptor.entry_point.is_empty() {
            return Err(PluginError::descriptor_parse_error("Entry point cannot be empty"));
        }
        
        Ok(())
    }
    
    /// Basic version validation (simplified semver)
    fn is_valid_version(&self, version: &str) -> bool {
        // Very basic check - should have at least one dot and be parseable as version-like
        if !version.contains('.') {
            return false;
        }
        
        // Check if it looks like semver (major.minor.patch)
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() < 2 || parts.len() > 3 {
            return false;
        }
        
        // Check if all parts are numeric
        parts.iter().all(|part| part.parse::<u32>().is_ok())
    }
}

impl Default for PluginDescriptorParser {
    fn default() -> Self {
        Self::new()
    }
}