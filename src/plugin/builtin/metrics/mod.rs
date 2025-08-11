//! Code Metrics Plugin
//! 
//! Built-in plugin for analyzing code quality metrics and statistics.
//! This plugin uses comprehensive EventProcessor implementations from the
//! plugin processors module for advanced analysis capabilities.

use crate::plugin::{
    Plugin, ScannerPlugin, PluginInfo, PluginContext, PluginRequest, PluginResponse,
    PluginResult, PluginError, traits::PluginType
};
use crate::scanner::{modes::ScanMode, messages::{ScanMessage, MessageHeader}};
use crate::scanner::async_engine::processors::{EventProcessor, EventProcessingCoordinator};
use crate::scanner::async_engine::shared_state::SharedProcessorState;
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
use std::sync::RwLock;
use log::debug;

/// Code Metrics Plugin using comprehensive event-driven processors
pub struct MetricsPlugin {
    info: PluginInfo,
    initialized: bool,
    processor_coordinator: Option<EventProcessingCoordinator>,
    results: RwLock<HashMap<String, serde_json::Value>>,
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
            results: RwLock::new(HashMap::new()),
        }
    }

    fn get_required_scan_modes(&self) -> ScanMode {
        ScanMode::CHANGE_FREQUENCY | ScanMode::FILES | ScanMode::METRICS
    }

    fn create_processors(&self, scan_modes: ScanMode) -> Vec<Box<dyn EventProcessor>> {
        let mut processors: Vec<Box<dyn EventProcessor>> = Vec::new();

        if scan_modes.contains(ScanMode::CHANGE_FREQUENCY) {
            processors.push(Box::new(ComprehensiveChangeFrequencyProcessor::new()));
        }

        if scan_modes.contains(ScanMode::METRICS) {
            processors.push(Box::new(ComprehensiveComplexityProcessor::new()));
            processors.push(Box::new(ComprehensiveHotspotProcessor::new()));
            processors.push(Box::new(ComprehensiveDebtAssessmentProcessor::new()));
        }

        if scan_modes.contains(ScanMode::FILES) {
            processors.push(Box::new(ComprehensiveFormatDetectionProcessor::new()));
            processors.push(Box::new(ComprehensiveDuplicationDetectorProcessor::new()));
        }

        processors
    }
}

impl Default for MetricsPlugin {
    fn default() -> Self {
        Self::new()
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
        let scan_modes = self.get_required_scan_modes();
        let processors = self.create_processors(scan_modes);
        
        for processor in processors {
            coordinator.add_processor(processor);
        }

        coordinator.initialize().await?;
        self.processor_coordinator = Some(coordinator);
        self.initialized = true;

        Ok(())
    }

    async fn execute(&self, _request: PluginRequest) -> PluginResult<PluginResponse> {
        // TODO: Implement plugin execution
        Err(PluginError::execution_failed("Not implemented yet"))
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        self.processor_coordinator = None;
        self.initialized = false;
        Ok(())
    }
}

#[async_trait]
impl ScannerPlugin for MetricsPlugin {
    fn supported_modes(&self) -> ScanMode {
        self.get_required_scan_modes()
    }

    async fn process_scan_data(&self, _data: &ScanMessage) -> PluginResult<Vec<ScanMessage>> {
        // TODO: Process scan data through processors
        Ok(vec![])
    }

    async fn aggregate_results(&self, _results: Vec<ScanMessage>) -> PluginResult<ScanMessage> {
        // TODO: Aggregate results from processors
        let header = MessageHeader::new(ScanMode::METRICS, 0);
        let data = crate::scanner::messages::MessageData::MetricInfo {
            file_count: 0,
            line_count: 0,
            complexity: 0.0,
        };
        Ok(ScanMessage::new(header, data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::async_engine::events::{RepositoryEvent, CommitInfo, FileChangeData, ChangeType};
    use std::time::SystemTime;

    #[tokio::test]
    async fn test_metrics_plugin_creation() {
        let plugin = MetricsPlugin::new();
        assert_eq!(plugin.info.name, "metrics");
        assert!(!plugin.initialized);
    }

    #[tokio::test]
    async fn test_metrics_plugin_comprehensive_processors() {
        let plugin = MetricsPlugin::new();
        let processors = plugin.create_processors(ScanMode::CHANGE_FREQUENCY | ScanMode::METRICS | ScanMode::FILES);
        
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
        let processors = plugin.create_processors(ScanMode::CHANGE_FREQUENCY | ScanMode::METRICS | ScanMode::FILES);
        
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
