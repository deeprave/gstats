//! Version Compatibility Checker
//! 
//! Validates plugin API versions and dependency requirements.

use std::collections::{HashMap, HashSet};
use crate::plugin::traits::PluginInfo;
use crate::plugin::error::{PluginError, PluginResult};

/// Checker for plugin version compatibility
pub struct VersionCompatibilityChecker {
    /// Current API version
    api_version: u32,
}

impl VersionCompatibilityChecker {
    /// Create a new version compatibility checker
    pub fn new(api_version: u32) -> Self {
        Self {
            api_version,
        }
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
    
    /// Check plugin compatibility
    pub fn check_plugin_compatibility(&self, plugin_info: &PluginInfo) -> PluginResult<()> {
        if !self.is_api_compatible(plugin_info.api_version) {
            return Err(PluginError::version_incompatible(format!(
                "Plugin '{}' requires API version {} but current version is {}",
                plugin_info.name,
                plugin_info.api_version,
                self.api_version
            )));
        }
        Ok(())
    }
    
    /// Validate plugin dependencies
    pub fn validate_dependencies(
        &self,
        plugin_info: &PluginInfo,
        available_plugins: &[PluginInfo],
    ) -> PluginResult<()> {
        // Create a map of available plugins
        let available_map: HashMap<String, &PluginInfo> = available_plugins
            .iter()
            .map(|p| (p.name.clone(), p))
            .collect();
        
        // Check for circular dependencies
        if self.has_circular_dependency(plugin_info, &available_map)? {
            return Err(PluginError::dependency_error(format!(
                "Circular dependency detected for plugin '{}'",
                plugin_info.name
            )));
        }
        
        // Check each dependency
        for dep in &plugin_info.dependencies {
            match available_map.get(&dep.name) {
                Some(available_plugin) => {
                    // Check version requirement
                    if !self.version_matches(&dep.version_requirement, &available_plugin.version) {
                        return Err(PluginError::dependency_error(format!(
                            "Plugin '{}' requires {} version {} but found version {}",
                            plugin_info.name,
                            dep.name,
                            dep.version_requirement,
                            available_plugin.version
                        )));
                    }
                }
                None => {
                    // Dependency not found
                    if !dep.optional {
                        return Err(PluginError::dependency_error(format!(
                            "Plugin '{}' requires dependency '{}' which is not available",
                            plugin_info.name,
                            dep.name
                        )));
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Check if version matches requirement
    pub fn version_matches(&self, requirement: &str, version: &str) -> bool {
        // Handle special cases
        if requirement == "*" {
            return true;
        }
        
        // Handle caret requirements (^)
        if let Some(req_version) = requirement.strip_prefix('^') {
            return self.matches_caret(req_version, version);
        }
        
        // Handle tilde requirements (~)
        if let Some(req_version) = requirement.strip_prefix('~') {
            return self.matches_tilde(req_version, version);
        }
        
        // Exact match
        requirement == version
    }
    
    /// Check if version matches caret requirement (compatible with same major version)
    fn matches_caret(&self, requirement: &str, version: &str) -> bool {
        let req_parts: Vec<u32> = requirement
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();
        
        let ver_parts: Vec<u32> = version
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();
        
        if req_parts.is_empty() || ver_parts.is_empty() {
            return false;
        }
        
        // Major version must match
        if req_parts[0] != ver_parts[0] {
            return false;
        }
        
        // Version must be >= requirement
        self.version_compare(&ver_parts, &req_parts) >= 0
    }
    
    /// Check if version matches tilde requirement (compatible with same major.minor)
    fn matches_tilde(&self, requirement: &str, version: &str) -> bool {
        let req_parts: Vec<u32> = requirement
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();
        
        let ver_parts: Vec<u32> = version
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();
        
        if req_parts.len() < 2 || ver_parts.len() < 2 {
            return false;
        }
        
        // Major and minor must match
        if req_parts[0] != ver_parts[0] || req_parts[1] != ver_parts[1] {
            return false;
        }
        
        // Version must be >= requirement
        self.version_compare(&ver_parts, &req_parts) >= 0
    }
    
    /// Compare two version arrays
    fn version_compare(&self, v1: &[u32], v2: &[u32]) -> i32 {
        for i in 0..std::cmp::max(v1.len(), v2.len()) {
            let part1 = v1.get(i).copied().unwrap_or(0);
            let part2 = v2.get(i).copied().unwrap_or(0);
            
            if part1 < part2 {
                return -1;
            } else if part1 > part2 {
                return 1;
            }
        }
        0
    }
    
    /// Check for circular dependencies
    fn has_circular_dependency(
        &self,
        plugin_info: &PluginInfo,
        available_map: &HashMap<String, &PluginInfo>,
    ) -> PluginResult<bool> {
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        
        self.check_circular_dependency_recursive(
            plugin_info,
            available_map,
            &mut visited,
            &mut path,
        )
    }
    
    /// Recursive helper for circular dependency detection
    fn check_circular_dependency_recursive(
        &self,
        plugin_info: &PluginInfo,
        available_map: &HashMap<String, &PluginInfo>,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> PluginResult<bool> {
        // If we've seen this plugin in the current path, it's circular
        if path.contains(&plugin_info.name) {
            return Ok(true);
        }
        
        // If we've already fully processed this plugin, skip it
        if visited.contains(&plugin_info.name) {
            return Ok(false);
        }
        
        // Add to path
        path.push(plugin_info.name.clone());
        
        // Check dependencies
        for dep in &plugin_info.dependencies {
            if let Some(dep_plugin) = available_map.get(&dep.name) {
                if self.check_circular_dependency_recursive(
                    dep_plugin,
                    available_map,
                    visited,
                    path,
                )? {
                    return Ok(true);
                }
            }
        }
        
        // Remove from path and mark as visited
        path.pop();
        visited.insert(plugin_info.name.clone());
        
        Ok(false)
    }
    
    /// Check all plugins for compatibility
    pub fn check_all_plugins(
        &self,
        plugins: &[PluginInfo],
    ) -> HashMap<String, PluginResult<()>> {
        let mut results = HashMap::new();
        
        for plugin in plugins {
            let result = self.check_plugin_compatibility(plugin)
                .and_then(|_| self.validate_dependencies(plugin, plugins));
            results.insert(plugin.name.clone(), result);
        }
        
        results
    }
}

impl Default for VersionCompatibilityChecker {
    fn default() -> Self {
        Self::new(crate::scanner::get_api_version() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::traits::PluginType;
    
    #[test]
    fn test_major_version_extraction() {
        let checker = VersionCompatibilityChecker::new(20250727);
        assert_eq!(checker.get_major_version(20250727), 2025);
        assert_eq!(checker.get_major_version(20240101), 2024);
    }
    
    #[test]
    fn test_api_compatibility() {
        let checker = VersionCompatibilityChecker::new(20250727);
        assert!(checker.is_api_compatible(20250101));
        assert!(checker.is_api_compatible(20251231));
        assert!(!checker.is_api_compatible(20240727));
        assert!(!checker.is_api_compatible(20260727));
    }
    
    #[test]
    fn test_version_matching() {
        let checker = VersionCompatibilityChecker::new(20250727);
        
        // Exact match
        assert!(checker.version_matches("1.0.0", "1.0.0"));
        assert!(!checker.version_matches("1.0.0", "1.0.1"));
        
        // Wildcard
        assert!(checker.version_matches("*", "1.0.0"));
        assert!(checker.version_matches("*", "2.5.3"));
        
        // Caret
        assert!(checker.version_matches("^1.0.0", "1.0.0"));
        assert!(checker.version_matches("^1.0.0", "1.9.9"));
        assert!(!checker.version_matches("^1.0.0", "2.0.0"));
        
        // Tilde
        assert!(checker.version_matches("~1.2.0", "1.2.0"));
        assert!(checker.version_matches("~1.2.0", "1.2.5"));
        assert!(!checker.version_matches("~1.2.0", "1.3.0"));
    }
    
    #[test]
    fn test_dependency_validation() {
        let checker = VersionCompatibilityChecker::new(20250727);
        
        let plugin = PluginInfo::new(
            "test".to_string(),
            "1.0.0".to_string(),
            20250727,
            "Test".to_string(),
            "Author".to_string(),
            PluginType::Scanner,
        ).with_dependency("dep".to_string(), "1.0.0".to_string(), false);
        
        let dep = PluginInfo::new(
            "dep".to_string(),
            "1.0.0".to_string(),
            20250727,
            "Dependency".to_string(),
            "Author".to_string(),
            PluginType::Scanner,
        );
        
        let available = vec![dep];
        assert!(checker.validate_dependencies(&plugin, &available).is_ok());
        
        // Missing dependency
        let empty: Vec<PluginInfo> = vec![];
        assert!(checker.validate_dependencies(&plugin, &empty).is_err());
    }
}