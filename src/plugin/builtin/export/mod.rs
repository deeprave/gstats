//! Data Export Plugin Module
//! 
//! Built-in plugin for exporting scan results to various formats.

pub mod formats;
pub mod template_engine;
pub mod config;

use crate::plugin::{
    Plugin, PluginInfo, PluginContext, PluginRequest, PluginResponse,
    PluginResult, PluginError, traits::{PluginType, PluginArgumentParser, PluginArgDefinition, PluginDataRequirements}
};
use crate::plugin::data_export::{PluginDataExport, DataPayload, ColumnType};
use crate::plugin::data_coordinator::DataCoordinator;
use crate::plugin::builtin::utils::format_detection::{FormatDetector, FormatDetectionResult};
use crate::notifications::events::PluginEvent;
use crate::notifications::traits::Subscriber;
use crate::notifications::{NotificationResult};
use crate::notifications::error::NotificationError;
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
    template_engine: Arc<RwLock<TemplateEngine>>,
    format_detector: FormatDetector,
    // Data coordination
    data_coordinator: Arc<RwLock<DataCoordinator>>,
    // Scan tracking
    current_scan_id: Arc<RwLock<Option<String>>>,
    export_triggered: Arc<RwLock<bool>>,
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
        .with_load_by_default(true);

        Self {
            info,
            initialized: false,
            export_config: Arc::new(RwLock::new(ExportConfig::default())),
            template_engine: Arc::new(RwLock::new(TemplateEngine::new())),
            format_detector: FormatDetector::new(),
            // Initialize data coordination
            data_coordinator: Arc::new(RwLock::new(
                DataCoordinator::with_expected_plugins(vec![
                    "commits".to_string(),
                    "metrics".to_string(),
                ])
            )),
            current_scan_id: Arc::new(RwLock::new(None)),
            export_triggered: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Perform export with collected data
    async fn perform_export(&self, data: Vec<Arc<PluginDataExport>>) -> PluginResult<()> {
        if data.is_empty() {
            log::warn!("No data to export");
            return Ok(());
        }
        
        let config = self.export_config.read().await;
        
        // Format the data
        let formatted = self.format_data(&data, &config).await?;
        
        // Check if we should output to file or console
        if let Some(ref output_path) = config.output_file {
            // File output
            std::fs::write(output_path, &formatted)
                .map_err(|e| PluginError::io_error(format!("Failed to write output file: {}", e)))?;
            log::info!("Exported data to {}", output_path.display());
        } else {
            // Console output
            println!("{}", formatted);
        }
        
        // Mark export as triggered
        {
            let mut triggered = self.export_triggered.write().await;
            *triggered = true;
        }
        
        Ok(())
    }
    
    /// Format data according to the configured format
    async fn format_data(&self, data: &[Arc<PluginDataExport>], config: &ExportConfig) -> PluginResult<String> {
        match config.output_format {
            ExportFormat::Json => self.format_json(data).await,
            ExportFormat::Csv => self.format_csv(data).await,
            ExportFormat::Xml => self.format_xml(data).await,
            ExportFormat::Yaml => self.format_yaml(data).await,
            ExportFormat::Html => self.format_html(data).await,
            ExportFormat::Markdown => self.format_markdown(data).await,
        }
    }
    
    /// Format as console table (default)
    pub async fn format_console(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        let mut output = String::new();
        
        for export in data {
            // Add section header
            output.push_str(&format!("\n{}\n", "=".repeat(export.title.len() + 4)));
            output.push_str(&format!("  {}  \n", export.title));
            output.push_str(&format!("{}\n", "=".repeat(export.title.len() + 4)));
            
            if let Some(ref desc) = export.description {
                output.push_str(&format!("{}\n\n", desc));
            }
            
            // Format based on data type
            match &export.data {
                DataPayload::Rows(rows) => {
                    // Create table for tabular data
                    if !export.schema.columns.is_empty() && !rows.is_empty() {
                        // Calculate column widths
                        let mut widths: Vec<usize> = export.schema.columns
                            .iter()
                            .map(|c| c.name.len())
                            .collect();
                        
                        for row in rows.iter() {
                            for (i, value) in row.values.iter().enumerate() {
                                if i < widths.len() {
                                    widths[i] = widths[i].max(value.to_string().len());
                                }
                            }
                        }
                        
                        // Print header
                        for (i, col) in export.schema.columns.iter().enumerate() {
                            if i > 0 {
                                output.push_str(" | ");
                            }
                            output.push_str(&format!("{:width$}", col.name, width = widths[i]));
                        }
                        output.push('\n');
                        
                        // Print separator
                        for (i, width) in widths.iter().enumerate() {
                            if i > 0 {
                                output.push_str("-+-");
                            }
                            output.push_str(&"-".repeat(*width));
                        }
                        output.push('\n');
                        
                        // Print rows
                        for row in rows.iter() {
                            for (i, value) in row.values.iter().enumerate() {
                                if i > 0 {
                                    output.push_str(" | ");
                                }
                                if i < widths.len() {
                                    let str_val = value.to_string();
                                    // Right-align numbers
                                    let is_numeric = matches!(export.schema.columns.get(i).map(|c| c.data_type), 
                                                            Some(ColumnType::Integer | ColumnType::Float));
                                    if is_numeric {
                                        output.push_str(&format!("{:>width$}", str_val, width = widths[i]));
                                    } else {
                                        output.push_str(&format!("{:width$}", str_val, width = widths[i]));
                                    }
                                }
                            }
                            output.push('\n');
                        }
                    }
                }
                DataPayload::KeyValue(map) => {
                    // Format key-value pairs
                    let max_key_len = map.keys().map(|k| k.len()).max().unwrap_or(0);
                    for (key, value) in map.iter() {
                        output.push_str(&format!("{:width$} : {}\n", key, value.to_string(), width = max_key_len));
                    }
                }
                DataPayload::Tree(root) => {
                    // Simple tree representation
                    output.push_str(&format!("Tree: {}\n", root.label));
                    // TODO: Implement proper tree formatting
                }
                DataPayload::Raw(text) => {
                    output.push_str(text);
                    output.push('\n');
                }
                DataPayload::Empty => {
                    output.push_str("(no data)\n");
                }
            }
            
            output.push('\n');
        }
        
        Ok(output)
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
        let mut output = String::new();
        
        for export in data {
            if let DataPayload::Rows(rows) = &export.data {
                // Write CSV header
                for (i, col) in export.schema.columns.iter().enumerate() {
                    if i > 0 {
                        output.push(',');
                    }
                    output.push_str(&col.name);
                }
                output.push('\n');
                
                // Write rows
                for row in rows.iter() {
                    for (i, value) in row.values.iter().enumerate() {
                        if i > 0 {
                            output.push(',');
                        }
                        // Quote strings that contain commas or quotes
                        let str_val = value.to_string();
                        if str_val.contains(',') || str_val.contains('"') {
                            output.push_str(&format!("\"{}\"", str_val.replace('"', "\"\"")));
                        } else {
                            output.push_str(&str_val);
                        }
                    }
                    output.push('\n');
                }
            }
        }
        
        Ok(output)
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
    
    async fn initialize(&mut self, _context: &PluginContext) -> PluginResult<()> {
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
        
        self.initialized = true;
        log::info!("Export plugin initialized");
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
        self.initialized = false;
        log::info!("Export plugin cleanup");
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
}

// Implement Subscriber trait for receiving data export notifications
#[async_trait]
impl Subscriber<PluginEvent> for ExportPlugin {
    async fn handle_event(&self, event: PluginEvent) -> NotificationResult<()> {
        match event {
            PluginEvent::DataReady { plugin_id, scan_id, export } => {
                log::info!("ExportPlugin received DataReady from '{}' for scan '{}'", plugin_id, scan_id);
                
                // Update scan ID if needed
                {
                    let mut scan_id_guard = self.current_scan_id.write().await;
                    if scan_id_guard.is_none() {
                        *scan_id_guard = Some(scan_id.clone());
                    }
                }
                
                // Add data to coordinator
                {
                    let mut coordinator = self.data_coordinator.write().await;
                    coordinator.add_data(plugin_id.clone(), export);
                    
                    // Check if all expected plugins have reported
                    if coordinator.is_complete() {
                        log::info!("All expected plugins have reported data, triggering export");
                        
                        // Get all data and trigger export
                        let all_data = coordinator.get_all_data();
                        
                        // Use console format by default for now
                        let formatted = self.format_console(&all_data).await.map_err(|e| {
                            NotificationError::processing(format!("Formatting failed: {}", e))
                        })?;
                        
                        println!("{}", formatted);
                        
                        // Clear for next scan
                        coordinator.clear();
                    } else {
                        let pending = coordinator.get_pending_plugins();
                        log::debug!("Waiting for plugins: {:?}", pending);
                    }
                }
                
                Ok(())
            }
            _ => {
                // Ignore other plugin events
                Ok(())
            }
        }
    }
    
    fn subscriber_id(&self) -> &str {
        "export_plugin"
    }
}

#[async_trait]
impl PluginArgumentParser for ExportPlugin {
    fn get_arg_schema(&self) -> Vec<PluginArgDefinition> {
        vec![
            PluginArgDefinition {
                name: "--outfile".to_string(),
                description: "Output file path (if not specified, output to console)".to_string(),
                required: false,
                default_value: None,
                arg_type: "path".to_string(),
                examples: vec!["report.json".to_string(), "data.csv".to_string()],
            },
            PluginArgDefinition {
                name: "--format".to_string(),
                description: "Output format (json, csv, xml, yaml, html, markdown)".to_string(),
                required: false,
                default_value: Some("console".to_string()),
                arg_type: "string".to_string(),
                examples: vec!["json".to_string(), "csv".to_string()],
            },
            PluginArgDefinition {
                name: "--template".to_string(),
                description: "Template file for custom formatting".to_string(),
                required: false,
                default_value: None,
                arg_type: "path".to_string(),
                examples: vec!["report.tera".to_string()],
            },
        ]
    }
    
    async fn parse_plugin_args(&mut self, args: &[String]) -> PluginResult<()> {
        let mut config = self.export_config.write().await;
        
        for i in 0..args.len() {
            match args[i].as_str() {
                "--outfile" | "-o" => {
                    if i + 1 < args.len() {
                        config.output_file = Some(PathBuf::from(&args[i + 1]));
                        
                        // Auto-detect format from extension
                        match self.format_detector.detect_format_from_path(&args[i + 1]) {
                            FormatDetectionResult::Detected(format) => {
                                config.output_format = format;
                            }
                            _ => {}
                        }
                    }
                }
                "--format" | "-f" => {
                    if i + 1 < args.len() {
                        config.output_format = match args[i + 1].to_lowercase().as_str() {
                            "json" => ExportFormat::Json,
                            "csv" => ExportFormat::Csv,
                            "xml" => ExportFormat::Xml,
                            "yaml" | "yml" => ExportFormat::Yaml,
                            "html" | "htm" => ExportFormat::Html,
                            "markdown" | "md" => ExportFormat::Markdown,
                            _ => return Err(PluginError::invalid_argument(
                                "--format",
                                &format!("Unknown format: {}", args[i + 1])
                            )),
                        };
                    }
                }
                "--template" | "-t" => {
                    if i + 1 < args.len() {
                        let template_path = PathBuf::from(&args[i + 1]);
                        config.template_file = Some(template_path.clone());
                        
                        // Load template
                        let mut engine = self.template_engine.write().await;
                        engine.load_template(&template_path)?;
                    }
                }
                _ => {}
            }
        }
        
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