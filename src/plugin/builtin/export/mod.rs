//! Data Export Plugin Module
//! 
//! Built-in plugin for exporting scan results to various formats.

pub mod template_engine;
pub mod config;
pub mod formats;

use crate::plugin::{
    Plugin, PluginInfo, PluginContext, PluginRequest, PluginResponse,
    PluginResult, PluginError, traits::{PluginType, PluginClapParser, PluginDataRequirements}
};
use crate::plugin::data_export::{PluginDataExport, DataPayload, ColumnType};
use crate::plugin::data_coordinator::DataCoordinator;
use crate::plugin::builtin::utils::format_detection::{FormatDetector, FormatDetectionResult};
use crate::notifications::events::PluginEvent;
use crate::notifications::traits::{Subscriber, NotificationManager, Publisher};
use crate::notifications::{NotificationResult, AsyncNotificationManager};
use crate::notifications::error::NotificationError;
use crate::display::ColourManager;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::json;

pub use config::{ExportConfig, ExportFormat};
pub use template_engine::TemplateEngine;

/// Data export plugin for various output formats
#[derive(Clone)]
pub struct ExportPlugin {
    /// Command name for clap integration
    command_name: String,
    
    /// Plugin settings (color preferences, etc.)
    settings: crate::plugin::PluginSettings,
    
    info: PluginInfo,
    initialized: bool,
    export_config: Arc<RwLock<ExportConfig>>,
    template_engine: Arc<RwLock<TemplateEngine>>,
    format_detector: FormatDetector,
    
    /// Data coordination
    data_coordinator: Arc<RwLock<DataCoordinator>>,
    
    /// Scan tracking and export state
    export_triggered: Arc<RwLock<bool>>,
    
    /// Notification manager for publishing events - REQUIRED for all plugins
    notification_manager: Arc<AsyncNotificationManager<PluginEvent>>,
    
    /// Color management
    colour_manager: Arc<RwLock<Option<Arc<ColourManager>>>>,
}

impl ExportPlugin {
    /// Create a new export plugin
    pub fn new() -> Self {
        let info = PluginInfo::new(
            "export".to_string(),
            "1.0.0".to_string(),
            crate::scanner::version::get_api_version() as u32,
            "Exports scan results and analysis data to various formats including JSON, CSV, XML, YAML, HTML, Markdown, and Templates".to_string(),
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
            "template_export".to_string(),
            "Custom output formatting using Tera templates (Jinja2-compatible)".to_string(),
            "1.0.0".to_string(),
        )
        .with_active_by_default(true);

        Self {
            command_name: "export".to_string(),
            settings: crate::plugin::PluginSettings::default(),
            info,
            initialized: false,
            export_config: Arc::new(RwLock::new(ExportConfig::default())),
            template_engine: Arc::new(RwLock::new(TemplateEngine::new())),
            format_detector: FormatDetector::new(),
            data_coordinator: Arc::new(RwLock::new(
                DataCoordinator::with_expected_plugins(vec![
                    "commits".to_string(),
                    "metrics".to_string(),
                ])
            )),
            export_triggered: Arc::new(RwLock::new(false)),
            notification_manager: Arc::new(AsyncNotificationManager::new()), // Temporary for deprecated constructor
            colour_manager: Arc::new(RwLock::new(None)),
        }
    }
    
    
    /// Create a new export plugin with all required dependencies (REQUIRED)
    /// This is the correct way to instantiate ExportPlugin - it MUST have notification manager
    pub fn with_dependencies(
        settings: crate::plugin::PluginSettings,
        notification_manager: Arc<AsyncNotificationManager<PluginEvent>>
    ) -> Self {
        let mut plugin = Self::new();
        plugin.settings = settings;
        plugin.notification_manager = notification_manager;
        plugin
    }
    
    
    /// Handle PluginEvent::DataReady - core export functionality
    async fn handle_data_ready_event(&self, 
        plugin_id: String, 
        scan_id: String, 
        export_data: Arc<PluginDataExport>
    ) -> PluginResult<()> {
        log::info!("ExportPlugin: Received DataReady from plugin '{}' for scan '{}'", plugin_id, scan_id);
        
        // Add the data to our coordinator
        {
            let mut coordinator = self.data_coordinator.write().await;
            coordinator.add_data(plugin_id.clone(), export_data.clone());
            
            // Check if we have all expected data
            if coordinator.is_complete() {
                log::info!("ExportPlugin: All expected data collected for scan '{}', triggering export", scan_id);
                
                // Collect all data
                let collected_data = coordinator.get_all_data();
                
                // Perform the export using the configured format
                let config = self.export_config.read().await;
                let formatted = self.format_data(&collected_data, &config).await?;
                
                // Output the formatted data
                if let Some(ref output_path) = config.output_file {
                    std::fs::write(output_path, &formatted)
                        .map_err(|e| PluginError::io_error(format!("Failed to write output file: {}", e)))?;
                    log::info!("Exported data to {}", output_path.display());
                } else {
                    println!("{}", formatted);
                }
                
                // Publish completion event using Publisher trait
                self.publish_export_completion_event(&scan_id, &plugin_id).await?;
                
                // Clear coordinator for next round
                coordinator.clear();
            } else {
                let pending = coordinator.get_pending_plugins();
                log::debug!("ExportPlugin: Still waiting for data from plugins: {:?}", pending);
            }
        }
        
        Ok(())
    }
    
    /// Handle other PluginEvent types
    async fn handle_other_plugin_event(&self, event: &PluginEvent) -> PluginResult<()> {
        match event {
            PluginEvent::PluginStarted { plugin_id, .. } => {
                log::debug!("ExportPlugin: Plugin '{}' started", plugin_id);
            }
            PluginEvent::PluginCompleted { plugin_id, .. } => {
                log::debug!("ExportPlugin: Plugin '{}' completed", plugin_id);
            }
            PluginEvent::ResultsReady { plugin_id, .. } => {
                log::debug!("ExportPlugin: Results ready from plugin '{}'", plugin_id);
            }
            PluginEvent::PluginError { plugin_id, error_message, .. } => {
                log::warn!("ExportPlugin: Plugin '{}' encountered error: {}", plugin_id, error_message);
            }
            PluginEvent::PluginStateChanged { plugin_id, old_state, new_state, .. } => {
                log::debug!("ExportPlugin: Plugin '{}' state changed from '{}' to '{}'", plugin_id, old_state, new_state);
            }
            PluginEvent::DataReady { .. } => {
                // This should be handled by handle_data_ready_event
                log::warn!("ExportPlugin: DataReady event received in wrong handler");
            }
        }
        Ok(())
    }
    
    /// Publish export completion event
    async fn publish_export_completion_event(&self, scan_id: &str, source_plugin_id: &str) -> PluginResult<()> {
        {
            let manager = &self.notification_manager;
            let event = PluginEvent::PluginCompleted {
                plugin_id: "export".to_string(),
                processing_time: std::time::Duration::from_secs(0), // TODO: Track actual processing time
                items_processed: 1, // Export processed one scan worth of data
                results_generated: 1, // Generated one export output
                completed_at: std::time::SystemTime::now(),
            };
            
            manager.publish(event).await.map_err(|e| {
                PluginError::execution_failed(format!("Failed to publish export completion event: {}", e))
            })?;
            
            log::info!("ExportPlugin: Published completion event for scan '{}' (triggered by '{}')", scan_id, source_plugin_id);
        }
        
        Ok(())
    }
    
    /// Publish shutdown event to coordinate clean shutdown
    async fn publish_shutdown_event(&self) -> PluginResult<()> {
        {
            let event = PluginEvent::PluginCompleted {
                plugin_id: "export".to_string(),
                processing_time: std::time::Duration::from_secs(0),
                items_processed: 0,
                results_generated: 0,
                completed_at: std::time::SystemTime::now(),
            };
            
            self.notification_manager.publish(event).await.map_err(|e| {
                PluginError::execution_failed(format!("Failed to publish shutdown event: {}", e))
            })?;
            
            log::info!("ExportPlugin: Published shutdown coordination events");
        }
        
        Ok(())
    }
    
    /// Stop the notification listener
    async fn stop_notification_listener(&self) -> PluginResult<()> {
        log::info!("ExportPlugin: Stopping notification listener");
        
        // Signal the listener to stop
        {
            let mut triggered = self.export_triggered.write().await;
            *triggered = true;
        }
        
        // TODO: Wait for listener task to complete if needed
        // For now, cleanup is handled through Subscriber trait unsubscription
        
        Ok(())
    }
    
    /// Perform export with collected data
    
    /// Format data according to the configured format
    async fn format_data(&self, data: &[Arc<PluginDataExport>], config: &ExportConfig) -> PluginResult<String> {
        match config.output_format {
            ExportFormat::Console => {
                // Use the ConsoleFormatter for console output with color support
                use self::formats::console::ConsoleFormatter;
                use self::formats::FormatExporter;
                
                // Check if we have a color manager available
                if let Some(colour_manager) = self.colour_manager.read().await.as_ref() {
                    let formatter = ConsoleFormatter::with_colors(Arc::clone(colour_manager));
                    formatter.format_with_colors(data)
                } else {
                    let formatter = ConsoleFormatter::new();
                    formatter.format_data(data)
                }
            },
            ExportFormat::Json => self.format_json(data).await,
            ExportFormat::Csv => self.format_csv(data).await,
            ExportFormat::Xml => self.format_xml(data).await,
            ExportFormat::Yaml => self.format_yaml(data).await,
            ExportFormat::Html => self.format_html(data).await,
            ExportFormat::Markdown => self.format_markdown(data).await,
            ExportFormat::Template => {
                // Use the template formatter from formats module
                use self::formats::template::TemplateExporter;
                use self::formats::FormatExporter;
                let template_file = config.template_file.as_ref()
                    .ok_or_else(|| PluginError::configuration_error("Template format selected but no template file configured".to_string()))?;
                let formatter = TemplateExporter::new(template_file);
                formatter.format_data(data)
            },
        }
    }
    

    
    /// Public method to format data as console output for testing and direct use
    pub async fn format_as_console(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        use self::formats::console::ConsoleFormatter;
        use self::formats::FormatExporter;
        
        // Check if we have a color manager available
        if let Some(colour_manager) = self.colour_manager.read().await.as_ref() {
            let formatter = ConsoleFormatter::with_colors(Arc::clone(colour_manager));
            formatter.format_with_colors(data)
        } else {
            let formatter = ConsoleFormatter::new();
            formatter.format_data(data)
        }
    }
    
    pub async fn format_json(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        let mut json_data = json!({});
        let json_obj = json_data.as_object_mut().unwrap();
        
        for export in data {
            let mut plugin_data = json!({
                "title": export.title,
                "description": export.description,
                "type": export.data_type,
            });
            
            // Add data based on type
            match &export.data {
                DataPayload::Rows(rows) => {
                    let mut json_rows = Vec::new();
                    for row in rows.iter() {
                        let mut json_row = json!({});
                        let row_obj = json_row.as_object_mut().unwrap();
                        
                        for (i, value) in row.values.iter().enumerate() {
                            if let Some(col) = export.schema.columns.get(i) {
                                row_obj.insert(col.name.clone(), json!(value.to_string()));
                            }
                        }
                        json_rows.push(json_row);
                    }
                    plugin_data["data"] = json!(json_rows);
                }
                DataPayload::KeyValue(map) => {
                    plugin_data["data"] = json!(**map);
                }
                _ => {
                    plugin_data["data"] = json!(null);
                }
            }
            
            json_obj.insert(export.plugin_id.clone(), plugin_data);
        }
        
        serde_json::to_string_pretty(&json_data)
            .map_err(|e| PluginError::generic(format!("JSON formatting failed: {}", e)))
    }
    
    pub async fn format_csv(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        use self::formats::csv::CsvFormatter;
        use self::formats::FormatExporter;
        
        let config = self.export_config.read().await;
        
        // Parse delimiter - default to comma, but use config if specified
        let delimiter = config.csv_delimiter.chars().next().unwrap_or(',');
        let quote_char = config.csv_quote_char.chars().next().unwrap_or('"');
        
        let formatter = CsvFormatter::with_config(delimiter, quote_char, config.csv_quoting_style);
        formatter.format_data(data)
    }
    
    pub async fn format_xml(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        let mut output = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<export>\n");
        
        for export in data {
            output.push_str(&format!("  <plugin id=\"{}\">\n", export.plugin_id));
            output.push_str(&format!("    <title>{}</title>\n", export.title));
            if let Some(ref desc) = export.description {
                output.push_str(&format!("    <description>{}</description>\n", desc));
            }
            
            match &export.data {
                DataPayload::Rows(rows) => {
                    output.push_str("    <data>\n");
                    for row in rows.iter() {
                        output.push_str("      <row>\n");
                        for (i, value) in row.values.iter().enumerate() {
                            if let Some(col) = export.schema.columns.get(i) {
                                output.push_str(&format!("        <{}>{}</{}>\n", 
                                                        col.name, value.to_string(), col.name));
                            }
                        }
                        output.push_str("      </row>\n");
                    }
                    output.push_str("    </data>\n");
                }
                _ => {}
            }
            
            output.push_str("  </plugin>\n");
        }
        
        output.push_str("</export>\n");
        Ok(output)
    }
    
    pub async fn format_yaml(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        let mut output = String::new();
        
        for export in data {
            output.push_str(&format!("{}:\n", export.plugin_id));
            output.push_str(&format!("  title: {}\n", export.title));
            if let Some(ref desc) = export.description {
                output.push_str(&format!("  description: {}\n", desc));
            }
            
            match &export.data {
                DataPayload::Rows(rows) => {
                    output.push_str("  data:\n");
                    for row in rows.iter() {
                        output.push_str("    - ");
                        for (i, value) in row.values.iter().enumerate() {
                            if let Some(col) = export.schema.columns.get(i) {
                                if i > 0 {
                                    output.push_str("\n      ");
                                }
                                output.push_str(&format!("{}: {}", col.name, value.to_string()));
                            }
                        }
                        output.push('\n');
                    }
                }
                _ => {}
            }
        }
        
        Ok(output)
    }
    
    pub async fn format_html(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        let mut output = String::from(r#"<!DOCTYPE html>
<html>
<head>
    <title>Export Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        h1 { color: #333; }
        h2 { color: #666; border-bottom: 1px solid #ccc; }
        table { border-collapse: collapse; width: 100%; margin: 20px 0; }
        th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }
        th { background-color: #f2f2f2; }
        tr:nth-child(even) { background-color: #f9f9f9; }
    </style>
</head>
<body>
    <h1>Export Report</h1>
"#);
        
        for export in data {
            output.push_str(&format!("    <h2>{}</h2>\n", export.title));
            if let Some(ref desc) = export.description {
                output.push_str(&format!("    <p>{}</p>\n", desc));
            }
            
            match &export.data {
                DataPayload::Rows(rows) if !rows.is_empty() => {
                    output.push_str("    <table>\n        <thead>\n            <tr>\n");
                    for col in &export.schema.columns {
                        output.push_str(&format!("                <th>{}</th>\n", col.name));
                    }
                    output.push_str("            </tr>\n        </thead>\n        <tbody>\n");
                    
                    for row in rows.iter() {
                        output.push_str("            <tr>\n");
                        for value in &row.values {
                            output.push_str(&format!("                <td>{}</td>\n", value.to_string()));
                        }
                        output.push_str("            </tr>\n");
                    }
                    output.push_str("        </tbody>\n    </table>\n");
                }
                _ => {}
            }
        }
        
        output.push_str("</body>\n</html>\n");
        Ok(output)
    }
    
    pub async fn format_markdown(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        let mut output = String::from("# Export Report\n\n");
        
        for export in data {
            output.push_str(&format!("## {}\n\n", export.title));
            if let Some(ref desc) = export.description {
                output.push_str(&format!("{}\n\n", desc));
            }
            
            match &export.data {
                DataPayload::Rows(rows) if !rows.is_empty() => {
                    // Table header
                    output.push('|');
                    for col in &export.schema.columns {
                        output.push_str(&format!(" {} |", col.name));
                    }
                    output.push('\n');
                    
                    // Separator
                    output.push('|');
                    for col in &export.schema.columns {
                        let is_numeric = matches!(col.data_type, ColumnType::Integer | ColumnType::Float);
                        if is_numeric {
                            output.push_str(" ---: |"); // Right align
                        } else {
                            output.push_str(" --- |");
                        }
                    }
                    output.push('\n');
                    
                    // Rows
                    for row in rows.iter() {
                        output.push('|');
                        for value in &row.values {
                            output.push_str(&format!(" {} |", value.to_string()));
                        }
                        output.push('\n');
                    }
                    output.push('\n');
                }
                _ => {}
            }
        }
        
        Ok(output)
    }
}

#[async_trait]
impl Plugin for ExportPlugin {
    fn plugin_info(&self) -> &PluginInfo {
        &self.info
    }
    
    async fn initialize(&mut self, context: &PluginContext) -> PluginResult<()> {
        if self.initialized {
            return Ok(());
        }
        
        // Initialize template engine if template file is configured
        {
            let config = self.export_config.read().await;
            if let Some(ref template_file) = config.template_file {
                let mut engine = self.template_engine.write().await;
                engine.load_template(template_file)?;
            }
        }
        
        // Subscribe to notifications if notification manager is available
        if let Some(ref manager) = context.notification_manager {
            // Create a subscriber handle for this plugin
            let subscriber = Arc::new(self.clone());
            manager.subscribe(subscriber).await
                .map_err(|e| PluginError::initialization_failed(format!("Failed to subscribe to notifications: {}", e)))?;
            log::info!("ExportPlugin subscribed to PluginEvent notifications");
        } else {
            log::debug!("ExportPlugin: No notification manager available in context");
        }
        
        // Store colour manager if available
        if let Some(ref colour_manager) = context.colour_manager {
            let mut manager_guard = self.colour_manager.write().await;
            *manager_guard = Some(colour_manager.clone());
            log::debug!("ExportPlugin: Color manager configured");
        } else {
            log::debug!("ExportPlugin: No color manager available in context");
        }
        
        // Start the comprehensive notification listener if needed
        // Note: The actual listening happens through the Subscriber trait
        // but we can start additional background tasks here if needed
        log::debug!("ExportPlugin: Comprehensive notification listener ready");
        
        self.initialized = true;
        log::info!("Export plugin initialized with comprehensive notification handling");
        Ok(())
    }
    
    async fn execute(&self, request: PluginRequest) -> PluginResult<PluginResponse> {
        // Export plugin primarily works through notifications, not direct execution
        let metadata = crate::plugin::context::ExecutionMetadata {
            duration_us: 0,
            memory_used: 0,
            entries_processed: 0,
            plugin_version: "1.0.0".to_string(),
            extra: std::collections::HashMap::new(),
        };
        
        match request {
            crate::plugin::context::PluginRequest::GetStatistics => {
                Ok(PluginResponse::success(
                    "export_statistics".to_string(),
                    serde_json::json!({"status": "Export plugin is notification-driven"}),
                    metadata
                ))
            }
            _ => {
                Ok(PluginResponse::success(
                    "export_info".to_string(),
                    serde_json::json!({"message": "Export plugin is notification-driven"}),
                    metadata
                ))
            }
        }
    }
    
    async fn cleanup(&mut self) -> PluginResult<()> {
        log::info!("ExportPlugin: Starting cleanup");
        
        // Stop the notification listener if running
        if let Err(e) = self.stop_notification_listener().await {
            log::warn!("ExportPlugin: Error stopping notification listener: {}", e);
        }
        
        // Clear any remaining data in coordinator
        {
            let mut coordinator = self.data_coordinator.write().await;
            coordinator.clear();
        }
        
        // Publish shutdown event
        self.publish_shutdown_event().await.unwrap_or_else(|e| {
            log::warn!("ExportPlugin: Error publishing shutdown event: {}", e);
        });
        
        self.initialized = false;
        log::info!("ExportPlugin: Cleanup completed");
        Ok(())
    }
    
    fn advertised_functions(&self) -> Vec<crate::plugin::traits::PluginFunction> {
        vec![
            crate::plugin::traits::PluginFunction {
                name: "output".to_string(),
                aliases: vec!["export".to_string()],
                description: "Export scan results to various formats (json, csv, xml, yaml, html, markdown)".to_string(),
                is_default: true,
            }
        ]
    }
    
    fn default_function(&self) -> Option<&str> {
        Some("output")
    }
    
    fn get_arg_schema(&self) -> Vec<crate::plugin::traits::PluginArgDefinition> {
        vec![]
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

// Implement Publisher trait for sending events
#[async_trait]
impl Publisher<PluginEvent> for ExportPlugin {
    async fn publish(&self, event: PluginEvent) -> crate::notifications::NotificationResult<()> {
        self.notification_manager.publish(event).await
    }
}

// Implement Subscriber trait for receiving data export notifications
#[async_trait]
impl Subscriber<PluginEvent> for ExportPlugin {
    fn subscriber_id(&self) -> &str {
        "export-plugin"
    }
    
    async fn handle_event(&self, event: PluginEvent) -> NotificationResult<()> {
        match event {
            PluginEvent::DataReady { plugin_id, scan_id, export } => {
                // Use the new comprehensive DataReady handler
                self.handle_data_ready_event(plugin_id, scan_id, export).await
                    .map_err(|e| NotificationError::delivery_failed("export-plugin", &e.to_string()))?;
                return Ok(());
            }
            _ => {
                // Handle all other PluginEvent types
                self.handle_other_plugin_event(&event).await
                    .map_err(|e| NotificationError::delivery_failed("export-plugin", &e.to_string()))?;
                return Ok(());
            }
        }
        
        // This is the old implementation that we're replacing:
        /*
        match event {
            PluginEvent::DataReady { plugin_id, scan_id, export } => {
                log::info!("ExportPlugin received DataReady from '{}' for scan '{}'", plugin_id, scan_id);
                // Old implementation removed - now handled by handle_data_ready_event()
                */
    }
}

/// Modern clap-based argument parsing implementation for export plugin
#[async_trait]
impl PluginClapParser for ExportPlugin {
    fn get_command_name(&self) -> impl Into<String> {
        &self.command_name
    }
    
    fn get_command_description(&self) -> &str {
        "Exports analysis results to various formats"
    }
    
    fn get_plugin_settings(&self) -> &crate::plugin::PluginSettings {
        &self.settings
    }
    
    fn add_plugin_args(&self, command: clap::Command) -> clap::Command {
        use clap::Arg;
        
        command
            .override_usage("export [OPTIONS]")
            .help_template("Usage: {usage}\n\nExports analysis results\n\nOptions:\n{options}\n{after-help}")
            .after_help("File extensions (.json, .csv, .xml, .yaml, .html, .md, .htm, .yml) auto-detect format when using --outfile.")
            .arg(Arg::new("outfile")
                .short('o')
                .long("outfile")
                .value_name("FILE")
                .help("Output file path (if not specified, output to console)")
                .value_hint(clap::ValueHint::FilePath))
            .arg(Arg::new("template")
                .short('t')
                .long("template")
                .value_name("FILE")
                .help("Template file for custom formatting (Tera/Jinja2-compatible)")
                .value_hint(clap::ValueHint::FilePath))
            .arg(Arg::new("format")
                .short('f')
                .long("format")
                .value_name("FORMAT")
                .help("Output format: json, csv, xml, yaml, html, markdown")
                .value_parser(["json", "csv", "xml", "yaml", "html", "markdown"])
                .hide_possible_values(true))
    }
    
    async fn configure_from_matches(&mut self, matches: &clap::ArgMatches) -> PluginResult<()> {
        let mut config = self.export_config.write().await;
        
        // Handle output file
        if let Some(outfile) = matches.get_one::<String>("outfile") {
            config.output_file = Some(PathBuf::from(outfile));
            
            // Auto-detect format from extension
            match self.format_detector.detect_format_from_path(outfile) {
                FormatDetectionResult::Detected(format) => {
                    config.output_format = format;
                }
                _ => {}
            }
        }
        
        // Handle format
        if let Some(format) = matches.get_one::<String>("format") {
            config.output_format = match format.to_lowercase().as_str() {
                "json" => ExportFormat::Json,
                "csv" => ExportFormat::Csv,
                "xml" => ExportFormat::Xml,
                "yaml" | "yml" => ExportFormat::Yaml,
                "html" | "htm" => ExportFormat::Html,
                "markdown" | "md" => ExportFormat::Markdown,
                _ => return Err(PluginError::invalid_argument(
                    "--format",
                    &format!("Unknown format: {}", format)
                )),
            };
        }
        
        // Handle template
        if let Some(template) = matches.get_one::<String>("template") {
            let template_path = PathBuf::from(template);
            config.template_file = Some(template_path.clone());
            config.output_format = ExportFormat::Template; // Set format to Template when --template is used
            
            // Load template
            let mut engine = self.template_engine.write().await;
            engine.load_template(&template_path)?;
        }
        
        log::debug!("Export plugin configured with clap: format={:?}, outfile={:?}", 
                   config.output_format, config.output_file);
        
        Ok(())
    }
}

impl PluginDataRequirements for ExportPlugin {
    fn requires_current_file_content(&self) -> bool {
        false // Export plugin doesn't need file content
    }
    
    fn requires_historical_file_content(&self) -> bool {
        false // Export plugin doesn't need historical content
    }
}

impl Default for ExportPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::AsyncNotificationManager;
    use crate::notifications::events::PluginEvent;
    use crate::plugin::data_export::{PluginDataExport, DataPayload, DataSchema, ColumnDef, ColumnType, Row, Value};
    use crate::scanner::{ScannerConfig, QueryParams};
    use std::sync::Arc;

    fn create_test_context() -> PluginContext {
        PluginContext::new(
            Arc::new(ScannerConfig::default()),
            Arc::new(QueryParams::default()),
        )
    }

    fn create_test_export_data() -> Arc<PluginDataExport> {
        let schema = DataSchema {
            columns: vec![
                ColumnDef::new("metric", ColumnType::String)
                    .with_description("Metric name".to_string()),
                ColumnDef::new("value", ColumnType::Integer)
                    .with_description("Metric value".to_string()),
            ],
            metadata: std::collections::HashMap::new(),
        };

        let rows = vec![
            Row::new(vec![
                Value::String("total_commits".to_string()),
                Value::Integer(100),
            ]),
            Row::new(vec![
                Value::String("total_authors".to_string()),
                Value::Integer(5),
            ]),
        ];

        Arc::new(PluginDataExport {
            plugin_id: "test".to_string(),
            title: "Test Data".to_string(),
            description: Some("Test export data".to_string()),
            data_type: crate::plugin::data_export::DataExportType::Tabular,
            schema,
            data: DataPayload::Rows(Arc::new(rows)),
            export_hints: crate::plugin::data_export::ExportHints::default(),
            timestamp: std::time::SystemTime::now(),
        })
    }

    #[tokio::test]
    async fn test_export_plugin_creation() {
        let plugin = ExportPlugin::new();
        assert_eq!(plugin.plugin_info().name, "export");
        assert_eq!(plugin.plugin_info().version, "1.0.0");
        assert_eq!(plugin.plugin_info().plugin_type, PluginType::Output);
        assert!(plugin.plugin_info().active_by_default);
    }

    #[tokio::test]
    async fn test_export_plugin_subscription() {
        let mut plugin = ExportPlugin::new();
        
        // Test initialization without notification manager
        let context_without = create_test_context();
        plugin.initialize(&context_without).await.unwrap();
        
        // Test initialization with notification manager
        let notification_manager = Arc::new(
            AsyncNotificationManager::<PluginEvent>::new()
        );
        let context_with = create_test_context()
            .with_notification_manager(notification_manager.clone());
        
        let mut plugin2 = ExportPlugin::new();
        plugin2.initialize(&context_with).await.unwrap();
        
        // Verify subscriber_id method
        assert_eq!(plugin2.subscriber_id(), "export-plugin");
        
        // Verify subscription was attempted (we can't easily verify success without complex mocking)
        // The initialize call should complete without error
    }

    #[tokio::test]
    async fn test_export_plugin_event_handling() {
        let plugin = ExportPlugin::new();
        
        // Test DataReady event handling
        let export_data = create_test_export_data();
        let event = PluginEvent::DataReady {
            plugin_id: "debug".to_string(),
            scan_id: "test-scan".to_string(),
            export: export_data.clone(),
        };
        
        // Handle the event
        let result = plugin.handle_event(event).await;
        assert!(result.is_ok());
        
        // Verify that data was added to coordinator
        let coordinator = plugin.data_coordinator.read().await;
        assert!(coordinator.has_data_from("debug"));
        
        // Verify scan ID was set
        let scan_id = plugin.scan_id.read().await;
        assert_eq!(scan_id.as_ref().unwrap(), "test-scan");
    }

    #[tokio::test]
    async fn test_export_plugin_formatting() {
        let plugin = ExportPlugin::new();
        let export_data = create_test_export_data();
        let data_vec = vec![export_data];
        
        // Test JSON formatting
        let json_result = plugin.format_json(&data_vec).await;
        assert!(json_result.is_ok());
        let json_output = json_result.unwrap();
        assert!(json_output.contains("Test Data"));
        assert!(json_output.contains("total_commits"));
        
        // Test CSV formatting
        let csv_result = plugin.format_csv(&data_vec).await;
        assert!(csv_result.is_ok());
        let csv_output = csv_result.unwrap();
        assert!(csv_output.contains("metric,value"));
        assert!(csv_output.contains("total_commits,100"));
        
        // Test HTML formatting
        let html_result = plugin.format_html(&data_vec).await;
        assert!(html_result.is_ok());
        let html_output = html_result.unwrap();
        assert!(html_output.contains("<html>"));
        assert!(html_output.contains("Test Data"));
        
        // Test Markdown formatting
        let md_result = plugin.format_markdown(&data_vec).await;
        assert!(md_result.is_ok());
        let md_output = md_result.unwrap();
        assert!(md_output.contains("# Export Report"));
        assert!(md_output.contains("Test Data"));
    }

    #[tokio::test]
    async fn test_export_plugin_clone() {
        let plugin = ExportPlugin::new();
        let cloned_plugin = plugin.clone();
        
        // Verify basic properties are the same
        assert_eq!(plugin.plugin_info().name, cloned_plugin.plugin_info().name);
        assert_eq!(plugin.plugin_info().version, cloned_plugin.plugin_info().version);
        assert_eq!(plugin.subscriber_id(), cloned_plugin.subscriber_id());
        
        // Verify color manager is cloned properly (both should be None initially)
        let plugin_cm = plugin.colour_manager.try_read().unwrap();
        let cloned_cm = cloned_plugin.colour_manager.try_read().unwrap();
        assert!(plugin_cm.is_none());
        assert!(cloned_cm.is_none());
    }

    #[tokio::test]
    async fn test_console_formatting_with_prettytable() {
        let plugin = ExportPlugin::new();
        let export_data = create_test_export_data();
        let data_vec = vec![export_data];
        
        // Test console formatting - should use prettytable-rs clean format
        let console_result = plugin.format_console(&data_vec).await;
        assert!(console_result.is_ok());
        let console_output = console_result.unwrap();
        
        // Should contain table formatting
        assert!(console_output.contains("metric"));
        assert!(console_output.contains("value"));
        assert!(console_output.contains("total_commits"));
        assert!(console_output.contains("100"));
        
        // Should have table format with pipe separators
        assert!(console_output.contains("| metric        | value |"));
        
        // Should have 2-space indent for table
        assert!(console_output.lines().any(|line| line.starts_with("  |")));
        
        // Should contain ASCII table borders (the current format)
        assert!(console_output.contains("+-------+"));
        assert!(console_output.contains("| metric"));
        assert!(console_output.contains("| value"));
        assert!(console_output.contains("| total_commits"));
        assert!(console_output.contains("| 100"));
    }

    #[tokio::test]
    async fn test_console_formatting_with_colors() {
        use crate::display::ColourManager;
        
        let plugin = ExportPlugin::new();
        let export_data = create_test_export_data();
        let data_vec = vec![export_data];
        
        // Test with color manager enabled
        let mut config = crate::display::ColourConfig::default();
        config.set_enabled(true);
        let color_manager = ColourManager::with_config(config);
        let console_result = plugin.format_console_with_colors(&data_vec, &color_manager).await;
        assert!(console_result.is_ok());
        let console_output = console_result.unwrap();
        
        // Should contain ANSI color codes when colors are enabled
        assert!(console_output.contains("\x1b[") || !color_manager.colours_enabled());
        assert!(console_output.contains("metric"));
        assert!(console_output.contains("value"));
        
        // Test with color manager disabled
        let mut no_config = crate::display::ColourConfig::default();
        no_config.set_enabled(false);
        let no_color_manager = ColourManager::with_config(no_config);
        let no_color_result = plugin.format_console_with_colors(&data_vec, &no_color_manager).await;
        assert!(no_color_result.is_ok());
        let no_color_output = no_color_result.unwrap();
        
        // Should NOT contain ANSI color codes when colors are disabled
        assert!(!no_color_output.contains("\x1b["));
        assert!(no_color_output.contains("metric"));
        assert!(no_color_output.contains("value"));
    }

    #[tokio::test]
    async fn test_table_alignment_and_formatting() {
        let plugin = ExportPlugin::new();
        
        // Create test data with mixed string and numeric columns
        let schema = DataSchema {
            columns: vec![
                ColumnDef::new("name", ColumnType::String),
                ColumnDef::new("count", ColumnType::Integer),
                ColumnDef::new("percentage", ColumnType::Float),
            ],
            metadata: std::collections::HashMap::new(),
        };

        let rows = vec![
            Row::new(vec![
                Value::String("Short".to_string()),
                Value::Integer(999),
                Value::Float(12.5),
            ]),
            Row::new(vec![
                Value::String("Very Long Name".to_string()),
                Value::Integer(1),
                Value::Float(100.0),
            ]),
        ];

        let export_data = Arc::new(PluginDataExport {
            plugin_id: "test".to_string(),
            title: "Alignment Test".to_string(),
            description: Some("Test data alignment".to_string()),
            data_type: crate::plugin::data_export::DataExportType::Tabular,
            schema,
            data: DataPayload::Rows(Arc::new(rows)),
            export_hints: crate::plugin::data_export::ExportHints::default(),
            timestamp: std::time::SystemTime::now(),
        });
        
        let data_vec = vec![export_data];
        let console_result = plugin.format_console(&data_vec).await;
        assert!(console_result.is_ok());
        let console_output = console_result.unwrap();
        
        // Should handle different column widths properly
        assert!(console_output.contains("Very Long Name"));
        assert!(console_output.contains("999"));
        assert!(console_output.contains("12.5"));
        
        // Should have consistent table structure
        let lines: Vec<&str> = console_output.lines().collect();
        assert!(lines.len() >= 4); // Header + separator + 2 data rows minimum
    }
}