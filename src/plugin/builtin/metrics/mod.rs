//! Code Metrics Plugin
//! 
//! Built-in plugin for analyzing code quality metrics and statistics.
//! This plugin uses comprehensive EventProcessor implementations from the
//! plugin processors module for advanced analysis capabilities.

use crate::plugin::{
    Plugin, PluginInfo, PluginContext, PluginRequest, PluginResponse,
    PluginResult, PluginError, traits::{PluginType, PluginDataRequirements, ConsumerPlugin, ConsumerPreferences}
};
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
