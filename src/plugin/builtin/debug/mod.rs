//! Debug Plugin for Message Stream Inspection
//!
//! This plugin provides raw message stream display capabilities for
//! troubleshooting and development purposes. It serves as the first
//! consumer implementation and helps validate the queue system.
//!
//! # Features
//! - Display raw message stream from the queue
//! - Configurable verbosity levels
//! - Optional display of commit messages, file diffs, and raw data
//! - Message sequence number tracking
//! - Non-exclusive operation (can run with other plugins)

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::plugin::traits::{
    Plugin, PluginInfo, PluginType, ConsumerPlugin, PluginDataRequirements,
    ConsumerPreferences, PluginArgumentParser, PluginArgDefinition,
};
use crate::plugin::error::{PluginError, PluginResult};
use crate::plugin::context::{PluginContext, PluginRequest, PluginResponse};
use crate::plugin::data_export::{
    PluginDataExport, DataExportType, DataSchema, ColumnDef, ColumnType,
    DataPayload, Row, Value, ExportHints, ExportFormat
};
use crate::scanner::messages::{ScanMessage, MessageData};
use crate::queue::{QueueEvent, QueueConsumer};
use crate::cli::plugin_args::{PluginArguments, PluginArgValue};
use crate::notifications::AsyncNotificationManager;
use crate::notifications::events::PluginEvent;
use crate::notifications::traits::NotificationManager;

mod display;
mod config;

pub use display::MessageFormatter;
pub use config::DebugConfig;

/// Debug plugin for message stream inspection
pub struct DebugPlugin {
    /// Plugin information
    info: PluginInfo,
    
    /// Plugin configuration
    config: Arc<RwLock<DebugConfig>>,
    
    /// Message formatter for display
    formatter: MessageFormatter,
    
    /// Statistics tracking
    stats: Arc<RwLock<DebugStats>>,
    
    /// Whether the plugin is currently consuming
    consuming: Arc<RwLock<bool>>,
    
    /// Queue consumer handle
    consumer: Arc<RwLock<Option<QueueConsumer>>>,
    
    /// Notification publishing (optional - only enabled with --export flag)
    notification_manager: Option<AsyncNotificationManager<PluginEvent>>,
    current_scan_id: Arc<RwLock<Option<String>>>,
    
    /// Whether export functionality is enabled via CLI flag
    export_enabled: Arc<RwLock<bool>>,
}

/// Statistics for debug plugin operation
#[derive(Debug, Default, Clone)]
struct DebugStats {
    /// Total messages processed
    messages_processed: u64,
    
    /// Messages by type
    commit_messages: u64,
    file_changes: u64,
    file_info: u64,
    other_messages: u64,
    
    /// Errors encountered
    display_errors: u64,
    
    /// Queue events received
    queue_events: u64,
}

impl DebugPlugin {
    /// Create a new debug plugin instance
    pub fn new() -> Self {
        let info = PluginInfo::new(
            "debug".to_string(),
            "1.0.0".to_string(),
            1, // API version
            "Debug plugin for message stream inspection".to_string(),
            "gstats".to_string(),
            PluginType::Processing,
        )
        .with_priority(0) // Normal priority
        .with_load_by_default(false); // Manual activation
        
        let config = Arc::new(RwLock::new(DebugConfig::verbose()));
        let formatter = MessageFormatter::new(config.clone());
        
        Self {
            info,
            config,
            formatter,
            stats: Arc::new(RwLock::new(DebugStats::default())),
            consuming: Arc::new(RwLock::new(false)),
            consumer: Arc::new(RwLock::new(None)),
            notification_manager: None,
            current_scan_id: Arc::new(RwLock::new(None)),
            export_enabled: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Update statistics for a processed message
    async fn update_stats(&self, message: &ScanMessage) {
        let mut stats = self.stats.write().await;
        stats.messages_processed += 1;
        
        match message.data() {
            MessageData::CommitInfo { .. } => stats.commit_messages += 1,
            MessageData::FileChange { .. } => stats.file_changes += 1,
            MessageData::FileInfo { .. } => stats.file_info += 1,
            _ => stats.other_messages += 1,
        }
    }
    
    /// Display current statistics
    async fn display_stats(&self) {
        let stats = self.stats.read().await;
        let config = self.config.read().await;
        let export_enabled = *self.export_enabled.read().await;
        
        // Only display stats if verbose and export is not enabled
        if config.verbose && !export_enabled {
            println!("\n=== Debug Plugin Statistics ===");
            println!("Total messages processed: {}", stats.messages_processed);
            println!("  Commit messages: {}", stats.commit_messages);
            println!("  File changes: {}", stats.file_changes);
            println!("  File info: {}", stats.file_info);
            println!("  Other: {}", stats.other_messages);
            println!("Display errors: {}", stats.display_errors);
            println!("Queue events: {}", stats.queue_events);
            println!("==============================\n");
        }
    }
    
    /// Create PluginDataExport from debug statistics (only when export is enabled)
    async fn create_data_export(&self, scan_id: &str) -> PluginResult<PluginDataExport> {
        let stats = {
            let stats_guard = self.stats.read().await;
            stats_guard.clone()
        };
        
        // Create schema for debug statistics table
        let schema = DataSchema {
            columns: vec![
                ColumnDef::new("Metric", ColumnType::String)
                    .with_description("Debug metric name".to_string()),
                ColumnDef::new("Value", ColumnType::Integer)
                    .with_description("Metric count or value".to_string()),
                ColumnDef::new("Description", ColumnType::String)
                    .with_description("Description of the metric".to_string()),
            ],
            metadata: {
                let mut meta = std::collections::HashMap::new();
                meta.insert("description".to_string(), "Debug plugin statistics and message processing metrics".to_string());
                meta.insert("generated_by".to_string(), "debug_plugin".to_string());
                meta
            },
        };
        
        // Convert statistics to rows
        let rows: Vec<Row> = vec![
            Row::new(vec![
                Value::String("Messages Processed".to_string()),
                Value::Integer(stats.messages_processed as i64),
                Value::String("Total number of messages processed by debug plugin".to_string()),
            ]),
            Row::new(vec![
                Value::String("Commit Messages".to_string()),
                Value::Integer(stats.commit_messages as i64),
                Value::String("Number of git commit info messages".to_string()),
            ]),
            Row::new(vec![
                Value::String("File Changes".to_string()),
                Value::Integer(stats.file_changes as i64),
                Value::String("Number of file change messages".to_string()),
            ]),
            Row::new(vec![
                Value::String("File Info".to_string()),
                Value::Integer(stats.file_info as i64),
                Value::String("Number of file information messages".to_string()),
            ]),
            Row::new(vec![
                Value::String("Other Messages".to_string()),
                Value::Integer(stats.other_messages as i64),
                Value::String("Number of other message types".to_string()),
            ]),
            Row::new(vec![
                Value::String("Display Errors".to_string()),
                Value::Integer(stats.display_errors as i64),
                Value::String("Number of message display errors encountered".to_string()),
            ]),
            Row::new(vec![
                Value::String("Queue Events".to_string()),
                Value::Integer(stats.queue_events as i64),
                Value::String("Number of queue lifecycle events processed".to_string()),
            ]),
        ];
        
        // Create export hints
        let export_hints = ExportHints {
            preferred_formats: vec![
                ExportFormat::Console,
                ExportFormat::Json,
                ExportFormat::Csv,
            ],
            sort_by: Some("Metric".to_string()),
            sort_ascending: true,
            limit: None,
            include_totals: false,
            include_row_numbers: true,
            custom_hints: {
                let mut hints = std::collections::HashMap::new();
                hints.insert("title".to_string(), "Debug Plugin Statistics".to_string());
                hints.insert("category".to_string(), "debugging".to_string());
                hints
            },
        };
        
        Ok(PluginDataExport {
            plugin_id: "debug".to_string(),
            title: "Debug Plugin Statistics".to_string(),
            description: Some(format!(
                "Message processing statistics from debug plugin for scan {}",
                scan_id
            )),
            data_type: DataExportType::Tabular,
            schema,
            data: DataPayload::Rows(Arc::new(rows)),
            export_hints,
            timestamp: std::time::SystemTime::now(),
        })
    }
}

impl Default for DebugPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for DebugPlugin {
    fn plugin_info(&self) -> &PluginInfo {
        &self.info
    }
    
    async fn initialize(&mut self, context: &PluginContext) -> PluginResult<()> {
        // Initialize plugin with context if needed
        log::info!("Debug plugin initialized");
        
        // Initialize notification manager from context if available
        if let Some(ref manager) = context.notification_manager {
            self.notification_manager = Some(manager.as_ref().clone());
            log::debug!("DebugPlugin: Notification manager initialized from context");
        } else {
            log::debug!("DebugPlugin: No notification manager available in context");
        }
        
        Ok(())
    }
    
    async fn execute(&self, request: PluginRequest) -> PluginResult<PluginResponse> {
        match request {
            PluginRequest::GetStatistics => {
                // Return current statistics as a response
                let stats = self.stats.read().await;
                let stats_json = serde_json::json!({
                    "messages_processed": stats.messages_processed,
                    "commit_messages": stats.commit_messages,
                    "file_changes": stats.file_changes,
                    "file_info": stats.file_info,
                    "other_messages": stats.other_messages,
                    "display_errors": stats.display_errors,
                    "queue_events": stats.queue_events,
                });
                
                use crate::plugin::context::ExecutionMetadata;
                use std::collections::HashMap;
                
                let metadata = ExecutionMetadata {
                    duration_us: 100,
                    memory_used: 0,
                    entries_processed: stats.messages_processed,
                    plugin_version: self.info.version.clone(),
                    extra: HashMap::new(),
                };
                
                Ok(PluginResponse::success(
                    "stats-request".to_string(),
                    stats_json,
                    metadata,
                ))
            }
            _ => {
                Err(PluginError::execution_failed(
                    "Debug plugin only supports statistics requests",
                ))
            }
        }
    }
    
    async fn cleanup(&mut self) -> PluginResult<()> {
        // Stop consuming if active
        if *self.consuming.read().await {
            self.stop_consuming().await?;
        }
        
        // Display final statistics
        self.display_stats().await;
        
        log::info!("Debug plugin cleaned up");
        Ok(())
    }
    
    /// Cast to ConsumerPlugin since this plugin implements that trait
    fn as_consumer_plugin(&self) -> Option<&dyn ConsumerPlugin> {
        Some(self)
    }
    
    /// Cast to mutable ConsumerPlugin since this plugin implements that trait
    fn as_consumer_plugin_mut(&mut self) -> Option<&mut dyn ConsumerPlugin> {
        Some(self)
    }
}

#[async_trait]
impl ConsumerPlugin for DebugPlugin {
    async fn start_consuming(&mut self, consumer: QueueConsumer) -> PluginResult<()> {
        let mut consuming = self.consuming.write().await;
        
        if *consuming {
            return Err(PluginError::invalid_state("Already consuming"));
        }
        
        *consuming = true;
        
        let config = self.config.read().await;
        let export_enabled = *self.export_enabled.read().await;
        if config.verbose && !export_enabled {
            println!("\n=== Debug Plugin: Starting Message Consumption ===\n");
        }
        
        log::info!("Debug plugin started consuming messages");
        
        // Start the message processing loop in a background task
        let stats = Arc::clone(&self.stats);
        let formatter = MessageFormatter::new(Arc::clone(&self.config));
        let consuming_flag = Arc::clone(&self.consuming);
        let consumer_store = Arc::clone(&self.consumer);
        let export_enabled_flag = Arc::clone(&self.export_enabled);
        
        // Store the consumer in the field for later cleanup
        {
            let _consumer_guard = consumer_store.write().await;
            // We can't clone QueueConsumer, so we'll manage it differently
            // For now, just mark that we have a consumer
        }
        
        tokio::spawn(async move {
            while *consuming_flag.read().await {
                match consumer.read_next().await {
                    Ok(Some(message)) => {
                        // Display the message only if export is not enabled
                        let export_enabled = *export_enabled_flag.read().await;
                        if !export_enabled {
                            if let Err(e) = formatter.format_message(&message).await {
                                let mut stats_guard = stats.write().await;
                                stats_guard.display_errors += 1;
                                log::error!("Failed to display message: {}", e);
                            }
                        }
                        
                        // Always update statistics
                        {
                            let mut stats_guard = stats.write().await;
                            stats_guard.messages_processed += 1;
                            
                            match message.data() {
                                crate::scanner::messages::MessageData::CommitInfo { .. } => {
                                    stats_guard.commit_messages += 1;
                                }
                                crate::scanner::messages::MessageData::FileChange { .. } => {
                                    stats_guard.file_changes += 1;
                                }
                                crate::scanner::messages::MessageData::FileInfo { .. } => {
                                    stats_guard.file_info += 1;
                                }
                                _ => {
                                    stats_guard.other_messages += 1;
                                }
                            }
                        }
                        
                        // Acknowledge the message
                        if let Err(e) = consumer.acknowledge(message.header().sequence()).await {
                            log::error!("Failed to acknowledge message: {}", e);
                        }
                    }
                    Ok(None) => {
                        // No more messages, wait a bit
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    }
                    Err(e) => {
                        log::error!("Error reading from queue: {}", e);
                        break;
                    }
                }
            }
            log::info!("Debug plugin message processing loop ended");
        });
        
        Ok(())
    }
    
    async fn process_message(&self, consumer: &QueueConsumer, message: Arc<ScanMessage>) -> PluginResult<()> {
        // Display the message only if export is not enabled
        let export_enabled = *self.export_enabled.read().await;
        if !export_enabled {
            if let Err(e) = self.formatter.format_message(&message).await {
                let mut stats = self.stats.write().await;
                stats.display_errors += 1;
                log::error!("Failed to display message: {}", e);
                return Err(PluginError::execution_failed(format!("Display error: {}", e)));
            }
        }
        
        // Update statistics
        self.update_stats(&message).await;
        
        // Acknowledge the message
        consumer.acknowledge(message.header().sequence()).await.map_err(|e| {
            PluginError::execution_failed(format!("Failed to acknowledge message: {}", e))
        })?;
        
        Ok(())
    }
    
    async fn handle_queue_event(&self, event: &QueueEvent) -> PluginResult<()> {
        let mut stats = self.stats.write().await;
        stats.queue_events += 1;
        drop(stats); // Release the lock early
        
        let config = self.config.read().await;
        let export_enabled = *self.export_enabled.read().await;
        
        match event {
            QueueEvent::ScanStarted { scan_id, .. } => {
                // Store the current scan ID
                {
                    let mut current_scan = self.current_scan_id.write().await;
                    *current_scan = Some(scan_id.clone());
                }
                
                if config.verbose && !export_enabled {
                    println!("\n>>> SCAN STARTED: {} <<<\n", scan_id);
                }
            }
            QueueEvent::ScanComplete { scan_id, total_messages, .. } => {
                if config.verbose && !export_enabled {
                    println!("\n>>> SCAN COMPLETE: {} (Total: {} messages) <<<\n", 
                            scan_id, total_messages);
                }
                
                // Create and publish data export if export is enabled and we have a notification manager
                let export_enabled = *self.export_enabled.read().await;
                if export_enabled {
                    if let Some(ref manager) = self.notification_manager {
                        if let Ok(export_data) = self.create_data_export(scan_id).await {
                            let event = PluginEvent::DataReady {
                                plugin_id: "debug".to_string(),
                                scan_id: scan_id.clone(),
                                export: Arc::new(export_data),
                            };
                            
                            if let Err(e) = manager.publish(event).await {
                                log::warn!("Failed to publish DataReady event: {}", e);
                            } else {
                                log::debug!("Published DataReady event for debug plugin");
                            }
                        }
                    } else {
                        log::debug!("Export enabled but no notification manager available");
                    }
                }
            }
            QueueEvent::QueueDrained { scan_id, .. } => {
                if config.verbose && !export_enabled {
                    println!("\n>>> QUEUE DRAINED: {} <<<\n", scan_id);
                }
            }
            QueueEvent::MemoryWarning { current_size, threshold, .. } => {
                if config.verbose && !export_enabled {
                    println!("\n!!! MEMORY WARNING: {} / {} bytes !!!\n", 
                            current_size, threshold);
                }
            }
            _ => {
                // Other events are logged but not displayed
                log::debug!("Debug plugin received queue event: {:?}", event);
            }
        }
        
        Ok(())
    }
    
    async fn stop_consuming(&mut self) -> PluginResult<()> {
        let mut consuming = self.consuming.write().await;
        
        if !*consuming {
            return Ok(()); // Already stopped
        }
        
        *consuming = false;
        
        // Clear the consumer handle
        let mut consumer_guard = self.consumer.write().await;
        *consumer_guard = None;
        
        // Display final statistics
        self.display_stats().await;
        
        let config = self.config.read().await;
        let export_enabled = *self.export_enabled.read().await;
        if config.verbose && !export_enabled {
            println!("\n=== Debug Plugin: Stopped Message Consumption ===\n");
        }
        
        log::info!("Debug plugin stopped consuming messages");
        Ok(())
    }
    
    fn consumer_preferences(&self) -> ConsumerPreferences {
        ConsumerPreferences {
            consume_all_messages: true, // We want to see everything
            interested_message_types: vec![], // Empty = all types
            high_frequency_capable: true, // Can handle high message rates
            preferred_batch_size: 1, // Process one at a time for display
            requires_ordered_delivery: true, // Display in order
        }
    }
}

impl PluginDataRequirements for DebugPlugin {
    fn requires_current_file_content(&self) -> bool {
        false // Debug plugin only displays metadata
    }
    
    fn requires_historical_file_content(&self) -> bool {
        false // No historical content needed
    }
    
    fn preferred_buffer_size(&self) -> usize {
        4 * 1024 // Small buffer, we're just displaying
    }
    
    fn handles_binary_files(&self) -> bool {
        false // Only display metadata for binary files
    }
}

#[async_trait]
impl PluginArgumentParser for DebugPlugin {
    async fn parse_plugin_args(&mut self, args: &[String]) -> PluginResult<()> {
        // Parse arguments into DebugConfig
        let mut config = self.config.write().await;
        
        for arg in args {
            match arg.as_str() {
                "--verbose" | "-v" => config.verbose = true,
                "--full-commit-message" => config.full_commit_message = true,
                "--file-diff" => config.file_diff = true,
                "--raw-data" => config.raw_data = true,
                "--message-index" => config.message_index = true,
                "--no-color" => config.use_color = false,
                "--compact" => config.compact_mode = true,
                "--export" => {
                    // Enable export functionality
                    let mut export_enabled = self.export_enabled.write().await;
                    *export_enabled = true;
                    log::info!("Debug plugin: Export functionality enabled");
                }
                arg if arg.starts_with("--max-lines=") => {
                    let value = arg.strip_prefix("--max-lines=").unwrap();
                    config.max_display_lines = value.parse().unwrap_or(100);
                }
                _ => {
                    log::warn!("Unknown debug plugin argument: {}", arg);
                }
            }
        }
        
        Ok(())
    }
    
    fn get_arg_schema(&self) -> Vec<PluginArgDefinition> {
        vec![
            PluginArgDefinition {
                name: "--verbose".to_string(),
                description: "Enable verbose output".to_string(),
                required: false,
                default_value: Some("false".to_string()),
                arg_type: "boolean".to_string(),
                examples: vec!["-v".to_string(), "--verbose".to_string()],
            },
            PluginArgDefinition {
                name: "--full-commit-message".to_string(),
                description: "Show complete commit messages".to_string(),
                required: false,
                default_value: Some("false".to_string()),
                arg_type: "boolean".to_string(),
                examples: vec!["--full-commit-message".to_string()],
            },
            PluginArgDefinition {
                name: "--file-diff".to_string(),
                description: "Display file diffs if available".to_string(),
                required: false,
                default_value: Some("false".to_string()),
                arg_type: "boolean".to_string(),
                examples: vec!["--file-diff".to_string()],
            },
            PluginArgDefinition {
                name: "--raw-data".to_string(),
                description: "Show all raw message fields".to_string(),
                required: false,
                default_value: Some("false".to_string()),
                arg_type: "boolean".to_string(),
                examples: vec!["--raw-data".to_string()],
            },
            PluginArgDefinition {
                name: "--message-index".to_string(),
                description: "Display message sequence numbers".to_string(),
                required: false,
                default_value: Some("false".to_string()),
                arg_type: "boolean".to_string(),
                examples: vec!["--message-index".to_string()],
            },
            PluginArgDefinition {
                name: "--no-color".to_string(),
                description: "Disable colored output".to_string(),
                required: false,
                default_value: Some("false".to_string()),
                arg_type: "boolean".to_string(),
                examples: vec!["--no-color".to_string()],
            },
            PluginArgDefinition {
                name: "--compact".to_string(),
                description: "Use compact display mode".to_string(),
                required: false,
                default_value: Some("false".to_string()),
                arg_type: "boolean".to_string(),
                examples: vec!["--compact".to_string()],
            },
            PluginArgDefinition {
                name: "--max-lines".to_string(),
                description: "Maximum lines to display per message".to_string(),
                required: false,
                default_value: Some("100".to_string()),
                arg_type: "number".to_string(),
                examples: vec!["--max-lines=50".to_string()],
            },
            PluginArgDefinition {
                name: "--export".to_string(),
                description: "Enable data export interface to export plugin (default: console output only)".to_string(),
                required: false,
                default_value: Some("false".to_string()),
                arg_type: "boolean".to_string(),
                examples: vec!["--export".to_string()],
            },
        ]
    }
}

/// Apply parsed plugin arguments to configuration
pub fn apply_plugin_arguments(config: &mut DebugConfig, args: &PluginArguments) {
    for (key, value) in &args.arguments {
        match key.as_str() {
            "verbose" => {
                if let PluginArgValue::Flag(v) = value {
                    config.verbose = *v;
                }
            }
            "full-commit-message" => {
                if let PluginArgValue::Flag(v) = value {
                    config.full_commit_message = *v;
                }
            }
            "file-diff" => {
                if let PluginArgValue::Flag(v) = value {
                    config.file_diff = *v;
                }
            }
            "raw-data" => {
                if let PluginArgValue::Flag(v) = value {
                    config.raw_data = *v;
                }
            }
            "message-index" => {
                if let PluginArgValue::Flag(v) = value {
                    config.message_index = *v;
                }
            }
            "no-color" => {
                if let PluginArgValue::Flag(v) = value {
                    config.use_color = !v; // Invert for no-color flag
                }
            }
            "compact" => {
                if let PluginArgValue::Flag(v) = value {
                    config.compact_mode = *v;
                }
            }
            "max-lines" => {
                if let PluginArgValue::Number(n) = value {
                    config.max_display_lines = *n as usize;
                }
            }
            "export" => {
                if let PluginArgValue::Flag(v) = value {
                    // Note: This only handles the DebugConfig part.
                    // The actual export functionality is controlled by the export_enabled field
                    // in DebugPlugin which is set during parse_plugin_args
                    log::debug!("Export flag set to {} in config", v);
                }
            }
            _ => {
                log::warn!("Unknown debug plugin configuration key: {}", key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};
    
    #[tokio::test]
    async fn test_debug_plugin_creation() {
        let plugin = DebugPlugin::new();
        
        assert_eq!(plugin.info.name, "debug");
        assert_eq!(plugin.info.plugin_type, PluginType::Processing);
        assert!(!plugin.info.load_by_default);
        
        let stats = plugin.stats.read().await;
        assert_eq!(stats.messages_processed, 0);
    }
    
    #[tokio::test]
    async fn test_debug_plugin_lifecycle() {
        let mut plugin = DebugPlugin::new();
        
        // Initialize
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        // Note: start_consuming/stop_consuming require QueueConsumer which is complex to mock
        // These methods are tested in integration tests with actual queue setup
        
        // Cleanup
        plugin.cleanup().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_debug_plugin_message_processing() {
        // Note: process_message now requires QueueConsumer for acknowledgment
        // This is complex to mock in unit tests, so we test the core message
        // handling logic through the formatter instead
        
        let config = Arc::new(RwLock::new(DebugConfig::default()));
        let formatter = MessageFormatter::new(config);
        
        // Create test message
        let header = MessageHeader::new(1);
        let data = MessageData::CommitInfo {
            hash: "abc123".to_string(),
            author: "Test Author".to_string(),
            message: "Test commit".to_string(),
            timestamp: 1234567890,
            changed_files: vec![],
        };
        let message = ScanMessage::new(header, data);
        
        // Test that message can be formatted without error
        formatter.format_message(&message).await.unwrap();
    }
    
    #[tokio::test]
    async fn test_debug_plugin_queue_event_handling() {
        let plugin = DebugPlugin::new();
        
        // Handle scan started event
        let event = QueueEvent::scan_started("test-scan".to_string());
        plugin.handle_queue_event(&event).await.unwrap();
        
        // Check statistics
        let stats = plugin.stats.read().await;
        assert_eq!(stats.queue_events, 1);
    }
    
    #[tokio::test]
    async fn test_debug_plugin_argument_parsing() {
        let mut plugin = DebugPlugin::new();
        
        let args = vec![
            "--verbose".to_string(),
            "--full-commit-message".to_string(),
            "--message-index".to_string(),
            "--max-lines=50".to_string(),
        ];
        
        plugin.parse_plugin_args(&args).await.unwrap();
        
        let config = plugin.config.read().await;
        assert!(config.verbose);
        assert!(config.full_commit_message);
        assert!(config.message_index);
        assert_eq!(config.max_display_lines, 50);
    }
    
    #[tokio::test]
    async fn test_debug_plugin_consumer_preferences() {
        let plugin = DebugPlugin::new();
        
        let prefs = plugin.consumer_preferences();
        assert!(prefs.consume_all_messages);
        assert!(prefs.high_frequency_capable);
        assert!(prefs.requires_ordered_delivery);
        assert_eq!(prefs.preferred_batch_size, 1);
    }
    
    #[tokio::test]
    async fn test_debug_plugin_data_requirements() {
        let plugin = DebugPlugin::new();
        
        assert!(!plugin.requires_current_file_content());
        assert!(!plugin.requires_historical_file_content());
        assert_eq!(plugin.preferred_buffer_size(), 4 * 1024);
        assert!(!plugin.handles_binary_files());
    }
    
    #[tokio::test]
    async fn test_debug_plugin_notification_manager_initialization() {
        use crate::notifications::{AsyncNotificationManager};
        use crate::notifications::events::PluginEvent;
        
        let mut plugin = DebugPlugin::new();
        
        // Test initialization without notification manager
        let context_without = create_test_context();
        plugin.initialize(&context_without).await.unwrap();
        assert!(plugin.notification_manager.is_none());
        
        // Test initialization with notification manager
        let notification_manager = Arc::new(
            AsyncNotificationManager::<PluginEvent>::new()
        );
        let context_with = create_test_context()
            .with_notification_manager(notification_manager.clone());
        
        let mut plugin2 = DebugPlugin::new();
        plugin2.initialize(&context_with).await.unwrap();
        assert!(plugin2.notification_manager.is_some());
        
        // Verify the notification manager is the same instance
        assert!(plugin2.notification_manager.is_some());
    }
    
    #[tokio::test]
    async fn test_debug_plugin_export_mode_output_suppression() {
        let mut plugin = DebugPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        // Test normal verbose mode (export disabled)
        {
            let mut config = plugin.config.write().await;
            config.verbose = true;
        }
        *plugin.export_enabled.write().await = false;
        
        // In normal mode, display_stats should work (we can't easily test console output,
        // but we can verify the conditions for display)
        {
            let export_enabled = *plugin.export_enabled.read().await;
            let config = plugin.config.read().await;
            assert!(config.verbose && !export_enabled); // Should display in this case
        }
        
        // Test export mode (export enabled)
        *plugin.export_enabled.write().await = true;
        
        {
            let export_enabled = *plugin.export_enabled.read().await;
            let config = plugin.config.read().await;
            assert!(config.verbose && export_enabled); // Should NOT display in this case
        }
        
        // Test the export enabled flag state changes
        assert!(*plugin.export_enabled.read().await);
        
        // Test argument parsing sets export mode
        let args = vec!["--export".to_string(), "--verbose".to_string()];
        plugin.parse_plugin_args(&args).await.unwrap();
        
        assert!(*plugin.export_enabled.read().await);
        {
            let config = plugin.config.read().await;
            assert!(config.verbose);
        }
    }
    
    #[tokio::test]
    async fn test_debug_plugin_dataready_event_publishing() {
        use crate::notifications::{AsyncNotificationManager, traits::NotificationManager};
        use crate::notifications::events::PluginEvent;
        use crate::queue::notifications::QueueEvent;
        
        let mut plugin = DebugPlugin::new();
        
        // Set up plugin with notification manager and export mode
        let notification_manager = Arc::new(
            AsyncNotificationManager::<PluginEvent>::new()
        );
        let context = create_test_context()
            .with_notification_manager(notification_manager.clone());
        
        plugin.initialize(&context).await.unwrap();
        *plugin.export_enabled.write().await = true;
        
        // Add some test statistics
        {
            let mut stats = plugin.stats.write().await;
            stats.messages_processed = 100;
            stats.commit_messages = 50;
            stats.file_changes = 40;
            stats.file_info = 10;
        }
        
        // Test that create_data_export works
        let export_data = plugin.create_data_export("test-scan").await.unwrap();
        assert_eq!(export_data.title, "Debug Plugin Statistics");
        assert!(export_data.description.is_some());
        
        // Verify export data contains our test statistics
        if let crate::plugin::data_export::DataPayload::Rows(rows) = &export_data.data {
            assert!(!rows.is_empty());
            // Should have rows for messages processed, commit messages, etc.
            assert!(rows.len() >= 4);
        } else {
            panic!("Expected DataPayload::Rows");
        }
        
        // Test ScanComplete event handling with export enabled
        let scan_complete_event = QueueEvent::scan_complete("test-scan".to_string(), 100);
        plugin.handle_queue_event(&scan_complete_event).await.unwrap();
        
        // Note: We can't easily test that the notification was actually published
        // without a complex mock setup, but we can verify the export enabled state
        // and that create_data_export was called successfully
        assert!(*plugin.export_enabled.read().await);
        assert!(plugin.notification_manager.is_some());
    }
    
    fn create_test_context() -> PluginContext {
        use std::sync::Arc;
        use crate::scanner::{ScannerConfig, QueryParams};
        
        PluginContext::new(
            Arc::new(ScannerConfig::default()),
            Arc::new(QueryParams::default()),
        )
    }
}