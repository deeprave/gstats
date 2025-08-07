//! Plugin Context and Communication Types
//! 
//! Defines the context and communication structures for plugin execution.

use std::collections::HashMap;
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use crate::scanner::{ScannerConfig, QueryParams};
use crate::scanner::modes::ScanMode;
use crate::git::RepositoryHandle;
use crate::display::CompactFormat;

/// Context provided to plugins during initialization and execution
#[derive(Clone)]
pub struct PluginContext {
    /// Scanner configuration
    pub scanner_config: Arc<ScannerConfig>,
    
    /// Repository handle for git operations
    pub repository: Arc<RepositoryHandle>,
    
    /// Query parameters for filtering
    pub query_params: Arc<QueryParams>,
    
    /// Plugin-specific configuration data
    pub plugin_config: HashMap<String, serde_json::Value>,
    
    /// Runtime environment information
    pub runtime_info: RuntimeInfo,
    
    /// Available capabilities
    pub capabilities: Vec<String>,
}

/// Runtime environment information
#[derive(Debug, Clone)]
pub struct RuntimeInfo {
    /// API version
    pub api_version: u32,
    
    /// Runtime version (e.g., tokio version)
    pub runtime_version: String,
    
    /// Available CPU cores
    pub cpu_cores: usize,
    
    /// Available memory
    pub available_memory: u64,
    
    /// Working directory
    pub working_directory: String,
}

/// How the plugin was invoked
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationType {
    /// Called as a specific function
    Function(String),
    /// Called by plugin name directly
    Direct,
    /// Using plugin's default function
    Default,
}

/// Request types for plugin execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginRequest {
    /// Execute scan with specified modes
    Execute {
        /// Request identifier for tracking
        request_id: String,
        /// Requested scan modes
        scan_modes: ScanMode,
        /// Request-specific parameters
        parameters: HashMap<String, serde_json::Value>,
        /// Maximum execution time in milliseconds
        timeout_ms: Option<u64>,
        /// Priority level for execution
        priority: RequestPriority,
        /// How the plugin was invoked (function name or direct)
        invoked_as: String,
        /// Type of invocation
        invocation_type: InvocationType,
    },
    /// Get plugin statistics
    GetStatistics,
    /// Get plugin capabilities
    GetCapabilities,
    /// Export data
    Export,
    /// Process specific data
    ProcessData {
        /// Data to process
        data: serde_json::Value,
    },
}

/// Response types from plugin execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginResponse {
    /// Execution response with full metadata
    Execute {
        /// Request identifier this response corresponds to
        request_id: String,
        /// Execution status
        status: ExecutionStatus,
        /// Response data
        data: serde_json::Value,
        /// Execution metadata
        metadata: ExecutionMetadata,
        /// Any errors that occurred
        errors: Vec<String>,
    },
    /// Statistics response
    Statistics(crate::scanner::messages::ScanMessage),
    /// Capabilities response
    Capabilities(Vec<crate::plugin::traits::PluginCapability>),
    /// Data export response
    Data(String),
    /// Process data response
    ProcessedData(Vec<crate::scanner::messages::ScanMessage>),
}

/// Request priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RequestPriority {
    /// Low priority - background processing
    Low = 1,
    /// Normal priority - standard operations
    Normal = 2,
    /// High priority - user-requested operations
    High = 3,
    /// Critical priority - system operations
    Critical = 4,
}

/// Execution status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Execution completed successfully
    Success,
    /// Execution completed with warnings
    Warning,
    /// Execution failed
    Failed,
    /// Execution was cancelled
    Cancelled,
    /// Execution timed out
    Timeout,
}

/// Execution metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    
    /// Memory usage in bytes
    pub memory_used: u64,
    
    /// Number of items processed
    pub items_processed: u64,
    
    /// Plugin version that executed the request
    pub plugin_version: String,
    
    /// Additional metadata
    pub extra: HashMap<String, serde_json::Value>,
}

impl PluginContext {
    /// Create a new plugin context
    pub fn new(
        scanner_config: Arc<ScannerConfig>,
        repository: Arc<RepositoryHandle>,
        query_params: Arc<QueryParams>,
    ) -> Self {
        Self {
            scanner_config,
            repository,
            query_params,
            plugin_config: HashMap::new(),
            runtime_info: RuntimeInfo::current(),
            capabilities: Vec::new(),
        }
    }
    
    /// Add plugin-specific configuration
    pub fn with_plugin_config(mut self, config: HashMap<String, serde_json::Value>) -> Self {
        self.plugin_config = config;
        self
    }
    
    /// Add capabilities
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }
    
    /// Get plugin configuration value
    pub fn get_config_value(&self, key: &str) -> Option<&serde_json::Value> {
        self.plugin_config.get(key)
    }
    
    /// Check if capability is available
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(&capability.to_string())
    }
}

impl RuntimeInfo {
    /// Get current runtime information
    pub fn current() -> Self {
        Self {
            api_version: crate::scanner::get_api_version() as u32,
            runtime_version: tokio::runtime::Handle::current()
                .runtime_flavor()
                .to_string(),
            cpu_cores: num_cpus::get(),
            available_memory: Self::get_available_memory(),
            working_directory: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
        }
    }
    
    fn get_available_memory() -> u64 {
        // This is a simplified implementation
        // In production, you might want to use system-specific APIs
        8 * 1024 * 1024 * 1024 // 8GB default
    }
}

impl PluginRequest {
    /// Create a new execute plugin request
    pub fn new(scan_modes: ScanMode) -> Self {
        Self::Execute {
            request_id: uuid::Uuid::now_v7().to_string(),
            scan_modes,
            parameters: HashMap::new(),
            timeout_ms: None,
            priority: RequestPriority::Normal,
            invoked_as: "default".to_string(),
            invocation_type: InvocationType::Default,
        }
    }
    
    /// Create a new execute plugin request with invocation context
    pub fn new_with_invocation(scan_modes: ScanMode, invoked_as: String, invocation_type: InvocationType) -> Self {
        Self::Execute {
            request_id: uuid::Uuid::now_v7().to_string(),
            scan_modes,
            parameters: HashMap::new(),
            timeout_ms: None,
            priority: RequestPriority::Normal,
            invoked_as,
            invocation_type,
        }
    }
    
    /// Set request priority (only for Execute requests)
    pub fn with_priority(self, priority: RequestPriority) -> Self {
        match self {
            Self::Execute { request_id, scan_modes, parameters, timeout_ms, invoked_as, invocation_type, .. } => {
                Self::Execute { request_id, scan_modes, parameters, timeout_ms, priority, invoked_as, invocation_type }
            }
            _ => self,
        }
    }
    
    /// Set timeout (only for Execute requests)
    pub fn with_timeout(self, timeout_ms: u64) -> Self {
        match self {
            Self::Execute { request_id, scan_modes, parameters, priority, invoked_as, invocation_type, .. } => {
                Self::Execute { request_id, scan_modes, parameters, timeout_ms: Some(timeout_ms), priority, invoked_as, invocation_type }
            }
            _ => self,
        }
    }
    
    /// Add parameter (only for Execute requests)
    pub fn with_parameter<T: Serialize>(self, key: String, value: T) -> Self {
        match self {
            Self::Execute { request_id, scan_modes, mut parameters, timeout_ms, priority, invoked_as, invocation_type } => {
                if let Ok(json_value) = serde_json::to_value(value) {
                    parameters.insert(key, json_value);
                }
                Self::Execute { request_id, scan_modes, parameters, timeout_ms, priority, invoked_as, invocation_type }
            }
            _ => self,
        }
    }
    
    /// Get parameter value (only for Execute requests)
    pub fn get_parameter(&self, key: &str) -> Option<&serde_json::Value> {
        match self {
            Self::Execute { parameters, .. } => parameters.get(key),
            _ => None,
        }
    }
    
    /// Get request ID
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Execute { request_id, .. } => Some(request_id),
            _ => None,
        }
    }
}

impl PluginResponse {
    /// Create a successful execute response
    pub fn success(request_id: String, data: serde_json::Value, metadata: ExecutionMetadata) -> Self {
        Self::Execute {
            request_id,
            status: ExecutionStatus::Success,
            data,
            metadata,
            errors: Vec::new(),
        }
    }
    
    /// Create a failed execute response
    pub fn failed(request_id: String, error: String, metadata: ExecutionMetadata) -> Self {
        Self::Execute {
            request_id,
            status: ExecutionStatus::Failed,
            data: serde_json::Value::Null,
            metadata,
            errors: vec![error],
        }
    }
    
    /// Create a warning execute response
    pub fn warning(request_id: String, data: serde_json::Value, warnings: Vec<String>, metadata: ExecutionMetadata) -> Self {
        Self::Execute {
            request_id,
            status: ExecutionStatus::Warning,
            data,
            metadata,
            errors: warnings,
        }
    }
    
    /// Check if response indicates success
    pub fn is_success(&self) -> bool {
        match self {
            Self::Execute { status, .. } => matches!(status, ExecutionStatus::Success | ExecutionStatus::Warning),
            Self::Statistics(_) | Self::Capabilities(_) | Self::Data(_) | Self::ProcessedData(_) => true,
        }
    }
    
    /// Check if response indicates failure
    pub fn is_failure(&self) -> bool {
        match self {
            Self::Execute { status, .. } => matches!(status, ExecutionStatus::Failed | ExecutionStatus::Cancelled | ExecutionStatus::Timeout),
            _ => false,
        }
    }
    
    /// Get errors if any
    pub fn get_errors(&self) -> &[String] {
        match self {
            Self::Execute { errors, .. } => errors,
            _ => &[],
        }
    }
}

impl Default for RequestPriority {
    fn default() -> Self {
        RequestPriority::Normal
    }
}

impl ExecutionMetadata {
    /// Create new execution metadata
    pub fn new(duration_ms: u64, memory_used: u64, items_processed: u64, plugin_version: String) -> Self {
        Self {
            duration_ms,
            memory_used,
            items_processed,
            plugin_version,
            extra: HashMap::new(),
        }
    }
    
    /// Add extra metadata
    pub fn with_extra(mut self, key: String, value: serde_json::Value) -> Self {
        self.extra.insert(key, value);
        self
    }
}

// Helper for tokio runtime flavor string conversion
trait RuntimeFlavorExt {
    fn to_string(&self) -> String;
}

impl RuntimeFlavorExt for tokio::runtime::RuntimeFlavor {
    fn to_string(&self) -> String {
        match self {
            tokio::runtime::RuntimeFlavor::CurrentThread => "current-thread".to_string(),
            tokio::runtime::RuntimeFlavor::MultiThread => "multi-thread".to_string(),
            #[allow(unreachable_patterns)]
            _ => "unknown".to_string(),
        }
    }
}

// CompactFormat implementations for plugin types
impl CompactFormat for PluginResponse {
    fn to_compact_format(&self) -> String {
        match self {
            PluginResponse::Execute { status, data, metadata, .. } => {
                let status_str = match status {
                    ExecutionStatus::Success => "✓",
                    ExecutionStatus::Warning => "⚠",
                    ExecutionStatus::Failed => "✗",
                    ExecutionStatus::Cancelled => "⊗",
                    ExecutionStatus::Timeout => "⧖",
                };
                
                // Extract key metrics from the data
                let summary = if let Some(function) = data.get("function").and_then(|f| f.as_str()) {
                    let function_summary = match function {
                        "commits" => {
                            let total = data.get("total_commits").and_then(|c| c.as_u64()).unwrap_or(0);
                            let authors = data.get("unique_authors").and_then(|a| a.as_u64()).unwrap_or(0);
                            format!("{} commits, {} authors", total, authors)
                        }
                        "authors" => {
                            let total = data.get("total_authors").and_then(|a| a.as_u64()).unwrap_or(0);
                            format!("{} authors", total)
                        }
                        "metrics" => {
                            let files = data.get("total_files").and_then(|f| f.as_u64()).unwrap_or(0);
                            let complexity = data.get("total_complexity").and_then(|c| c.as_f64()).unwrap_or(0.0);
                            format!("{} files, complexity: {:.1}", files, complexity)
                        }
                        _ => format!("{} function", function),
                    };
                    format!("{}: {}", function, function_summary)
                } else {
                    "execution complete".to_string()
                };
                
                format!("{} {} | {}", status_str, summary, metadata.to_compact_format())
            }
            PluginResponse::Statistics(msg) => {
                format!("📊 Statistics: {:?}", msg)
                    .replace('\n', " ")
                    .chars()
                    .take(80)
                    .collect::<String>()
            }
            PluginResponse::Capabilities(caps) => {
                format!("🔧 {} capabilities", caps.len())
            }
            PluginResponse::Data(data) => {
                let preview = data.replace('\n', " ").chars().take(50).collect::<String>();
                format!("📄 Data: {}...", preview)
            }
            PluginResponse::ProcessedData(messages) => {
                format!("🔄 {} processed messages", messages.len())
            }
        }
    }
}

impl CompactFormat for ExecutionMetadata {
    fn to_compact_format(&self) -> String {
        let duration = if self.duration_ms < 1000 {
            format!("{}ms", self.duration_ms)
        } else {
            format!("{:.1}s", self.duration_ms as f64 / 1000.0)
        };
        
        let memory = if self.memory_used < 1024 {
            format!("{}B", self.memory_used)
        } else if self.memory_used < 1024 * 1024 {
            format!("{:.1}KB", self.memory_used as f64 / 1024.0)
        } else {
            format!("{:.1}MB", self.memory_used as f64 / (1024.0 * 1024.0))
        };
        
        format!("{} items in {} ({})", self.items_processed, duration, memory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git;
    
    #[tokio::test]
    async fn test_plugin_context_creation() {
        let repo = git::resolve_repository_handle(None).unwrap();
        let scanner_config = Arc::new(ScannerConfig::default());
        let query_params = Arc::new(QueryParams::default());
        
        let context = PluginContext::new(
            scanner_config,
            Arc::new(repo),
            query_params,
        );
        
        assert!(context.plugin_config.is_empty());
        assert!(context.capabilities.is_empty());
        assert!(context.runtime_info.api_version > 0);
    }
    
    #[tokio::test]
    async fn test_plugin_context_with_config() {
        let repo = git::resolve_repository_handle(None).unwrap();
        let scanner_config = Arc::new(ScannerConfig::default());
        let query_params = Arc::new(QueryParams::default());
        
        let mut config = HashMap::new();
        config.insert("test_key".to_string(), serde_json::Value::String("test_value".to_string()));
        
        let context = PluginContext::new(
            scanner_config,
            Arc::new(repo),
            query_params,
        ).with_plugin_config(config);
        
        assert_eq!(context.get_config_value("test_key"), Some(&serde_json::Value::String("test_value".to_string())));
        assert_eq!(context.get_config_value("missing_key"), None);
    }
    
    #[test]
    fn test_plugin_request_creation() {
        let request = PluginRequest::new(ScanMode::FILES)
            .with_priority(RequestPriority::High)
            .with_timeout(5000)
            .with_parameter("limit".to_string(), 100);
        
        match request {
            PluginRequest::Execute { scan_modes, priority, timeout_ms, .. } => {
                assert_eq!(scan_modes, ScanMode::FILES);
                assert_eq!(priority, RequestPriority::High);
                assert_eq!(timeout_ms, Some(5000));
            }
            _ => panic!("Expected Execute request"),
        }
        assert!(request.get_parameter("limit").is_some());
    }
    
    #[test]
    fn test_plugin_response_creation() {
        let metadata = ExecutionMetadata::new(100, 1024, 5, "1.0.0".to_string());
        
        let success_response = PluginResponse::success(
            "test-id".to_string(),
            serde_json::Value::String("test data".to_string()),
            metadata.clone(),
        );
        
        assert!(success_response.is_success());
        assert!(!success_response.is_failure());
        assert_eq!(success_response.get_errors().len(), 0);
        
        let failed_response = PluginResponse::failed(
            "test-id".to_string(),
            "Test error".to_string(),
            metadata,
        );
        
        assert!(!failed_response.is_success());
        assert!(failed_response.is_failure());
        assert_eq!(failed_response.get_errors().len(), 1);
    }
    
    #[test]
    fn test_request_priority_ordering() {
        assert!(RequestPriority::Critical > RequestPriority::High);
        assert!(RequestPriority::High > RequestPriority::Normal);
        assert!(RequestPriority::Normal > RequestPriority::Low);
    }
    
    #[test]
    fn test_execution_metadata() {
        let mut metadata = ExecutionMetadata::new(150, 2048, 10, "1.1.0".to_string());
        metadata = metadata.with_extra("custom_field".to_string(), serde_json::Value::Bool(true));
        
        assert_eq!(metadata.duration_ms, 150);
        assert_eq!(metadata.memory_used, 2048);
        assert_eq!(metadata.items_processed, 10);
        assert_eq!(metadata.plugin_version, "1.1.0");
        assert_eq!(metadata.extra.get("custom_field"), Some(&serde_json::Value::Bool(true)));
    }
    
    #[tokio::test]
    async fn test_runtime_info() {
        let runtime_info = RuntimeInfo::current();
        
        assert!(runtime_info.api_version > 0);
        assert!(!runtime_info.runtime_version.is_empty());
        assert!(runtime_info.cpu_cores > 0);
        assert!(runtime_info.available_memory > 0);
        assert!(!runtime_info.working_directory.is_empty());
    }
    
    #[tokio::test]
    async fn test_context_capabilities() {
        let repo = git::resolve_repository_handle(None).unwrap();
        let scanner_config = Arc::new(ScannerConfig::default());
        let query_params = Arc::new(QueryParams::default());
        
        let context = PluginContext::new(
            scanner_config,
            Arc::new(repo),
            query_params,
        ).with_capabilities(vec!["async".to_string(), "streaming".to_string()]);
        
        assert!(context.has_capability("async"));
        assert!(context.has_capability("streaming"));
        assert!(!context.has_capability("missing"));
    }
}