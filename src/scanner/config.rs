//! Scanner Configuration and Plugin Requirements
//! 
//! This module provides comprehensive configuration management for the scanner,
//! including static configuration parameters and dynamic runtime configuration
//! derived from plugin data requirements analysis.
//!
//! ## Key Features
//!
//! - **Static Configuration**: Base scanner parameters (memory, threads, branches)
//! - **Plugin Requirements**: Configuration for conditional file checkout
//! - **Runtime Analysis**: Dynamic config derived from active plugins
//! - **Validation**: Comprehensive config validation with clear error messages
//! - **Builder Pattern**: Fluent API for configuration construction
//!
//! ## Architecture
//!
//! ```text
//! Configuration System
//! ├── ScannerConfig (Static base configuration)
//! │   ├── Memory and performance settings
//! │   ├── Branch detection preferences
//! │   └── PluginRequirementsConfig
//! ├── RuntimeScannerConfig (Dynamic analysis)
//! │   ├── Plugin requirements analysis
//! │   ├── Effective checkout configuration
//! │   └── Runtime optimization flags
//! └── ConfigBuilder (Fluent construction API)
//! ```
//!
//! ## Plugin Requirements Integration
//!
//! The configuration system analyzes active plugins to determine runtime requirements:
//!
//! - **File Content Access**: Whether plugins need actual file content
//! - **Checkout Optimization**: Only checkout files when necessary
//! - **Binary File Handling**: Support for plugins that process binary files
//! - **Size Limitations**: Respect plugin-specific file size limits
//!
//! ## Usage Patterns
//!
//! ### Basic Configuration
//! ```rust,no_run
//! use gstats::scanner::config::ScannerConfig;
//! 
//! let config = ScannerConfig::default();
//! ```
//!
//! ### Builder Pattern
//! ```rust,no_run
//! use gstats::scanner::config::ScannerConfig;
//! 
//! let config = ScannerConfig::builder()
//!     .with_max_memory(128 * 1024 * 1024)
//!     .with_queue_size(2000)
//!     .with_default_branch("main".to_string())
//!     .build()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ### Runtime Analysis
//! ```rust,no_run
//! use gstats::plugin::traits::PluginDataRequirements;
//! use gstats::scanner::config::ScannerConfig;
//! 
//! let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![]; // Plugin list here
//! let config = ScannerConfig::default();
//! let runtime_config = config.analyze_plugins(&plugins);
//! 
//! if runtime_config.requires_checkout {
//!     // Setup checkout manager
//! }
//! ```

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::plugin::traits::PluginDataRequirements;

/// Scanner configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerConfig {
    /// Maximum memory usage for queues (in bytes)
    pub max_memory_bytes: usize,
    /// Default queue size
    pub queue_size: usize,
    /// Maximum number of threads for async operations
    pub max_threads: Option<usize>,
    /// Default branch to use if available
    pub default_branch: Option<String>,
    /// List of fallback branches in priority order
    pub branch_fallbacks: Vec<String>,
    /// Default remote to use for remote branch detection
    pub default_remote: Option<String>,
    /// Plugin data requirements configuration
    pub plugin_requirements: PluginRequirementsConfig,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 64 * 1024 * 1024, // 64MB
            queue_size: 1000,
            max_threads: None,
            default_branch: None,
            branch_fallbacks: vec!["main".to_string(), "master".to_string(), "develop".to_string(), "trunk".to_string()],
            default_remote: None,
            plugin_requirements: PluginRequirementsConfig::default(),
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    InvalidMaxMemory,
    InsufficientMemory,
    InvalidQueueSize,
    InsufficientQueueSize,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::InvalidMaxMemory => write!(f, "Invalid max memory configuration"),
            ConfigError::InsufficientMemory => write!(f, "Insufficient memory configuration"),
            ConfigError::InvalidQueueSize => write!(f, "Invalid queue size configuration"),
            ConfigError::InsufficientQueueSize => write!(f, "Insufficient queue size configuration"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl ScannerConfig {
    /// Create a new configuration builder
    pub fn builder() -> ScannerConfigBuilder {
        ScannerConfigBuilder::new()
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.max_memory_bytes == 0 {
            return Err(ConfigError::InvalidMaxMemory);
        }
        
        if self.max_memory_bytes < 1024 * 1024 { // Less than 1MB
            return Err(ConfigError::InsufficientMemory);
        }
        
        if self.queue_size == 0 {
            return Err(ConfigError::InvalidQueueSize);
        }
        
        if self.queue_size < 10 {
            return Err(ConfigError::InsufficientQueueSize);
        }
        
        Ok(())
    }
    
    /// Get a human-readable memory display
    pub fn memory_display(&self) -> String {
        let bytes = self.max_memory_bytes as f64;
        if bytes >= 1024.0 * 1024.0 * 1024.0 {
            format!("{:.1} GB", bytes / (1024.0 * 1024.0 * 1024.0))
        } else if bytes >= 1024.0 * 1024.0 {
            format!("{} MB", (bytes / (1024.0 * 1024.0)) as u64)
        } else if bytes >= 1024.0 {
            format!("{} KB", (bytes / 1024.0) as u64)
        } else {
            format!("{} B", bytes as u64)
        }
    }
    
    /// Analyze plugins to create runtime configuration
    pub fn analyze_plugins(&self, plugins: &[Box<dyn PluginDataRequirements>]) -> RuntimeScannerConfig {
        let requires_current_content = plugins.iter().any(|p| p.requires_current_file_content());
        let requires_historical_content = plugins.iter().any(|p| p.requires_historical_file_content());
        let requires_checkout = requires_current_content || requires_historical_content;
        
        let effective_checkout_dir = if requires_checkout {
            self.plugin_requirements.checkout_base_dir.clone()
                .or_else(|| Some(std::env::temp_dir().join("gstats-checkout")))
        } else {
            None
        };
        
        RuntimeScannerConfig {
            requires_checkout,
            requires_current_content,
            requires_historical_content,
            base_config: self.clone(),
            effective_checkout_dir,
        }
    }
}

/// Builder for ScannerConfig
#[derive(Debug, Default)]
pub struct ScannerConfigBuilder {
    config: ScannerConfig,
}

impl ScannerConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: ScannerConfig::default(),
        }
    }
    
    pub fn with_max_memory(mut self, max_memory_bytes: usize) -> Self {
        self.config.max_memory_bytes = max_memory_bytes;
        self
    }
    
    pub fn with_queue_size(mut self, queue_size: usize) -> Self {
        self.config.queue_size = queue_size;
        self
    }
    
    pub fn with_default_branch(mut self, branch: String) -> Self {
        self.config.default_branch = Some(branch);
        self
    }
    
    pub fn with_branch_fallbacks(mut self, fallbacks: Vec<String>) -> Self {
        self.config.branch_fallbacks = fallbacks;
        self
    }
    
    pub fn with_default_remote(mut self, remote: String) -> Self {
        self.config.default_remote = Some(remote);
        self
    }
    
    pub fn build(self) -> Result<ScannerConfig, ConfigError> {
        self.config.validate()?;
        Ok(self.config)
    }
}

/// Configuration for plugin data requirements and file checkout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRequirementsConfig {
    /// Whether to enable conditional file checkout based on plugin requirements
    pub enable_conditional_checkout: bool,
    
    /// Base directory for file checkouts (None = use temp directory)
    pub checkout_base_dir: Option<PathBuf>,
    
    /// Maximum number of concurrent checkouts
    pub max_concurrent_checkouts: usize,
    
    /// Whether to cache checkout directories between commits
    pub cache_checkout_dirs: bool,
    
    /// File extensions that should always be checked out regardless of plugin requirements
    pub force_checkout_extensions: Vec<String>,
    
    /// Maximum file size (in bytes) for checkout (larger files will be skipped)
    pub max_checkout_file_size: Option<u64>,
    
    /// Whether to clean up checkout directories immediately after use
    pub cleanup_immediately: bool,
}

impl Default for PluginRequirementsConfig {
    fn default() -> Self {
        Self {
            enable_conditional_checkout: true,
            checkout_base_dir: None,
            max_concurrent_checkouts: 10,
            cache_checkout_dirs: false,
            force_checkout_extensions: vec![],
            max_checkout_file_size: Some(10 * 1024 * 1024), // 10MB limit
            cleanup_immediately: true,
        }
    }
}

/// Runtime configuration derived from plugins and scanner config
#[derive(Debug, Clone)]
pub struct RuntimeScannerConfig {
    /// Whether any plugins require file checkout
    pub requires_checkout: bool,
    
    /// Whether any plugins require current file content
    pub requires_current_content: bool,
    
    /// Whether any plugins require historical file content
    pub requires_historical_content: bool,
    
    /// Base configuration
    pub base_config: ScannerConfig,
    
    /// Effective checkout directory
    pub effective_checkout_dir: Option<PathBuf>,
}

impl RuntimeScannerConfig {
    /// Check if a file should be checked out based on configuration and plugin requirements
    pub fn should_checkout_file(&self, file_path: &str, file_size: Option<u64>) -> bool {
        if !self.requires_checkout {
            return false;
        }
        
        // Check file size limits
        if let (Some(size), Some(max_size)) = (file_size, self.base_config.plugin_requirements.max_checkout_file_size) {
            if size > max_size {
                return false;
            }
        }
        
        // Check if extension is in forced checkout list
        if !self.base_config.plugin_requirements.force_checkout_extensions.is_empty() {
            if let Some(extension) = std::path::Path::new(file_path).extension() {
                let ext_str = extension.to_string_lossy().to_lowercase();
                return self.base_config.plugin_requirements.force_checkout_extensions.iter()
                    .any(|req_ext| req_ext.to_lowercase() == ext_str);
            }
            return false; // No extension and we have forced requirements
        }
        
        // Default: checkout if any plugin requires it
        self.requires_current_content || self.requires_historical_content
    }
    
    /// Get effective buffer size for reading files
    pub fn get_file_buffer_size(&self) -> usize {
        // Use a reasonable default buffer size
        64 * 1024 // 64KB
    }
    
    /// Check if checkout directories should be cached
    pub fn should_cache_checkout_dirs(&self) -> bool {
        self.base_config.plugin_requirements.cache_checkout_dirs
    }
    
    /// Get maximum number of concurrent checkouts
    pub fn get_max_concurrent_checkouts(&self) -> usize {
        self.base_config.plugin_requirements.max_concurrent_checkouts
    }
    
    /// Check if checkout directories should be cleaned up immediately
    pub fn should_cleanup_immediately(&self) -> bool {
        self.base_config.plugin_requirements.cleanup_immediately
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ScannerConfig::default();
        assert_eq!(config.max_memory_bytes, 64 * 1024 * 1024);
        assert_eq!(config.queue_size, 1000);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_builder() {
        let config = ScannerConfig::builder()
            .with_max_memory(128 * 1024 * 1024)
            .with_queue_size(2000)
            .build()
            .expect("Failed to build config");

        assert_eq!(config.max_memory_bytes, 128 * 1024 * 1024);
        assert_eq!(config.queue_size, 2000);
    }

    #[test]
    fn test_config_validation() {
        let mut config = ScannerConfig::default();
        assert!(config.validate().is_ok());

        // Test invalid memory
        config.max_memory_bytes = 0;
        assert!(matches!(config.validate(), Err(ConfigError::InvalidMaxMemory)));

        config.max_memory_bytes = 512 * 1024; // 512KB
        assert!(matches!(config.validate(), Err(ConfigError::InsufficientMemory)));

        // Reset memory, test invalid queue
        config.max_memory_bytes = 64 * 1024 * 1024;
        config.queue_size = 0;
        assert!(matches!(config.validate(), Err(ConfigError::InvalidQueueSize)));

        config.queue_size = 5;
        assert!(matches!(config.validate(), Err(ConfigError::InsufficientQueueSize)));
    }

    #[test]
    fn test_memory_display() {
        let config = ScannerConfig::builder()
            .with_max_memory(64 * 1024 * 1024)
            .build()
            .expect("Failed to build config");
        assert_eq!(config.memory_display(), "64 MB");

        let config = ScannerConfig::builder()
            .with_max_memory(2 * 1024 * 1024 * 1024) // 2GB
            .build()
            .expect("Failed to build config");
        assert_eq!(config.memory_display(), "2.0 GB");
    }

    #[test]
    fn test_branch_configuration_defaults() {
        let config = ScannerConfig::default();
        assert!(config.default_branch.is_none());
        assert_eq!(config.branch_fallbacks, vec!["main", "master", "develop", "trunk"]);
        assert!(config.default_remote.is_none());
    }

    #[test]
    fn test_branch_configuration_builder() {
        let config = ScannerConfig::builder()
            .with_default_branch("develop".to_string())
            .with_branch_fallbacks(vec!["develop".to_string(), "main".to_string()])
            .with_default_remote("upstream".to_string())
            .build()
            .expect("Failed to build config");
        
        // These should fail until branch fields are implemented
        assert_eq!(config.default_branch, Some("develop".to_string()));
        assert_eq!(config.branch_fallbacks, vec!["develop", "main"]);
        assert_eq!(config.default_remote, Some("upstream".to_string()));
    }
    
    // Mock plugin implementations for testing
    struct MockRequiringPlugin;
    impl PluginDataRequirements for MockRequiringPlugin {
        fn requires_current_file_content(&self) -> bool { true }
        fn requires_historical_file_content(&self) -> bool { false }
    }
    
    struct MockNonRequiringPlugin;
    impl PluginDataRequirements for MockNonRequiringPlugin {
        fn requires_current_file_content(&self) -> bool { false }
        fn requires_historical_file_content(&self) -> bool { false }
    }
    
    #[test]
    fn test_plugin_requirements_defaults() {
        let config = ScannerConfig::default();
        assert!(config.plugin_requirements.enable_conditional_checkout);
        assert!(config.plugin_requirements.checkout_base_dir.is_none());
        assert_eq!(config.plugin_requirements.max_concurrent_checkouts, 10);
        assert!(!config.plugin_requirements.cache_checkout_dirs);
        assert!(config.plugin_requirements.force_checkout_extensions.is_empty());
        assert_eq!(config.plugin_requirements.max_checkout_file_size, Some(10 * 1024 * 1024));
        assert!(config.plugin_requirements.cleanup_immediately);
    }
    
    #[test]
    fn test_plugin_analysis_requiring_checkout() {
        let config = ScannerConfig::default();
        
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockRequiringPlugin),
            Box::new(MockNonRequiringPlugin),
        ];
        
        let runtime_config = config.analyze_plugins(&plugins);
        
        assert!(runtime_config.requires_checkout);
        assert!(runtime_config.requires_current_content);
        assert!(!runtime_config.requires_historical_content);
        assert!(runtime_config.effective_checkout_dir.is_some());
    }
    
    #[test]
    fn test_plugin_analysis_no_requirements() {
        let config = ScannerConfig::default();
        
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockNonRequiringPlugin),
        ];
        
        let runtime_config = config.analyze_plugins(&plugins);
        
        assert!(!runtime_config.requires_checkout);
        assert!(!runtime_config.requires_current_content);
        assert!(!runtime_config.requires_historical_content);
        assert!(runtime_config.effective_checkout_dir.is_none());
    }
    
    #[test]
    fn test_should_checkout_file() {
        let config = ScannerConfig::default();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockRequiringPlugin),
        ];
        let runtime_config = config.analyze_plugins(&plugins);
        
        // Should checkout when requirements are met
        assert!(runtime_config.should_checkout_file("test.rs", Some(1024)));
        
        // Should not checkout large files if limit is set
        assert!(!runtime_config.should_checkout_file("large.rs", Some(20 * 1024 * 1024))); // 20MB > 10MB limit
        
        // Should checkout files without size info
        assert!(runtime_config.should_checkout_file("unknown.rs", None));
    }
    
    #[test]
    fn test_runtime_config_with_forced_extensions() {
        let mut config = ScannerConfig::default();
        config.plugin_requirements.force_checkout_extensions = vec!["rs".to_string(), "toml".to_string()];
        
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockRequiringPlugin),
        ];
        let runtime_config = config.analyze_plugins(&plugins);
        
        // Should checkout .rs files (in forced list)
        assert!(runtime_config.should_checkout_file("test.rs", Some(1024)));
        
        // Should checkout .toml files (in forced list)
        assert!(runtime_config.should_checkout_file("Cargo.toml", Some(1024)));
        
        // Should NOT checkout .py files (not in forced list)
        assert!(!runtime_config.should_checkout_file("test.py", Some(1024)));
        
        // Should NOT checkout files without extensions when forced list exists
        assert!(!runtime_config.should_checkout_file("Makefile", Some(1024)));
    }
    
    #[test]
    fn test_runtime_config_helpers() {
        let config = ScannerConfig::default();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockRequiringPlugin),
        ];
        let runtime_config = config.analyze_plugins(&plugins);
        
        assert_eq!(runtime_config.get_file_buffer_size(), 64 * 1024);
        assert!(!runtime_config.should_cache_checkout_dirs()); // Default is false
        assert_eq!(runtime_config.get_max_concurrent_checkouts(), 10);
        assert!(runtime_config.should_cleanup_immediately()); // Default is true
    }
}
