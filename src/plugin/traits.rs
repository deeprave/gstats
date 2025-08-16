//! Core Plugin Traits
//! 
//! Defines the fundamental trait hierarchy for the plugin system.

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use super::error::{PluginError, PluginResult};
use super::context::{PluginContext, PluginRequest, PluginResponse};
use crate::queue::{QueueEvent, QueueConsumer};
use crate::scanner::messages::ScanMessage;

/// Function that a plugin can provide
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginFunction {
    /// Primary function name
    pub name: String,
    
    /// Alternative names/aliases for this function
    pub aliases: Vec<String>,
    
    /// Human-readable description
    pub description: String,
    
    /// Whether this is the default function when plugin is invoked directly
    pub is_default: bool,
}

/// Core plugin interface that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Get plugin metadata information
    fn plugin_info(&self) -> &PluginInfo;
    
    /// Initialize the plugin with the given context
    async fn initialize(&mut self, context: &PluginContext) -> PluginResult<()>;
    
    /// Get argument schema for this plugin (if it supports argument parsing)
    /// 
    /// This provides a unified way to access plugin argument schemas without
    /// requiring separate trait casting. Plugins that don't support arguments
    /// should return an empty vector.
    fn get_arg_schema(&self) -> Vec<PluginArgDefinition> {
        vec![]
    }
    
    /// Generate help text for this plugin's arguments
    /// 
    /// This method attempts to use clap-based help generation if the plugin
    /// implements PluginClapParser, otherwise falls back to legacy help.
    /// Plugins should override this to provide custom help formatting.
    fn get_plugin_help(&self) -> Option<String> {
        None // Override in implementations that support argument parsing
    }
    
    /// Build a clap Command for this plugin if it supports clap-based parsing
    /// 
    /// This provides a unified way to access plugin clap commands without
    /// requiring separate trait casting. Plugins that don't use clap should
    /// return None.
    fn build_clap_command(&self) -> Option<clap::Command> {
        None // Override in implementations that use PluginClapParser
    }
    
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
    
    /// Get all functions this plugin can handle
    fn advertised_functions(&self) -> Vec<PluginFunction> {
        // Default implementation returns empty vec
        Vec::new()
    }
    
    /// Get the default function name if any
    fn default_function(&self) -> Option<&str> {
        // Default implementation returns None
        None
    }
    
    /// Cast to ConsumerPlugin if this plugin implements that trait
    fn as_consumer_plugin(&self) -> Option<&dyn ConsumerPlugin> {
        None
    }
    
    /// Cast to mutable ConsumerPlugin if this plugin implements that trait
    fn as_consumer_plugin_mut(&mut self) -> Option<&mut dyn ConsumerPlugin> {
        None
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
    
    /// Plugin execution priority (higher values = higher priority, default = 0)
    pub priority: i32,
    
    /// Whether this plugin should be activated by default (default = false)
    /// Export plugins typically set this to true
    #[serde(default)]
    pub load_by_default: bool,
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
    
    /// Function name this capability represents (if applicable)
    pub function_name: Option<String>,
    
    /// Aliases for the function
    pub aliases: Vec<String>,
    
    /// Whether this is a default function
    pub is_default: bool,
}

/// Plugin type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginType {
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
    
    /// Plugin is initialized and ready (idle)
    Initialized,
    
    /// Plugin is currently executing
    Running,
    
    /// Plugin is actively processing work (GS-65 coordination state)
    Processing,
    
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
    
    
    /// Entries processed
    pub entries_processed: u64,
    
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
            priority: 5, // Default priority
            load_by_default: false, // Default to manual activation
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
            function_name: None,
            aliases: Vec::new(),
            is_default: false,
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
    
    /// Set plugin execution priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
    
    /// Set whether plugin should be activated by default
    pub fn with_load_by_default(mut self, load_by_default: bool) -> Self {
        self.load_by_default = load_by_default;
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
    pub fn new(scan_id: String, entries_processed: u64, current_phase: String) -> Self {
        Self {
            scan_id,
            entries_processed,
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
            self.progress_percentage = self.entries_processed as f64 / total_items as f64;
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

/// Plugin argument parsing trait for handling plugin-specific command line arguments
#[async_trait]
/// Modern clap-based plugin argument parsing trait
/// 
/// This trait provides a clap-based approach to plugin argument parsing,
/// offering consistency with the main CLI and automatic help generation.
#[async_trait]
pub trait PluginClapParser {
    /// Build a clap Command for this plugin
    /// 
    /// This method should return a clap Command that defines all the arguments
    /// this plugin supports. The Command will be used for parsing and help generation.
    fn build_clap_command(&self) -> clap::Command;
    
    /// Parse arguments using clap and configure the plugin
    /// 
    /// This method receives the parsed clap ArgMatches and should configure
    /// the plugin based on the provided arguments.
    async fn configure_from_matches(&mut self, matches: &clap::ArgMatches) -> PluginResult<()>;
    
    /// Generate help text for this plugin using clap
    /// 
    /// This provides automatic help generation using clap's built-in help system.
    /// Plugins can override this for custom help formatting.
    fn generate_help(&self) -> String {
        let mut command = self.build_clap_command();
        let mut help_output = Vec::new();
        let _ = command.write_help(&mut help_output);
        String::from_utf8_lossy(&help_output).to_string()
    }
}

/// Legacy plugin argument parsing trait (deprecated)
/// 
/// This trait is maintained for backward compatibility but should be migrated
/// to PluginClapParser for better consistency and functionality.
#[deprecated(note = "Use PluginClapParser for better consistency and automatic help generation")]
#[async_trait]
pub trait PluginArgumentParser {
    /// Parse plugin-specific arguments
    /// 
    /// This method is called with the raw arguments that were captured after the plugin command.
    /// The plugin should parse these arguments and store configuration appropriately.
    /// 
    /// # Arguments
    /// * `args` - Raw command line arguments captured for this plugin
    /// 
    /// # Returns
    /// * `Ok(())` if parsing was successful
    /// * `Err(PluginError)` if parsing failed or arguments were invalid
    async fn parse_plugin_args(&mut self, args: &[String]) -> PluginResult<()>;
    
    /// Get argument schema for help generation and validation
    /// 
    /// This method allows plugins to describe their available arguments for help display
    /// and argument validation. The schema includes argument names, descriptions, types,
    /// and whether they are required.
    /// 
    /// # Returns
    /// Vector of argument definitions that this plugin supports
    fn get_arg_schema(&self) -> Vec<PluginArgDefinition>;
    
    /// Generate help text for this plugin's arguments
    /// 
    /// This method should return formatted help text that explains the plugin's
    /// command line arguments. This will be displayed when users request plugin-specific help.
    /// 
    /// # Returns
    /// Formatted help text as a String
    fn get_args_help(&self) -> String {
        let schema = self.get_arg_schema();
        if schema.is_empty() {
            return "No plugin-specific arguments available.".to_string();
        }
        
        let mut help = String::new();
        help.push_str("Plugin-specific arguments:\n");
        
        for arg in schema {
            let required_marker = if arg.required { " (required)" } else { "" };
            let default_text = if let Some(ref default) = arg.default_value {
                format!(" [default: {}]", default)
            } else {
                String::new()
            };
            
            help.push_str(&format!(
                "  {:<20} {}{}{}\n",
                arg.name, arg.description, required_marker, default_text
            ));
        }
        
        help
    }
}

/// Plugin argument definition for schema and help generation
#[derive(Debug, Clone)]
pub struct PluginArgDefinition {
    /// Argument name (e.g., "--output", "--csv-delimiter")
    pub name: String,
    
    /// Human-readable description of the argument
    pub description: String,
    
    /// Whether this argument is required
    pub required: bool,
    
    /// Default value if not provided
    pub default_value: Option<String>,
    
    /// Argument type for validation (e.g., "string", "number", "boolean")
    pub arg_type: String,
    
    /// Example values to show in help
    pub examples: Vec<String>,
}

/// Plugin data requirements trait for scanner optimization
/// 
/// This trait allows plugins to specify what data they need from the scanner,
/// enabling the scanner to conditionally provide file content only when needed.
/// Most plugins only need metadata, avoiding expensive file checkout operations.
pub trait PluginDataRequirements {
    /// Whether this plugin needs current (HEAD) file content for analysis
    /// 
    /// Examples of plugins that need current content:
    /// - Complexity analysis (needs to parse current code)
    /// - Tech debt analysis (needs current code structure)
    /// - Security scanning (needs current code patterns)
    /// 
    /// # Returns
    /// `true` if plugin requires current file content checkout, `false` for metadata only
    fn requires_current_file_content(&self) -> bool {
        false // Default: metadata only
    }
    
    /// Whether this plugin needs historical file content from past commits
    /// 
    /// Examples of plugins that need historical content:
    /// - Change analysis comparing versions
    /// - Code evolution tracking
    /// - Regression analysis
    /// 
    /// # Returns
    /// `true` if plugin requires historical file content checkout, `false` for metadata only
    fn requires_historical_file_content(&self) -> bool {
        false // Default: metadata only
    }
    
    /// Preferred buffer size for file reading operations
    /// 
    /// This allows plugins to optimize for their specific use cases:
    /// - Small buffers (4KB) for line-by-line analysis
    /// - Large buffers (64KB+) for bulk processing
    /// - Filesystem-aligned buffers (32KB) for optimal I/O
    /// 
    /// # Returns
    /// Preferred buffer size in bytes, default is 32KB (filesystem block aligned)
    fn preferred_buffer_size(&self) -> usize {
        32 * 1024 // 32KB default - filesystem block aligned
    }
    
    /// Maximum file size this plugin will process
    /// 
    /// Plugins can set limits to avoid processing extremely large files:
    /// - Memory-intensive analysis might limit to 1MB
    /// - Line-counting plugins might handle larger files
    /// - Binary analysis might have different limits
    /// 
    /// # Returns
    /// `Some(size)` to limit file size, `None` for no limit
    fn max_file_size(&self) -> Option<usize> {
        None // Default: no limit
    }
    
    /// Whether this plugin can handle binary files
    /// 
    /// # Returns
    /// `true` if plugin can process binary files, `false` if text only
    fn handles_binary_files(&self) -> bool {
        false // Default: text files only
    }
}

/// Consumer Plugin trait for plugins that consume messages from the queue
/// 
/// This trait extends the base Plugin trait to provide message consumption
/// capabilities for plugins that need to process the message stream from
/// the scanner. Consumer plugins receive a QueueConsumer handle and can
/// process messages independently with acknowledgment support.
#[async_trait]
pub trait ConsumerPlugin: Plugin {
    /// Start consuming messages with the provided queue consumer
    /// 
    /// This method is called when the plugin should begin consuming messages
    /// using the provided QueueConsumer handle. The plugin should use the
    /// consumer to read messages and acknowledge them after processing.
    /// 
    /// # Arguments
    /// * `consumer` - The queue consumer handle for reading messages
    /// 
    /// # Returns
    /// Result indicating success or failure to start consuming
    async fn start_consuming(&mut self, consumer: QueueConsumer) -> PluginResult<()>;
    
    /// Process a single message from the queue with acknowledgment
    /// 
    /// This method is called for each message that the plugin should process.
    /// The plugin should handle the message according to its functionality,
    /// then acknowledge it using the consumer handle when processing is complete.
    /// 
    /// # Arguments
    /// * `consumer` - The queue consumer handle for acknowledgment
    /// * `message` - The scan message to process (Arc-wrapped for efficiency)
    /// 
    /// # Returns
    /// Result indicating success or failure of message processing
    async fn process_message(&self, consumer: &QueueConsumer, message: Arc<ScanMessage>) -> PluginResult<()>;
    
    /// Handle queue events (scan start/complete/error)
    /// 
    /// This method is called when queue events occur, such as scan start,
    /// completion, or errors. Plugins can use this to perform setup,
    /// cleanup, or other lifecycle operations.
    /// 
    /// # Arguments
    /// * `event` - The queue event to handle
    /// 
    /// # Returns
    /// Result indicating success or failure of event handling
    async fn handle_queue_event(&self, event: &QueueEvent) -> PluginResult<()>;
    
    /// Stop consuming messages and cleanup
    /// 
    /// This method is called when the plugin should stop consuming messages
    /// and perform any necessary cleanup. The consumer handle will be
    /// deregistered after this method completes.
    /// 
    /// # Returns
    /// Result indicating success or failure of cleanup
    async fn stop_consuming(&mut self) -> PluginResult<()>;
    
    /// Get consumer configuration
    /// 
    /// This method returns the consumer configuration for this plugin,
    /// which includes preferences for message consumption behavior.
    /// 
    /// # Returns
    /// Consumer configuration preferences
    fn consumer_preferences(&self) -> ConsumerPreferences {
        ConsumerPreferences::default()
    }
}

/// Consumer preferences for message consumption behavior
#[derive(Debug, Clone, PartialEq)]
pub struct ConsumerPreferences {
    /// Whether this consumer wants to receive all message types
    pub consume_all_messages: bool,
    
    /// Specific message types this consumer is interested in
    pub interested_message_types: Vec<String>,
    
    /// Whether this consumer can handle high-frequency message streams
    pub high_frequency_capable: bool,
    
    /// Preferred batch size for message processing (0 = no batching)
    pub preferred_batch_size: usize,
    
    /// Whether this consumer requires ordered message delivery
    pub requires_ordered_delivery: bool,
}

impl Default for ConsumerPreferences {
    fn default() -> Self {
        Self {
            consume_all_messages: true,
            interested_message_types: vec![],
            high_frequency_capable: true,
            preferred_batch_size: 0, // No batching by default
            requires_ordered_delivery: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::ScanMessage;
    use crate::queue::QueueEvent;

    struct TestPlugin;
    impl PluginDataRequirements for TestPlugin {}

    #[test]
    fn test_plugin_data_requirements_defaults() {
        let plugin = TestPlugin;
        
        // Test default values
        assert!(!plugin.requires_current_file_content());
        assert!(!plugin.requires_historical_file_content());
        assert_eq!(plugin.preferred_buffer_size(), 32 * 1024);
        assert_eq!(plugin.max_file_size(), None);
        assert!(!plugin.handles_binary_files());
    }

    struct CustomPlugin;
    impl PluginDataRequirements for CustomPlugin {
        fn requires_current_file_content(&self) -> bool {
            true
        }
        
        fn preferred_buffer_size(&self) -> usize {
            64 * 1024
        }
        
        fn max_file_size(&self) -> Option<usize> {
            Some(1024 * 1024) // 1MB limit
        }
        
        fn handles_binary_files(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_plugin_data_requirements_custom() {
        let plugin = CustomPlugin;
        
        // Test custom values
        assert!(plugin.requires_current_file_content());
        assert!(!plugin.requires_historical_file_content()); // Still default
        assert_eq!(plugin.preferred_buffer_size(), 64 * 1024);
        assert_eq!(plugin.max_file_size(), Some(1024 * 1024));
        assert!(plugin.handles_binary_files());
    }

    #[test]
    fn test_consumer_preferences_defaults() {
        let prefs = ConsumerPreferences::default();
        
        assert!(prefs.consume_all_messages);
        assert!(prefs.interested_message_types.is_empty());
        assert!(prefs.high_frequency_capable);
        assert_eq!(prefs.preferred_batch_size, 0);
        assert!(prefs.requires_ordered_delivery);
    }

    #[test]
    fn test_consumer_preferences_custom() {
        let prefs = ConsumerPreferences {
            consume_all_messages: false,
            interested_message_types: vec!["FileChange".to_string(), "CommitInfo".to_string()],
            high_frequency_capable: false,
            preferred_batch_size: 50,
            requires_ordered_delivery: false,
        };
        
        assert!(!prefs.consume_all_messages);
        assert_eq!(prefs.interested_message_types.len(), 2);
        assert!(!prefs.high_frequency_capable);
        assert_eq!(prefs.preferred_batch_size, 50);
        assert!(!prefs.requires_ordered_delivery);
    }

    // Mock consumer plugin for testing trait methods
    struct MockConsumerPlugin {
        started: bool,
        prefs: ConsumerPreferences,
        info: PluginInfo,
        consumer: Option<QueueConsumer>,
    }

    #[async_trait]
    impl Plugin for MockConsumerPlugin {
        fn plugin_info(&self) -> &PluginInfo {
            &self.info
        }
        
        async fn initialize(&mut self, _context: &PluginContext) -> PluginResult<()> {
            Ok(())
        }
        
        async fn execute(&self, _request: PluginRequest) -> PluginResult<PluginResponse> {
            use crate::plugin::context::ExecutionMetadata;
            use std::collections::HashMap;
            let metadata = ExecutionMetadata {
                duration_us: 1000,
                memory_used: 1024,
                entries_processed: 0,
                plugin_version: "1.0.0".to_string(),
                extra: HashMap::new(),
            };
            Ok(PluginResponse::success(
                "test-request".to_string(),
                serde_json::Value::Null,
                metadata
            ))
        }
        
        async fn cleanup(&mut self) -> PluginResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl ConsumerPlugin for MockConsumerPlugin {
        async fn start_consuming(&mut self, consumer: QueueConsumer) -> PluginResult<()> {
            self.started = true;
            self.consumer = Some(consumer);
            Ok(())
        }
        
        async fn process_message(&self, consumer: &QueueConsumer, message: Arc<ScanMessage>) -> PluginResult<()> {
            // Acknowledge the message after processing
            consumer.acknowledge(message.header().sequence()).await.map_err(|e| {
                PluginError::execution_failed(format!("Failed to acknowledge message: {}", e))
            })?;
            Ok(())
        }
        
        async fn handle_queue_event(&self, _event: &QueueEvent) -> PluginResult<()> {
            Ok(())
        }
        
        async fn stop_consuming(&mut self) -> PluginResult<()> {
            self.started = false;
            self.consumer = None;
            Ok(())
        }
        
        fn consumer_preferences(&self) -> ConsumerPreferences {
            self.prefs.clone()
        }
    }

    impl MockConsumerPlugin {
        fn new() -> Self {
            Self {
                started: false,
                prefs: ConsumerPreferences::default(),
                info: PluginInfo {
                    name: "mock-consumer".to_string(),
                    version: "1.0.0".to_string(),
                    api_version: 1,
                    description: "Mock consumer plugin for testing".to_string(),
                    author: "Test".to_string(),
                    url: None,
                    dependencies: Vec::new(),
                    capabilities: Vec::new(),
                    plugin_type: PluginType::Processing,
                    license: None,
                    priority: 0,
                    load_by_default: false,
                },
                consumer: None,
            }
        }
        
        fn with_preferences(mut self, prefs: ConsumerPreferences) -> Self {
            self.prefs = prefs;
            self
        }
    }

    // Note: These tests are simplified since we can't easily create a QueueConsumer
    // in unit tests without a full MultiConsumerQueue setup. Integration tests
    // will test the actual functionality with real queue consumers.
    
    #[test]
    fn test_consumer_plugin_basic_lifecycle() {
        let plugin = MockConsumerPlugin::new();
        
        // Initial state
        assert!(!plugin.started);
        assert!(plugin.consumer.is_none());
        
        // Test that we can construct and check preferences
        let prefs = plugin.consumer_preferences();
        assert_eq!(prefs, ConsumerPreferences::default());
    }

    #[tokio::test]
    async fn test_consumer_plugin_queue_event_handling() {
        let plugin = MockConsumerPlugin::new();
        
        // Create test queue event
        let event = QueueEvent::scan_started("test-scan".to_string());
        
        // Should handle without error
        plugin.handle_queue_event(&event).await.unwrap();
    }

    #[test]
    fn test_consumer_plugin_preferences() {
        let custom_prefs = ConsumerPreferences {
            consume_all_messages: false,
            interested_message_types: vec!["FileChange".to_string()],
            high_frequency_capable: false,
            preferred_batch_size: 10,
            requires_ordered_delivery: true,
        };
        
        let plugin = MockConsumerPlugin::new().with_preferences(custom_prefs.clone());
        let returned_prefs = plugin.consumer_preferences();
        
        assert_eq!(returned_prefs, custom_prefs);
    }
}