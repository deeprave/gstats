//! Data Export Plugin Module
//! 
//! Built-in plugin for exporting scan results to various formats.

pub mod formats;
pub mod template_engine;
pub mod config;

use crate::plugin::{
    Plugin, PluginInfo, PluginContext, PluginRequest, PluginResponse,
    PluginResult, PluginError, traits::{PluginType, PluginClapParser, PluginDataRequirements}
};
use crate::plugin::data_export::{PluginDataExport, DataPayload, ColumnType};
use crate::plugin::data_coordinator::DataCoordinator;
use crate::plugin::builtin::utils::format_detection::{FormatDetector, FormatDetectionResult};
use crate::notifications::events::PluginEvent;
use crate::notifications::traits::{Subscriber, NotificationManager};
use crate::notifications::{NotificationResult};
use crate::notifications::error::NotificationError;
use crate::display::ColourManager;
use async_trait::async_trait;
use prettytable::{Table, Row, Cell};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::json;

pub use config::{ExportConfig, ExportFormat};
pub use template_engine::TemplateEngine;

/// Data export plugin for various output formats
#[derive(Clone)]
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
    // Color management
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
            colour_manager: Arc::new(RwLock::new(None)),
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
                    // Create clean table format using prettytable
                    if !export.schema.columns.is_empty() && !rows.is_empty() {
                        let mut table = Table::new();
                        
                        // Use default format to include both pipe separators and dashes
                        // Tests expect both "|" and "-" characters in table formatting
                        
                        // Create header row
                        let header_cells: Vec<Cell> = export.schema.columns.iter()
                            .map(|col| Cell::new(&col.name))
                            .collect();
                        table.add_row(Row::new(header_cells));
                        
                        // Add data rows
                        for row in rows.iter() {
                            let data_cells: Vec<Cell> = row.values.iter()
                                .map(|value| Cell::new(&value.to_string()))
                                .collect();
                            table.add_row(Row::new(data_cells));
                        }
                        
                        // Add 2-space indent to each line
                        let table_output = table.to_string();
                        for line in table_output.lines() {
                            output.push_str("  ");
                            output.push_str(line);
                            output.push('\n');
                        }
                    }
                }
                DataPayload::KeyValue(map) => {
                    // Format key-value pairs as simple key: value lines for console readability
                    if !map.is_empty() {
                        for (key, value) in map.iter() {
                            output.push_str(&format!("  {}: {}\n", key, value.to_string()));
                        }
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

    pub async fn format_console_with_colors(&self, data: &[Arc<PluginDataExport>], colour_manager: &ColourManager) -> PluginResult<String> {
        if data.is_empty() {
            return Ok("No data available for export.\n".to_string());
        }
        
        let mut output = String::new();
        
        for export in data {
            output.push_str(&format!("## {}\n", export.title));
            if let Some(description) = &export.description {
                output.push_str(&format!("{}\n", description));
            }
            output.push('\n');
            
            match &export.data {
                DataPayload::Rows(rows) => {
                    // Create clean table format with colors using prettytable
                    if !export.schema.columns.is_empty() && !rows.is_empty() {
                        let mut table = Table::new();
                        // Use default table format (includes borders and separators)
                        
                        // Create header row with colors
                        let header_cells: Vec<Cell> = export.schema.columns.iter()
                            .map(|col| {
                                let header_text = if colour_manager.colours_enabled() {
                                    colour_manager.highlight(&col.name).to_string()
                                } else {
                                    col.name.clone()
                                };
                                Cell::new(&header_text)
                            })
                            .collect();
                        table.add_row(Row::new(header_cells));
                        
                        // Add data rows with colors
                        for row in rows.iter() {
                            let data_cells: Vec<Cell> = row.values.iter()
                                .enumerate()
                                .map(|(i, value)| {
                                    let str_val = value.to_string();
                                    let is_numeric = matches!(export.schema.columns.get(i).map(|c| c.data_type), 
                                                            Some(ColumnType::Integer | ColumnType::Float));
                                    
                                    let formatted_value = if colour_manager.colours_enabled() && is_numeric {
                                        colour_manager.info(&str_val).to_string()
                                    } else {
                                        str_val
                                    };
                                    
                                    Cell::new(&formatted_value)
                                })
                                .collect();
                            table.add_row(Row::new(data_cells));
                        }
                        
                        // Add 2-space indent to each line
                        let table_output = table.to_string();
                        for line in table_output.lines() {
                            output.push_str("  ");
                            output.push_str(line);
                            output.push('\n');
                        }
                    }
                }
                DataPayload::KeyValue(map) => {
                    // Format key-value pairs using prettytable with clean format and colors
                    if !map.is_empty() {
                        let mut table = Table::new();
                        // Use default table format (includes borders and separators)
                        
                        // Add header with colors
                        let key_header = if colour_manager.colours_enabled() {
                            colour_manager.highlight("Key").to_string()
                        } else {
                            "Key".to_string()
                        };
                        let value_header = if colour_manager.colours_enabled() {
                            colour_manager.highlight("Value").to_string()
                        } else {
                            "Value".to_string()
                        };
                        table.add_row(Row::new(vec![Cell::new(&key_header), Cell::new(&value_header)]));
                        
                        // Add key-value pairs
                        for (key, value) in map.iter() {
                            table.add_row(Row::new(vec![Cell::new(key), Cell::new(&value.to_string())]));
                        }
                        
                        // Add 2-space indent to each line
                        let table_output = table.to_string();
                        for line in table_output.lines() {
                            output.push_str("  ");
                            output.push_str(line);
                            output.push('\n');
                        }
                    }
                }
                DataPayload::Tree(root) => {
                    output.push_str(&format!("Tree: {}\n", root.label));
                    // TODO: Implement proper tree formatting with prettytable
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
                        
                        // Use color-aware console formatting if color manager is available
                        let formatted = {
                            let colour_manager_guard = self.colour_manager.read().await;
                            if let Some(ref colour_manager) = *colour_manager_guard {
                                self.format_console_with_colors(&all_data, colour_manager).await.map_err(|e| {
                                    NotificationError::processing(format!("Color formatting failed: {}", e))
                                })?
                            } else {
                                // Fallback to basic console formatting
                                self.format_console(&all_data).await.map_err(|e| {
                                    NotificationError::processing(format!("Formatting failed: {}", e))
                                })?
                            }
                        };
                        
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
}

/// Modern clap-based argument parsing implementation for export plugin
#[async_trait]
impl PluginClapParser for ExportPlugin {
    fn build_clap_command(&self) -> clap::Command {
        use clap::{Arg, Command};
        
        Command::new("export")
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
        assert!(plugin.plugin_info().load_by_default);
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
        let scan_id = plugin.current_scan_id.read().await;
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