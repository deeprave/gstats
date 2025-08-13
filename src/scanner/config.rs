//! Scanner Configuration
//! 
//! Configuration structures for scanner parameters.

/// Scanner configuration parameters
#[derive(Debug, Clone)]
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
}

/// Configuration builder for fluent API
#[derive(Debug)]
pub struct ScannerConfigBuilder {
    max_memory_bytes: usize,
    queue_size: usize,
    max_threads: Option<usize>,
    default_branch: Option<String>,
    branch_fallbacks: Vec<String>,
    default_remote: Option<String>,
}

/// Configuration validation error
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Maximum memory bytes must be greater than zero")]
    InvalidMaxMemory,
    #[error("Queue size must be greater than zero")]
    InvalidQueueSize,
    #[error("Maximum memory bytes must be at least 1MB")]
    InsufficientMemory,
    #[error("Queue size must be at least 10")]
    InsufficientQueueSize,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 64 * 1024 * 1024, // 64 MB default
            queue_size: 1000,
            max_threads: None, // Use system default (num_cpus)
            default_branch: None,
            branch_fallbacks: vec![
                "main".to_string(),
                "master".to_string(),
                "develop".to_string(),
                "trunk".to_string(),
            ],
            default_remote: None,
        }
    }
}

impl ScannerConfig {
    /// Create a new configuration builder
    pub fn builder() -> ScannerConfigBuilder {
        ScannerConfigBuilder {
            max_memory_bytes: 64 * 1024 * 1024, // 64 MB default
            queue_size: 1000,
            max_threads: None,
            default_branch: None,
            branch_fallbacks: vec![
                "main".to_string(),
                "master".to_string(),
                "develop".to_string(),
                "trunk".to_string(),
            ],
            default_remote: None,
        }
    }
    
    /// Create a new configuration builder (deprecated, use builder())
    pub fn new() -> ScannerConfigBuilder {
        Self::builder()
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.max_memory_bytes == 0 {
            return Err(ConfigError::InvalidMaxMemory);
        }

        if self.max_memory_bytes < 1024 * 1024 {
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

    /// Get memory usage as a human-readable string
    pub fn memory_display(&self) -> String {
        let mb = self.max_memory_bytes / (1024 * 1024);
        if mb >= 1024 {
            let gb = mb as f64 / 1024.0;
            format!("{:.1} GB", gb)
        } else {
            format!("{} MB", mb)
        }
    }
}

impl ScannerConfigBuilder {
    /// Set maximum memory usage in bytes
    pub fn with_max_memory(mut self, bytes: usize) -> Self {
        self.max_memory_bytes = bytes;
        self
    }

    /// Set queue size
    pub fn with_queue_size(mut self, size: usize) -> Self {
        self.queue_size = size;
        self
    }
    
    /// Set maximum threads
    pub fn max_threads(mut self, threads: usize) -> Self {
        self.max_threads = Some(threads);
        self
    }
    
    /// Set chunk size (stub for API compatibility)
    pub fn chunk_size(self, _size: usize) -> Self {
        // TODO: Implement when chunking is added
        self
    }
    
    /// Set buffer size (stub for API compatibility)
    pub fn buffer_size(self, _size: usize) -> Self {
        // TODO: Implement when buffering is added
        self
    }
    
    /// Enable/disable performance mode (stub for API compatibility)
    pub fn performance_mode(self, _enabled: bool) -> Self {
        // TODO: Implement when performance mode is added
        self
    }

    /// Set default branch
    pub fn with_default_branch(mut self, branch: String) -> Self {
        self.default_branch = Some(branch);
        self
    }

    /// Set branch fallbacks
    pub fn with_branch_fallbacks(mut self, fallbacks: Vec<String>) -> Self {
        self.branch_fallbacks = fallbacks;
        self
    }

    /// Set default remote
    pub fn with_default_remote(mut self, remote: String) -> Self {
        self.default_remote = Some(remote);
        self
    }

    /// Build the configuration
    pub fn build(self) -> Result<ScannerConfig, ConfigError> {
        let config = ScannerConfig {
            max_memory_bytes: self.max_memory_bytes,
            queue_size: self.queue_size,
            max_threads: self.max_threads,
            default_branch: self.default_branch,
            branch_fallbacks: self.branch_fallbacks,
            default_remote: self.default_remote,
        };
        config.validate()?;
        Ok(config)
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
        let config = ScannerConfig::new()
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
        let config = ScannerConfig::new()
            .with_max_memory(64 * 1024 * 1024)
            .build()
            .expect("Failed to build config");
        assert_eq!(config.memory_display(), "64 MB");

        let config = ScannerConfig::new()
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
        let config = ScannerConfig::new()
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
}
