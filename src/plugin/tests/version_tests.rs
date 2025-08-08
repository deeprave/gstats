//! Tests for Version Compatibility Checker
//! 
//! Tests API version compatibility logic and dependency resolution.

use crate::plugin::compatibility::VersionCompatibilityChecker;
use crate::plugin::traits::{PluginInfo, PluginType};
use crate::plugin::error::PluginError;

#[test]
fn test_version_compatibility_same_major() {
    let checker = VersionCompatibilityChecker::new(20250727);
    
    // Same major version should be compatible
    assert!(checker.is_api_compatible(20250727));
    assert!(checker.is_api_compatible(20250729));
    assert!(checker.is_api_compatible(20250700));
}

#[test]
fn test_version_compatibility_different_major() {
    let checker = VersionCompatibilityChecker::new(20250727);
    
    // Different major version should be incompatible
    assert!(!checker.is_api_compatible(20240727));
    assert!(!checker.is_api_compatible(20260727));
}

#[test]
fn test_check_plugin_compatibility_success() {
    let checker = VersionCompatibilityChecker::new(20250727);
    
    let plugin_info = PluginInfo::new(
        "test-plugin".to_string(),
        "1.0.0".to_string(),
        20250727,
        "Test plugin".to_string(),
        "Test Author".to_string(),
        PluginType::Scanner,
    );
    
    let result = checker.check_plugin_compatibility(&plugin_info);
    assert!(result.is_ok());
}

#[test]
fn test_check_plugin_compatibility_failure() {
    let checker = VersionCompatibilityChecker::new(20250727);
    
    let plugin_info = PluginInfo::new(
        "test-plugin".to_string(),
        "1.0.0".to_string(),
        20240727, // Different major version
        "Test plugin".to_string(),
        "Test Author".to_string(),
        PluginType::Scanner,
    );
    
    let result = checker.check_plugin_compatibility(&plugin_info);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), PluginError::VersionIncompatible { .. }));
}

#[test]
fn test_validate_dependencies_no_deps() {
    let checker = VersionCompatibilityChecker::new(20250727);
    
    let plugin_info = PluginInfo::new(
        "test-plugin".to_string(),
        "1.0.0".to_string(),
        20250727,
        "Test plugin".to_string(),
        "Test Author".to_string(),
        PluginType::Scanner,
    );
    
    let available_plugins = vec![];
    let result = checker.validate_dependencies(&plugin_info, &available_plugins);
    assert!(result.is_ok());
}

#[test]
fn test_validate_dependencies_satisfied() {
    let checker = VersionCompatibilityChecker::new(20250727);
    
    let mut plugin_info = PluginInfo::new(
        "test-plugin".to_string(),
        "1.0.0".to_string(),
        20250727,
        "Test plugin".to_string(),
        "Test Author".to_string(),
        PluginType::Scanner,
    ).with_dependency(
        "dependency-plugin".to_string(),
        "1.0.0".to_string(),
        false,
    );
    
    let dependency_info = PluginInfo::new(
        "dependency-plugin".to_string(),
        "1.0.0".to_string(),
        20250727,
        "Dependency plugin".to_string(),
        "Test Author".to_string(),
        PluginType::Scanner,
    );
    
    let available_plugins = vec![dependency_info];
    let result = checker.validate_dependencies(&plugin_info, &available_plugins);
    assert!(result.is_ok());
}

#[test]
fn test_validate_dependencies_missing() {
    let checker = VersionCompatibilityChecker::new(20250727);
    
    let plugin_info = PluginInfo::new(
        "test-plugin".to_string(),
        "1.0.0".to_string(),
        20250727,
        "Test plugin".to_string(),
        "Test Author".to_string(),
        PluginType::Scanner,
    ).with_dependency(
        "missing-plugin".to_string(),
        "1.0.0".to_string(),
        false, // Not optional
    );
    
    let available_plugins = vec![];
    let result = checker.validate_dependencies(&plugin_info, &available_plugins);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), PluginError::DependencyError { .. }));
}

#[test]
fn test_validate_dependencies_optional_missing() {
    let checker = VersionCompatibilityChecker::new(20250727);
    
    let plugin_info = PluginInfo::new(
        "test-plugin".to_string(),
        "1.0.0".to_string(),
        20250727,
        "Test plugin".to_string(),
        "Test Author".to_string(),
        PluginType::Scanner,
    ).with_dependency(
        "optional-plugin".to_string(),
        "1.0.0".to_string(),
        true, // Optional
    );
    
    let available_plugins = vec![];
    let result = checker.validate_dependencies(&plugin_info, &available_plugins);
    assert!(result.is_ok()); // Should succeed because dependency is optional
}

#[test]
fn test_validate_dependencies_version_mismatch() {
    let checker = VersionCompatibilityChecker::new(20250727);
    
    let plugin_info = PluginInfo::new(
        "test-plugin".to_string(),
        "1.0.0".to_string(),
        20250727,
        "Test plugin".to_string(),
        "Test Author".to_string(),
        PluginType::Scanner,
    ).with_dependency(
        "dependency-plugin".to_string(),
        "^2.0.0".to_string(), // Requires version 2.x
        false,
    );
    
    let dependency_info = PluginInfo::new(
        "dependency-plugin".to_string(),
        "1.0.0".to_string(), // Version 1.0.0 available
        20250727,
        "Dependency plugin".to_string(),
        "Test Author".to_string(),
        PluginType::Scanner,
    );
    
    let available_plugins = vec![dependency_info];
    let result = checker.validate_dependencies(&plugin_info, &available_plugins);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), PluginError::DependencyError { .. }));
}

#[test]
fn test_get_major_version() {
    let checker = VersionCompatibilityChecker::new(20250727);
    
    assert_eq!(checker.get_major_version(20250727), 2025);
    assert_eq!(checker.get_major_version(20240101), 2024);
    assert_eq!(checker.get_major_version(20261231), 2026);
}

#[test]
fn test_parse_version_requirement() {
    let checker = VersionCompatibilityChecker::new(20250727);
    
    // Test exact version
    assert!(checker.version_matches("1.0.0", "1.0.0"));
    assert!(!checker.version_matches("1.0.0", "1.0.1"));
    
    // Test caret requirements (^)
    assert!(checker.version_matches("^1.0.0", "1.0.0"));
    assert!(checker.version_matches("^1.0.0", "1.0.5"));
    assert!(checker.version_matches("^1.0.0", "1.9.9"));
    assert!(!checker.version_matches("^1.0.0", "2.0.0"));
    
    // Test tilde requirements (~)
    assert!(checker.version_matches("~1.2.0", "1.2.0"));
    assert!(checker.version_matches("~1.2.0", "1.2.5"));
    assert!(!checker.version_matches("~1.2.0", "1.3.0"));
    
    // Test wildcard (*)
    assert!(checker.version_matches("*", "1.0.0"));
    assert!(checker.version_matches("*", "2.5.3"));
}

#[test]
fn test_check_all_plugins() {
    let checker = VersionCompatibilityChecker::new(20250727);
    
    let plugin1 = PluginInfo::new(
        "plugin1".to_string(),
        "1.0.0".to_string(),
        20250727,
        "Plugin 1".to_string(),
        "Test Author".to_string(),
        PluginType::Scanner,
    );
    
    let plugin2 = PluginInfo::new(
        "plugin2".to_string(),
        "1.0.0".to_string(),
        20240727, // Incompatible version
        "Plugin 2".to_string(),
        "Test Author".to_string(),
        PluginType::Scanner,
    );
    
    let plugins = vec![plugin1, plugin2];
    let results = checker.check_all_plugins(&plugins);
    
    assert_eq!(results.len(), 2);
    assert!(results.get("plugin1").unwrap().is_ok());
    assert!(results.get("plugin2").unwrap().is_err());
}

#[test]
fn test_circular_dependency_detection() {
    let checker = VersionCompatibilityChecker::new(20250727);
    
    // Plugin A depends on B
    let plugin_a = PluginInfo::new(
        "plugin-a".to_string(),
        "1.0.0".to_string(),
        20250727,
        "Plugin A".to_string(),
        "Test Author".to_string(),
        PluginType::Scanner,
    ).with_dependency("plugin-b".to_string(), "1.0.0".to_string(), false);
    
    // Plugin B depends on A (circular)
    let plugin_b = PluginInfo::new(
        "plugin-b".to_string(),
        "1.0.0".to_string(),
        20250727,
        "Plugin B".to_string(),
        "Test Author".to_string(),
        PluginType::Scanner,
    ).with_dependency("plugin-a".to_string(), "1.0.0".to_string(), false);
    
    let available_plugins = vec![plugin_a.clone(), plugin_b];
    let result = checker.validate_dependencies(&plugin_a, &available_plugins);
    
    // Should detect circular dependency
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), PluginError::DependencyError { .. }));
}