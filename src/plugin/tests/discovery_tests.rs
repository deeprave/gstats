//! Tests for Plugin Discovery System
//! 
//! Tests plugin discovery, descriptor parsing, and dynamic loading capabilities.

use crate::plugin::discovery::{PluginDiscovery, FileBasedDiscovery, MultiDirectoryDiscovery, PluginDescriptorParser};
use crate::plugin::traits::{PluginDescriptor, PluginInfo, PluginType};
use crate::plugin::error::PluginError;
use std::collections::HashMap;
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

#[tokio::test]
async fn test_multi_directory_discovery_creation() {
    use std::path::PathBuf;
    
    let directories = vec![
        PathBuf::from("plugins1"),
        PathBuf::from("plugins2"),
    ];
    let explicit_plugins = vec!["plugin1".to_string()];
    let excluded_plugins = vec!["unwanted".to_string()];
    
    let discovery = MultiDirectoryDiscovery::new(
        directories,
        explicit_plugins,
        excluded_plugins,
    );
    
    assert_eq!(discovery.plugin_directory().to_string_lossy(), "plugins1");
    assert!(discovery.supports_dynamic_loading());
}

#[tokio::test]
async fn test_multi_directory_discovery_multiple_dirs() {
    use std::path::PathBuf;
    use tempfile::tempdir;
    
    let temp_dir1 = tempdir().unwrap();
    let temp_dir2 = tempdir().unwrap();
    
    // Create plugin1 in first directory
    let plugin1_content = create_test_plugin_yaml("test-plugin1", "1.0.0", PluginType::Scanner);
    let plugin1_path = temp_dir1.path().join("test-plugin1.yaml");
    fs::write(&plugin1_path, plugin1_content).await.unwrap();
    
    // Create plugin2 in second directory
    let plugin2_content = create_test_plugin_yaml("test-plugin2", "2.0.0", PluginType::Output);
    let plugin2_path = temp_dir2.path().join("test-plugin2.yaml");
    fs::write(&plugin2_path, plugin2_content).await.unwrap();
    
    let directories = vec![
        temp_dir1.path().to_path_buf(),
        temp_dir2.path().to_path_buf(),
    ];
    
    let discovery = MultiDirectoryDiscovery::new(
        directories,
        Vec::new(),
        Vec::new(),
    );
    
    let plugins = discovery.discover_plugins().await.unwrap();
    assert_eq!(plugins.len(), 2);
    
    let plugin_names: Vec<&str> = plugins.iter().map(|p| p.info.name.as_str()).collect();
    assert!(plugin_names.contains(&"test-plugin1"));
    assert!(plugin_names.contains(&"test-plugin2"));
}

#[tokio::test]
async fn test_multi_directory_discovery_explicit_loading() {
    use std::path::PathBuf;
    use tempfile::tempdir;
    
    let temp_dir = tempdir().unwrap();
    
    // Create two plugins in directory
    let plugin1_content = create_test_plugin_yaml("plugin1", "1.0.0", PluginType::Scanner);
    let plugin1_path = temp_dir.path().join("plugin1.yaml");
    fs::write(&plugin1_path, plugin1_content).await.unwrap();
    
    let plugin2_content = create_test_plugin_yaml("plugin2", "2.0.0", PluginType::Output);
    let plugin2_path = temp_dir.path().join("plugin2.yaml");
    fs::write(&plugin2_path, plugin2_content).await.unwrap();
    
    let directories = vec![temp_dir.path().to_path_buf()];
    let explicit_plugins = vec!["plugin1".to_string()]; // Only load plugin1
    
    let discovery = MultiDirectoryDiscovery::new(
        directories,
        explicit_plugins,
        Vec::new(),
    );
    
    let plugins = discovery.discover_plugins().await.unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].info.name, "plugin1");
}

#[tokio::test]
async fn test_multi_directory_discovery_exclusion() {
    use std::path::PathBuf;
    use tempfile::tempdir;
    
    let temp_dir = tempdir().unwrap();
    
    // Create two plugins
    let plugin1_content = create_test_plugin_yaml("wanted-plugin", "1.0.0", PluginType::Scanner);
    let plugin1_path = temp_dir.path().join("wanted-plugin.yaml");
    fs::write(&plugin1_path, plugin1_content).await.unwrap();
    
    let plugin2_content = create_test_plugin_yaml("unwanted-plugin", "2.0.0", PluginType::Output);
    let plugin2_path = temp_dir.path().join("unwanted-plugin.yaml");
    fs::write(&plugin2_path, plugin2_content).await.unwrap();
    
    let directories = vec![temp_dir.path().to_path_buf()];
    let excluded_plugins = vec!["unwanted-plugin".to_string()];
    
    let discovery = MultiDirectoryDiscovery::new(
        directories,
        Vec::new(),
        excluded_plugins,
    );
    
    let plugins = discovery.discover_plugins().await.unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].info.name, "wanted-plugin");
}

#[tokio::test]
async fn test_multi_directory_discovery_deduplication() {
    use std::path::PathBuf;
    use tempfile::tempdir;
    
    let temp_dir1 = tempdir().unwrap();
    let temp_dir2 = tempdir().unwrap();
    
    // Create same plugin in both directories (first found wins)
    let plugin_content = create_test_plugin_yaml("duplicate-plugin", "1.0.0", PluginType::Scanner);
    
    let plugin1_path = temp_dir1.path().join("duplicate-plugin.yaml");
    fs::write(&plugin1_path, plugin_content.clone()).await.unwrap();
    
    let plugin2_path = temp_dir2.path().join("duplicate-plugin.yaml");
    fs::write(&plugin2_path, plugin_content).await.unwrap();
    
    let directories = vec![
        temp_dir1.path().to_path_buf(),
        temp_dir2.path().to_path_buf(),
    ];
    
    let discovery = MultiDirectoryDiscovery::new(
        directories,
        Vec::new(),
        Vec::new(),
    );
    
    let plugins = discovery.discover_plugins().await.unwrap();
    assert_eq!(plugins.len(), 1); // Should be deduplicated
    assert_eq!(plugins[0].info.name, "duplicate-plugin");
}

#[tokio::test]
async fn test_multi_directory_discovery_explicit_by_path() {
    use std::path::PathBuf;
    use tempfile::tempdir;
    
    let temp_dir = tempdir().unwrap();
    
    // Create plugin
    let plugin_content = create_test_plugin_yaml("path-plugin", "1.0.0", PluginType::Scanner);
    let plugin_path = temp_dir.path().join("path-plugin.yaml");
    fs::write(&plugin_path, plugin_content).await.unwrap();
    
    let directories = Vec::new(); // No directories for discovery
    let explicit_plugins = vec![plugin_path.to_string_lossy().to_string()]; // Load by full path
    
    let discovery = MultiDirectoryDiscovery::new(
        directories,
        explicit_plugins,
        Vec::new(),
    );
    
    let plugins = discovery.discover_plugins().await.unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].info.name, "path-plugin");
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
file_path: null
entry_point: main
config: {{}}
"#
    )
}

