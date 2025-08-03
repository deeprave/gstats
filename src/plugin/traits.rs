//! Core Plugin Traits
//! 
//! Defines the fundamental trait hierarchy for the plugin system.

use std::collections::HashMap;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use crate::scanner::modes::ScanMode;
use crate::scanner::messages::ScanMessage;
use super::error::{PluginError, PluginResult};
use super::context::{PluginContext, PluginRequest, PluginResponse};

/// Core plugin interface that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Get plugin metadata information
    fn plugin_info(&self) -> &PluginInfo;
    
    /// Initialize the plugin with the given context
    async fn initialize(&mut self, context: &PluginContext) -> PluginResult<()>;
    
    /// Execute a plugin request
    async fn execute(&self, request: PluginRequest) -> PluginResult<PluginResponse>;
    
    /// Cleanup plugin resources
    async fn cleanup(&mut self) -> PluginResult<()>;
    
    /// Check if plugin supports a specific capability
    fn supports_capability(&self, capability: &str) -> bool {
        self.plugin_info().capabilities.iter()
            .any(|cap| cap.name == capability)
    }
    
    /// Get plugin state
    fn plugin_state(&self) -> PluginState {
        PluginState::Initialized // Default implementation
    }
}

/// Scanner-specific plugin capabilities extending the base Plugin trait
#[async_trait]
pub trait ScannerPlugin: Plugin {
    /// Get supported scan modes
    fn supported_modes(&self) -> ScanMode;
    
    /// Process scan data and return processed messages
    async fn process_scan_data(&self, data: &ScanMessage) -> PluginResult<Vec<ScanMessage>>;
    
    /// Aggregate multiple scan results into a summary
    async fn aggregate_results(&self, results: Vec<ScanMessage>) -> PluginResult<ScanMessage>;
    
    /// Estimate processing time for given scan modes
    fn estimate_processing_time(&self, modes: ScanMode, item_count: usize) -> Option<std::time::Duration> {
        // Default implementation returns None (unknown)
        let _ = (modes, item_count);
        None
    }
    
    /// Get scanner-specific configuration schema
    fn config_schema(&self) -> serde_json::Value {
        serde_json::json!({}) // Default empty schema
    }
}

/// Notification capabilities for plugins that respond to system events
#[async_trait]
pub trait NotificationPlugin: Plugin {
    /// Handle queue update notifications
    async fn on_queue_update(&self, update: QueueUpdate) -> PluginResult<()>;
    
    /// Handle scan progress notifications
    async fn on_scan_progress(&self, progress: ScanProgress) -> PluginResult<()>;
    
    /// Handle error notifications
    async fn on_error(&self, error: PluginError) -> PluginResult<()>;
    
    /// Handle system event notifications
    async fn on_system_event(&self, event: SystemEvent) -> PluginResult<()>;
    
    /// Get notification preferences (which events to receive)
    fn notification_preferences(&self) -> NotificationPreferences {
        NotificationPreferences::default()
    }
}

/// Plugin lifecycle management trait
#[async_trait]
pub trait PluginLifecycle: Send + Sync {
    /// Load plugin from descriptor
    async fn load(descriptor: &PluginDescriptor) -> PluginResult<Box<dyn Plugin>>;
    
    /// Unload plugin and free resources
    async fn unload(&mut self) -> PluginResult<()>;
    
    /// Reload plugin with new configuration
    async fn reload(&mut self, context: &PluginContext) -> PluginResult<()>;
    
    /// Validate plugin before loading
    fn validate(descriptor: &PluginDescriptor) -> PluginResult<()>;
}

/// Plugin metadata and information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Plugin name (unique identifier)
    pub name: String,
    
    /// Plugin version
    pub version: String,
    
    /// API version this plugin targets
    pub api_version: u32,
    
    /// Human-readable description
    pub description: String,
    
    /// Plugin author
    pub author: String,
    
    /// Plugin website or repository URL
    pub url: Option<String>,
    
    /// Plugin dependencies
    pub dependencies: Vec<PluginDependency>,
    
    /// Plugin capabilities
    pub capabilities: Vec<PluginCapability>,
    
    /// Plugin type
    pub plugin_type: PluginType,
    
    /// License information
    pub license: Option<String>,
}

/// Plugin dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    /// Dependency name
    pub name: String,
    
    /// Version requirement (semver)
    pub version_requirement: String,
    
    /// Whether dependency is optional
    pub optional: bool,
}

/// Plugin capability specification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginCapability {
    /// Capability name
    pub name: String,
    
    /// Capability description
    pub description: String,
    
    /// Capability version
    pub version: String,
}

/// Plugin type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginType {
    /// Scanner plugin for data collection
    Scanner,
    
    /// Notification plugin for event handling
    Notification,
    
    /// Processing plugin for data transformation
    Processing,
    
    /// Output plugin for data formatting
    Output,
    
    /// Composite plugin with multiple capabilities
    Composite,
}

/// Plugin state tracking
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    /// Plugin is not loaded
    Unloaded,
    
    /// Plugin is loaded but not initialized
    Loaded,
    
    /// Plugin is initialized and ready
    Initialized,
    
    /// Plugin is currently executing
    Running,
    
    /// Plugin is in error state
    Error(String),
    
    /// Plugin is being shut down
    ShuttingDown,
}

/// Queue update notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueUpdate {
    /// Queue identifier
    pub queue_id: String,
    
    /// Update type
    pub update_type: QueueUpdateType,
    
    /// Current queue size
    pub queue_size: usize,
    
    /// Memory usage
    pub memory_usage: u64,
    
    /// Timestamp of update
    pub timestamp: std::time::SystemTime,
}

/// Types of queue updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueueUpdateType {
    /// Message enqueued
    MessageEnqueued,
    
    /// Message dequeued
    MessageDequeued,
    
    /// Queue full
    QueueFull,
    
    /// Queue empty
    QueueEmpty,
    
    /// Memory pressure
    MemoryPressure,
}

/// Scan progress notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    /// Scan identifier
    pub scan_id: String,
    
    /// Scan mode
    pub scan_mode: ScanMode,
    
    /// Items processed
    pub items_processed: u64,
    
    /// Total items (if known)
    pub total_items: Option<u64>,
    
    /// Progress percentage (0.0 to 1.0)
    pub progress_percentage: f64,
    
    /// Estimated time remaining
    pub estimated_remaining: Option<std::time::Duration>,
    
    /// Current phase
    pub current_phase: String,
}

/// System event notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEvent {
    /// Event type
    pub event_type: SystemEventType,
    
    /// Event data
    pub event_data: serde_json::Value,
    
    /// Event timestamp
    pub timestamp: std::time::SystemTime,
}

/// Types of system events
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemEventType {
    /// System started
    SystemStartup,
    
    /// System shutting down
    SystemShutdown,
    
    /// Configuration changed
    ConfigurationChanged,
    
    /// Plugin registered
    PluginRegistered,
    
    /// Plugin unregistered
    PluginUnregistered,
    
    /// Memory warning
    MemoryWarning,
    
    /// Performance alert
    PerformanceAlert,
}

/// Notification preferences for plugins
#[derive(Debug, Clone)]
pub struct NotificationPreferences {
    /// Subscribe to queue updates
    pub queue_updates: bool,
    
    /// Subscribe to scan progress
    pub scan_progress: bool,
    
    /// Subscribe to error notifications
    pub error_notifications: bool,
    
    /// Subscribe to system events
    pub system_events: Vec<SystemEventType>,
    
    /// Maximum notification frequency (per second)
    pub max_frequency: Option<u32>,
}

/// Plugin descriptor for loading and discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDescriptor {
    /// Plugin information
    pub info: PluginInfo,
    
    /// Plugin file path (for dynamic loading)
    pub file_path: Option<std::path::PathBuf>,
    
    /// Plugin entry point
    pub entry_point: String,
    
    /// Plugin configuration
    pub config: HashMap<String, serde_json::Value>,
}

impl PluginInfo {
    /// Create a new PluginInfo
    pub fn new(
        name: String,
        version: String,
        api_version: u32,
        description: String,
        author: String,
        plugin_type: PluginType,
    ) -> Self {
        Self {
            name,
            version,
            api_version,
            description,
            author,
            url: None,
            dependencies: Vec::new(),
            capabilities: Vec::new(),
            plugin_type,
            license: None,
        }
    }
    
    /// Add a dependency
    pub fn with_dependency(mut self, name: String, version_requirement: String, optional: bool) -> Self {
        self.dependencies.push(PluginDependency {
            name,
            version_requirement,
            optional,
        });
        self
    }
    
    /// Add a capability
    pub fn with_capability(mut self, name: String, description: String, version: String) -> Self {
        self.capabilities.push(PluginCapability {
            name,
            description,
            version,
        });
        self
    }
    
    /// Set URL
    pub fn with_url(mut self, url: String) -> Self {
        self.url = Some(url);
        self
    }
    
    /// Set license
    pub fn with_license(mut self, license: String) -> Self {
        self.license = Some(license);
        self
    }
    
    /// Check if plugin is compatible with API version
    pub fn is_compatible_with_api(&self, api_version: u32) -> bool {
        // Simple compatibility check - same major version
        self.api_version / 10000 == api_version / 10000
    }
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            queue_updates: false,
            scan_progress: false,
            error_notifications: true, // Most plugins want error notifications
            system_events: vec![SystemEventType::SystemShutdown], // Most plugins want shutdown notifications
            max_frequency: Some(10), // 10 notifications per second max
        }
    }
}

impl QueueUpdate {
    /// Create a new queue update
    pub fn new(queue_id: String, update_type: QueueUpdateType, queue_size: usize, memory_usage: u64) -> Self {
        Self {
            queue_id,
            update_type,
            queue_size,
            memory_usage,
            timestamp: std::time::SystemTime::now(),
        }
    }
}

impl ScanProgress {
    /// Create a new scan progress notification
    pub fn new(scan_id: String, scan_mode: ScanMode, items_processed: u64, current_phase: String) -> Self {
        Self {
            scan_id,
            scan_mode,
            items_processed,
            total_items: None,
            progress_percentage: 0.0,
            estimated_remaining: None,
            current_phase,
        }
    }
    
    /// Update progress with total items
    pub fn with_total_items(mut self, total_items: u64) -> Self {
        self.total_items = Some(total_items);
        if total_items > 0 {
            self.progress_percentage = self.items_processed as f64 / total_items as f64;
        }
        self
    }
    
    /// Update with estimated remaining time
    pub fn with_estimated_remaining(mut self, remaining: std::time::Duration) -> Self {
        self.estimated_remaining = Some(remaining);
        self
    }
}

impl SystemEvent {
    /// Create a new system event
    pub fn new(event_type: SystemEventType, event_data: serde_json::Value) -> Self {
        Self {
            event_type,
            event_data,
            timestamp: std::time::SystemTime::now(),
        }
    }
}

impl PluginDescriptor {
    /// Create a new plugin descriptor
    pub fn new(info: PluginInfo, entry_point: String) -> Self {
        Self {
            info,
            file_path: None,
            entry_point,
            config: HashMap::new(),
        }
    }
    
    /// Set file path
    pub fn with_file_path(mut self, file_path: std::path::PathBuf) -> Self {
        self.file_path = Some(file_path);
        self
    }
    
    /// Add configuration
    pub fn with_config(mut self, config: HashMap<String, serde_json::Value>) -> Self {
        self.config = config;
        self
    }
}