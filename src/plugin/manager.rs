//! Plugin Manager
//! 
//! Central coordinator for plugin lifecycle, compatibility checking, and plugin proxy management.
//! Owns the plugin registry and provides high-level plugin management operations.

use crate::plugin::{SharedPluginRegistry, traits::PluginInfo, error::{PluginError, PluginResult}};

/// Central plugin manager responsible for:
/// - Plugin lifecycle management 
/// - Version compatibility checking
/// - Plugin proxy coordination
/// - Plugin registry ownership
pub struct PluginManager {
    /// The plugin registry (owned by this manager)
    registry: SharedPluginRegistry,
    
    /// Current API version for compatibility checking
    api_version: u32,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(api_version: u32) -> Self {
        Self {
            registry: SharedPluginRegistry::new(),
            api_version,
        }
    }
    
    /// Get shared access to the plugin registry
    pub fn registry(&self) -> &SharedPluginRegistry {
        &self.registry
    }
    
    /// Check if a plugin API version is compatible
    pub fn is_api_compatible(&self, plugin_api_version: u32) -> bool {
        // Same major version (year) is compatible
        self.get_major_version(self.api_version) == self.get_major_version(plugin_api_version)
    }
    
    /// Get major version (year) from API version
    pub fn get_major_version(&self, api_version: u32) -> u32 {
        api_version / 10000
    }
    
    /// Validate plugin compatibility before registration
    pub fn validate_plugin_compatibility(&self, plugin_info: &PluginInfo) -> PluginResult<()> {
        if !self.is_api_compatible(plugin_info.api_version) {
            return Err(PluginError::VersionIncompatible {
                message: format!(
                    "Plugin '{}' has incompatible API version {} (expected major version {})",
                    plugin_info.name,
                    plugin_info.api_version,
                    self.get_major_version(self.api_version)
                ),
            });
        }
        Ok(())
    }
}

/// Plugin Proxy Design Requirements (to be implemented)
/// 
/// PluginProxy should provide a clean interface for external components to interact with plugins
/// without needing to know plugin implementation details. It should replace PluginDescriptor
/// usage throughout the system.
/// 
/// Key responsibilities:
/// - Plugin invocation interface
/// - Plugin state management
/// - Plugin capability queries
/// - Plugin metadata access
/// - Plugin error handling and recovery
/// 
/// This will be the primary interface used by execution.rs and other components.
pub struct PluginProxy {
    // TODO: Implement PluginProxy design
    // This should replace current PluginDescriptor usage patterns
}