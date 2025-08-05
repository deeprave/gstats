//! Plugin Error Types
//! 
//! Comprehensive error handling for plugin operations with context-aware error types.

use thiserror::Error;

/// Result type for plugin operations
pub type PluginResult<T> = Result<T, PluginError>;

/// Comprehensive error types for plugin operations
#[derive(Error, Debug, Clone)]
pub enum PluginError {
    /// Plugin initialization failed
    #[error("Plugin initialization failed: {message}")]
    InitializationFailed { message: String },
    
    /// Plugin execution error
    #[error("Plugin execution error: {message}")]
    ExecutionFailed { message: String },
    
    /// Plugin not found
    #[error("Plugin not found: {plugin_name}")]
    PluginNotFound { plugin_name: String },
    
    /// Plugin already registered
    #[error("Plugin already registered: {plugin_name}")]
    PluginAlreadyRegistered { plugin_name: String },
    
    /// Version compatibility error
    #[error("Version compatibility error: {message}")]
    VersionIncompatible { message: String },
    
    /// Plugin dependency error
    #[error("Plugin dependency error: {message}")]
    DependencyError { message: String },
    
    /// Configuration error
    #[error("Plugin configuration error: {message}")]
    ConfigurationError { message: String },
    
    /// Notification delivery failed
    #[error("Notification delivery failed: {message}")]
    NotificationFailed { message: String },
    
    /// Plugin discovery error
    #[error("Plugin discovery error: {message}")]
    DiscoveryFailed { message: String },
    
    /// Plugin discovery error (alias)
    #[error("Discovery error: {message}")]
    DiscoveryError { message: String },
    
    /// Plugin descriptor parsing error
    #[error("Descriptor parse error: {message}")]
    DescriptorParseError { message: String },
    
    /// Plugin loading error
    #[error("Plugin loading error: {message}")]
    LoadingFailed { message: String },
    
    /// Plugin registry error
    #[error("Plugin registry error: {message}")]
    RegistryError { message: String },
    
    /// Async operation error
    #[error("Async operation error: {message}")]
    AsyncError { message: String },
    
    /// Invalid plugin state
    #[error("Invalid plugin state: {message}")]
    InvalidState { message: String },
    
    /// Timeout error
    #[error("Plugin operation timed out: {message}")]
    Timeout { message: String },
    
    /// Generic plugin error
    #[error("Plugin error: {message}")]
    Generic { message: String },
}

impl PluginError {
    /// Create an initialization error
    pub fn initialization_failed<S: Into<String>>(message: S) -> Self {
        Self::InitializationFailed { message: message.into() }
    }
    
    /// Create an execution error
    pub fn execution_failed<S: Into<String>>(message: S) -> Self {
        Self::ExecutionFailed { message: message.into() }
    }
    
    /// Create a plugin not found error
    pub fn plugin_not_found<S: Into<String>>(plugin_name: S) -> Self {
        Self::PluginNotFound { plugin_name: plugin_name.into() }
    }
    
    /// Create a plugin already registered error
    pub fn plugin_already_registered<S: Into<String>>(plugin_name: S) -> Self {
        Self::PluginAlreadyRegistered { plugin_name: plugin_name.into() }
    }
    
    /// Create a version incompatible error
    pub fn version_incompatible<S: Into<String>>(message: S) -> Self {
        Self::VersionIncompatible { message: message.into() }
    }
    
    /// Create a dependency error
    pub fn dependency_error<S: Into<String>>(message: S) -> Self {
        Self::DependencyError { message: message.into() }
    }
    
    /// Create a configuration error
    pub fn configuration_error<S: Into<String>>(message: S) -> Self {
        Self::ConfigurationError { message: message.into() }
    }
    
    /// Create a notification failed error
    pub fn notification_failed<S: Into<String>>(message: S) -> Self {
        Self::NotificationFailed { message: message.into() }
    }
    
    /// Create a discovery failed error
    pub fn discovery_failed<S: Into<String>>(message: S) -> Self {
        Self::DiscoveryFailed { message: message.into() }
    }
    
    /// Create a discovery error
    pub fn discovery_error<S: Into<String>>(message: S) -> Self {
        Self::DiscoveryError { message: message.into() }
    }
    
    /// Create a descriptor parse error
    pub fn descriptor_parse_error<S: Into<String>>(message: S) -> Self {
        Self::DescriptorParseError { message: message.into() }
    }
    
    /// Create a loading failed error
    pub fn loading_failed<S: Into<String>>(message: S) -> Self {
        Self::LoadingFailed { message: message.into() }
    }
    
    /// Create a registry error
    pub fn registry_error<S: Into<String>>(message: S) -> Self {
        Self::RegistryError { message: message.into() }
    }
    
    /// Create an async error
    pub fn async_error<S: Into<String>>(message: S) -> Self {
        Self::AsyncError { message: message.into() }
    }
    
    /// Create an invalid state error
    pub fn invalid_state<S: Into<String>>(message: S) -> Self {
        Self::InvalidState { message: message.into() }
    }
    
    /// Create a timeout error
    pub fn timeout<S: Into<String>>(message: S) -> Self {
        Self::Timeout { message: message.into() }
    }
    
    /// Create a generic error
    pub fn generic<S: Into<String>>(message: S) -> Self {
        Self::Generic { message: message.into() }
    }
    
    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(self, 
            PluginError::ExecutionFailed { .. } |
            PluginError::NotificationFailed { .. } |
            PluginError::AsyncError { .. } |
            PluginError::Timeout { .. }
        )
    }
    
    /// Check if error is a configuration issue
    pub fn is_configuration_error(&self) -> bool {
        matches!(self,
            PluginError::ConfigurationError { .. } |
            PluginError::VersionIncompatible { .. } |
            PluginError::DependencyError { .. }
        )
    }
    
    /// Check if error is related to plugin lifecycle
    pub fn is_lifecycle_error(&self) -> bool {
        matches!(self,
            PluginError::InitializationFailed { .. } |
            PluginError::PluginNotFound { .. } |
            PluginError::PluginAlreadyRegistered { .. } |
            PluginError::LoadingFailed { .. } |
            PluginError::InvalidState { .. }
        )
    }
}

// Allow conversion from common error types
impl From<std::io::Error> for PluginError {
    fn from(err: std::io::Error) -> Self {
        PluginError::generic(format!("IO error: {}", err))
    }
}

impl From<serde_json::Error> for PluginError {
    fn from(err: serde_json::Error) -> Self {
        PluginError::configuration_error(format!("JSON error: {}", err))
    }
}

impl From<tokio::task::JoinError> for PluginError {
    fn from(err: tokio::task::JoinError) -> Self {
        PluginError::async_error(format!("Task join error: {}", err))
    }
}

impl From<serde_yaml::Error> for PluginError {
    fn from(err: serde_yaml::Error) -> Self {
        PluginError::descriptor_parse_error(format!("YAML error: {}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let error = PluginError::initialization_failed("Test initialization error");
        assert!(matches!(error, PluginError::InitializationFailed { .. }));
        assert!(error.to_string().contains("Test initialization error"));
    }
    
    #[test]
    fn test_error_classification() {
        let config_error = PluginError::configuration_error("Bad config");
        assert!(config_error.is_configuration_error());
        assert!(!config_error.is_recoverable());
        
        let exec_error = PluginError::execution_failed("Runtime error");
        assert!(exec_error.is_recoverable());
        assert!(!exec_error.is_configuration_error());
        
        let lifecycle_error = PluginError::initialization_failed("Init failed");
        assert!(lifecycle_error.is_lifecycle_error());
    }
    
    #[test]
    fn test_error_conversions() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let plugin_error: PluginError = io_error.into();
        assert!(matches!(plugin_error, PluginError::Generic { .. }));
        assert!(plugin_error.to_string().contains("IO error"));
    }
    
    #[test]
    fn test_error_display() {
        let error = PluginError::plugin_not_found("test-plugin");
        assert_eq!(error.to_string(), "Plugin not found: test-plugin");
    }
    
    #[test]
    fn test_all_error_variants() {
        // Test all error creation methods
        let errors = vec![
            PluginError::initialization_failed("init"),
            PluginError::execution_failed("exec"),
            PluginError::plugin_not_found("missing"),
            PluginError::plugin_already_registered("duplicate"),
            PluginError::version_incompatible("version"),
            PluginError::dependency_error("dep"),
            PluginError::configuration_error("config"),
            PluginError::notification_failed("notify"),
            PluginError::discovery_failed("discovery"),
            PluginError::loading_failed("load"),
            PluginError::registry_error("registry"),
            PluginError::async_error("async"),
            PluginError::invalid_state("state"),
            PluginError::timeout("timeout"),
            PluginError::generic("generic"),
        ];
        
        // All should be displayable
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}