//! Plugin Discovery (Placeholder)
//! 
//! Plugin discovery mechanism - to be implemented in Phase 3

use super::error::PluginResult;
use super::traits::PluginDescriptor;

/// Placeholder for plugin discovery trait
pub trait PluginDiscovery {
    async fn discover_plugins(&self) -> PluginResult<Vec<PluginDescriptor>> {
        Ok(Vec::new())
    }
}