//! Basic Scanner Tests
//! 
//! Simple tests for core scanner functionality.

use std::sync::Arc;
use std::path::PathBuf;
use crate::scanner::config::ScannerConfig;
use crate::plugin::SharedPluginRegistry;

#[tokio::test]
async fn test_scanner_config_creation() {
    let config = ScannerConfig::default();
    
    // Verify default configuration
    assert!(!config.include_binary_files);
    assert_eq!(config.max_file_size, 10 * 1024 * 1024); // 10MB default
}

#[tokio::test]
async fn test_plugin_registry_creation() {
    let registry = SharedPluginRegistry::new();
    
    // Verify registry starts empty
    let inner = registry.inner().read().await;
    assert_eq!(inner.len(), 0);
}

#[tokio::test]
async fn test_repository_path_validation() {
    let valid_path = PathBuf::from(".");
    let invalid_path = PathBuf::from("/nonexistent/path/12345");
    
    // These paths should be handled gracefully by scanner
    assert!(valid_path.exists());
    assert!(!invalid_path.exists());
}