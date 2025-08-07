//! Data Export Plugin
//! 
//! Built-in plugin for exporting scan results to various formats.

use crate::plugin::{
    Plugin, PluginInfo, PluginContext, PluginRequest, PluginResponse,
    PluginResult, PluginError, traits::{PluginType, PluginFunction, PluginArgumentParser, PluginArgDefinition}
};
use crate::scanner::{modes::ScanMode, messages::{ScanMessage, MessageData, MessageHeader}};
use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;

/// Data export plugin for various output formats
pub struct ExportPlugin {
    info: PluginInfo,
    initialized: bool,
    export_config: ExportConfig,
    collected_data: Vec<ScanMessage>,
}

#[derive(Debug, Clone)]
struct ExportConfig {
    output_format: ExportFormat,
    output_path: String,
    include_metadata: bool,
    max_entries: Option<usize>,
    output_all: bool,
    csv_delimiter: String,
    csv_quote_char: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    Json,
    Csv,
    Xml,
    Yaml,
    Html,
    Markdown,
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
        );

        Self {
            info,
            initialized: false,
            export_config: ExportConfig::default(),
            collected_data: Vec::new(),
        }
    }

    /// Configure export settings
    pub fn configure(&mut self, format: ExportFormat, output_path: &str) -> PluginResult<()> {
        self.export_config.output_format = format;
        self.export_config.output_path = output_path.to_string();
        Ok(())
    }

    /// Add data for export
    pub fn add_data(&mut self, message: ScanMessage) -> PluginResult<()> {
        if let Some(max_entries) = self.export_config.max_entries {
            if self.collected_data.len() >= max_entries {
                return Err(PluginError::execution_failed("Maximum entries limit reached"));
            }
        }
        self.collected_data.push(message);
        Ok(())
    }

    /// Get data to export with limit applied
    fn get_data_to_export(&self) -> Vec<&ScanMessage> {
        if let Some(max_entries) = self.export_config.max_entries {
            self.collected_data.iter().take(max_entries).collect()
        } else if self.export_config.output_all {
            self.collected_data.iter().collect()
        } else {
            self.collected_data.iter().take(10).collect() // Default limit
        }
    }

    /// Export collected data to the configured format
    pub async fn export_data(&self) -> PluginResult<String> {
        if !self.initialized {
            return Err(PluginError::invalid_state("Plugin not initialized"));
        }

        match self.export_config.output_format {
            ExportFormat::Json => self.export_json(),
            ExportFormat::Csv => self.export_csv(),
            ExportFormat::Xml => self.export_xml(),
            ExportFormat::Yaml => self.export_yaml(),
            ExportFormat::Html => self.export_html(),
            ExportFormat::Markdown => self.export_markdown(),
        }
    }

    /// Export data as JSON
    fn export_json(&self) -> PluginResult<String> {
        let mut export_data = HashMap::new();
        
        let data_to_export = self.get_data_to_export();

        if self.export_config.include_metadata {
            export_data.insert("metadata", json!({
                "export_timestamp": std::time::SystemTime::now(),
                "total_entries": self.collected_data.len(),
                "exported_entries": data_to_export.len(),
                "format": "json",
                "plugin_version": self.info.version,
            }));
        }

        let data: Vec<serde_json::Value> = data_to_export.iter()
            .map(|msg| json!({
                "header": {
                    "scan_mode": format!("{:?}", msg.header.scan_mode),
                    "timestamp": msg.header.timestamp,
                },
                "data": msg.data,
            }))
            .collect();

        export_data.insert("scan_results", json!(data));

        serde_json::to_string_pretty(&export_data)
            .map_err(|e| PluginError::execution_failed(format!("JSON serialization failed: {}", e)))
    }

    /// Export data as CSV
    fn export_csv(&self) -> PluginResult<String> {
        let mut csv_content = String::new();
        let delimiter = &self.export_config.csv_delimiter;
        let quote_char = &self.export_config.csv_quote_char;

        // CSV header
        csv_content.push_str(&format!("timestamp{}scan_mode{}data_json\n", delimiter, delimiter));

        let data_to_export = self.get_data_to_export();

        // CSV rows
        for message in data_to_export {
            let timestamp = message.header.timestamp;
            let scan_mode = format!("{:?}", message.header.scan_mode);
            let data_json = serde_json::to_string(&message.data)
                .map_err(|e| PluginError::execution_failed(format!("JSON serialization failed: {}", e)))?;

            // Escape CSV values based on quote character
            let escaped_json = if quote_char == "\"" {
                data_json.replace('"', "\"\"")
            } else {
                data_json.replace(quote_char, &format!("{}{}", quote_char, quote_char))
            };
            
            csv_content.push_str(&format!(
                "{}{}{}{}{}{}{}\n", 
                timestamp, delimiter, 
                scan_mode, delimiter,
                quote_char, escaped_json, quote_char
            ));
        }

        Ok(csv_content)
    }

    /// Export data as XML
    fn export_xml(&self) -> PluginResult<String> {
        let mut xml_content = String::new();
        xml_content.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml_content.push_str("<scan_results>\n");

        if self.export_config.include_metadata {
            xml_content.push_str("  <metadata>\n");
            xml_content.push_str(&format!("    <export_timestamp>{:?}</export_timestamp>\n", std::time::SystemTime::now()));
            xml_content.push_str(&format!("    <total_entries>{}</total_entries>\n", self.collected_data.len()));
            xml_content.push_str(&format!("    <plugin_version>{}</plugin_version>\n", self.info.version));
            xml_content.push_str("  </metadata>\n");
        }

        let data_to_export = self.get_data_to_export();

        xml_content.push_str("  <entries>\n");
        for message in data_to_export {
            xml_content.push_str("    <entry>\n");
            xml_content.push_str(&format!("      <timestamp>{}</timestamp>\n", message.header.timestamp));
            xml_content.push_str(&format!("      <scan_mode>{:?}</scan_mode>\n", message.header.scan_mode));
            xml_content.push_str("      <data>\n");
            
            // Serialize MessageData to JSON and then convert to XML
            match serde_json::to_value(&message.data) {
                Ok(json_value) => {
                    if let serde_json::Value::Object(map) = json_value {
                        for (key, value) in map {
                            let value_str = match value {
                                serde_json::Value::String(s) => s,
                                _ => value.to_string(),
                            };
                            xml_content.push_str(&format!("        <{}>{}</{}>\n", key, 
                                self.escape_xml(&value_str), key));
                        }
                    }
                }
                Err(_) => {
                    xml_content.push_str("        <error>Failed to serialize data</error>\n");
                }
            }
            
            xml_content.push_str("      </data>\n");
            xml_content.push_str("    </entry>\n");
        }
        xml_content.push_str("  </entries>\n");
        xml_content.push_str("</scan_results>\n");

        Ok(xml_content)
    }

    /// Export data as YAML
    fn export_yaml(&self) -> PluginResult<String> {
        let mut export_data = HashMap::new();
        
        let data_to_export = self.get_data_to_export();

        if self.export_config.include_metadata {
            export_data.insert("metadata", json!({
                "export_timestamp": format!("{:?}", std::time::SystemTime::now()),
                "total_entries": self.collected_data.len(),
                "exported_entries": data_to_export.len(),
                "format": "yaml",
                "plugin_version": self.info.version,
            }));
        }

        let data: Vec<serde_json::Value> = data_to_export.iter()
            .map(|msg| json!({
                "header": {
                    "scan_mode": format!("{:?}", msg.header.scan_mode),
                    "timestamp": msg.header.timestamp,
                },
                "data": msg.data,
            }))
            .collect();

        export_data.insert("scan_results", json!(data));

        serde_yaml::to_string(&export_data)
            .map_err(|e| PluginError::execution_failed(format!("YAML serialization failed: {}", e)))
    }

    /// Export data as HTML
    fn export_html(&self) -> PluginResult<String> {
        let mut html_content = String::new();
        
        // HTML header
        html_content.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
        html_content.push_str("  <meta charset=\"UTF-8\">\n");
        html_content.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
        html_content.push_str("  <title>Git Analytics Report</title>\n");
        html_content.push_str("  <style>\n");
        html_content.push_str(self.get_html_styles());
        html_content.push_str("  </style>\n");
        html_content.push_str("</head>\n<body>\n");

        // HTML body
        html_content.push_str("  <div class=\"container\">\n");
        html_content.push_str("    <h1>Git Analytics Report</h1>\n");

        if self.export_config.include_metadata {
            html_content.push_str("    <div class=\"metadata\">\n");
            html_content.push_str(&format!("      <p><strong>Generated:</strong> {:?}</p>\n", std::time::SystemTime::now()));
            html_content.push_str(&format!("      <p><strong>Total Entries:</strong> {}</p>\n", self.collected_data.len()));
            html_content.push_str(&format!("      <p><strong>Plugin Version:</strong> {}</p>\n", self.info.version));
            html_content.push_str("    </div>\n");
        }

        let data_to_export = self.get_data_to_export();

        // Group data by scan mode for better presentation
        let mut grouped_data: HashMap<String, Vec<&ScanMessage>> = HashMap::new();
        for message in &data_to_export {
            let scan_mode_str = format!("{:?}", message.header.scan_mode);
            grouped_data.entry(scan_mode_str)
                .or_default()
                .push(message);
        }

        for (scan_mode, messages) in grouped_data {
            html_content.push_str(&format!("    <div class=\"section\">\n"));
            html_content.push_str(&format!("      <h2>{}</h2>\n", scan_mode));
            html_content.push_str("      <div class=\"entries\">\n");
            
            for message in messages {
                html_content.push_str("        <div class=\"entry\">\n");
                html_content.push_str(&format!("          <div class=\"timestamp\">{}</div>\n", message.header.timestamp));
                html_content.push_str("          <div class=\"data\">\n");
                
                // Serialize MessageData to JSON and then display as HTML
                match serde_json::to_value(&message.data) {
                    Ok(json_value) => {
                        if let serde_json::Value::Object(map) = json_value {
                            for (key, value) in map {
                                let value_str = match value {
                                    serde_json::Value::String(s) => s,
                                    _ => value.to_string(),
                                };
                                html_content.push_str(&format!("            <div class=\"data-item\"><strong>{}:</strong> {}</div>\n", 
                                    key, self.escape_html(&value_str)));
                            }
                        }
                    }
                    Err(_) => {
                        html_content.push_str("            <div class=\"data-item\"><strong>Error:</strong> Failed to serialize data</div>\n");
                    }
                }
                
                html_content.push_str("          </div>\n");
                html_content.push_str("        </div>\n");
            }
            
            html_content.push_str("      </div>\n");
            html_content.push_str("    </div>\n");
        }

        html_content.push_str("  </div>\n");
        html_content.push_str("</body>\n</html>\n");

        Ok(html_content)
    }

    /// Export data as Markdown
    fn export_markdown(&self) -> PluginResult<String> {
        let mut md_content = String::new();
        
        // Markdown header
        md_content.push_str("# Git Analytics Report\n\n");

        if self.export_config.include_metadata {
            md_content.push_str("## Report Metadata\n\n");
            md_content.push_str(&format!("- **Generated:** {:?}\n", std::time::SystemTime::now()));
            md_content.push_str(&format!("- **Total Entries:** {}\n", self.collected_data.len()));
            md_content.push_str(&format!("- **Plugin Version:** {}\n\n", self.info.version));
        }

        let data_to_export = self.get_data_to_export();

        // Group data by scan mode for better presentation
        let mut grouped_data: HashMap<String, Vec<&ScanMessage>> = HashMap::new();
        for message in &data_to_export {
            let scan_mode_str = format!("{:?}", message.header.scan_mode);
            // Strip "ScanMode(" prefix and ")" suffix if present for cleaner display
            let clean_mode = if scan_mode_str.starts_with("ScanMode(") && scan_mode_str.ends_with(')') {
                scan_mode_str[9..scan_mode_str.len()-1].to_string()
            } else {
                scan_mode_str
            };
            grouped_data.entry(clean_mode)
                .or_default()
                .push(message);
        }

        for (scan_mode, messages) in grouped_data {
            md_content.push_str(&format!("## {}\n\n", scan_mode));
            
            for (idx, message) in messages.iter().enumerate() {
                md_content.push_str(&format!("### Entry {} ({})\n\n", idx + 1, message.header.timestamp));
                
                // Display MessageData fields based on the message type
                use crate::scanner::messages::MessageData;
                match &message.data {
                    MessageData::CommitInfo { hash, author, message: commit_msg, timestamp, changed_files } => {
                        md_content.push_str(&format!("- **Hash:** {}\n", hash));
                        md_content.push_str(&format!("- **Author:** {}\n", author));
                        md_content.push_str(&format!("- **Message:** {}\n", commit_msg));
                        md_content.push_str(&format!("- **Timestamp:** {}\n", timestamp));
                        md_content.push_str(&format!("- **Changed Files:** {}\n", changed_files.len()));
                        for file in changed_files {
                            md_content.push_str(&format!("  - **{}:** +{} -{}\n", file.path, file.lines_added, file.lines_removed));
                        }
                    }
                    MessageData::FileInfo { path, size, lines } => {
                        md_content.push_str(&format!("- **Path:** {}\n", path));
                        md_content.push_str(&format!("- **Size:** {} bytes\n", size));
                        md_content.push_str(&format!("- **Lines:** {}\n", lines));
                    }
                    MessageData::MetricInfo { file_count, line_count, complexity } => {
                        md_content.push_str(&format!("- **File Count:** {}\n", file_count));
                        md_content.push_str(&format!("- **Line Count:** {}\n", line_count));
                        md_content.push_str(&format!("- **Complexity:** {:.2}\n", complexity));
                    }
                    MessageData::SecurityInfo { vulnerability, severity, location } => {
                        md_content.push_str(&format!("- **Vulnerability:** {}\n", vulnerability));
                        md_content.push_str(&format!("- **Severity:** {}\n", severity));
                        md_content.push_str(&format!("- **Location:** {}\n", location));
                    }
                    MessageData::DependencyInfo { name, version, license } => {
                        md_content.push_str(&format!("- **Name:** {}\n", name));
                        md_content.push_str(&format!("- **Version:** {}\n", version));
                        if let Some(lic) = license {
                            md_content.push_str(&format!("- **License:** {}\n", lic));
                        }
                    }
                    _ => {
                        // Fallback to JSON serialization for other types
                        match serde_json::to_value(&message.data) {
                            Ok(json_value) => {
                                if let serde_json::Value::Object(map) = json_value {
                                    for (key, value) in map {
                                        let value_str = match value {
                                            serde_json::Value::String(s) => s,
                                            _ => value.to_string(),
                                        };
                                        md_content.push_str(&format!("- **{}:** {}\n", key, value_str));
                                    }
                                }
                            }
                            Err(_) => {
                                md_content.push_str("- **Error:** Failed to serialize data\n");
                            }
                        }
                    }
                }
                
                md_content.push_str("\n");
            }
        }

        Ok(md_content)
    }

    /// Get CSS styles for HTML export
    fn get_html_styles(&self) -> &'static str {
        r#"
    body { font-family: Arial, sans-serif; margin: 0; padding: 20px; background-color: #f5f5f5; }
    .container { max-width: 1200px; margin: 0 auto; background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
    h1 { color: #333; border-bottom: 2px solid #007bff; padding-bottom: 10px; }
    h2 { color: #555; margin-top: 30px; }
    .metadata { background: #f8f9fa; padding: 15px; border-radius: 5px; margin-bottom: 20px; }
    .metadata p { margin: 5px 0; }
    .section { margin-bottom: 30px; }
    .entries { display: grid; gap: 15px; }
    .entry { background: #fff; border: 1px solid #ddd; border-radius: 5px; padding: 15px; }
    .timestamp { font-size: 0.9em; color: #666; margin-bottom: 10px; }
    .data-item { margin: 5px 0; }
    .data-item strong { color: #007bff; }
"#
    }

    /// Escape HTML entities
    fn escape_html(&self, text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;")
    }

    /// Escape XML entities
    fn escape_xml(&self, text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    /// Get export statistics
    pub fn get_export_stats(&self) -> HashMap<String, serde_json::Value> {
        let mut stats = HashMap::new();
        stats.insert("total_entries".to_string(), json!(self.collected_data.len()));
        stats.insert("output_format".to_string(), json!(format!("{:?}", self.export_config.output_format)));
        stats.insert("output_path".to_string(), json!(self.export_config.output_path));
        stats.insert("include_metadata".to_string(), json!(self.export_config.include_metadata));
        
        // Group by scan mode
        let mut mode_counts: HashMap<String, usize> = HashMap::new();
        for message in &self.collected_data {
            let scan_mode_str = format!("{:?}", message.header.scan_mode);
            *mode_counts.entry(scan_mode_str).or_insert(0) += 1;
        }
        stats.insert("scan_mode_counts".to_string(), json!(mode_counts));
        
        stats
    }
    
    /// Execute data export function (using configured format)
    async fn execute_data_export(&self) -> PluginResult<PluginResponse> {
        let exported_data = self.export_data().await?;
        let format_name = format!("{:?}", self.export_config.output_format).to_lowercase();
        
        let data = json!({
            "exported_data": exported_data,
            "format": format_name,
            "entries_count": self.collected_data.len(),
            "output_path": self.export_config.output_path,
            "function": "export"
        });
        
        Ok(PluginResponse::Execute {
            request_id: "data_export".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_ms: 0,
                memory_used: 0,
                items_processed: self.collected_data.len() as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
    
    /// Execute JSON export function
    async fn execute_json_export(&self) -> PluginResult<PluginResponse> {
        let json_data = self.export_json()?;
        
        let data = json!({
            "json_data": json_data,
            "entries_count": self.collected_data.len(),
            "function": "json"
        });
        
        Ok(PluginResponse::Execute {
            request_id: "json_export".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_ms: 0,
                memory_used: 0,
                items_processed: self.collected_data.len() as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
    
    /// Execute CSV export function
    async fn execute_csv_export(&self) -> PluginResult<PluginResponse> {
        let csv_data = self.export_csv()?;
        
        let data = json!({
            "csv_data": csv_data,
            "entries_count": self.collected_data.len(),
            "function": "csv"
        });
        
        Ok(PluginResponse::Execute {
            request_id: "csv_export".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_ms: 0,
                memory_used: 0,
                items_processed: self.collected_data.len() as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
    
    /// Execute HTML export function
    async fn execute_html_export(&self) -> PluginResult<PluginResponse> {
        let html_data = self.export_html()?;
        
        let data = json!({
            "html_data": html_data,
            "entries_count": self.collected_data.len(),
            "function": "html"
        });
        
        Ok(PluginResponse::Execute {
            request_id: "html_export".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_ms: 0,
                memory_used: 0,
                items_processed: self.collected_data.len() as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
    
    /// Execute XML export function
    async fn execute_xml_export(&self) -> PluginResult<PluginResponse> {
        let xml_data = self.export_xml()?;
        
        let data = json!({
            "xml_data": xml_data,
            "entries_count": self.collected_data.len(),
            "function": "xml"
        });
        
        Ok(PluginResponse::Execute {
            request_id: "xml_export".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_ms: 0,
                memory_used: 0,
                items_processed: self.collected_data.len() as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
    
    /// Execute YAML export function
    async fn execute_yaml_export(&self) -> PluginResult<PluginResponse> {
        let yaml_data = self.export_yaml()?;
        
        let data = json!({
            "yaml_data": yaml_data,
            "entries_count": self.collected_data.len(),
            "function": "yaml"
        });
        
        Ok(PluginResponse::Execute {
            request_id: "yaml_export".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_ms: 0,
                memory_used: 0,
                items_processed: self.collected_data.len() as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
    
    /// Execute Markdown export function
    async fn execute_markdown_export(&self) -> PluginResult<PluginResponse> {
        let markdown_data = self.export_markdown()?;
        
        let data = json!({
            "markdown_data": markdown_data,
            "entries_count": self.collected_data.len(),
            "function": "markdown"
        });
        
        Ok(PluginResponse::Execute {
            request_id: "markdown_export".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_ms: 0,
                memory_used: 0,
                items_processed: self.collected_data.len() as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
}

impl Default for ExportPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            output_format: ExportFormat::Json,
            output_path: "gstats_export.json".to_string(),
            include_metadata: true,
            max_entries: None,
            output_all: false,
            csv_delimiter: ",".to_string(),
            csv_quote_char: "\"".to_string(),
        }
    }
}

#[async_trait]
impl Plugin for ExportPlugin {
    fn plugin_info(&self) -> &PluginInfo {
        &self.info
    }

    async fn initialize(&mut self, context: &PluginContext) -> PluginResult<()> {
        if self.initialized {
            return Err(PluginError::initialization_failed("Plugin already initialized"));
        }

        // Configure from context if available
        if let Some(config) = context.plugin_config.get("export") {
            if let Some(format_str) = config.get("format").and_then(|v| v.as_str()) {
                self.export_config.output_format = match format_str.to_lowercase().as_str() {
                    "json" => ExportFormat::Json,
                    "csv" => ExportFormat::Csv,
                    "xml" => ExportFormat::Xml,
                    "yaml" => ExportFormat::Yaml,
                    "html" => ExportFormat::Html,
                    "markdown" | "md" => ExportFormat::Markdown,
                    _ => ExportFormat::Json,
                };
            }

            if let Some(path) = config.get("output_path").and_then(|v| v.as_str()) {
                self.export_config.output_path = path.to_string();
            }

            if let Some(metadata) = config.get("include_metadata").and_then(|v| v.as_bool()) {
                self.export_config.include_metadata = metadata;
            }

            if let Some(max) = config.get("max_entries").and_then(|v| v.as_u64()) {
                self.export_config.max_entries = Some(max as usize);
            }
        }

        self.collected_data.clear();
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
                    crate::plugin::InvocationType::Direct => self.default_function().unwrap_or("export"),
                    crate::plugin::InvocationType::Default => "export",
                };
                
                // Route to appropriate function
                match function_name {
                    "export" | "save" | "output" => {
                        self.execute_data_export().await
                    }
                    "json" => {
                        self.execute_json_export().await
                    }
                    "csv" => {
                        self.execute_csv_export().await
                    }
                    "html" | "report" => {
                        self.execute_html_export().await
                    }
                    "xml" => {
                        self.execute_xml_export().await
                    }
                    "yaml" => {
                        self.execute_yaml_export().await
                    }
                    "markdown" | "md" => {
                        self.execute_markdown_export().await
                    }
                    _ => Err(PluginError::execution_failed(
                        format!("Unknown function: {}", function_name)
                    )),
                }
            }
            PluginRequest::GetStatistics => {
                // Create a metric info containing export statistics
                let data = MessageData::MetricInfo {
                    file_count: self.collected_data.len() as u32,
                    line_count: 0, // Not applicable for export
                    complexity: 0.0, // Not applicable for export
                };

                let header = MessageHeader::new(
                    ScanMode::FILES, // Use a generic scan mode
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                );

                let stats_message = ScanMessage::new(header, data);
                Ok(PluginResponse::Statistics(stats_message))
            }
            PluginRequest::GetCapabilities => {
                Ok(PluginResponse::Capabilities(self.info.capabilities.clone()))
            }
            PluginRequest::Export => {
                let exported_data = self.export_data().await?;
                Ok(PluginResponse::Data(exported_data))
            }
            _ => Err(PluginError::execution_failed("Unsupported request type")),
        }
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        self.initialized = false;
        self.collected_data.clear();
        self.export_config = ExportConfig::default();
        Ok(())
    }
    
    /// Get all functions this plugin can handle
    fn advertised_functions(&self) -> Vec<PluginFunction> {
        vec![
            PluginFunction {
                name: "export".to_string(),
                aliases: vec!["save".to_string(), "output".to_string()],
                description: "Export data in the configured format (JSON by default)".to_string(),
                is_default: true,
            },
            PluginFunction {
                name: "json".to_string(),
                aliases: vec![],
                description: "Export data as structured JSON format".to_string(),
                is_default: false,
            },
            PluginFunction {
                name: "csv".to_string(),
                aliases: vec![],
                description: "Export data as comma-separated values for spreadsheet applications".to_string(),
                is_default: false,
            },
            PluginFunction {
                name: "html".to_string(),
                aliases: vec!["report".to_string()],
                description: "Generate HTML reports with interactive visualizations".to_string(),
                is_default: false,
            },
            PluginFunction {
                name: "xml".to_string(),
                aliases: vec![],
                description: "Export data as XML format".to_string(),
                is_default: false,
            },
            PluginFunction {
                name: "yaml".to_string(),
                aliases: vec![],
                description: "Export data as YAML format".to_string(),
                is_default: false,
            },
            PluginFunction {
                name: "markdown".to_string(),
                aliases: vec!["md".to_string()],
                description: "Export data as Markdown format".to_string(),
                is_default: false,
            },
        ]
    }
    
    /// Get the default function name
    fn default_function(&self) -> Option<&str> {
        Some("export")
    }
}

#[async_trait]
impl PluginArgumentParser for ExportPlugin {
    async fn parse_plugin_args(&mut self, args: &[String]) -> PluginResult<()> {
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--output" | "-o" => {
                    if i + 1 < args.len() {
                        self.export_config.output_path = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err(PluginError::execution_failed("--output requires a value"));
                    }
                }
                "--format" | "-f" => {
                    if i + 1 < args.len() {
                        self.export_config.output_format = match args[i + 1].to_lowercase().as_str() {
                            "json" => ExportFormat::Json,
                            "csv" => ExportFormat::Csv,
                            "xml" => ExportFormat::Xml,
                            "yaml" => ExportFormat::Yaml,
                            "html" => ExportFormat::Html,
                            "markdown" | "md" => ExportFormat::Markdown,
                            _ => return Err(PluginError::execution_failed(
                                format!("Unknown format: {}", args[i + 1])
                            )),
                        };
                        i += 2;
                    } else {
                        return Err(PluginError::execution_failed("--format requires a value"));
                    }
                }
                "--all" => {
                    self.export_config.output_all = true;
                    i += 1;
                }
                "--output-limit" => {
                    if i + 1 < args.len() {
                        match args[i + 1].parse::<usize>() {
                            Ok(limit) => {
                                self.export_config.max_entries = Some(limit);
                                self.export_config.output_all = false; // --output-limit overrides --all
                                i += 2;
                            }
                            Err(_) => {
                                return Err(PluginError::execution_failed(
                                    format!("Invalid limit value: {}", args[i + 1])
                                ));
                            }
                        }
                    } else {
                        return Err(PluginError::execution_failed("--output-limit requires a value"));
                    }
                }
                "--csv-delimiter" => {
                    if i + 1 < args.len() {
                        self.export_config.csv_delimiter = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err(PluginError::execution_failed("--csv-delimiter requires a value"));
                    }
                }
                "--csv-quote" => {
                    if i + 1 < args.len() {
                        self.export_config.csv_quote_char = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err(PluginError::execution_failed("--csv-quote requires a value"));
                    }
                }
                "--include-metadata" => {
                    self.export_config.include_metadata = true;
                    i += 1;
                }
                "--no-metadata" => {
                    self.export_config.include_metadata = false;
                    i += 1;
                }
                _ => {
                    return Err(PluginError::execution_failed(
                        format!("Unknown argument: {}", args[i])
                    ));
                }
            }
        }
        Ok(())
    }

    fn get_arg_schema(&self) -> Vec<PluginArgDefinition> {
        vec![
            PluginArgDefinition {
                name: "--output, -o".to_string(),
                description: "Output file path".to_string(),
                required: false,
                default_value: Some("gstats_export.json".to_string()),
                arg_type: "string".to_string(),
                examples: vec!["data.json".to_string(), "report.csv".to_string()],
            },
            PluginArgDefinition {
                name: "--format, -f".to_string(),
                description: "Output format (json, csv, xml, yaml, html, markdown)".to_string(),
                required: false,
                default_value: Some("json".to_string()),
                arg_type: "string".to_string(),
                examples: vec!["json".to_string(), "csv".to_string(), "html".to_string()],
            },
            PluginArgDefinition {
                name: "--all".to_string(),
                description: "Export all entries (override default limit of 10)".to_string(),
                required: false,
                default_value: Some("false".to_string()),
                arg_type: "boolean".to_string(),
                examples: vec![],
            },
            PluginArgDefinition {
                name: "--output-limit".to_string(),
                description: "Limit number of entries to export (overrides --all)".to_string(),
                required: false,
                default_value: None,
                arg_type: "number".to_string(),
                examples: vec!["100".to_string(), "500".to_string()],
            },
            PluginArgDefinition {
                name: "--csv-delimiter".to_string(),
                description: "CSV field delimiter character".to_string(),
                required: false,
                default_value: Some(",".to_string()),
                arg_type: "string".to_string(),
                examples: vec![",".to_string(), "\t".to_string(), ";".to_string()],
            },
            PluginArgDefinition {
                name: "--csv-quote".to_string(),
                description: "CSV quote character for field values".to_string(),
                required: false,
                default_value: Some("\"".to_string()),
                arg_type: "string".to_string(),
                examples: vec!["\"".to_string(), "'".to_string()],
            },
            PluginArgDefinition {
                name: "--include-metadata".to_string(),
                description: "Include export metadata in output".to_string(),
                required: false,
                default_value: Some("true".to_string()),
                arg_type: "boolean".to_string(),
                examples: vec![],
            },
            PluginArgDefinition {
                name: "--no-metadata".to_string(),
                description: "Exclude export metadata from output".to_string(),
                required: false,
                default_value: Some("false".to_string()),
                arg_type: "boolean".to_string(),
                examples: vec![],
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::context::PluginContext;
    use crate::scanner::messages::MessageHeader;

    fn create_test_message(scan_mode: ScanMode, data: MessageData) -> ScanMessage {
        let header = MessageHeader::new(
            scan_mode,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        
        ScanMessage::new(header, data)
    }

    fn create_test_context() -> PluginContext {
        let repo = crate::git::resolve_repository_handle(None).unwrap();
        let scanner_config = std::sync::Arc::new(crate::scanner::ScannerConfig::default());
        let query_params = std::sync::Arc::new(crate::scanner::QueryParams::default());
        
        PluginContext::new(
            scanner_config,
            std::sync::Arc::new(repo),
            query_params,
        )
    }

    #[tokio::test]
    async fn test_export_plugin_creation() {
        let plugin = ExportPlugin::new();
        assert_eq!(plugin.plugin_info().name, "export");
        assert_eq!(plugin.plugin_info().plugin_type, PluginType::Output);
        assert!(!plugin.initialized);
    }

    #[tokio::test]
    async fn test_export_plugin_initialization() {
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();

        assert!(plugin.initialize(&context).await.is_ok());
        assert!(plugin.initialized);
    }

    #[tokio::test]
    async fn test_export_plugin_configuration() {
        let mut plugin = ExportPlugin::new();
        
        assert!(plugin.configure(ExportFormat::Csv, "output.csv").is_ok());
        assert_eq!(plugin.export_config.output_format, ExportFormat::Csv);
        assert_eq!(plugin.export_config.output_path, "output.csv");
    }

    #[tokio::test]
    async fn test_export_plugin_add_data() {
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        let data = MessageData::FileInfo {
            path: "test.txt".to_string(),
            size: 100,
            lines: 10,
        };
        let message = create_test_message(ScanMode::FILES, data);

        assert!(plugin.add_data(message).is_ok());
        assert_eq!(plugin.collected_data.len(), 1);
    }

    #[tokio::test]
    async fn test_export_plugin_json_export() {
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        // Add test data
        let data = MessageData::FileInfo {
            path: "test.rs".to_string(),
            size: 200,
            lines: 20,
        };
        let message = create_test_message(ScanMode::FILES, data);
        plugin.add_data(message).unwrap();

        let json_output = plugin.export_json().unwrap();
        assert!(json_output.contains("scan_results"));
        assert!(json_output.contains("metadata"));
        assert!(json_output.contains("FILES"));
        assert!(json_output.contains("test.rs"));
    }

    #[tokio::test]
    async fn test_export_plugin_csv_export() {
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        let data = MessageData::CommitInfo {
            hash: "abc123".to_string(),
            author: "test@example.com".to_string(),
            message: "Test commit".to_string(),
            timestamp: 123456789,
            changed_files: vec![crate::scanner::messages::FileChangeData {
                path: "src/main.rs".to_string(),
                lines_added: 12,
                lines_removed: 4,
            }],
        };
        let message = create_test_message(ScanMode::HISTORY, data);
        plugin.add_data(message).unwrap();

        let csv_output = plugin.export_csv().unwrap();
        assert!(csv_output.contains("timestamp,scan_mode,data_json"));
        assert!(csv_output.contains("HISTORY"));
        assert!(csv_output.contains("Test commit"));
    }

    #[tokio::test]
    async fn test_export_plugin_html_export() {
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        let data = MessageData::MetricInfo {
            file_count: 1,
            line_count: 100,
            complexity: 2.5,
        };
        let message = create_test_message(ScanMode::FILES, data);
        plugin.add_data(message).unwrap();

        let html_output = plugin.export_html().unwrap();
        assert!(html_output.contains("<!DOCTYPE html>"));
        assert!(html_output.contains("Git Analytics Report"));
        assert!(html_output.contains("FILES"));
        assert!(html_output.contains("file_count"));
    }

    #[tokio::test]
    async fn test_export_plugin_xml_export() {
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        let data = MessageData::SecurityInfo {
            vulnerability: "XSS".to_string(),
            severity: "High".to_string(),
            location: "line 42".to_string(),
        };
        let message = create_test_message(ScanMode::SECURITY, data);
        plugin.add_data(message).unwrap();

        let xml_output = plugin.export_xml().unwrap();
        assert!(xml_output.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml_output.contains("<scan_results>"));
        println!("XML output: {}", xml_output);
        assert!(xml_output.contains("SECURITY"));
        assert!(xml_output.contains("XSS"));
    }

    #[tokio::test]
    async fn test_export_plugin_yaml_export() {
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        let data = MessageData::DependencyInfo {
            name: "tokio".to_string(),
            version: "1.0.0".to_string(),
            license: Some("MIT".to_string()),
        };
        let message = create_test_message(ScanMode::DEPENDENCIES, data);
        plugin.add_data(message).unwrap();

        let yaml_output = plugin.export_yaml().unwrap();
        assert!(yaml_output.contains("scan_results"));
        assert!(yaml_output.contains("DEPENDENCIES"));
        assert!(yaml_output.contains("tokio"));
    }

    #[tokio::test]
    async fn test_export_plugin_execute() {
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        // Test get capabilities
        let response = plugin.execute(PluginRequest::GetCapabilities).await.unwrap();
        match response {
            PluginResponse::Capabilities(caps) => {
                assert_eq!(caps.len(), 4);
                assert!(caps.iter().any(|c| c.name == "json_export"));
            }
            _ => panic!("Unexpected response type"),
        }

        // Test export
        let response = plugin.execute(PluginRequest::Export).await.unwrap();
        match response {
            PluginResponse::Data(data) => {
                assert!(data.contains("scan_results"));
            }
            _ => panic!("Unexpected response type"),
        }
    }

    #[tokio::test]
    async fn test_export_plugin_cleanup() {
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        // Add some data
        let data = MessageData::FileInfo {
            path: "test.txt".to_string(),
            size: 50,
            lines: 5,
        };
        let message = create_test_message(ScanMode::FILES, data);
        plugin.add_data(message).unwrap();

        assert!(plugin.cleanup().await.is_ok());
        assert!(!plugin.initialized);
        assert!(plugin.collected_data.is_empty());
    }

    #[tokio::test]
    async fn test_export_plugin_escape_functions() {
        let plugin = ExportPlugin::new();
        
        // Test HTML escaping
        assert_eq!(plugin.escape_html("<script>alert('test')</script>"), 
                   "&lt;script&gt;alert(&#x27;test&#x27;)&lt;/script&gt;");
        
        // Test XML escaping
        assert_eq!(plugin.escape_xml("AT&T \"quoted\" text"), 
                   "AT&amp;T &quot;quoted&quot; text");
    }

    #[tokio::test]
    async fn test_plugin_argument_parsing() {
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        // Test --all argument
        let args = vec!["--all".to_string()];
        assert!(plugin.parse_plugin_args(&args).await.is_ok());
        assert!(plugin.export_config.output_all);

        // Reset plugin
        let mut plugin = ExportPlugin::new();
        plugin.initialize(&context).await.unwrap();

        // Test --output-limit argument
        let args = vec!["--output-limit".to_string(), "50".to_string()];
        assert!(plugin.parse_plugin_args(&args).await.is_ok());
        assert_eq!(plugin.export_config.max_entries, Some(50));
        assert!(!plugin.export_config.output_all); // Should be false when limit is set

        // Reset plugin
        let mut plugin = ExportPlugin::new();
        plugin.initialize(&context).await.unwrap();

        // Test CSV-specific options
        let args = vec![
            "--csv-delimiter".to_string(), ";".to_string(),
            "--csv-quote".to_string(), "'".to_string()
        ];
        assert!(plugin.parse_plugin_args(&args).await.is_ok());
        assert_eq!(plugin.export_config.csv_delimiter, ";");
        assert_eq!(plugin.export_config.csv_quote_char, "'");

        // Reset plugin
        let mut plugin = ExportPlugin::new();
        plugin.initialize(&context).await.unwrap();

        // Test format and output arguments
        let args = vec![
            "--format".to_string(), "markdown".to_string(),
            "--output".to_string(), "report.md".to_string()
        ];
        assert!(plugin.parse_plugin_args(&args).await.is_ok());
        assert_eq!(plugin.export_config.output_format, ExportFormat::Markdown);
        assert_eq!(plugin.export_config.output_path, "report.md");

        // Reset plugin
        let mut plugin = ExportPlugin::new();
        plugin.initialize(&context).await.unwrap();

        // Test invalid format
        let args = vec!["--format".to_string(), "invalid".to_string()];
        assert!(plugin.parse_plugin_args(&args).await.is_err());
    }

    #[tokio::test]
    async fn test_markdown_export() {
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        // Add test data
        let data = MessageData::CommitInfo {
            hash: "abc123".to_string(),
            author: "test@example.com".to_string(),
            message: "Test commit".to_string(),
            timestamp: 123456789,
            changed_files: vec![crate::scanner::messages::FileChangeData {
                path: "src/main.rs".to_string(),
                lines_added: 12,
                lines_removed: 4,
            }],
        };
        let message = create_test_message(ScanMode::HISTORY, data);
        plugin.add_data(message).unwrap();

        let markdown_output = plugin.export_markdown().unwrap();
        assert!(markdown_output.contains("# Git Analytics Report"));
        assert!(markdown_output.contains("## Report Metadata"));
        assert!(markdown_output.contains("## HISTORY"));
        assert!(markdown_output.contains("- **Message:** Test commit"));
        assert!(markdown_output.contains("- **Hash:** abc123"));
    }

    #[tokio::test]
    async fn test_get_arg_schema() {
        let plugin = ExportPlugin::new();
        let schema = plugin.get_arg_schema();
        
        assert!(!schema.is_empty());
        assert!(schema.iter().any(|arg| arg.name.contains("--output")));
        assert!(schema.iter().any(|arg| arg.name.contains("--format")));
        assert!(schema.iter().any(|arg| arg.name.contains("--all")));
        assert!(schema.iter().any(|arg| arg.name.contains("--output-limit")));
        assert!(schema.iter().any(|arg| arg.name.contains("--csv-delimiter")));
    }
}