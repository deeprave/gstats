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
use crate::scanner::messages::{ScanMessage, MessageData};
use crate::queue::{QueueEvent, QueueConsumer};
use crate::cli::plugin_args::{PluginArguments, PluginArgValue};

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
        
        if config.verbose {
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
        
        // Check for any plugin-specific configuration in context
        if let Some(debug_config) = context.plugin_config.get("debug") {
            // Apply any configuration from context
            log::debug!("Debug plugin config from context: {:?}", debug_config);
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
        if config.verbose {
            println!("\n=== Debug Plugin: Starting Message Consumption ===\n");
        }
        
        log::info!("Debug plugin started consuming messages");
        
        // Start the message processing loop in a background task
        let stats = Arc::clone(&self.stats);
        let formatter = MessageFormatter::new(Arc::clone(&self.config));
        let consuming_flag = Arc::clone(&self.consuming);
        let consumer_store = Arc::clone(&self.consumer);
        
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
                        // Display the message
                        if let Err(e) = formatter.format_message(&message).await {
                            let mut stats_guard = stats.write().await;
                            stats_guard.display_errors += 1;
                            log::error!("Failed to display message: {}", e);
                        } else {
                            // Update statistics
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
        // Display the message
        if let Err(e) = self.formatter.format_message(&message).await {
            let mut stats = self.stats.write().await;
            stats.display_errors += 1;
            log::error!("Failed to display message: {}", e);
            return Err(PluginError::execution_failed(format!("Display error: {}", e)));
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
        
        let config = self.config.read().await;
        
        if config.verbose {
            match event {
                QueueEvent::ScanStarted { scan_id, .. } => {
                    println!("\n>>> SCAN STARTED: {} <<<\n", scan_id);
                }
                QueueEvent::ScanComplete { scan_id, total_messages, .. } => {
                    println!("\n>>> SCAN COMPLETE: {} (Total: {} messages) <<<\n", 
                            scan_id, total_messages);
                }
                QueueEvent::QueueDrained { scan_id, .. } => {
                    println!("\n>>> QUEUE DRAINED: {} <<<\n", scan_id);
                }
                QueueEvent::MemoryWarning { current_size, threshold, .. } => {
                    println!("\n!!! MEMORY WARNING: {} / {} bytes !!!\n", 
                            current_size, threshold);
                }
                _ => {
                    // Other events are logged but not displayed
                    log::debug!("Debug plugin received queue event: {:?}", event);
                }
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
        if config.verbose {
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
    
    fn create_test_context() -> PluginContext {
        use std::sync::Arc;
        use crate::scanner::{ScannerConfig, QueryParams};
        
        PluginContext::new(
            Arc::new(ScannerConfig::default()),
            Arc::new(QueryParams::default()),
        )
    }
}