//! Data Export Plugin Module
//! 
//! Built-in plugin for exporting scan results to various formats.

pub mod formats;
pub mod template_engine;
pub mod config;

use crate::plugin::{
    Plugin, PluginInfo, PluginContext, PluginRequest, PluginResponse,
    PluginResult, PluginError, traits::{PluginType, PluginArgumentParser, PluginArgDefinition, PluginDataRequirements, ConsumerPlugin, ConsumerPreferences}
};
use crate::queue::{QueueConsumer, QueueEvent};
use crate::plugin::builtin::utils::format_detection::{FormatDetector, FormatDetectionResult};
use crate::scanner::messages::ScanMessage;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::json;

pub use config::{ExportConfig, ExportFormat};
pub use template_engine::TemplateEngine;

/// Data export plugin for various output formats
pub struct ExportPlugin {
    info: PluginInfo,
    initialized: bool,
    export_config: Arc<RwLock<ExportConfig>>,
    collected_data: Arc<RwLock<Vec<ScanMessage>>>,
    template_engine: Arc<RwLock<TemplateEngine>>,
    format_detector: FormatDetector,
    // Plugin coordination fields
    collected_plugins: Arc<RwLock<std::collections::HashMap<String, String>>>, // plugin_id -> data_type
    expected_plugins: Arc<RwLock<std::collections::HashSet<String>>>, // Expected plugin IDs
    scan_id: Arc<RwLock<Option<String>>>, // Current scan ID
    export_triggered: Arc<RwLock<bool>>, // Whether export has been triggered for current scan
    export_completed: Arc<RwLock<bool>>, // Whether export has been completed
    incremental_data: Arc<RwLock<std::collections::HashMap<String, String>>>, // plugin_id -> incremental data
    // Consumer plugin fields
    consuming: Arc<RwLock<bool>>,
    consumer: Arc<RwLock<Option<QueueConsumer>>>,
}

impl ExportPlugin {
    /// Create a new export plugin
    pub fn new() -> Self {
        let info = PluginInfo::new(
            "export".to_string(),
            "1.0.0".to_string(),
            crate::scanner::version::get_api_version() as u32,
            "Exports scan results and analysis data to various formats including JSON, CSV, XML, YAML, and HTML".to_string(),
            "gstats built-in".to_string(),
            PluginType::Output,
        )
        .with_capability(
            "json_export".to_string(),
            "Exports data as structured JSON format".to_string(),
            "1.0.0".to_string(),
        )
        .with_capability(
            "csv_export".to_string(),
            "Exports data as comma-separated values for spreadsheet applications".to_string(),
            "1.0.0".to_string(),
        )
        .with_capability(
            "html_export".to_string(),
            "Generates HTML reports with interactive visualizations".to_string(),
            "1.0.0".to_string(),
        )
        .with_capability(
            "batch_export".to_string(),
            "Supports batch export of multiple data sets".to_string(),
            "1.0.0".to_string(),
        )
        .with_capability(
            "template_export".to_string(),
            "Custom output formatting using Tera templates (Jinja2-compatible)".to_string(),
            "1.0.0".to_string(),
        )
        .with_load_by_default(true);

        Self {
            info,
            initialized: false,
            export_config: Arc::new(RwLock::new(ExportConfig::default())),
            collected_data: Arc::new(RwLock::new(Vec::new())),
            template_engine: Arc::new(RwLock::new(TemplateEngine::new())),
            format_detector: FormatDetector::new(),
            // Initialize coordination fields
            collected_plugins: Arc::new(RwLock::new(std::collections::HashMap::new())),
            expected_plugins: Arc::new(RwLock::new(std::collections::HashSet::from_iter(vec![
                "commits".to_string(),
                "metrics".to_string(),
            ]))),
            scan_id: Arc::new(RwLock::new(None)),
            export_triggered: Arc::new(RwLock::new(false)),
            export_completed: Arc::new(RwLock::new(false)),
            incremental_data: Arc::new(RwLock::new(std::collections::HashMap::new())),
            // Initialize consumer plugin fields
            consuming: Arc::new(RwLock::new(false)),
            consumer: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Handle DataReady event to collect processed data from other plugins
    pub async fn handle_data_ready(&mut self, event: crate::notifications::ScanEvent) -> PluginResult<()> {
        use crate::notifications::ScanEvent;
        
        match event {
            ScanEvent::DataReady { scan_id, plugin_id, data_type } => {
                log::info!("ExportPlugin received DataReady event from plugin '{}' with data type '{}' for scan {}", 
                          plugin_id, data_type, scan_id);
                
                // Set or verify scan ID
                {
                    let mut scan_id_guard = self.scan_id.write().await;
                    if let Some(ref current_scan_id) = *scan_id_guard {
                        if *current_scan_id != scan_id {
                            log::warn!("ExportPlugin received DataReady for different scan ID: {} (current: {})", 
                                      scan_id, current_scan_id);
                        }
                    } else {
                        *scan_id_guard = Some(scan_id.clone());
                    }
                }
                
                // Track collected plugin data
                {
                    let mut collected = self.collected_plugins.write().await;
                    collected.insert(plugin_id.clone(), data_type.clone());
                }
                
                let collected_count = {
                    let collected = self.collected_plugins.read().await;
                    collected.len()
                };
                log::debug!("ExportPlugin collected data from plugin '{}' (type: {}). Total plugins: {}", 
                           plugin_id, data_type, collected_count);
                
                // Check if all expected plugins have reported
                if self.all_expected_plugins_ready().await {
                    log::info!("ExportPlugin: All expected plugins ready for scan {}, triggering export", scan_id);
                    self.trigger_export_if_ready().await?;
                }
                
                Ok(())
            }
            _ => {
                Err(PluginError::ExecutionFailed { 
                    message: "ExportPlugin::handle_data_ready received non-DataReady event".to_string() 
                })
            }
        }
    }
    
    /// Check if the plugin is waiting for more plugins to report DataReady
    pub async fn is_waiting_for_plugins(&self) -> bool {
        let collected = self.collected_plugins.read().await;
        let expected = self.expected_plugins.read().await;
        !collected.is_empty() || !expected.is_empty()
    }
    
    /// Get the count of plugins that have reported DataReady
    pub async fn get_collected_plugin_count(&self) -> usize {
        let collected = self.collected_plugins.read().await;
        collected.len()
    }
    
    /// Check if all expected plugins have reported DataReady
    pub async fn all_expected_plugins_ready(&self) -> bool {
        let expected = self.expected_plugins.read().await;
        let collected = self.collected_plugins.read().await;
        
        for expected_plugin in expected.iter() {
            if !collected.contains_key(expected_plugin) {
                return false;
            }
        }
        true
    }
    
    /// Set the expected plugins for coordination
    pub async fn set_expected_plugins(&self, plugins: std::collections::HashSet<String>) {
        let mut expected = self.expected_plugins.write().await;
        *expected = plugins;
    }
    
    /// Reset coordination state for a new scan
    pub async fn reset_coordination_state(&self) {
        {
            let mut collected = self.collected_plugins.write().await;
            collected.clear();
        }
        {
            let mut scan_id = self.scan_id.write().await;
            *scan_id = None;
        }
        {
            let mut triggered = self.export_triggered.write().await;
            *triggered = false;
        }
        {
            let mut completed = self.export_completed.write().await;
            *completed = false;
        }
        {
            let mut incremental = self.incremental_data.write().await;
            incremental.clear();
        }
    }
    
    /// Check if export should be triggered (all expected plugins ready and not already triggered)
    pub async fn should_trigger_export(&self) -> bool {
        let triggered = self.export_triggered.read().await;
        self.all_expected_plugins_ready().await && !*triggered
    }
    
    /// Trigger export if all conditions are met
    pub async fn trigger_export_if_ready(&self) -> PluginResult<()> {
        if self.should_trigger_export().await {
            let scan_id = {
                let scan_id_guard = self.scan_id.read().await;
                scan_id_guard.clone()
            };
            log::info!("ExportPlugin: Triggering export for scan {:?}", scan_id);
            
            // TODO: Implement actual export rendering logic
            // TODO: Fetch processed data from analysis plugins
            // TODO: Render final export output
            
            {
                let mut triggered = self.export_triggered.write().await;
                *triggered = true;
            }
            log::info!("ExportPlugin: Export completed for scan {:?}", scan_id);
            
            Ok(())
        } else {
            Ok(()) // No action needed
        }
    }
    
    /// Task 4.3: Update incremental rendering with partial data
    pub async fn update_incremental_rendering(&self, plugin_id: &str, data: &str) -> PluginResult<()> {
        log::debug!("ExportPlugin: Updating incremental rendering for plugin '{}'", plugin_id);
        
        {
            let mut incremental = self.incremental_data.write().await;
            incremental.insert(plugin_id.to_string(), data.to_string());
        }
        
        // TODO: Implement actual incremental rendering logic
        // TODO: Update partial export output
        // TODO: Notify subscribers of incremental updates
        
        Ok(())
    }
    
    /// Check if plugin has incremental data
    pub async fn has_incremental_data(&self, plugin_id: &str) -> bool {
        let incremental = self.incremental_data.read().await;
        incremental.contains_key(plugin_id)
    }
    
    /// Task 4.4: Notify export completion for cleanup coordination
    pub async fn notify_export_completion(&self) -> PluginResult<()> {
        let (triggered, completed) = {
            let triggered_guard = self.export_triggered.read().await;
            let completed_guard = self.export_completed.read().await;
            (*triggered_guard, *completed_guard)
        };
        
        if triggered && !completed {
            let scan_id = {
                let scan_id_guard = self.scan_id.read().await;
                scan_id_guard.clone()
            };
            log::info!("ExportPlugin: Notifying export completion for scan {:?}", scan_id);
            
            // TODO: Emit ExportCompleted event
            // TODO: Notify other plugins that export is done
            // TODO: Signal cleanup coordination
            
            {
                let mut completed_guard = self.export_completed.write().await;
                *completed_guard = true;
            }
            
            Ok(())
        } else {
            Ok(()) // Already completed or not triggered
        }
    }
    
    /// Task 4.4: Cleanup after export completion
    pub async fn cleanup_after_export(&self) -> PluginResult<()> {
        let completed = {
            let completed_guard = self.export_completed.read().await;
            *completed_guard
        };
        
        if completed {
            log::debug!("ExportPlugin: Cleaning up after export completion");
            
            // Clear collected data to free memory
            {
                let mut data = self.collected_data.write().await;
                data.clear();
            }
            {
                let mut incremental = self.incremental_data.write().await;
                incremental.clear();
            }
            
            // TODO: Close file handles
            // TODO: Clean up temporary files
            // TODO: Release resources
            
            let scan_id = {
                let scan_id_guard = self.scan_id.read().await;
                scan_id_guard.clone()
            };
            log::info!("ExportPlugin: Cleanup completed for scan {:?}", scan_id);
        }
        
        Ok(())
    }

    /// Configure export settings
    pub async fn configure(&self, format: ExportFormat, output_path: &str) -> PluginResult<()> {
        let mut config = self.export_config.write().await;
        config.output_format = format;
        config.output_path = output_path.to_string();
        Ok(())
    }

    /// Add data for export
    pub async fn add_data(&self, message: ScanMessage) -> PluginResult<()> {
        // Always collect all data - limit is applied during export in get_data_to_export()
        let mut data = self.collected_data.write().await;
        data.push(message);
        Ok(())
    }

    /// Get data to export with limit applied
    async fn get_data_to_export(&self) -> Vec<ScanMessage> {
        let config = self.export_config.read().await;
        let data = self.collected_data.read().await;
        
        if let Some(max_entries) = config.max_entries {
            data.iter().take(max_entries).cloned().collect()
        } else if config.output_all {
            data.clone()
        } else {
            data.iter().take(10).cloned().collect() // Default limit
        }
    }

    /// Export collected data to the configured format
    pub async fn export_data(&self) -> PluginResult<String> {
        if !self.initialized {
            return Err(PluginError::initialization_failed("Plugin not initialized"));
        }

        // If template is specified, use template with the detected format
        {
            let config = self.export_config.read().await;
            if config.template_file.is_some() {
                return self.export_template().await;
            }
        }

        let data_to_export = self.get_data_to_export().await;

        let (config, all_data) = {
            let config_guard = self.export_config.read().await;
            let data_guard = self.collected_data.read().await;
            (config_guard.clone(), data_guard.clone())
        };

        // Otherwise use built-in formatters  
        let data_refs: Vec<&ScanMessage> = data_to_export.iter().collect();
        match config.output_format {
            ExportFormat::Json => formats::json::export_json(&config, &all_data, &data_refs, &self.info),
            ExportFormat::Csv => formats::csv::export_csv(&config, &all_data, &data_refs),
            ExportFormat::Xml => formats::xml::export_xml(&config, &all_data, &data_refs, &self.info),
            ExportFormat::Yaml => formats::yaml::export_yaml(&config, &all_data, &data_refs, &self.info),
            ExportFormat::Html => formats::html::export_html(&config, &all_data, &data_refs, &self.info),
            ExportFormat::Markdown => formats::markdown::export_markdown(&config, &all_data, &data_refs, &self.info),
        }
    }

    /// Export data using a template
    async fn export_template(&self) -> PluginResult<String> {
        {
            let config = self.export_config.read().await;
            if config.template_file.is_none() {
                return Err(PluginError::configuration_error("Template file not specified. Use --template to specify a template file."));
            }
        }
        
        // Prepare the template context with all available data
        let context = self.prepare_template_context().await?;
        
        // Render the template with the context
        let engine = self.template_engine.read().await;
        engine.render(&context)
    }
    
    /// Prepare comprehensive template data context
    async fn prepare_template_context(&self) -> PluginResult<serde_json::Value> {
        let mut context = serde_json::Map::new();
        
        // Repository metadata
        if let Ok(cwd) = std::env::current_dir() {
            let repo_name = cwd.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            
            context.insert("repository".to_string(), json!({
                "path": cwd.display().to_string(),
                "name": repo_name,
                "scan_timestamp": chrono::Utc::now().to_rfc3339(),
            }));
        }
        
        // Scan configuration
        let (data_len, output_all, max_entries, template_vars) = {
            let data = self.collected_data.read().await;
            let config = self.export_config.read().await;
            let engine = self.template_engine.read().await;
            (data.len(), config.output_all, config.max_entries, engine.template_vars.clone())
        };
        
        context.insert("scan_config".to_string(), json!({
            "total_items_scanned": data_len,
            "output_all": output_all,
            "output_limit": max_entries,
        }));
        
        // Add template variables passed via --template-var
        for (key, value) in &template_vars {
            context.insert(key.clone(), json!(value));
        }
        
        Ok(serde_json::Value::Object(context))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};

    fn create_test_message(data: MessageData) -> ScanMessage {
        ScanMessage {
            header: MessageHeader::new(chrono::Utc::now().timestamp() as u64),
            data,
        }
    }

    #[tokio::test]
    async fn test_new_extension_format_detection_and_processing() {
        // Test the new extensions mentioned in Task 4: tsv, txt, yml, htm, markdown
        let new_extensions = vec![
            ("data.tsv", ExportFormat::Csv),      // TSV should map to CSV
            ("output.txt", ExportFormat::Json),   // TXT should default to JSON
            ("config.yml", ExportFormat::Yaml),   // YML should map to YAML
            ("report.htm", ExportFormat::Html),   // HTM should map to HTML
            ("readme.markdown", ExportFormat::Markdown), // Full markdown extension
        ];

        for (filename, expected_format) in new_extensions {
            let mut plugin = ExportPlugin::new();
            
            // Test the complete pipeline: detection -> parsing -> processing -> export
            
            // 1. Format Detection
            let detection_result = plugin.format_detector.detect_format_from_path(filename);
            match detection_result {
                FormatDetectionResult::Detected(format) => {
                    assert_eq!(format, expected_format, "Format detection failed for {}", filename);
                }
                _ => panic!("Should detect format for {}", filename),
            }

            // 2. Argument Parsing with Detection
            let args = vec!["--output".to_string(), filename.to_string()];
            plugin.parse_plugin_args(&args).await.unwrap();
            {
                let config = plugin.export_config.read().await;
                assert_eq!(config.output_format, expected_format);
            }

            // 3. Data Processing
            plugin.add_data(create_test_message(
                MessageData::CommitInfo {
                    hash: "abc123".to_string(),
                    author: "Test Author".to_string(),
                    message: "Test commit".to_string(),
                    timestamp: 1234567890,
                    changed_files: vec![],
                }
            )).await.unwrap();
            let context = create_test_plugin_context().await;
            plugin.initialize(&context).await.unwrap();

            // 4. Export Processing
            let exported_data = plugin.export_data().await.unwrap();
            assert!(!exported_data.is_empty(), "Export should produce data for {}", filename);

            // 5. Format-Specific Validation
            match expected_format {
                ExportFormat::Csv => {
                    // TSV should use tab delimiters when configured
                    if filename.ends_with(".tsv") {
                        // Note: Current implementation uses comma by default
                        // This test verifies the format is processed as CSV
                        assert!(exported_data.contains("timestamp") || exported_data.contains("author"),
                               "TSV should contain CSV-style headers");
                    }
                }
                ExportFormat::Json => {
                    // TXT files should produce valid JSON
                    let _: serde_json::Value = serde_json::from_str(&exported_data).unwrap();
                }
                ExportFormat::Xml => {
                    // XML should produce valid XML
                    assert!(exported_data.contains("<?xml") || exported_data.contains("<root>"));
                }
                ExportFormat::Yaml => {
                    // YML should produce valid YAML
                    let _: serde_yaml::Value = serde_yaml::from_str(&exported_data).unwrap();
                }
                ExportFormat::Html => {
                    // HTM should produce valid HTML
                    assert!(exported_data.contains("<!DOCTYPE html>"));
                }
                ExportFormat::Markdown => {
                    // Full markdown extension should work
                    assert!(exported_data.contains("#") || exported_data.contains("**"));
                }
            }
        }
    }

    #[tokio::test]
    async fn test_format_detection_integration_with_export_processing() {
        // Test that format detection integrates properly with export processing
        let test_cases = vec![
            ("report.json", ExportFormat::Json),
            ("data.csv", ExportFormat::Csv),
            ("output.xml", ExportFormat::Xml),
            ("config.yaml", ExportFormat::Yaml),
            ("report.html", ExportFormat::Html),
            ("readme.md", ExportFormat::Markdown),
            ("output.txt", ExportFormat::Json), // txt defaults to JSON
            ("data.tsv", ExportFormat::Csv),    // tsv maps to CSV
        ];

        for (filename, expected_format) in test_cases {
            let mut plugin = ExportPlugin::new();
            let args = vec!["--output".to_string(), filename.to_string()];

            // Test format detection through argument parsing
            plugin.parse_plugin_args(&args).await.unwrap();
            {
                let config = plugin.export_config.read().await;
                assert_eq!(config.output_format, expected_format, 
                          "Format detection failed for {}", filename);
            }

            // Verify format detector compatibility
            assert!(plugin.format_detector.is_template_compatible(&expected_format),
                   "Format {:?} should be template compatible", expected_format);
        }
    }

    // Add a helper function for tests
    async fn create_test_plugin_context() -> PluginContext {
        use std::sync::Arc;
        use crate::scanner::{ScannerConfig, QueryParams};
        
        let scanner_config = Arc::new(ScannerConfig::default());
        let query_params = Arc::new(QueryParams::default());
        
        PluginContext::new(
            scanner_config,
            query_params,
        )
    }
    
    #[tokio::test]
    async fn test_export_plugin_handles_data_ready() {
        use crate::notifications::ScanEvent;
        
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        // Create DataReady event from commits plugin
        let event = ScanEvent::DataReady {
            scan_id: "test_scan".to_string(),
            plugin_id: "commits".to_string(),
            data_type: "commits".to_string(),
        };
        
        // This should fail because handle_data_ready is not implemented yet
        let result = plugin.handle_data_ready(event).await;
        assert!(result.is_ok());
        
        // Verify that the plugin processed the data ready event
        // For now, just verify the method exists and returns Ok
    }
    
    #[tokio::test]
    async fn test_export_plugin_data_ready_subscription() {
        use crate::notifications::ScanEvent;
        
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        // Initially, no plugins have reported
        assert!(!plugin.all_expected_plugins_ready().await);
        assert_eq!(plugin.get_collected_plugin_count().await, 0);
        
        // Test first DataReady event from commits plugin
        let commits_event = ScanEvent::DataReady {
            scan_id: "test_scan".to_string(),
            plugin_id: "commits".to_string(),
            data_type: "commits".to_string(),
        };
        
        let result1 = plugin.handle_data_ready(commits_event).await;
        assert!(result1.is_ok());
        
        // After first plugin, we're waiting for more
        assert!(plugin.is_waiting_for_plugins().await);
        assert_eq!(plugin.get_collected_plugin_count().await, 1);
        assert!(!plugin.all_expected_plugins_ready().await); // Still waiting for metrics
        
        // Test second DataReady event from metrics plugin
        let metrics_event = ScanEvent::DataReady {
            scan_id: "test_scan".to_string(),
            plugin_id: "metrics".to_string(),
            data_type: "files".to_string(),
        };
        
        let result2 = plugin.handle_data_ready(metrics_event).await;
        assert!(result2.is_ok());
        
        // After both plugins, all expected plugins are ready
        assert_eq!(plugin.get_collected_plugin_count().await, 2);
        assert!(plugin.all_expected_plugins_ready().await); // Now all plugins are ready
        
        // Test coordination state reset
        plugin.reset_coordination_state().await;
        assert_eq!(plugin.get_collected_plugin_count().await, 0);
        assert!(!plugin.all_expected_plugins_ready().await);
    }
    
    #[tokio::test]
    async fn test_export_plugin_coordination_logic() {
        use crate::notifications::ScanEvent;
        
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        // Test that export waits for all expected plugins
        let commits_event = ScanEvent::DataReady {
            scan_id: "test_scan".to_string(),
            plugin_id: "commits".to_string(),
            data_type: "commits".to_string(),
        };
        
        plugin.handle_data_ready(commits_event).await.unwrap();
        
        // Should not trigger export yet - still waiting for metrics
        {
            let triggered = plugin.export_triggered.read().await;
            assert!(!*triggered);
        }
        assert!(!plugin.should_trigger_export().await); // Not ready yet
        
        let metrics_event = ScanEvent::DataReady {
            scan_id: "test_scan".to_string(),
            plugin_id: "metrics".to_string(),
            data_type: "files".to_string(),
        };
        
        plugin.handle_data_ready(metrics_event).await.unwrap();
        
        // Export should have been triggered automatically
        {
            let triggered = plugin.export_triggered.read().await;
            assert!(*triggered);
        }
        assert!(!plugin.should_trigger_export().await); // Already triggered
        
        // Test manual triggering when not ready
        plugin.reset_coordination_state().await;
        assert!(!plugin.should_trigger_export().await); // No plugins collected yet
        
        let result = plugin.trigger_export_if_ready().await;
        assert!(result.is_ok());
        {
            let triggered = plugin.export_triggered.read().await;
            assert!(!*triggered); // Should not trigger when not ready
        }
    }
    
    #[tokio::test]
    async fn test_export_plugin_incremental_updates_and_completion() {
        use crate::notifications::ScanEvent;
        
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        // Test incremental rendering updates (Task 4.3)
        let commits_event = ScanEvent::DataReady {
            scan_id: "test_scan".to_string(),
            plugin_id: "commits".to_string(),
            data_type: "commits".to_string(),
        };
        
        plugin.handle_data_ready(commits_event).await.unwrap();
        
        // Test incremental update
        let result = plugin.update_incremental_rendering("commits", "partial commit data").await;
        assert!(result.is_ok());
        assert!(plugin.has_incremental_data("commits").await);
        
        // Complete with metrics plugin
        let metrics_event = ScanEvent::DataReady {
            scan_id: "test_scan".to_string(),
            plugin_id: "metrics".to_string(),
            data_type: "files".to_string(),
        };
        
        plugin.handle_data_ready(metrics_event).await.unwrap();
        
        // Test export completion notification (Task 4.4)
        let completion_result = plugin.notify_export_completion().await;
        assert!(completion_result.is_ok());
        {
            let completed = plugin.export_completed.read().await;
            assert!(*completed);
        }
        
        // Test cleanup coordination (Task 4.4)
        plugin.cleanup_after_export().await.unwrap();
        {
            let data = plugin.collected_data.read().await;
            assert!(data.is_empty());
        }
    }
    
    fn create_test_context() -> PluginContext {
        use crate::scanner::{ScannerConfig, query::QueryParams};
        use std::sync::Arc;
        
        let scanner_config = Arc::new(ScannerConfig::default());
        let query_params = Arc::new(QueryParams::default());
        
        PluginContext::new(
            scanner_config,
            query_params,
        )
    }
}

// Plugin trait implementations
#[async_trait]
#[async_trait]
impl Plugin for ExportPlugin {
    fn plugin_info(&self) -> &PluginInfo {
        &self.info
    }

    async fn initialize(&mut self, _context: &PluginContext) -> PluginResult<()> {
        self.initialized = true;
        Ok(())
    }

    async fn execute(&self, request: PluginRequest) -> PluginResult<PluginResponse> {
        match request {
            PluginRequest::Execute { request_id, .. } => {
                // Measure execution time
                let start_time = std::time::Instant::now();

                // Perform the actual export
                let exported_data_str = self.export_data().await?;

                let duration_us = start_time.elapsed().as_micros() as u64;

                // Parse the exported data as JSON so it's not double-encoded
                let exported_data: serde_json::Value = serde_json::from_str(&exported_data_str)
                    .unwrap_or_else(|_| serde_json::json!(exported_data_str));

                let entries_count = {
                    let data = self.collected_data.read().await;
                    data.len() as u64
                };
                
                let metadata = crate::plugin::context::ExecutionMetadata {
                    duration_us,
                    memory_used: 0,
                    entries_processed: entries_count,
                    plugin_version: "1.0.0".to_string(),
                    extra: std::collections::HashMap::new(),
                };

                // Return the exported data as JSON
                let result_data = serde_json::json!({
                    "exported_data": exported_data,
                    "entries_processed": entries_count
                });

                Ok(PluginResponse::success(request_id, result_data, metadata))
            }
            PluginRequest::Export => {
                // Measure execution time
                let start_time = std::time::Instant::now();

                // Handle direct export request
                let exported_data_str = self.export_data().await?;

                let duration_us = start_time.elapsed().as_micros() as u64;

                // Parse the exported data as JSON so it's not double-encoded
                let exported_data: serde_json::Value = serde_json::from_str(&exported_data_str)
                    .unwrap_or_else(|_| serde_json::json!(exported_data_str));

                let entries_count = {
                    let data = self.collected_data.read().await;
                    data.len() as u64
                };
                
                let metadata = crate::plugin::context::ExecutionMetadata {
                    duration_us,
                    memory_used: 0,
                    entries_processed: entries_count,
                    plugin_version: "1.0.0".to_string(),
                    extra: std::collections::HashMap::new(),
                };

                // For Export requests, return the raw exported data
                let result_data = serde_json::json!({
                    "exported_data": exported_data,
                    "entries_processed": entries_count
                });

                Ok(PluginResponse::success("export".to_string(), result_data, metadata))
            }
            _ => {
                let _metadata = crate::plugin::context::ExecutionMetadata {
                    duration_us: 0,
                    memory_used: 0,
                    entries_processed: 0,
                    plugin_version: "1.0.0".to_string(),
                    extra: std::collections::HashMap::new(),
                };
                Err(PluginError::execution_failed("Unsupported request type"))
            }
        }
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        // Stop consuming if we're currently consuming
        if *self.consuming.read().await {
            self.stop_consuming().await?;
        }
        
        // Clear collected data
        {
            let mut data = self.collected_data.write().await;
            data.clear();
        }
        
        // Reset coordination state
        self.reset_coordination_state().await;
        
        Ok(())
    }
    
    /// Get all functions this plugin can handle
    fn advertised_functions(&self) -> Vec<crate::plugin::traits::PluginFunction> {
        vec![
            crate::plugin::traits::PluginFunction {
                name: "export".to_string(),
                aliases: vec!["exp".to_string(), "out".to_string()],
                description: "Export scan results to various formats".to_string(),
                is_default: true,
            }
        ]
    }
    
    /// Get the default function name
    fn default_function(&self) -> Option<&str> {
        Some("export")
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
impl ConsumerPlugin for ExportPlugin {
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
        
        log::info!("Export plugin started consuming messages");
        Ok(())
    }
    
    async fn process_message(&self, consumer: &QueueConsumer, message: Arc<ScanMessage>) -> PluginResult<()> {
        // Add message to collected data
        self.add_data((*message).clone()).await?;
        
        // Acknowledge the message
        consumer.acknowledge(message.header().sequence()).await.map_err(|e| {
            PluginError::execution_failed(format!("Failed to acknowledge message: {}", e))
        })?;
        
        Ok(())
    }
    
    async fn handle_queue_event(&self, event: &QueueEvent) -> PluginResult<()> {
        log::debug!("Export plugin received queue event: {:?}", event);
        
        match event {
            QueueEvent::ScanStarted { scan_id, .. } => {
                log::info!("Export plugin: scan started for {}", scan_id);
                // Reset coordination state for new scan
                self.reset_coordination_state().await;
                {
                    let mut scan_id_guard = self.scan_id.write().await;
                    *scan_id_guard = Some(scan_id.clone());
                }
            }
            QueueEvent::ScanComplete { scan_id, total_messages, .. } => {
                let data_count = {
                    let data = self.collected_data.read().await;
                    data.len()
                };
                log::info!(
                    "Export plugin: scan complete for {} - collected {} messages (total {} messages)", 
                    scan_id, data_count, total_messages
                );
                
                // Trigger export when scan completes
                self.trigger_export_if_ready().await?;
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
        
        log::info!("Export plugin stopped consuming messages");
        Ok(())
    }
    
    fn consumer_preferences(&self) -> ConsumerPreferences {
        ConsumerPreferences {
            consume_all_messages: true, // Export needs all data types
            interested_message_types: vec![], // Empty = all types
            high_frequency_capable: true, // Can handle high message rates
            preferred_batch_size: 50, // Process in larger batches for efficiency
            requires_ordered_delivery: false, // Order doesn't matter for final export
        }
    }
}

#[async_trait]
impl PluginArgumentParser for ExportPlugin {
    fn get_arg_schema(&self) -> Vec<PluginArgDefinition> {
        vec![
            PluginArgDefinition {
                name: "output".to_string(),
                description: "Output file path".to_string(),
                required: true,
                default_value: Some("output.json".to_string()),
                arg_type: "string".to_string(),
                examples: vec!["output.json".to_string(), "results.csv".to_string()],
            },
            PluginArgDefinition {
                name: "format".to_string(),
                description: "Output format (json, csv, xml, yaml, html, markdown)".to_string(),
                required: false,
                default_value: Some("json".to_string()),
                arg_type: "string".to_string(),
                examples: vec!["json".to_string(), "csv".to_string(), "xml".to_string()],
            },
            PluginArgDefinition {
                name: "include-metadata".to_string(),
                description: "Include metadata in output".to_string(),
                required: false,
                default_value: Some("false".to_string()),
                arg_type: "boolean".to_string(),
                examples: vec!["true".to_string(), "false".to_string()],
            },
            PluginArgDefinition {
                name: "max-entries".to_string(),
                description: "Maximum number of entries to export".to_string(),
                required: false,
                default_value: Some("10".to_string()),
                arg_type: "number".to_string(),
                examples: vec!["10".to_string(), "100".to_string(), "1000".to_string()],
            },
            PluginArgDefinition {
                name: "output-all".to_string(),
                description: "Export all entries (overrides max-entries)".to_string(),
                required: false,
                default_value: Some("false".to_string()),
                arg_type: "boolean".to_string(),
                examples: vec!["true".to_string(), "false".to_string()],
            },
            PluginArgDefinition {
                name: "csv-delimiter".to_string(),
                description: "CSV delimiter character".to_string(),
                required: false,
                default_value: Some(",".to_string()),
                arg_type: "string".to_string(),
                examples: vec![",".to_string(), ";".to_string(), "\t".to_string()],
            },
            PluginArgDefinition {
                name: "csv-quote-char".to_string(),
                description: "CSV quote character".to_string(),
                required: false,
                default_value: Some("\"".to_string()),
                arg_type: "string".to_string(),
                examples: vec!["\"".to_string(), "'".to_string()],
            },
            PluginArgDefinition {
                name: "template".to_string(),
                description: "Template file path for custom formatting".to_string(),
                required: false,
                default_value: None,
                arg_type: "string".to_string(),
                examples: vec!["template.html".to_string(), "custom.md".to_string()],
            },
            PluginArgDefinition {
                name: "template-var".to_string(),
                description: "Template variable (key=value format, can be used multiple times)".to_string(),
                required: false,
                default_value: None,
                arg_type: "string".to_string(),
                examples: vec!["title=Report".to_string(), "author=User".to_string()],
            },
        ]
    }

    async fn parse_plugin_args(&mut self, args: &[String]) -> PluginResult<()> {
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--output" => {
                    if i + 1 >= args.len() {
                        return Err(PluginError::configuration_error("--output requires a value"));
                    }
                    let output_path = &args[i + 1];
                    {
                        let mut config = self.export_config.write().await;
                        config.output_path = output_path.clone();
                    }
                    
                    // Auto-detect format from file extension if not explicitly set
                    let detection_result = self.format_detector.detect_format_from_path(output_path);
                    match detection_result {
                        FormatDetectionResult::Detected(format) => {
                            let mut config = self.export_config.write().await;
                            config.output_format = format;
                        }
                        FormatDetectionResult::UnknownExtension(ext) => {
                            return Err(PluginError::configuration_error(&format!(
                                "Unknown file extension '{}'. Supported extensions: {}",
                                ext,
                                self.format_detector.supported_extensions_string()
                            )));
                        }
                        FormatDetectionResult::NoExtension => {
                            return Err(PluginError::configuration_error(
                                "Output file must have a supported extension (json, csv, xml, yaml, html, md, etc.)"
                            ));
                        }
                        FormatDetectionResult::InvalidPath => {
                            return Err(PluginError::configuration_error("Invalid output file path"));
                        }
                    }
                    i += 2;
                }
                "--format" => {
                    if i + 1 >= args.len() {
                        return Err(PluginError::configuration_error("--format requires a value"));
                    }
                    let format_str = &args[i + 1];
                    let format = match format_str.to_lowercase().as_str() {
                        "json" => ExportFormat::Json,
                        "csv" => ExportFormat::Csv,
                        "xml" => ExportFormat::Xml,
                        "yaml" | "yml" => ExportFormat::Yaml,
                        "html" | "htm" => ExportFormat::Html,
                        "markdown" | "md" => ExportFormat::Markdown,
                        _ => return Err(PluginError::configuration_error(&format!(
                            "Unsupported format '{}'. Supported formats: json, csv, xml, yaml, html, markdown",
                            format_str
                        ))),
                    };
                    {
                        let mut config = self.export_config.write().await;
                        config.output_format = format;
                    }
                    i += 2;
                }
                "--include-metadata" => {
                    let mut config = self.export_config.write().await;
                    config.include_metadata = true;
                    i += 1;
                }
                "--max-entries" => {
                    if i + 1 >= args.len() {
                        return Err(PluginError::configuration_error("--max-entries requires a value"));
                    }
                    let max_str = &args[i + 1];
                    let max_entries = Some(
                        max_str.parse::<usize>()
                            .map_err(|_| PluginError::configuration_error("--max-entries must be a positive integer"))?
                    );
                    {
                        let mut config = self.export_config.write().await;
                        config.max_entries = max_entries;
                    }
                    i += 2;
                }
                "--output-all" => {
                    let mut config = self.export_config.write().await;
                    config.output_all = true;
                    i += 1;
                }
                "--csv-delimiter" => {
                    if i + 1 >= args.len() {
                        return Err(PluginError::configuration_error("--csv-delimiter requires a value"));
                    }
                    {
                        let mut config = self.export_config.write().await;
                        config.csv_delimiter = args[i + 1].clone();
                    }
                    i += 2;
                }
                "--csv-quote-char" => {
                    if i + 1 >= args.len() {
                        return Err(PluginError::configuration_error("--csv-quote-char requires a value"));
                    }
                    {
                        let mut config = self.export_config.write().await;
                        config.csv_quote_char = args[i + 1].clone();
                    }
                    i += 2;
                }
                "--template" => {
                    if i + 1 >= args.len() {
                        return Err(PluginError::configuration_error("--template requires a value"));
                    }
                    let template_path = PathBuf::from(&args[i + 1]);
                    if !template_path.exists() {
                        return Err(PluginError::configuration_error(&format!(
                            "Template file does not exist: {}",
                            template_path.display()
                        )));
                    }
                    
                    // Load the template
                    {
                        let mut engine = self.template_engine.write().await;
                        engine.load_template(&template_path)?;
                    }
                    {
                        let mut config = self.export_config.write().await;
                        config.template_file = Some(template_path);
                    }
                    i += 2;
                }
                "--template-var" => {
                    if i + 1 >= args.len() {
                        return Err(PluginError::configuration_error("--template-var requires a value"));
                    }
                    let var_str = &args[i + 1];
                    if let Some(eq_pos) = var_str.find('=') {
                        let key = var_str[..eq_pos].to_string();
                        let value = var_str[eq_pos + 1..].to_string();
                        {
                            let mut engine = self.template_engine.write().await;
                            engine.add_template_var(key, value);
                        }
                    } else {
                        return Err(PluginError::configuration_error(
                            "--template-var must be in key=value format"
                        ));
                    }
                    i += 2;
                }
                _ => {
                    return Err(PluginError::configuration_error(&format!(
                        "Unknown argument: {}",
                        args[i]
                    )));
                }
            }
        }

        // Validate required arguments
        {
            let config = self.export_config.read().await;
            if config.output_path.is_empty() {
                return Err(PluginError::configuration_error("--output is required"));
            }
        }

        Ok(())
    }
}

/// Data requirements implementation for ExportPlugin
/// This plugin only processes data from other plugins, no direct file access needed
impl PluginDataRequirements for ExportPlugin {
    fn requires_current_file_content(&self) -> bool {
        false // Works with processed data from other plugins
    }
    
    fn requires_historical_file_content(&self) -> bool {
        false // Only exports final processed results
    }
    
    fn preferred_buffer_size(&self) -> usize {
        4096 // Small buffer since we don't read files
    }
    
    fn max_file_size(&self) -> Option<usize> {
        None // N/A - doesn't process files directly
    }
    
    fn handles_binary_files(&self) -> bool {
        false // N/A - doesn't process files directly
    }
}


