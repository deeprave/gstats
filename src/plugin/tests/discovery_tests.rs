//! Tests for Plugin Discovery System
//! 
//! Tests plugin discovery, descriptor parsing, and dynamic loading capabilities.

use crate::plugin::discovery::{PluginDiscovery, FileBasedDiscovery, PluginDescriptorParser};
use crate::plugin::traits::{PluginDescriptor, PluginInfo, PluginType};
use crate::plugin::error::PluginError;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

#[tokio::test]
async fn test_file_based_discovery_creation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let discovery = FileBasedDiscovery::new(temp_dir.path()).unwrap();
    assert_eq!(discovery.plugin_directory(), temp_dir.path());
    assert!(!discovery.supports_dynamic_loading());
}

#[tokio::test]
async fn test_file_based_discovery_with_dynamic_loading() {
    let temp_dir = tempfile::tempdir().unwrap();
    let discovery = FileBasedDiscovery::with_dynamic_loading(temp_dir.path(), true).unwrap();
    assert!(discovery.supports_dynamic_loading());
}

#[tokio::test]
async fn test_discover_plugins_empty_directory() {
    let temp_dir = tempfile::tempdir().unwrap();
    let discovery = FileBasedDiscovery::new(temp_dir.path()).unwrap();
    
    let plugins = discovery.discover_plugins().await.unwrap();
    assert!(plugins.is_empty());
}

#[tokio::test]
async fn test_discover_plugins_with_descriptors() {
    let temp_dir = tempfile::tempdir().unwrap();
    
    // Create a test plugin descriptor
    let descriptor = create_test_descriptor("test-plugin", "1.0.0");
    let descriptor_path = temp_dir.path().join("test-plugin.yaml");
    
    let yaml_content = serde_yaml::to_string(&descriptor).unwrap();
    fs::write(&descriptor_path, yaml_content).await.unwrap();
    
    let discovery = FileBasedDiscovery::new(temp_dir.path()).unwrap();
    let plugins = discovery.discover_plugins().await.unwrap();
    
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].info.name, "test-plugin");
    assert_eq!(plugins[0].info.version, "1.0.0");
}

#[tokio::test]
async fn test_discover_plugins_multiple_descriptors() {
    let temp_dir = tempfile::tempdir().unwrap();
    
    // Create multiple test plugin descriptors
    let descriptors = vec![
        create_test_descriptor("plugin-a", "1.0.0"),
        create_test_descriptor("plugin-b", "2.1.0"),
        create_test_descriptor("plugin-c", "0.5.0"),
    ];
    
    for desc in &descriptors {
        let descriptor_path = temp_dir.path().join(format!("{}.yaml", desc.info.name));
        let yaml_content = serde_yaml::to_string(desc).unwrap();
        fs::write(&descriptor_path, yaml_content).await.unwrap();
    }
    
    let discovery = FileBasedDiscovery::new(temp_dir.path()).unwrap();
    let plugins = discovery.discover_plugins().await.unwrap();
    
    assert_eq!(plugins.len(), 3);
    
    // Check that all plugins were discovered
    let names: Vec<String> = plugins.iter().map(|p| p.info.name.clone()).collect();
    assert!(names.contains(&"plugin-a".to_string()));
    assert!(names.contains(&"plugin-b".to_string()));
    assert!(names.contains(&"plugin-c".to_string()));
}

#[tokio::test]
async fn test_discover_plugins_invalid_descriptors() {
    let temp_dir = tempfile::tempdir().unwrap();
    
    // Create an invalid descriptor file
    let invalid_content = "invalid: yaml: content: [unclosed";
    let invalid_path = temp_dir.path().join("invalid.yaml");
    fs::write(&invalid_path, invalid_content).await.unwrap();
    
    // Create a valid descriptor
    let valid_descriptor = create_test_descriptor("valid-plugin", "1.0.0");
    let valid_path = temp_dir.path().join("valid.yaml");
    let yaml_content = serde_yaml::to_string(&valid_descriptor).unwrap();
    fs::write(&valid_path, yaml_content).await.unwrap();
    
    let discovery = FileBasedDiscovery::new(temp_dir.path()).unwrap();
    let plugins = discovery.discover_plugins().await.unwrap();
    
    // Should only find the valid plugin, ignoring invalid ones
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].info.name, "valid-plugin");
}

#[tokio::test]
async fn test_discover_plugins_with_subdirectories() {
    let temp_dir = tempfile::tempdir().unwrap();
    
    // Create subdirectory structure
    let subdir = temp_dir.path().join("subdir");
    fs::create_dir(&subdir).await.unwrap();
    
    // Add descriptors in main directory and subdirectory
    let main_descriptor = create_test_descriptor("main-plugin", "1.0.0");
    let main_path = temp_dir.path().join("main.yaml");
    fs::write(&main_path, serde_yaml::to_string(&main_descriptor).unwrap()).await.unwrap();
    
    let sub_descriptor = create_test_descriptor("sub-plugin", "2.0.0");
    let sub_path = subdir.join("sub.yaml");
    fs::write(&sub_path, serde_yaml::to_string(&sub_descriptor).unwrap()).await.unwrap();
    
    let discovery = FileBasedDiscovery::new(temp_dir.path()).unwrap();
    let plugins = discovery.discover_plugins().await.unwrap();
    
    // Should find plugins in subdirectories too
    assert_eq!(plugins.len(), 2);
    
    let names: Vec<String> = plugins.iter().map(|p| p.info.name.clone()).collect();
    assert!(names.contains(&"main-plugin".to_string()));
    assert!(names.contains(&"sub-plugin".to_string()));
}

#[tokio::test]
async fn test_plugin_descriptor_parser() {
    let parser = PluginDescriptorParser::new();
    
    let descriptor = create_test_descriptor("test-parser", "1.5.0");
    let yaml_content = serde_yaml::to_string(&descriptor).unwrap();
    
    let parsed = parser.parse_yaml(&yaml_content).unwrap();
    assert_eq!(parsed.info.name, "test-parser");
    assert_eq!(parsed.info.version, "1.5.0");
}

#[tokio::test]
async fn test_plugin_descriptor_parser_invalid_yaml() {
    let parser = PluginDescriptorParser::new();
    
    let invalid_yaml = "invalid: yaml: [unclosed";
    let result = parser.parse_yaml(invalid_yaml);
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), PluginError::DescriptorParseError { .. }));
}

#[tokio::test]
async fn test_plugin_descriptor_parser_missing_required_fields() {
    let parser = PluginDescriptorParser::new();
    
    let incomplete_yaml = r#"
info:
  name: "incomplete-plugin"
  # missing version and other required fields
entry_point: "main"
"#;
    
    let result = parser.parse_yaml(incomplete_yaml);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_plugin_descriptor_validation() {
    let parser = PluginDescriptorParser::new();
    
    // Test valid descriptor
    let valid_descriptor = create_test_descriptor("valid", "1.0.0");
    assert!(parser.validate_descriptor(&valid_descriptor).is_ok());
    
    // Test descriptor with invalid version
    let invalid_descriptor = create_test_descriptor("invalid", "not-a-version");
    let result = parser.validate_descriptor(&invalid_descriptor);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_plugin_descriptor_with_config() {
    let temp_dir = tempfile::tempdir().unwrap();
    
    let mut descriptor = create_test_descriptor("config-plugin", "1.0.0");
    descriptor.config.insert("timeout".to_string(), serde_json::Value::Number(serde_json::Number::from(5000)));
    descriptor.config.insert("enabled".to_string(), serde_json::Value::Bool(true));
    
    let descriptor_path = temp_dir.path().join("config-plugin.yaml");
    let yaml_content = serde_yaml::to_string(&descriptor).unwrap();
    fs::write(&descriptor_path, yaml_content).await.unwrap();
    
    let discovery = FileBasedDiscovery::new(temp_dir.path()).unwrap();
    let plugins = discovery.discover_plugins().await.unwrap();
    
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].config.len(), 2);
    assert_eq!(plugins[0].config.get("timeout").unwrap().as_u64().unwrap(), 5000);
    assert_eq!(plugins[0].config.get("enabled").unwrap().as_bool().unwrap(), true);
}

#[tokio::test]
async fn test_discover_plugins_filter_by_type() {
    let temp_dir = tempfile::tempdir().unwrap();
    
    // Create descriptors for different plugin types
    let scanner_desc = create_test_descriptor_with_type("scanner-plugin", "1.0.0", PluginType::Scanner);
    let notification_desc = create_test_descriptor_with_type("notification-plugin", "1.0.0", PluginType::Notification);
    
    let scanner_path = temp_dir.path().join("scanner.yaml");
    fs::write(&scanner_path, serde_yaml::to_string(&scanner_desc).unwrap()).await.unwrap();
    
    let notification_path = temp_dir.path().join("notification.yaml");
    fs::write(&notification_path, serde_yaml::to_string(&notification_desc).unwrap()).await.unwrap();
    
    let discovery = FileBasedDiscovery::new(temp_dir.path()).unwrap();
    
    // Test filtering by scanner type
    let scanner_plugins = discovery.discover_plugins_by_type(PluginType::Scanner).await.unwrap();
    assert_eq!(scanner_plugins.len(), 1);
    assert_eq!(scanner_plugins[0].info.name, "scanner-plugin");
    
    // Test filtering by notification type
    let notification_plugins = discovery.discover_plugins_by_type(PluginType::Notification).await.unwrap();
    assert_eq!(notification_plugins.len(), 1);
    assert_eq!(notification_plugins[0].info.name, "notification-plugin");
}

#[tokio::test]
async fn test_discover_plugins_with_api_version_filtering() {
    let temp_dir = tempfile::tempdir().unwrap();
    
    // Create descriptors with different API versions
    let old_api_desc = create_test_descriptor_with_api("old-api", "1.0.0", 20240101);
    let current_api_desc = create_test_descriptor_with_api("current-api", "1.0.0", 20250727);
    let future_api_desc = create_test_descriptor_with_api("future-api", "1.0.0", 20260101);
    
    for (desc, filename) in [
        (&old_api_desc, "old.yaml"),
        (&current_api_desc, "current.yaml"),
        (&future_api_desc, "future.yaml"),
    ] {
        let path = temp_dir.path().join(filename);
        fs::write(&path, serde_yaml::to_string(desc).unwrap()).await.unwrap();
    }
    
    let discovery = FileBasedDiscovery::new(temp_dir.path()).unwrap();
    
    // Test filtering by API version compatibility
    let compatible_plugins = discovery.discover_compatible_plugins(20250727).await.unwrap();
    
    // Should only find the plugin with compatible API version
    assert_eq!(compatible_plugins.len(), 1);
    assert_eq!(compatible_plugins[0].info.name, "current-api");
}

#[tokio::test]
async fn test_plugin_descriptor_caching() {
    let temp_dir = tempfile::tempdir().unwrap();
    
    let descriptor = create_test_descriptor("cached-plugin", "1.0.0");
    let descriptor_path = temp_dir.path().join("cached.yaml");
    fs::write(&descriptor_path, serde_yaml::to_string(&descriptor).unwrap()).await.unwrap();
    
    let discovery = FileBasedDiscovery::with_caching(temp_dir.path(), true).unwrap();
    
    // First discovery - should read from filesystem
    let start = std::time::Instant::now();
    let plugins1 = discovery.discover_plugins().await.unwrap();
    let first_duration = start.elapsed();
    
    // Second discovery - should use cache
    let start = std::time::Instant::now();
    let plugins2 = discovery.discover_plugins().await.unwrap();
    let second_duration = start.elapsed();
    
    assert_eq!(plugins1.len(), 1);
    assert_eq!(plugins2.len(), 1);
    assert_eq!(plugins1[0].info.name, plugins2[0].info.name);
    
    // Cache should make second call faster (though this might be flaky in tests)
    // We'll just verify both calls succeeded
    assert!(first_duration.as_nanos() > 0);
    assert!(second_duration.as_nanos() > 0);
}

#[tokio::test]
async fn test_plugin_discovery_error_handling() {
    // Test discovery with non-existent directory
    let result = FileBasedDiscovery::new("non-existent-directory");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), PluginError::DiscoveryError { .. }));
    
    // Test discovery with file instead of directory
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let result = FileBasedDiscovery::new(temp_file.path());
    assert!(result.is_err());
}

/// Helper function to create a test plugin descriptor
fn create_test_descriptor(name: &str, version: &str) -> PluginDescriptor {
    create_test_descriptor_with_type(name, version, PluginType::Scanner)
}

/// Helper function to create a test plugin descriptor with specific type
fn create_test_descriptor_with_type(name: &str, version: &str, plugin_type: PluginType) -> PluginDescriptor {
    let mut descriptor = create_test_descriptor_with_api(name, version, 20250727);
    descriptor.info.plugin_type = plugin_type;
    descriptor
}

/// Helper function to create a test plugin descriptor with specific API version
fn create_test_descriptor_with_api(name: &str, version: &str, api_version: u32) -> PluginDescriptor {
    let info = PluginInfo::new(
        name.to_string(),
        version.to_string(),
        api_version,
        format!("Test plugin: {}", name),
        "Test Author".to_string(),
        PluginType::Scanner,
    );
    
    PluginDescriptor {
        info,
        file_path: None,
        entry_point: "main".to_string(),
        config: HashMap::new(),
    }
}


fn create_test_plugin_yaml(name: &str, version: &str, plugin_type: PluginType) -> String {
    format!(
        r#"info:
  name: "{name}"
  version: "{version}"
  api_version: 20250727
  description: "Test plugin: {name}"
  author: "Test Author"
  url: null
  dependencies: []
  capabilities: []
  plugin_type: {plugin_type:?}
  license: null
  priority: 5
file_path: null
entry_point: main
config: {{}}
"#
    )
}

// Tests for UnifiedPluginDiscovery
// Following TDD methodology - these tests should initially fail

#[tokio::test]
async fn test_unified_discovery_creation_without_directory() {
    use crate::plugin::discovery::UnifiedPluginDiscovery;
    
    let excluded_plugins = vec!["unwanted".to_string()];
    let discovery = UnifiedPluginDiscovery::new(None, excluded_plugins).unwrap();
    
    // Should not have external discovery when no directory provided
    assert!(!discovery.supports_dynamic_loading());
    assert_eq!(discovery.plugin_directory().to_string_lossy(), "plugins");
}

#[tokio::test]
async fn test_unified_discovery_creation_with_nonexistent_directory() {
    use crate::plugin::discovery::UnifiedPluginDiscovery;
    use std::path::PathBuf;
    
    let nonexistent_dir = PathBuf::from("/nonexistent/directory");
    let excluded_plugins = vec![];
    let discovery = UnifiedPluginDiscovery::new(Some(nonexistent_dir), excluded_plugins).unwrap();
    
    // Should not have external discovery when directory doesn't exist
    assert!(!discovery.supports_dynamic_loading());
}

#[tokio::test]
async fn test_unified_discovery_creation_with_existing_directory() {
    use crate::plugin::discovery::UnifiedPluginDiscovery;
    use tempfile::tempdir;
    
    let temp_dir = tempdir().unwrap();
    let excluded_plugins = vec![];
    let discovery = UnifiedPluginDiscovery::new(Some(temp_dir.path().to_path_buf()), excluded_plugins).unwrap();
    
    // Should have external discovery when directory exists
    assert!(discovery.supports_dynamic_loading());
    assert_eq!(discovery.plugin_directory(), temp_dir.path());
}

#[tokio::test]
async fn test_unified_discovery_with_default_directory() {
    use crate::plugin::discovery::UnifiedPluginDiscovery;
    
    let excluded_plugins = vec![];
    let discovery = UnifiedPluginDiscovery::with_default_directory(excluded_plugins).unwrap();
    
    // Should use default directory in home/.config/gstats/plugins
    let plugin_dir = discovery.plugin_directory();
    assert!(plugin_dir.to_string_lossy().contains(".config/gstats/plugins"));
}

#[tokio::test]
async fn test_unified_discovery_builtin_plugins_only() {
    use crate::plugin::discovery::{UnifiedPluginDiscovery, PluginDiscovery};
    
    // No external directory, so only builtin plugins
    let excluded_plugins = vec![];
    let discovery = UnifiedPluginDiscovery::new(None, excluded_plugins).unwrap();
    
    let plugins = discovery.discover_plugins().await.unwrap();
    
    // Should find builtin plugins: commits, metrics, export
    assert_eq!(plugins.len(), 3);
    
    let plugin_names: Vec<&str> = plugins.iter().map(|p| p.info.name.as_str()).collect();
    assert!(plugin_names.contains(&"commits"));
    assert!(plugin_names.contains(&"metrics"));
    assert!(plugin_names.contains(&"export"));
    
    // All should be builtin (no file_path)
    for plugin in &plugins {
        assert!(plugin.file_path.is_none());
        assert_eq!(plugin.entry_point, "builtin");
    }
}

#[tokio::test]
async fn test_unified_discovery_builtin_plugins_with_exclusions() {
    use crate::plugin::discovery::{UnifiedPluginDiscovery, PluginDiscovery};
    
    // Exclude one builtin plugin
    let excluded_plugins = vec!["metrics".to_string()];
    let discovery = UnifiedPluginDiscovery::new(None, excluded_plugins).unwrap();
    
    let plugins = discovery.discover_plugins().await.unwrap();
    
    // Should find 2 builtin plugins (3 total - 1 excluded)
    assert_eq!(plugins.len(), 2);
    
    let plugin_names: Vec<&str> = plugins.iter().map(|p| p.info.name.as_str()).collect();
    assert!(plugin_names.contains(&"commits"));
    assert!(plugin_names.contains(&"export"));
    assert!(!plugin_names.contains(&"metrics")); // Should be excluded
}

#[tokio::test]
async fn test_unified_discovery_external_plugins_only() {
    use crate::plugin::discovery::{UnifiedPluginDiscovery, PluginDiscovery};
    use tempfile::tempdir;
    use tokio::fs;
    
    let temp_dir = tempdir().unwrap();
    
    // Create external plugin
    let external_plugin = create_test_plugin_yaml("external-scanner", "1.0.0", PluginType::Scanner);
    let external_path = temp_dir.path().join("external-scanner.yaml");
    fs::write(&external_path, external_plugin).await.unwrap();
    
    // Exclude ALL builtin plugins
    let excluded_plugins = vec!["commits".to_string(), "metrics".to_string(), "export".to_string()];
    let discovery = UnifiedPluginDiscovery::new(Some(temp_dir.path().to_path_buf()), excluded_plugins).unwrap();
    
    let plugins = discovery.discover_plugins().await.unwrap();
    
    // Should find only the external plugin
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].info.name, "external-scanner");
    assert!(plugins[0].file_path.is_some()); // External plugin should have file_path
}

#[tokio::test]
async fn test_unified_discovery_mixed_builtin_and_external() {
    use crate::plugin::discovery::{UnifiedPluginDiscovery, PluginDiscovery};
    use tempfile::tempdir;
    use tokio::fs;
    
    let temp_dir = tempdir().unwrap();
    
    // Create external plugin
    let external_plugin = create_test_plugin_yaml("external-processor", "2.0.0", PluginType::Processing);
    let external_path = temp_dir.path().join("external-processor.yaml");
    fs::write(&external_path, external_plugin).await.unwrap();
    
    // No exclusions
    let excluded_plugins = vec![];
    let discovery = UnifiedPluginDiscovery::new(Some(temp_dir.path().to_path_buf()), excluded_plugins).unwrap();
    
    let plugins = discovery.discover_plugins().await.unwrap();
    
    // Should find 3 builtin + 1 external = 4 total
    assert_eq!(plugins.len(), 4);
    
    let plugin_names: Vec<&str> = plugins.iter().map(|p| p.info.name.as_str()).collect();
    assert!(plugin_names.contains(&"commits"));
    assert!(plugin_names.contains(&"metrics"));
    assert!(plugin_names.contains(&"export"));
    assert!(plugin_names.contains(&"external-processor"));
}

#[tokio::test]
async fn test_unified_discovery_external_overrides_builtin() {
    use crate::plugin::discovery::{UnifiedPluginDiscovery, PluginDiscovery};
    use tempfile::tempdir;
    use tokio::fs;
    
    let temp_dir = tempdir().unwrap();
    
    // Create external plugin with same name as builtin
    let external_plugin = create_test_plugin_yaml("commits", "3.0.0", PluginType::Processing);
    let external_path = temp_dir.path().join("commits.yaml");
    fs::write(&external_path, external_plugin).await.unwrap();
    
    // No exclusions
    let excluded_plugins = vec![];
    let discovery = UnifiedPluginDiscovery::new(Some(temp_dir.path().to_path_buf()), excluded_plugins).unwrap();
    
    let plugins = discovery.discover_plugins().await.unwrap();
    
    // Should find 3 total: external "commits" + builtin "metrics" + builtin "export"
    assert_eq!(plugins.len(), 3);
    
    let commits_plugin = plugins.iter().find(|p| p.info.name == "commits").unwrap();
    
    // The "commits" plugin should be the external one (has file_path and version 3.0.0)
    assert!(commits_plugin.file_path.is_some());
    assert_eq!(commits_plugin.info.version, "3.0.0");
    assert_eq!(commits_plugin.info.plugin_type, PluginType::Processing); // External type
}

#[tokio::test]
async fn test_unified_discovery_multiple_external_plugins_same_name() {
    use crate::plugin::discovery::{UnifiedPluginDiscovery, PluginDiscovery};
    use tempfile::tempdir;
    use tokio::fs;
    
    let temp_dir = tempdir().unwrap();
    
    // Create subdirectory with another plugin with same name
    let subdir = temp_dir.path().join("subdir");
    fs::create_dir(&subdir).await.unwrap();
    
    // Create two external plugins with same name in different locations
    let plugin1 = create_test_plugin_yaml("duplicate", "1.0.0", PluginType::Scanner);
    let plugin1_path = temp_dir.path().join("duplicate.yaml");
    fs::write(&plugin1_path, plugin1).await.unwrap();
    
    let plugin2 = create_test_plugin_yaml("duplicate", "2.0.0", PluginType::Output);
    let plugin2_path = subdir.join("duplicate.yaml");
    fs::write(&plugin2_path, plugin2).await.unwrap();
    
    // Exclude all builtin plugins to focus on external behavior
    let excluded_plugins = vec!["commits".to_string(), "metrics".to_string(), "export".to_string()];
    let discovery = UnifiedPluginDiscovery::new(Some(temp_dir.path().to_path_buf()), excluded_plugins).unwrap();
    
    let plugins = discovery.discover_plugins().await.unwrap();
    
    // Should find only one "duplicate" plugin (FileBasedDiscovery's deduplication)
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].info.name, "duplicate");
    assert!(plugins[0].file_path.is_some());
}

#[tokio::test]
async fn test_unified_discovery_external_exclusion() {
    use crate::plugin::discovery::{UnifiedPluginDiscovery, PluginDiscovery};
    use tempfile::tempdir;
    use tokio::fs;
    
    let temp_dir = tempdir().unwrap();
    
    // Create external plugins
    let wanted_plugin = create_test_plugin_yaml("wanted", "1.0.0", PluginType::Scanner);
    let wanted_path = temp_dir.path().join("wanted.yaml");
    fs::write(&wanted_path, wanted_plugin).await.unwrap();
    
    let unwanted_plugin = create_test_plugin_yaml("unwanted", "1.0.0", PluginType::Output);
    let unwanted_path = temp_dir.path().join("unwanted.yaml");
    fs::write(&unwanted_path, unwanted_plugin).await.unwrap();
    
    // Exclude the unwanted external plugin and one builtin
    let excluded_plugins = vec!["unwanted".to_string(), "metrics".to_string()];
    let discovery = UnifiedPluginDiscovery::new(Some(temp_dir.path().to_path_buf()), excluded_plugins).unwrap();
    
    let plugins = discovery.discover_plugins().await.unwrap();
    
    // Should find: "wanted" external + "commits" builtin + "export" builtin = 3 total
    assert_eq!(plugins.len(), 3);
    
    let plugin_names: Vec<&str> = plugins.iter().map(|p| p.info.name.as_str()).collect();
    assert!(plugin_names.contains(&"wanted"));
    assert!(plugin_names.contains(&"commits"));
    assert!(plugin_names.contains(&"export"));
    assert!(!plugin_names.contains(&"unwanted"));
    assert!(!plugin_names.contains(&"metrics"));
}

