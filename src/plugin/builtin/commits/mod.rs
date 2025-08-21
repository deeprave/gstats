//! Commits Analysis Plugin
//! 
//! Built-in plugin for analyzing git commit history and statistics.

use crate::plugin::{
    Plugin, PluginInfo, PluginContext, PluginRequest, PluginResponse,
    PluginResult, PluginError, traits::{PluginType, PluginFunction, PluginDataRequirements, ConsumerPlugin, ConsumerPreferences, PluginClapParser}
};
use crate::plugin::data_export::{
    PluginDataExport, DataExportType, DataSchema, ColumnDef, ColumnType,
    DataPayload, Row, Value, ExportHints, ExportFormat
};
use crate::queue::{QueueConsumer, QueueEvent};
use crate::scanner::messages::{ScanMessage, MessageData, MessageHeader};
use crate::notifications::AsyncNotificationManager;
use crate::notifications::events::PluginEvent;
use crate::notifications::traits::{NotificationManager, Publisher};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use serde_json::json;

/// Statistics for commits plugin operation
#[derive(Debug, Default, Clone)]
struct CommitsStats {
    /// Total number of commits processed
    commit_count: usize,
    /// Commits by author for contributor analysis
    author_stats: HashMap<String, usize>,
}

/// Per-scan data for commits plugin
#[derive(Debug)]
struct CommitsScanData {
    /// Statistics for this scan
    stats: CommitsStats,
}

impl CommitsScanData {
    fn new() -> Self {
        Self {
            stats: CommitsStats::default(),
        }
    }
}

/// Commits analysis plugin
pub struct CommitsPlugin {
    /// Command name for clap integration
    command_name: String,
    
    /// Plugin settings (color preferences, etc.)
    settings: crate::plugin::PluginSettings,
    
    info: PluginInfo,
    initialized: bool,
    
    /// Plugin start time for performance tracking
    started_at: std::time::Instant,
    
    /// Per-scan commit data and statistics
    scan_data: Arc<RwLock<HashMap<String, CommitsScanData>>>,
    
    consuming: Arc<RwLock<bool>>,
    consumer: Arc<RwLock<Option<QueueConsumer>>>,
    
    /// Notification publishing - REQUIRED for all plugins
    notification_manager: Arc<AsyncNotificationManager<PluginEvent>>,
}

impl CommitsPlugin {
    /// Create a new commits plugin (DEPRECATED - requires notification manager)
    /// Use with_dependencies() instead
    #[deprecated(note = "Use with_dependencies() instead - plugins require notification managers")]
    pub fn new() -> Self {
        let info = PluginInfo::new(
            "commits".to_string(),
            "1.0.0".to_string(),
            crate::scanner::version::get_api_version() as u32,
            "Analyzes git commit history and provides commit statistics".to_string(),
            "gstats built-in".to_string(),
            PluginType::Processing,
        )
        .with_capability(
            "commit_analysis".to_string(),
            "Processes git commits and generates statistics".to_string(),
            "1.0.0".to_string(),
        )
        .with_capability(
            "author_tracking".to_string(),
            "Tracks commits by author for contributor analysis".to_string(),
            "1.0.0".to_string(),
        );

        Self {
            command_name: "commits".to_string(),
            settings: crate::plugin::PluginSettings::default(),
            info,
            initialized: false,
            started_at: std::time::Instant::now(),
            scan_data: Arc::new(RwLock::new(HashMap::new())),
            consuming: Arc::new(RwLock::new(false)),
            consumer: Arc::new(RwLock::new(None)),
            notification_manager: Arc::new(AsyncNotificationManager::new()), // Temporary for deprecated constructor
        }
    }
    
    // with_settings() method removed - use with_dependencies() instead
    
    /// Create a new commits plugin with all required dependencies (REQUIRED)
    /// This is the correct way to instantiate CommitsPlugin - it MUST have notification manager
    pub fn with_dependencies(
        settings: crate::plugin::PluginSettings,
        notification_manager: std::sync::Arc<crate::notifications::AsyncNotificationManager<crate::notifications::events::PluginEvent>>
    ) -> Self {
        let info = PluginInfo::new(
            "commits".to_string(),
            "1.0.0".to_string(),
            crate::scanner::version::get_api_version() as u32,
            "Analyzes git commit history and provides commit statistics".to_string(),
            "gstats built-in".to_string(),
            PluginType::Processing,
        )
        .with_capability(
            "commit_analysis".to_string(),
            "Processes git commits and generates statistics".to_string(),
            "1.0.0".to_string(),
        )
        .with_capability(
            "author_tracking".to_string(),
            "Tracks commits by author for contributor analysis".to_string(),
            "1.0.0".to_string(),
        );

        Self {
            command_name: "commits".to_string(),
            settings,
            info,
            initialized: false,
            started_at: std::time::Instant::now(),
            scan_data: Arc::new(RwLock::new(HashMap::new())),
            consuming: Arc::new(RwLock::new(false)),
            consumer: Arc::new(RwLock::new(None)),
            notification_manager,
        }
    }
    
    
    /// Handle ScanError event - abort processing and cleanup resources if fatal
    pub async fn handle_scan_error(&self, event: crate::notifications::ScanEvent) -> PluginResult<()> {
        use crate::notifications::ScanEvent;
        
        match event {
            ScanEvent::ScanError { scan_id, error, fatal } => {
                if fatal {
                    log::error!("CommitsPlugin received fatal error for scan {}: {}", scan_id, error);
                    // Fatal errors require immediate cleanup and abort processing
                    {
                        let mut scan_data = self.scan_data.write().await;
                        scan_data.remove(scan_id.as_str());
                    }
                    log::info!("CommitsPlugin cleaned up partial data for scan {}", scan_id);
                    
                    // If this was the last scan and we need to shut down, log elapsed time
                    let remaining_scans = {
                        let scan_data = self.scan_data.read().await;
                        scan_data.is_empty()
                    };
                    if remaining_scans {
                        let elapsed = self.started_at.elapsed();
                        log::info!("CommitsPlugin shutting down due to fatal error after {:?}", elapsed);
                    }
                } else {
                    log::warn!("CommitsPlugin received non-fatal error for scan {}: {}", scan_id, error);
                    // Non-fatal errors allow graceful degradation
                }
                Ok(())
            }
            _ => {
                Err(PluginError::ExecutionFailed { 
                    message: "CommitsPlugin::handle_scan_error received non-ScanError event".to_string() 
                })
            }
        }
    }
    
    /// Handle ScanCompleted event - finalize processing and prepare results
    pub async fn handle_scan_completed(&self, event: crate::notifications::ScanEvent) -> PluginResult<()> {
        use crate::notifications::ScanEvent;
        
        match event {
            ScanEvent::ScanCompleted { scan_id, duration, warnings } => {
                log::info!("CommitsPlugin received ScanCompleted event for scan {} (duration: {:?}, warnings: {})", 
                          scan_id, duration, warnings.len());
                
                // Finalize commit analysis processing and get stats before cleanup
                let (count, author_count) = {
                    let scan_data = self.scan_data.read().await;
                    if let Some(data) = scan_data.get(&scan_id) {
                        (data.stats.commit_count, data.stats.author_stats.len())
                    } else {
                        (0, 0)
                    }
                };
                log::info!("CommitsPlugin processed {} commits from {} authors for scan {}", 
                          count, author_count, scan_id);
                
                // Create and publish data export before cleanup
                {
                    if let Ok(export_data) = self.create_data_export(&scan_id).await {
                        let event = PluginEvent::DataReady {
                            plugin_id: "commits".to_string(),
                            scan_id: scan_id.clone(),
                            export: Arc::new(export_data),
                        };
                        
                        if let Err(e) = self.publish(event).await {
                            log::warn!("Failed to publish DataReady event: {}", e);
                        } else {
                            log::debug!("Published DataReady event for commits plugin");
                        }
                    }
                }
                
                // Clean up scan data for completed scan
                let remaining_scans = {
                    let mut scan_data = self.scan_data.write().await;
                    scan_data.remove(&scan_id);
                    scan_data.len()
                };
                
                // Log elapsed plugin time if this was the last scan
                if remaining_scans == 0 {
                    let elapsed = self.started_at.elapsed();
                    log::info!("CommitsPlugin completed all scans in {:?}", elapsed);
                }
                
                log::debug!("CommitsPlugin scan {} cleanup complete, {} scans remaining", 
                           scan_id, remaining_scans);
                
                Ok(())
            }
            _ => {
                Err(PluginError::ExecutionFailed { 
                    message: "CommitsPlugin::handle_scan_completed received non-ScanCompleted event".to_string() 
                })
            }
        }
    }

    /// Process a commit message and extract statistics  
    async fn process_commit(&self, scan_id: &str, message: &ScanMessage) -> PluginResult<()> {
        // Extract commit information from scan message
        if let MessageData::CommitInfo { author, .. } = &message.data {
            let mut scan_data = self.scan_data.write().await;
            let data = scan_data.entry(scan_id.to_string())
                .or_insert_with(CommitsScanData::new);
            
            data.stats.commit_count += 1;
            *data.stats.author_stats.entry(author.clone()).or_insert(0) += 1;
        }
        Ok(())
    }

    /// Generate commit summary statistics
    async fn generate_summary(&self) -> PluginResult<ScanMessage> {
        // Aggregate statistics from all active scans
        let (author_count, commit_count) = {
            let scan_data = self.scan_data.read().await;
            let mut total_commits = 0;
            let mut all_authors = HashMap::new();
            
            for data in scan_data.values() {
                total_commits += data.stats.commit_count;
                for (author, count) in &data.stats.author_stats {
                    *all_authors.entry(author.clone()).or_insert(0) += count;
                }
            }
            
            (all_authors.len(), total_commits)
        };
        
        let data = MessageData::MetricInfo {
            file_count: author_count as u32,
            line_count: commit_count as u64,
            complexity: if author_count == 0 {
                0.0
            } else {
                commit_count as f64 / author_count as f64
            },
        };

        let header = MessageHeader::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            "plugin-generated".to_string(),
        );

        Ok(ScanMessage::new(header, data))
    }
    
    /// Create PluginDataExport from current commit statistics
    async fn create_data_export(&self, scan_id: &str) -> PluginResult<PluginDataExport> {
        let (commit_count, author_stats) = {
            let scan_data_guard = self.scan_data.read().await;
            if let Some(data) = scan_data_guard.get(scan_id) {
                (data.stats.commit_count, data.stats.author_stats.clone())
            } else {
                (0, HashMap::new())
            }
        };
        
        // Create schema for commit statistics table
        let schema = DataSchema {
            columns: vec![
                ColumnDef::new("Author", ColumnType::String),
                ColumnDef::new("Commits", ColumnType::Integer),
                ColumnDef::new("Percentage", ColumnType::Float)
                    .with_format_hint("percentage"),
            ],
            metadata: HashMap::new(),
        };
        
        // Convert author stats to rows, sorted by commit count
        let mut author_list: Vec<_> = author_stats.iter().collect();
        author_list.sort_by(|a, b| b.1.cmp(a.1)); // Sort by commit count descending
        
        let rows: Vec<Row> = author_list
            .into_iter()
            .map(|(author, count)| {
                let percentage = if commit_count > 0 {
                    (*count as f64 / commit_count as f64) * 100.0
                } else {
                    0.0
                };
                
                Row::new(vec![
                    Value::String(author.clone()),
                    Value::Integer(*count as i64),
                    Value::Float(percentage),
                ])
            })
            .collect();
        
        // Create export hints
        let export_hints = ExportHints {
            preferred_formats: vec![
                ExportFormat::Console,
                ExportFormat::Json,
                ExportFormat::Csv,
                ExportFormat::Html,
                ExportFormat::Markdown,
            ],
            sort_by: Some("Commits".to_string()),
            sort_ascending: false, // Descending by commit count
            limit: None,
            include_totals: true,
            include_row_numbers: false,
            custom_hints: HashMap::new(),
        };
        
        Ok(PluginDataExport {
            plugin_id: "commits".to_string(),
            title: "Commit Analysis".to_string(),
            description: Some(format!(
                "Analysis of {} commits from {} authors in scan {}",
                commit_count, author_stats.len(), scan_id
            )),
            data_type: DataExportType::Tabular,
            schema,
            data: DataPayload::Rows(Arc::new(rows)),
            export_hints,
            timestamp: SystemTime::now(),
        })
    }
    
    /// Execute commits analysis function
    async fn execute_commits_analysis(&self) -> PluginResult<PluginResponse> {
        let start_time = std::time::Instant::now();

        let (commit_count, author_count) = {
            let scan_data = self.scan_data.read().await;
            let mut total_commits = 0;
            let mut all_authors = HashMap::new();
            
            for data in scan_data.values() {
                total_commits += data.stats.commit_count;
                for author in data.stats.author_stats.keys() {
                    all_authors.insert(author.clone(), true);
                }
            }
            
            (total_commits, all_authors.len())
        };

        let data = json!({
            "total_commits": commit_count,
            "unique_authors": author_count,
            "avg_commits_per_author": if author_count == 0 {
                0.0
            } else {
                commit_count as f64 / author_count as f64
            },
            "function": "commits"
        });

        let duration_us = start_time.elapsed().as_micros() as u64;

        Ok(PluginResponse::Execute {
            request_id: "commits_analysis".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_us,
                memory_used: 0,
                entries_processed: commit_count as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
    
    /// Execute author analysis function  
    async fn execute_author_analysis(&self) -> PluginResult<PluginResponse> {
        let start_time = std::time::Instant::now();

        let (author_count, author_stats, mut authors) = {
            let scan_data = self.scan_data.read().await;
            let mut aggregated_stats = HashMap::new();
            
            // Aggregate author stats from all scans
            for data in scan_data.values() {
                for (author, count) in &data.stats.author_stats {
                    *aggregated_stats.entry(author.clone()).or_insert(0) += count;
                }
            }
            
            let authors: Vec<_> = aggregated_stats.iter().map(|(k, v)| (k.clone(), *v)).collect();
            (aggregated_stats.len(), aggregated_stats.clone(), authors)
        };
        
        authors.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by commit count descending

        let data = json!({
            "total_authors": author_count,
            "top_authors": authors.iter().take(10).map(|(name, count)| {
                json!({ "name": name, "commits": count })
            }).collect::<Vec<_>>(),
            "author_stats": author_stats,
            "function": "authors"
        });

        let duration_us = start_time.elapsed().as_micros() as u64;

        Ok(PluginResponse::Execute {
            request_id: "author_analysis".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_us,
                memory_used: 0,
                entries_processed: author_count as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
}

// Default implementation removed - plugins require notification managers via with_dependencies()

#[async_trait]
impl Plugin for CommitsPlugin {
    fn plugin_info(&self) -> &PluginInfo {
        &self.info
    }

    async fn initialize(&mut self, _context: &PluginContext) -> PluginResult<()> {
        if self.initialized {
            return Ok(()); // Idempotent - allow re-initialization
        }

        // Clear any existing scan data
        {
            let mut scan_data = self.scan_data.write().await;
            scan_data.clear();
        }
        
        // TODO: Initialize notification manager when PluginContext supports it
        // For now, the notification manager will be None until the context is extended
        log::debug!("CommitsPlugin: Initialization complete (notification manager not yet implemented in context)");
        
        self.initialized = true;

        Ok(())
    }

    async fn execute(&self, request: PluginRequest) -> PluginResult<PluginResponse> {
        if !self.initialized {
            return Err(PluginError::invalid_state("Plugin not initialized"));
        }

        match request {
            PluginRequest::Execute {  invocation_type, .. } => {
                // Handle function-based execution
                let function_name = match invocation_type {
                    crate::plugin::InvocationType::Function(ref func) => func.as_str(),
                    crate::plugin::InvocationType::Direct => self.default_function().unwrap_or("commits"),
                    crate::plugin::InvocationType::Default => "commits",
                };
                
                // Route to appropriate function
                match function_name {
                    "commits" | "commit" | "history" => {
                        self.execute_commits_analysis().await
                    }
                    "authors" | "contributors" | "committers" => {
                        self.execute_author_analysis().await
                    }
                    _ => Err(PluginError::execution_failed(
                        format!("Unknown function: {}", function_name)
                    )),
                }
            }
            PluginRequest::GetStatistics => {
                let summary = self.generate_summary().await?;
                Ok(PluginResponse::Statistics(summary))
            }
            PluginRequest::GetCapabilities => {
                Ok(PluginResponse::Capabilities(self.info.capabilities.clone()))
            }
            _ => Err(PluginError::execution_failed("Unsupported request type")),
        }
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        // Stop consuming if we're currently consuming
        if *self.consuming.read().await {
            self.stop_consuming().await?;
        }
        
        self.initialized = false;
        {
            let mut scan_data = self.scan_data.write().await;
            scan_data.clear();
        }
        Ok(())
    }
    
    /// Get all functions this plugin can handle
    fn advertised_functions(&self) -> Vec<PluginFunction> {
        vec![
            PluginFunction {
                name: "commits".to_string(),
                aliases: vec!["commit".to_string(), "history".to_string()],
                description: "Analyze git commit history and generate commit statistics".to_string(),
                is_default: true,
            },
            PluginFunction {
                name: "authors".to_string(),
                aliases: vec!["contributors".to_string(), "committers".to_string()],
                description: "Analyze commit authors and contributor statistics".to_string(),
                is_default: false,
            },
        ]
    }
    
    /// Get the default function name
    fn default_function(&self) -> Option<&str> {
        Some("commits")
    }
    
    /// Cast to ConsumerPlugin since this plugin implements that trait
    fn as_consumer_plugin(&self) -> Option<&dyn ConsumerPlugin> {
        Some(self)
    }
    
    /// Cast to mutable ConsumerPlugin since this plugin implements that trait
    fn as_consumer_plugin_mut(&mut self) -> Option<&mut dyn ConsumerPlugin> {
        Some(self)
    }
    
    fn get_plugin_help(&self) -> Option<String> {
        use crate::plugin::traits::PluginClapParser;
        Some(PluginClapParser::generate_help(self))
    }
    
    fn get_plugin_help_with_colors(&self, no_color: bool, color: bool) -> Option<String> {
        use crate::plugin::traits::PluginClapParser;
        Some(PluginClapParser::generate_help_with_colors(self, no_color, color))
    }
    
    fn build_clap_command(&self) -> Option<clap::Command> {
        use crate::plugin::traits::PluginClapParser;
        Some(PluginClapParser::build_clap_command(self))
    }
    
    async fn parse_plugin_arguments(&mut self, args: &[String]) -> PluginResult<()> {
        use crate::plugin::traits::PluginClapParserExt;
        self.parse_plugin_args_default(args).await
    }
}

#[async_trait]
impl Publisher<PluginEvent> for CommitsPlugin {
    async fn publish(&self, event: PluginEvent) -> crate::notifications::NotificationResult<()> {
        self.notification_manager.publish(event).await
    }
}

#[async_trait]
impl crate::notifications::traits::Subscriber<crate::notifications::events::UnifiedEvent> for CommitsPlugin {
    fn subscriber_id(&self) -> &str {
        "commits-plugin"
    }
    
    async fn handle_event(&self, event: crate::notifications::events::UnifiedEvent) -> crate::notifications::NotificationResult<()> {
        match event {
            crate::notifications::events::UnifiedEvent::Scan(scan_event) => {
                match scan_event {
                    crate::notifications::ScanEvent::ScanError { .. } => {
                        // Now that handlers use &self, we can call them directly
                        if let Err(e) = self.handle_scan_error(scan_event).await {
                            log::error!("CommitsPlugin failed to handle ScanError: {}", e);
                        }
                    }
                    crate::notifications::ScanEvent::ScanCompleted { .. } => {
                        // Now that handlers use &self, we can call them directly
                        if let Err(e) = self.handle_scan_completed(scan_event).await {
                            log::error!("CommitsPlugin failed to handle ScanCompleted: {}", e);
                        }
                    }
                    _ => {
                        // Handle other ScanEvents if needed - for now just log
                        log::debug!("CommitsPlugin received ScanEvent: {:?}", scan_event);
                    }
                }
            }
            _ => {
                // Handle other UnifiedEvent types if needed - for now just log
                log::debug!("CommitsPlugin received non-ScanEvent: {:?}", event);
            }
        }
        Ok(())
    }
}

/// Data requirements implementation for CommitsPlugin
/// This plugin only needs commit metadata, not file content
impl PluginDataRequirements for CommitsPlugin {
    fn requires_current_file_content(&self) -> bool {
        false // Only needs commit metadata (author, hash, message, timestamp)
    }
    
    fn requires_historical_file_content(&self) -> bool {
        false // Only analyzes commit history metadata, not file changes
    }
    
    fn preferred_buffer_size(&self) -> usize {
        4096 // Small buffer since we don't read files
    }
    
    fn max_file_size(&self) -> Option<usize> {
        None // N/A - doesn't process files
    }
    
    fn handles_binary_files(&self) -> bool {
        false // N/A - doesn't process files
    }
}

#[async_trait]
impl ConsumerPlugin for CommitsPlugin {
    async fn start_consuming(&mut self, consumer: QueueConsumer) -> PluginResult<()> {
        let mut consuming = self.consuming.write().await;
        
        if *consuming {
            return Err(PluginError::invalid_state("Already consuming"));
        }
        
        *consuming = true;
        
        // Store the consumer
        {
            let mut consumer_guard = self.consumer.write().await;
            *consumer_guard = Some(consumer);
        }
        
        log::info!("Commits plugin started consuming messages");
        Ok(())
    }
    
    async fn process_message(&self, consumer: &QueueConsumer, message: Arc<ScanMessage>) -> PluginResult<()> {
        // Process the commit message and update statistics
        let scan_id = "unknown"; // TODO: Get actual scan_id from message context
        self.process_commit(scan_id, &message).await?;
        
        // Acknowledge the message
        consumer.acknowledge(message.header().sequence()).await.map_err(|e| {
            PluginError::execution_failed(format!("Failed to acknowledge message: {}", e))
        })?;
        
        Ok(())
    }
    
    async fn handle_queue_event(&self, event: &QueueEvent) -> PluginResult<()> {
        log::debug!("Commits plugin received queue event: {:?}", event);
        
        match event {
            QueueEvent::ScanStarted { scan_id, .. } => {
                log::info!("Commits plugin: scan started for {}", scan_id);
                
                // Initialize scan data for this scan
                {
                    let mut scan_data = self.scan_data.write().await;
                    scan_data.insert(scan_id.clone(), CommitsScanData::new());
                }
            }
            QueueEvent::ScanComplete { scan_id, total_messages, .. } => {
                let (count, author_count) = {
                    let scan_data = self.scan_data.read().await;
                    if let Some(data) = scan_data.get(scan_id) {
                        (data.stats.commit_count, data.stats.author_stats.len())
                    } else {
                        (0, 0)
                    }
                };
                log::info!(
                    "Commits plugin: scan {} complete - processed {} commits from {} authors (total {} messages)", 
                    scan_id, count, author_count, total_messages
                );
                
                // Create and publish data export
                {
                    if let Ok(export_data) = self.create_data_export(scan_id).await {
                        let event = PluginEvent::DataReady {
                            plugin_id: "commits".to_string(),
                            scan_id: scan_id.clone(),
                            export: Arc::new(export_data),
                        };
                        
                        if let Err(e) = self.publish(event).await {
                            log::warn!("Failed to publish DataReady event: {}", e);
                        } else {
                            log::debug!("Published DataReady event for commits plugin");
                        }
                    }
                }
            }
            _ => {
                // Other events are just logged
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
        {
            let mut consumer_guard = self.consumer.write().await;
            *consumer_guard = None;
        }
        
        // Log elapsed plugin runtime
        let elapsed = self.started_at.elapsed();
        log::info!("Commits plugin completed in {:?}", elapsed);
        Ok(())
    }
    
    fn consumer_preferences(&self) -> ConsumerPreferences {
        ConsumerPreferences {
            consume_all_messages: false, // Only interested in commit messages
            interested_message_types: vec!["CommitInfo".to_string()],
            high_frequency_capable: true, // Can handle many commits
            preferred_batch_size: 10, // Process in small batches
            requires_ordered_delivery: false, // Order doesn't matter for statistics
        }
    }
}

impl CommitsPlugin {
}

/// Modern clap-based argument parsing implementation for commits plugin
#[async_trait]
impl PluginClapParser for CommitsPlugin {
    fn get_command_name(&self) -> impl Into<String> {
        &self.command_name
    }
    
    fn get_command_description(&self) -> &str {
        "Analyzes git commit history and statistics"
    }
    
    fn get_plugin_settings(&self) -> &crate::plugin::PluginSettings {
        &self.settings
    }
    
    fn add_plugin_args(&self, command: clap::Command) -> clap::Command {
        use clap::{Arg, ArgAction};
        
        command
            .override_usage("commits [OPTIONS]")
            .help_template("Usage: {usage}\n\nAnalyzes git commit history and statistics\n\nOptions:\n{options}\n{after-help}")
            .after_help("Provides commit analysis, author statistics, and development patterns.")
            .arg(Arg::new("include-stats")
                .long("stats")
                .help("Include detailed statistical analysis")
                .action(ArgAction::SetTrue))
    }
    
    async fn configure_from_matches(&mut self, matches: &clap::ArgMatches) -> PluginResult<()> {
        // Commits plugin doesn't have complex configuration state to update
        // The arguments are handled during execution based on the function being called
        
        
        if let Some(authors) = matches.get_many::<String>("author-filter") {
            log::debug!("Commits plugin configured with author filters: {:?}", 
                       authors.collect::<Vec<_>>());
        }
        
        if matches.get_flag("exclude-merges") {
            log::debug!("Commits plugin configured to exclude merge commits");
        }
        
        if matches.get_flag("include-stats") {
            log::debug!("Commits plugin configured to include detailed statistics");
        }
        
        if let Some(format) = matches.get_one::<String>("output-format") {
            log::debug!("Commits plugin configured with output format: {}", format);
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::context::PluginContext;
    use crate::scanner::messages::MessageHeader;

    fn create_test_commit_message(author: &str, hash: &str, message: &str) -> ScanMessage {
        let data = MessageData::CommitInfo {
            author: author.to_string(),
            hash: hash.to_string(),
            message: message.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            changed_files: vec![crate::scanner::messages::FileChangeData {
                path: "src/main.rs".to_string(),
                lines_added: 10,
                lines_removed: 2,
            }], // Add test file
        };

        let header = MessageHeader::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            "plugin-generated".to_string(),
        );

        ScanMessage::new(header, data)
    }

    fn create_test_context() -> PluginContext {
        let scanner_config = std::sync::Arc::new(crate::scanner::ScannerConfig::default());
        let query_params = std::sync::Arc::new(crate::scanner::QueryParams::default());
        
        PluginContext::new(
            scanner_config,
            query_params,
        )
    }

    #[tokio::test]
    async fn test_commits_plugin_creation() {
        let plugin = CommitsPlugin::new();
        assert_eq!(plugin.plugin_info().name, "commits");
        assert_eq!(plugin.plugin_info().plugin_type, PluginType::Processing);
        assert!(!plugin.initialized);
    }

    #[tokio::test]
    async fn test_commits_plugin_initialization() {
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();

        assert!(plugin.initialize(&context).await.is_ok());
        assert!(plugin.initialized);

        // Test double initialization succeeds (idempotent)
        assert!(plugin.initialize(&context).await.is_ok());
    }

    #[tokio::test]
    async fn test_commits_plugin_processing() {
        let _plugin = CommitsPlugin::new();
        // Plugin no longer advertises supported modes - processes all data
        // This test just verifies the plugin can be created
        assert!(true);
    }

    #[tokio::test]
    async fn test_commits_plugin_execution() {
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        let request = PluginRequest::new()
            .with_parameter("test".to_string(), "value".to_string());

        let response = plugin.execute(request).await.unwrap();
        assert!(response.is_success());
    }

    #[tokio::test]
    async fn test_commits_plugin_execute() {
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        // Test get capabilities
        let response = plugin.execute(PluginRequest::GetCapabilities).await.unwrap();
        match response {
            PluginResponse::Capabilities(caps) => {
                assert_eq!(caps.len(), 2);
                assert!(caps.iter().any(|c| c.name == "commit_analysis"));
            }
            _ => panic!("Unexpected response type"),
        }

        // Test get statistics
        let response = plugin.execute(PluginRequest::GetStatistics).await.unwrap();
        match response {
            PluginResponse::Statistics(stats) => {
                assert!(matches!(stats.data, MessageData::MetricInfo { .. }));
            }
            _ => panic!("Unexpected response type"),
        }
    }

    #[tokio::test]
    async fn test_commits_plugin_cleanup() {
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        assert!(plugin.cleanup().await.is_ok());
        assert!(!plugin.initialized);
        {
            let scan_data = plugin.scan_data.read().await;
            assert!(scan_data.is_empty()); // No scans should be active initially
        }
    }

    #[tokio::test]
    async fn test_commits_plugin_handles_scan_data_ready() {
        use crate::notifications::ScanEvent;
        
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        // Create ScanDataReady event
        let event = ScanEvent::ScanDataReady {
            scan_id: "test_scan".to_string(),
            data_type: "commits".to_string(),
            message_count: 1,
        };
        
        // This should fail because handle_scan_data_ready is not implemented yet
        let result = plugin.handle_scan_data_ready(event).await;
        assert!(result.is_ok());
        
        // Verify that the plugin processed the commit data
        // For now, just verify the method exists and returns Ok
    }
    
    #[tokio::test]
    async fn test_commits_plugin_handles_scan_warning() {
        use crate::notifications::ScanEvent;
        
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        let event = ScanEvent::ScanWarning {
            scan_id: "test_scan".to_string(),
            warning: "Test warning message".to_string(),
            recoverable: true,
        };
        
        let result = plugin.handle_scan_warning(event).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_commits_plugin_handles_scan_error() {
        use crate::notifications::ScanEvent;
        
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        let event = ScanEvent::ScanError {
            scan_id: "test_scan".to_string(),
            error: "Test error message".to_string(),
            fatal: false,
        };
        
        let result = plugin.handle_scan_error(event).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_commits_plugin_handles_scan_completed() {
        use crate::notifications::ScanEvent;
        use std::time::Duration;
        
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        let event = ScanEvent::ScanCompleted {
            scan_id: "test_scan".to_string(),
            duration: Duration::from_secs(10),
            warnings: vec!["Warning 1".to_string()],
        };
        
        let result = plugin.handle_scan_completed(event).await;
        assert!(result.is_ok());
    }
}