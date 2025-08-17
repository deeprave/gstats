//! Code Metrics Plugin
//! 
//! Built-in plugin for analyzing code quality metrics and statistics.
//! This plugin uses comprehensive EventProcessor implementations from the
//! plugin processors module for advanced analysis capabilities.

use crate::plugin::{
    Plugin, PluginInfo, PluginContext, PluginRequest, PluginResponse,
    PluginResult, PluginError, traits::{PluginType, PluginDataRequirements, ConsumerPlugin, ConsumerPreferences, PluginClapParser}
};
use crate::plugin::data_export::{
    PluginDataExport, DataExportType, DataSchema, ColumnDef, ColumnType,
    DataPayload, Row, Value, ExportHints, ExportFormat
};
use crate::notifications::AsyncNotificationManager;
use crate::notifications::events::PluginEvent;
use crate::notifications::traits::NotificationManager;
use crate::queue::{QueueConsumer, QueueEvent};
use crate::scanner::async_engine::processors::{EventProcessor, EventProcessingCoordinator};
use crate::plugin::processors::{
    ChangeFrequencyProcessor as ComprehensiveChangeFrequencyProcessor,
    ComplexityProcessor as ComprehensiveComplexityProcessor,
    HotspotProcessor as ComprehensiveHotspotProcessor,
    DebtAssessmentProcessor as ComprehensiveDebtAssessmentProcessor,
    FormatDetectionProcessor as ComprehensiveFormatDetectionProcessor,
    DuplicationDetectorProcessor as ComprehensiveDuplicationDetectorProcessor,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Code Metrics Plugin using comprehensive event-driven processors
pub struct MetricsPlugin {
    info: PluginInfo,
    initialized: bool,
    processor_coordinator: Option<EventProcessingCoordinator>,
    results: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    // Consumer plugin fields
    consuming: Arc<RwLock<bool>>,
    consumer: Arc<RwLock<Option<QueueConsumer>>>,
    // Notification publishing
    notification_manager: Option<AsyncNotificationManager<PluginEvent>>,
    current_scan_id: Arc<RwLock<Option<String>>>,
}

impl MetricsPlugin {
    pub fn new() -> Self {
        let info = PluginInfo::new(
            "metrics".to_string(),
            "1.0.0".to_string(),
            crate::scanner::version::get_api_version() as u32,
            "Analyzes code quality metrics using comprehensive event-driven processors".to_string(),
            "gstats built-in".to_string(),
            PluginType::Processing,
        )
        .with_capability(
            "change_frequency".to_string(),
            "Analyzes file change frequency patterns".to_string(),
            "1.0.0".to_string(),
        )
        .with_capability(
            "complexity".to_string(),
            "Calculates code complexity metrics".to_string(),
            "1.0.0".to_string(),
        )
        .with_capability(
            "hotspots".to_string(),
            "Identifies code hotspots".to_string(),
            "1.0.0".to_string(),
        )
        .with_capability(
            "debt_assessment".to_string(),
            "Assesses technical debt".to_string(),
            "1.0.0".to_string(),
        )
        .with_capability(
            "format_detection".to_string(),
            "Detects file formats".to_string(),
            "1.0.0".to_string(),
        )
        .with_capability(
            "duplication_detection".to_string(),
            "Detects code duplication".to_string(),
            "1.0.0".to_string(),
        );

        Self {
            info,
            initialized: false,
            processor_coordinator: None,
            results: Arc::new(RwLock::new(HashMap::new())),
            // Initialize consumer plugin fields
            consuming: Arc::new(RwLock::new(false)),
            consumer: Arc::new(RwLock::new(None)),
            notification_manager: None,
            current_scan_id: Arc::new(RwLock::new(None)),
        }
    }

    fn create_processors(&self) -> Vec<Box<dyn EventProcessor>> {
        let mut processors: Vec<Box<dyn EventProcessor>> = Vec::new();

        // All processors run without mode filtering
        processors.push(Box::new(ComprehensiveChangeFrequencyProcessor::new()));
        processors.push(Box::new(ComprehensiveComplexityProcessor::new()));
        processors.push(Box::new(ComprehensiveHotspotProcessor::new()));
        processors.push(Box::new(ComprehensiveDebtAssessmentProcessor::new()));
        processors.push(Box::new(ComprehensiveFormatDetectionProcessor::new()));
        processors.push(Box::new(ComprehensiveDuplicationDetectorProcessor::new()));

        processors
    }
    
    /// Create PluginDataExport from current metrics results
    async fn create_data_export(&self, scan_id: &str) -> PluginResult<PluginDataExport> {
        let results = {
            let results_guard = self.results.read().await;
            results_guard.clone()
        };
        
        // Create schema for metrics table
        let schema = DataSchema {
            columns: vec![
                ColumnDef::new("Metric", ColumnType::String)
                    .with_description("Metric name or category".to_string()),
                ColumnDef::new("Value", ColumnType::String)
                    .with_description("Metric value (formatted)".to_string()),
                ColumnDef::new("Type", ColumnType::String)
                    .with_description("Type of metric (complexity, hotspot, etc.)".to_string()),
            ],
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("description".to_string(), "Code quality metrics and analysis results".to_string());
                meta.insert("generated_by".to_string(), "metrics_plugin".to_string());
                meta
            },
        };
        
        // Convert results to rows
        let rows: Vec<Row> = results
            .iter()
            .map(|(key, value)| {
                // Determine metric type from key
                let metric_type = if key.contains("complexity") {
                    "Complexity"
                } else if key.contains("hotspot") {
                    "Hotspot"
                } else if key.contains("change_frequency") {
                    "Change Frequency"
                } else if key.contains("debt") {
                    "Technical Debt"
                } else if key.contains("duplication") {
                    "Duplication"
                } else if key.contains("format") {
                    "Format Detection"
                } else {
                    "General"
                };
                
                // Format value based on type
                let formatted_value = match value {
                    serde_json::Value::Number(n) => {
                        if n.is_f64() {
                            format!("{:.2}", n.as_f64().unwrap_or(0.0))
                        } else {
                            n.to_string()
                        }
                    }
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Array(arr) => format!("Array[{}]", arr.len()),
                    serde_json::Value::Object(obj) => format!("Object[{}]", obj.len()),
                    serde_json::Value::Null => "null".to_string(),
                };
                
                Row::new(vec![
                    Value::String(key.clone()),
                    Value::String(formatted_value),
                    Value::String(metric_type.to_string()),
                ])
            })
            .collect();
        
        // Create export hints
        let export_hints = ExportHints {
            preferred_formats: vec![
                ExportFormat::Console,
                ExportFormat::Json,
                ExportFormat::Html,
                ExportFormat::Csv,
            ],
            sort_by: Some("Type".to_string()),
            sort_ascending: true,
            limit: None,
            include_totals: false,
            include_row_numbers: true,
            custom_hints: {
                let mut hints = HashMap::new();
                hints.insert("title".to_string(), "Code Quality Metrics".to_string());
                hints.insert("highlight_high_values".to_string(), "true".to_string());
                hints
            },
        };
        
        Ok(PluginDataExport {
            plugin_id: "metrics".to_string(),
            title: "Code Quality Metrics".to_string(),
            description: Some(format!(
                "Comprehensive code quality analysis with {} metrics for scan {}",
                results.len(), scan_id
            )),
            data_type: DataExportType::Tabular,
            schema,
            data: DataPayload::Rows(Arc::new(rows)),
            export_hints,
            timestamp: std::time::SystemTime::now(),
        })
    }
}

impl Default for MetricsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConsumerPlugin for MetricsPlugin {
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
        
        log::info!("Metrics plugin started consuming messages");
        Ok(())
    }
    
    async fn process_message(&self, consumer: &QueueConsumer, message: Arc<crate::scanner::messages::ScanMessage>) -> PluginResult<()> {
        // Process the message through our event processors
        // For now, just acknowledge the message
        // TODO: Integrate with the event processing coordinator
        
        // Acknowledge the message
        consumer.acknowledge(message.header().sequence()).await.map_err(|e| {
            PluginError::execution_failed(format!("Failed to acknowledge message: {}", e))
        })?;
        
        Ok(())
    }
    
    async fn handle_queue_event(&self, event: &QueueEvent) -> PluginResult<()> {
        log::debug!("Metrics plugin received queue event: {:?}", event);
        
        match event {
            QueueEvent::ScanStarted { scan_id, .. } => {
                log::info!("Metrics plugin: scan started for {}", scan_id);
                
                // Store the current scan ID
                {
                    let mut current_scan = self.current_scan_id.write().await;
                    *current_scan = Some(scan_id.clone());
                }
                
                // Reset results for new scan
                {
                    let mut results = self.results.write().await;
                    results.clear();
                }
            }
            QueueEvent::ScanComplete { scan_id, total_messages, .. } => {
                let result_count = {
                    let results = self.results.read().await;
                    results.len()
                };
                log::info!(
                    "Metrics plugin: scan complete for {} - generated {} metrics (total {} messages)", 
                    scan_id, result_count, total_messages
                );
                
                // Create and publish data export if we have a notification manager
                if let Some(ref manager) = self.notification_manager {
                    if let Ok(export_data) = self.create_data_export(scan_id).await {
                        let event = PluginEvent::DataReady {
                            plugin_id: "metrics".to_string(),
                            scan_id: scan_id.clone(),
                            export: Arc::new(export_data),
                        };
                        
                        if let Err(e) = manager.publish(event).await {
                            log::warn!("Failed to publish DataReady event: {}", e);
                        } else {
                            log::debug!("Published DataReady event for metrics plugin");
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
        
        log::info!("Metrics plugin stopped consuming messages");
        Ok(())
    }
    
    fn consumer_preferences(&self) -> ConsumerPreferences {
        ConsumerPreferences {
            consume_all_messages: true, // Metrics needs all data types for comprehensive analysis
            interested_message_types: vec![], // Empty = all types
            high_frequency_capable: true, // Can handle high message rates
            preferred_batch_size: 100, // Process in larger batches for efficiency
            requires_ordered_delivery: false, // Order doesn't matter for metrics
        }
    }
}

#[async_trait]
impl Plugin for MetricsPlugin {
    fn plugin_info(&self) -> &PluginInfo {
        &self.info
    }

    async fn initialize(&mut self, _context: &PluginContext) -> PluginResult<()> {
        if self.initialized {
            return Ok(());
        }

        let mut coordinator = EventProcessingCoordinator::new();
        let processors = self.create_processors();
        
        for processor in processors {
            coordinator.add_processor(processor);
        }

        coordinator.initialize().await?;
        self.processor_coordinator = Some(coordinator);
        
        // TODO: Initialize notification manager when PluginContext supports it
        // For now, the notification manager will be None until the context is extended
        log::debug!("MetricsPlugin: Initialization complete (notification manager not yet implemented in context)");
        
        self.initialized = true;

        Ok(())
    }

    async fn execute(&self, request: PluginRequest) -> PluginResult<PluginResponse> {
        match request {
            PluginRequest::Execute { request_id, .. } => {
                // Measure execution time
                let start_time = std::time::Instant::now();
                
                // TODO: Implement actual metrics analysis based on collected results
                let results = {
                    let results_guard = self.results.read().await;
                    results_guard.clone()
                };
                
                let duration_us = start_time.elapsed().as_micros() as u64;
                
                let data = serde_json::json!({
                    "metrics_count": results.len(),
                    "available_metrics": results.keys().collect::<Vec<_>>(),
                    "function": "metrics"
                });
                
                let metadata = crate::plugin::context::ExecutionMetadata {
                    duration_us,
                    memory_used: 0,
                    entries_processed: results.len() as u64,
                    plugin_version: self.info.version.clone(),
                    extra: HashMap::new(),
                };
                
                Ok(PluginResponse::success(request_id, data, metadata))
            }
            PluginRequest::GetCapabilities => {
                Ok(PluginResponse::Capabilities(self.info.capabilities.clone()))
            }
            PluginRequest::GetStatistics => {
                // Create a metric info message for statistics
                use crate::scanner::messages::{ScanMessage, MessageData, MessageHeader};
                
                let results_count = {
                    let results = self.results.read().await;
                    results.len()
                };
                
                let data = MessageData::MetricInfo {
                    file_count: results_count as u32,
                    line_count: 0, // Not applicable for metrics
                    complexity: 0.0, // Will be calculated by individual processors
                };
                
                let header = MessageHeader::new(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                );
                
                Ok(PluginResponse::Statistics(ScanMessage::new(header, data)))
            }
            _ => Err(PluginError::execution_failed("Unsupported request type")),
        }
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        // Stop consuming if we're currently consuming
        if *self.consuming.read().await {
            self.stop_consuming().await?;
        }
        
        self.processor_coordinator = None;
        self.initialized = false;
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
    
    /// Get all functions this plugin can handle
    fn advertised_functions(&self) -> Vec<crate::plugin::traits::PluginFunction> {
        vec![
            crate::plugin::traits::PluginFunction {
                name: "metrics".to_string(),
                aliases: vec!["stats".to_string(), "analysis".to_string()],
                description: "Analyze code quality metrics and statistics".to_string(),
                is_default: true,
            },
            crate::plugin::traits::PluginFunction {
                name: "complexity".to_string(),
                aliases: vec!["complex".to_string()],
                description: "Calculate code complexity metrics".to_string(),
                is_default: false,
            },
            crate::plugin::traits::PluginFunction {
                name: "hotspots".to_string(),
                aliases: vec!["hot".to_string()],
                description: "Identify code hotspots and problem areas".to_string(),
                is_default: false,
            },
        ]
    }
    
    /// Get the default function name
    fn default_function(&self) -> Option<&str> {
        Some("metrics")
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
}

/// Data requirements implementation for MetricsPlugin
/// This plugin needs current file content for code complexity and quality analysis
impl PluginDataRequirements for MetricsPlugin {
    fn requires_current_file_content(&self) -> bool {
        true // Needs to analyze code complexity, duplication, etc.
    }
    
    fn requires_historical_file_content(&self) -> bool {
        false // Focuses on current state metrics, not historical comparison
    }
    
    fn preferred_buffer_size(&self) -> usize {
        64 * 1024 // 64KB buffer for efficient file processing
    }
    
    fn max_file_size(&self) -> Option<usize> {
        Some(5 * 1024 * 1024) // 5MB limit to avoid memory issues with very large files
    }
    
    fn handles_binary_files(&self) -> bool {
        false // Only analyzes text-based source code
    }
}

/// Modern clap-based argument parsing implementation for metrics plugin
#[async_trait]
impl PluginClapParser for MetricsPlugin {
    fn build_clap_command(&self) -> clap::Command {
        use clap::{Arg, ArgAction, Command};
        
        Command::new("metrics")
            .override_usage("metrics [OPTIONS]")
            .help_template("Usage: {usage}\n\nAnalyzes code quality metrics and statistics\n\nOptions:\n{options}\n{after-help}")
            .after_help("Analyzes complexity, duplication, hotspots, and code quality indicators.")
            .arg(Arg::new("complexity-threshold")
                .short('c')
                .long("complexity")
                .value_name("NUMBER")
                .help("Complexity threshold for reporting")
                .value_parser(clap::value_parser!(u32))
                .default_value("10"))
            .arg(Arg::new("exclude-tests")
                .long("no-tests")
                .help("Exclude test files from analysis")
                .action(ArgAction::SetTrue))
            .arg(Arg::new("duplication-threshold")
                .short('d')
                .long("duplication")
                .value_name("LINES")
                .help("Minimum lines for duplication detection")
                .value_parser(clap::value_parser!(u32))
                .default_value("5"))
            .arg(Arg::new("hotspot-threshold")
                .short('t')
                .long("hotspot")
                .value_name("COUNT")
                .help("Minimum change count for hotspot detection")
                .value_parser(clap::value_parser!(u32))
                .default_value("5"))
            .arg(Arg::new("detailed")
                .long("detailed")
                .help("Include detailed metrics breakdown")
                .action(ArgAction::SetTrue))
    }
    
    async fn configure_from_matches(&mut self, matches: &clap::ArgMatches) -> PluginResult<()> {
        // Metrics plugin doesn't have complex configuration state to update
        // The arguments are handled during execution based on the function being called
        
        if let Some(threshold) = matches.get_one::<u32>("complexity-threshold") {
            log::debug!("Metrics plugin configured with complexity threshold: {}", threshold);
        }
        
        
        if matches.get_flag("exclude-tests") {
            log::debug!("Metrics plugin configured to exclude test files");
        }
        
        if let Some(dup_threshold) = matches.get_one::<u32>("duplication-threshold") {
            log::debug!("Metrics plugin configured with duplication threshold: {}", dup_threshold);
        }
        
        if let Some(hotspot_threshold) = matches.get_one::<u32>("hotspot-threshold") {
            log::debug!("Metrics plugin configured with hotspot threshold: {}", hotspot_threshold);
        }
        
        if matches.get_flag("detailed") {
            log::debug!("Metrics plugin configured for detailed analysis");
        }
        
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_plugin_creation() {
        let plugin = MetricsPlugin::new();
        assert_eq!(plugin.info.name, "metrics");
        assert!(!plugin.initialized);
    }

    #[tokio::test]
    async fn test_metrics_plugin_comprehensive_processors() {
        let plugin = MetricsPlugin::new();
        let processors = plugin.create_processors();
        
        // Should create all comprehensive processors for the given modes
        assert!(processors.len() >= 6); // All 6 comprehensive processors
        
        let processor_names: Vec<&str> = processors.iter().map(|p| p.name()).collect();
        
        // Verify we're using the comprehensive processors (they use lowercase names)
        assert!(processor_names.contains(&"change_frequency"));
        assert!(processor_names.contains(&"complexity"));
        assert!(processor_names.contains(&"hotspot"));
        assert!(processor_names.contains(&"debt_assessment"));
        assert!(processor_names.contains(&"format_detection"));
        assert!(processor_names.contains(&"duplication_detector"));
    }

    #[tokio::test]
    async fn test_metrics_plugin_processors() {
        let plugin = MetricsPlugin::new();
        let processors = plugin.create_processors();
        
        // Should create all processors for the given modes
        assert!(processors.len() >= 5); // change_frequency, complexity, hotspot, debt_assessment, format_detection, duplication_detector
        
        let processor_names: Vec<&str> = processors.iter().map(|p| p.name()).collect();
        assert!(processor_names.contains(&"change_frequency"));
        assert!(processor_names.contains(&"complexity"));
        assert!(processor_names.contains(&"hotspot"));
        assert!(processor_names.contains(&"debt_assessment"));
        assert!(processor_names.contains(&"format_detection"));
        assert!(processor_names.contains(&"duplication_detector"));
    }
}
