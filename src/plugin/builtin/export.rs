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
use std::path::{Path, PathBuf};
use serde_json::json;
use tera::{Tera, Context};

/// Data export plugin for various output formats
pub struct ExportPlugin {
    info: PluginInfo,
    initialized: bool,
    export_config: ExportConfig,
    collected_data: Vec<ScanMessage>,
    template_engine: TemplateEngine,
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
    template_file: Option<PathBuf>,
}

/// Template engine for custom output formatting using Tera (Jinja2-like syntax)
struct TemplateEngine {
    tera: Tera,
    template_path: Option<PathBuf>,
    template_vars: HashMap<String, String>,
}

impl TemplateEngine {
    fn new() -> Self {
        let mut tera = Tera::default();
        // Disable auto-escaping by default (users can use |safe or |escape filters as needed)
        tera.autoescape_on(vec![]);
        
        Self {
            tera,
            template_path: None,
            template_vars: HashMap::new(),
        }
    }
    
    fn load_template(&mut self, template_path: &Path) -> PluginResult<()> {
        let content = std::fs::read_to_string(template_path)
            .map_err(|e| PluginError::execution_failed(format!("Failed to read template file: {}", e)))?;
        
        // Add the template to Tera with a name based on the file path
        let template_name = template_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("template");
            
        self.tera.add_raw_template(template_name, &content)
            .map_err(|e| PluginError::configuration_error(format!("Template syntax error: {}", e)))?;
            
        self.template_path = Some(template_path.to_path_buf());
        
        // Register custom filters
        self.register_custom_filters();
        
        Ok(())
    }
    
    fn register_custom_filters(&mut self) {
        // Register a number formatting filter
        self.tera.register_filter("number_format", |value: &tera::Value, _: &HashMap<String, tera::Value>| {
            match value {
                tera::Value::Number(n) => {
                    if let Some(i) = n.as_u64() {
                        // Simple thousands separator formatting
                        let s = i.to_string();
                        let mut result = String::new();
                        let chars: Vec<char> = s.chars().collect();
                        for (pos, ch) in chars.iter().rev().enumerate() {
                            if pos > 0 && pos % 3 == 0 {
                                result.push(',');
                            }
                            result.push(*ch);
                        }
                        Ok(tera::Value::String(result.chars().rev().collect()))
                    } else if let Some(f) = n.as_f64() {
                        Ok(tera::Value::String(format!("{:.2}", f)))
                    } else {
                        Ok(value.clone())
                    }
                }
                _ => Ok(value.clone()),
            }
        });
        
        // Register a percentage filter
        self.tera.register_filter("percentage", |value: &tera::Value, args: &HashMap<String, tera::Value>| {
            let precision = args.get("precision")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as usize;
            
            match value {
                tera::Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        Ok(tera::Value::String(format!("{:.precision$}%", f, precision = precision)))
                    } else {
                        Ok(value.clone())
                    }
                }
                _ => Ok(value.clone()),
            }
        });
        
        // Register a slice filter for arrays
        self.tera.register_filter("slice", |value: &tera::Value, args: &HashMap<String, tera::Value>| {
            match value {
                tera::Value::Array(arr) => {
                    let start = args.get("start")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                    let end = args.get("end")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as usize)
                        .unwrap_or(arr.len());
                    
                    let sliced: Vec<tera::Value> = arr.iter()
                        .skip(start)
                        .take(end.saturating_sub(start))
                        .cloned()
                        .collect();
                    
                    Ok(tera::Value::Array(sliced))
                }
                tera::Value::String(s) => {
                    let start = args.get("start")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                    let end = args.get("end")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as usize)
                        .unwrap_or(s.len());
                    
                    let chars: Vec<char> = s.chars().collect();
                    let sliced: String = chars.iter()
                        .skip(start)
                        .take(end.saturating_sub(start))
                        .collect();
                    
                    Ok(tera::Value::String(sliced))
                }
                _ => Ok(value.clone()),
            }
        });
        
        // Register a first_line filter to extract the first line of multi-line strings
        self.tera.register_filter("first_line", |value: &tera::Value, _: &HashMap<String, tera::Value>| {
            match value {
                tera::Value::String(s) => {
                    let first_line = s.lines().next().unwrap_or("").trim().to_string();
                    Ok(tera::Value::String(first_line))
                }
                _ => Ok(value.clone()),
            }
        });
        
        // Register a round filter for rounding numbers
        self.tera.register_filter("round", |value: &tera::Value, args: &HashMap<String, tera::Value>| {
            let precision = args.get("precision")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as i32;
                
            match value {
                tera::Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        let multiplier = 10.0_f64.powi(precision);
                        let rounded = (f * multiplier).round() / multiplier;
                        Ok(tera::Value::Number(serde_json::Number::from_f64(rounded).unwrap_or(serde_json::Number::from(0))))
                    } else {
                        Ok(value.clone())
                    }
                }
                _ => Ok(value.clone()),
            }
        });
        
        // Register a simple date filter (just format timestamp)
        self.tera.register_filter("date", |value: &tera::Value, _: &HashMap<String, tera::Value>| {
            match value {
                tera::Value::String(s) => {
                    // For now, just return a simplified format since we're getting ISO timestamps
                    if let Ok(datetime) = chrono::DateTime::parse_from_rfc3339(s) {
                        Ok(tera::Value::String(datetime.format("%Y-%m-%d %H:%M UTC").to_string()))
                    } else {
                        Ok(value.clone())
                    }
                }
                _ => Ok(value.clone()),
            }
        });
    }
    
    fn add_template_var(&mut self, key: String, value: String) {
        self.template_vars.insert(key, value);
    }
    
    fn render(&self, data: &serde_json::Value) -> PluginResult<String> {
        // Create a Tera context from the JSON data
        let mut context = Context::new();
        
        // Add all data from the JSON value
        if let Some(obj) = data.as_object() {
            for (key, value) in obj {
                context.insert(key, value);
            }
        }
        
        // Add template variables
        for (key, value) in &self.template_vars {
            context.insert(key, value);
        }
        
        // Get the template name
        let template_name = self.template_path.as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("template");
        
        // Render the template
        self.tera.render(template_name, &context)
            .map_err(|e| PluginError::execution_failed(format!("Template rendering error: {}", e)))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    Json,
    Csv,
    Xml,
    Yaml,
    Html,
    Markdown,
    Template,
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
        );

        Self {
            info,
            initialized: false,
            export_config: ExportConfig::default(),
            collected_data: Vec::new(),
            template_engine: TemplateEngine::new(),
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
        // Always collect all data - limit is applied during export in get_data_to_export()
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
            ExportFormat::Template => self.export_template(),
        }
    }

    /// Export data as JSON
    fn export_json(&self) -> PluginResult<String> {
        let mut export_data = HashMap::new();
        
        let data_to_export = self.get_data_to_export();

        if self.export_config.include_metadata {
            export_data.insert("metadata", json!({
                "export_timestamp": chrono::Utc::now().to_rfc3339(),
                "total_entries": self.collected_data.len(),
                "exported_entries": data_to_export.len(),
                "format": "json",
                "plugin_version": self.info.version,
            }));
        }

        // Check if this is primarily commit data (for authors reports)
        let is_commit_data = data_to_export.iter()
            .any(|msg| matches!(msg.data, crate::scanner::messages::MessageData::CommitInfo { .. }));

        if is_commit_data {
            // Generate authors summary for JSON
            let authors_summary = self.generate_authors_json_summary(&data_to_export);
            export_data.insert("authors", authors_summary);
        } else {
            // Regular data export for non-author reports
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
        }

        serde_json::to_string_pretty(&export_data)
            .map_err(|e| PluginError::execution_failed(format!("JSON serialization failed: {}", e)))
    }

    /// Export data as CSV
    fn export_csv(&self) -> PluginResult<String> {
        let mut csv_content = String::new();
        let delimiter = &self.export_config.csv_delimiter;
        let quote_char = &self.export_config.csv_quote_char;

        let data_to_export = self.get_data_to_export();

        // Check if this is primarily commit data (for authors reports)
        let is_commit_data = data_to_export.iter()
            .any(|msg| matches!(msg.data, crate::scanner::messages::MessageData::CommitInfo { .. }));

        if is_commit_data {
            // Generate authors summary for CSV
            self.export_authors_csv_summary(&mut csv_content, delimiter, quote_char)?;
        } else {
            // Regular data export for non-author reports
            csv_content.push_str(&format!("timestamp{}scan_mode{}data_json\n", delimiter, delimiter));

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
            xml_content.push_str(&format!("    <export_timestamp>{}</export_timestamp>\n", 
                chrono::Utc::now().to_rfc3339()));
            xml_content.push_str(&format!("    <total_entries>{}</total_entries>\n", self.collected_data.len()));
            xml_content.push_str(&format!("    <plugin_version>{}</plugin_version>\n", self.info.version));
            xml_content.push_str("  </metadata>\n");
        }

        let data_to_export = self.get_data_to_export();

        // Check if this is primarily commit data (for authors reports)
        let is_commit_data = data_to_export.iter()
            .any(|msg| matches!(msg.data, crate::scanner::messages::MessageData::CommitInfo { .. }));

        if is_commit_data {
            // Generate authors summary for XML
            self.export_authors_xml_summary(&mut xml_content, &data_to_export);
        } else {
            // Regular data export for non-author reports
            xml_content.push_str("  <entries>\n");
            for message in data_to_export {
                xml_content.push_str("    <entry>\n");
                xml_content.push_str(&format!("      <timestamp>{}</timestamp>\n", message.header.timestamp));
                xml_content.push_str(&format!("      <scan_mode>{:?}</scan_mode>\n", message.header.scan_mode));
                xml_content.push_str("      <data>\n");
                
                // Serialize MessageData to properly formatted XML based on type
                use crate::scanner::messages::MessageData;
                match &message.data {
                    MessageData::CommitInfo { hash, author, message: commit_msg, timestamp, changed_files } => {
                        xml_content.push_str(&format!("        <hash>{}</hash>\n", self.escape_xml(hash)));
                        xml_content.push_str(&format!("        <author>{}</author>\n", self.escape_xml(author)));
                        let first_line = commit_msg.lines().next().unwrap_or("").trim();
                        xml_content.push_str(&format!("        <message>{}</message>\n", self.escape_xml(first_line)));
                        xml_content.push_str(&format!("        <timestamp>{}</timestamp>\n", timestamp));
                        xml_content.push_str(&format!("        <files_changed>{}</files_changed>\n", changed_files.len()));
                        let total_added: u32 = changed_files.iter().map(|f| f.lines_added as u32).sum();
                        let total_removed: u32 = changed_files.iter().map(|f| f.lines_removed as u32).sum();
                        xml_content.push_str(&format!("        <lines_added>{}</lines_added>\n", total_added));
                        xml_content.push_str(&format!("        <lines_removed>{}</lines_removed>\n", total_removed));
                    }
                    MessageData::FileInfo { path, size, lines } => {
                        xml_content.push_str(&format!("        <path>{}</path>\n", self.escape_xml(path)));
                        xml_content.push_str(&format!("        <size>{}</size>\n", size));
                        xml_content.push_str(&format!("        <lines>{}</lines>\n", lines));
                    }
                    MessageData::MetricInfo { file_count, line_count, complexity } => {
                        xml_content.push_str(&format!("        <file_count>{}</file_count>\n", file_count));
                        xml_content.push_str(&format!("        <line_count>{}</line_count>\n", line_count));
                        xml_content.push_str(&format!("        <complexity>{}</complexity>\n", complexity));
                    }
                    _ => {
                        // Fallback for other types
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
                    }
                }
                
                xml_content.push_str("      </data>\n");
                xml_content.push_str("    </entry>\n");
            }
            xml_content.push_str("  </entries>\n");
        }
        
        xml_content.push_str("</scan_results>\n");

        Ok(xml_content)
    }

    /// Export data as YAML
    fn export_yaml(&self) -> PluginResult<String> {
        let mut export_data = HashMap::new();
        
        let data_to_export = self.get_data_to_export();

        if self.export_config.include_metadata {
            export_data.insert("metadata", json!({
                "export_timestamp": chrono::Utc::now().to_rfc3339(),
                "total_entries": self.collected_data.len(),
                "exported_entries": data_to_export.len(),
                "format": "yaml",
                "plugin_version": self.info.version,
            }));
        }

        // Check if this is primarily commit data (for authors reports)
        let is_commit_data = data_to_export.iter()
            .any(|msg| matches!(msg.data, crate::scanner::messages::MessageData::CommitInfo { .. }));

        if is_commit_data {
            // Generate authors summary for YAML (reuse JSON structure)
            let authors_summary = self.generate_authors_json_summary(&data_to_export);
            export_data.insert("authors", authors_summary);
        } else {
            // Regular data export for non-author reports
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
        }

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

        // Check if this is primarily commit data (for authors reports)
        let is_commit_data = data_to_export.iter()
            .any(|msg| matches!(msg.data, crate::scanner::messages::MessageData::CommitInfo { .. }));

        if is_commit_data {
            // Generate authors summary table
            self.export_authors_html_table(&mut html_content, &data_to_export);
        } else {
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
                
                // Display MessageData fields based on the message type
                use crate::scanner::messages::MessageData;
                match &message.data {
                    MessageData::CommitInfo { hash, author, message: commit_msg, timestamp, changed_files } => {
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>Hash:</strong> {}</div>\n", self.escape_html(hash)));
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>Author:</strong> {}</div>\n", self.escape_html(author)));
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>Message:</strong> {}</div>\n", self.escape_html(commit_msg)));
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>Timestamp:</strong> {}</div>\n", timestamp));
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>Changed Files:</strong> {}</div>\n", changed_files.len()));
                        for file in changed_files {
                            html_content.push_str(&format!("            <div class=\"data-item file-change\"><strong>{}:</strong> +{} -{} lines</div>\n", 
                                self.escape_html(&file.path), file.lines_added, file.lines_removed));
                        }
                    }
                    MessageData::FileInfo { path, size, lines } => {
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>Path:</strong> {}</div>\n", self.escape_html(path)));
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>Size:</strong> {} bytes</div>\n", size));
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>Lines:</strong> {}</div>\n", lines));
                    }
                    MessageData::MetricInfo { file_count, line_count, complexity } => {
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>File Count:</strong> {}</div>\n", file_count));
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>Line Count:</strong> {}</div>\n", line_count));
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>Complexity:</strong> {:.2}</div>\n", complexity));
                    }
                    MessageData::SecurityInfo { vulnerability, severity, location } => {
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>Vulnerability:</strong> {}</div>\n", self.escape_html(vulnerability)));
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>Severity:</strong> {}</div>\n", self.escape_html(severity)));
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>Location:</strong> {}</div>\n", self.escape_html(location)));
                    }
                    MessageData::DependencyInfo { name, version, license } => {
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>Name:</strong> {}</div>\n", self.escape_html(name)));
                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>Version:</strong> {}</div>\n", self.escape_html(version)));
                        if let Some(lic) = license {
                            html_content.push_str(&format!("            <div class=\"data-item\"><strong>License:</strong> {}</div>\n", self.escape_html(lic)));
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
                                        html_content.push_str(&format!("            <div class=\"data-item\"><strong>{}:</strong> {}</div>\n", 
                                            key, self.escape_html(&value_str)));
                                    }
                                }
                            }
                            Err(_) => {
                                html_content.push_str("            <div class=\"data-item\"><strong>Error:</strong> Failed to serialize data</div>\n");
                            }
                        }
                    }
                }
                
                html_content.push_str("          </div>\n");
                html_content.push_str("        </div>\n");
            }
            
            html_content.push_str("      </div>\n");
            html_content.push_str("    </div>\n");
            }
        }

        html_content.push_str("  </div>\n");
        html_content.push_str("</body>\n</html>\n");

        Ok(html_content)
    }

    /// Generate authors summary table for HTML export
    fn export_authors_html_table(&self, html_content: &mut String, data: &[&crate::scanner::messages::ScanMessage]) {
        use crate::scanner::messages::MessageData;
        use std::collections::HashMap;
        
        // Aggregate ALL commit data by author to get accurate totals
        let mut author_stats: HashMap<String, (u32, u32, u32, Vec<(String, String, u64, u32)>)> = HashMap::new();
        
        // Process ALL data for accurate statistics
        for message in &self.collected_data {
            if let MessageData::CommitInfo { hash, author, message: commit_msg, timestamp, changed_files } = &message.data {
                let total_files = changed_files.len() as u32;
                let total_added: u32 = changed_files.iter().map(|f| f.lines_added as u32).sum();
                let total_removed: u32 = changed_files.iter().map(|f| f.lines_removed as u32).sum();
                
                let entry = author_stats.entry(author.clone()).or_insert((0, 0, 0, Vec::new()));
                entry.0 += 1; // commit count
                entry.1 += total_added; // lines added
                entry.2 += total_removed; // lines removed
                
                // Store commit summary (hash, first line of message, timestamp, files changed)
                let first_line = commit_msg.lines().next().unwrap_or("").trim().to_string();
                entry.3.push((hash.clone(), first_line, *timestamp as u64, total_files));
            }
        }
        
        // Sort authors by commit count (descending)
        let mut sorted_authors: Vec<_> = author_stats.into_iter().collect();
        sorted_authors.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        
        // Determine how many recent commits to show (respecting output limit)
        let recent_limit = if self.export_config.output_all {
            usize::MAX
        } else {
            3.min(self.export_config.max_entries.unwrap_or(3)) // Default to 3 recent commits per author
        };
        
        html_content.push_str("    <div class=\"section\">\n");
        html_content.push_str("      <h2>Authors Summary</h2>\n");
        html_content.push_str("      <table class=\"authors-table\">\n");
        html_content.push_str("        <thead>\n");
        html_content.push_str("          <tr>\n");
        html_content.push_str("            <th>Author</th>\n");
        html_content.push_str("            <th>Commits</th>\n");
        html_content.push_str("            <th>Lines Added</th>\n");
        html_content.push_str("            <th>Lines Removed</th>\n");
        html_content.push_str("            <th>Recent Activity</th>\n");
        html_content.push_str("          </tr>\n");
        html_content.push_str("        </thead>\n");
        html_content.push_str("        <tbody>\n");
        
        for (author, (commits, lines_added, lines_removed, commit_summaries)) in sorted_authors {
            html_content.push_str("          <tr>\n");
            html_content.push_str(&format!("            <td class=\"author-name\">{}</td>\n", self.escape_html(&author)));
            html_content.push_str(&format!("            <td class=\"commits-count\">{}</td>\n", commits));
            html_content.push_str(&format!("            <td class=\"lines-added\">+{}</td>\n", lines_added));
            html_content.push_str(&format!("            <td class=\"lines-removed\">-{}</td>\n", lines_removed));
            html_content.push_str("            <td class=\"recent-activity\">\n");
            
            // Show recent commits respecting output limit
            for (hash, first_line, timestamp, files_changed) in commit_summaries.iter().take(recent_limit) {
                let date_time = self.format_timestamp(*timestamp);
                html_content.push_str("              <div class=\"commit-summary\">\n");
                html_content.push_str(&format!("                <div class=\"commit-hash\">{}</div>\n", &hash[..8]));
                html_content.push_str(&format!("                <div class=\"commit-message\">{}</div>\n", self.escape_html(first_line)));
                html_content.push_str(&format!("                <div class=\"commit-details\">{} • {} files</div>\n", date_time, files_changed));
                html_content.push_str("              </div>\n");
            }
            
            html_content.push_str("            </td>\n");
            html_content.push_str("          </tr>\n");
        }
        
        html_content.push_str("        </tbody>\n");
        html_content.push_str("      </table>\n");
        html_content.push_str("    </div>\n");
    }

    /// Format timestamp as human-readable date
    fn format_timestamp(&self, timestamp: u64) -> String {
        use std::time::{UNIX_EPOCH, Duration};
        
        if let Some(datetime) = UNIX_EPOCH.checked_add(Duration::from_secs(timestamp)) {
            // Format as YYYY-MM-DD HH:MM
            let since_epoch = datetime.duration_since(UNIX_EPOCH).unwrap().as_secs();
            let days = since_epoch / 86400;
            let hours = (since_epoch % 86400) / 3600;
            let minutes = (since_epoch % 3600) / 60;
            
            // Simple date calculation (approximate)
            let year = 1970 + (days / 365);
            let day_of_year = days % 365;
            let month = (day_of_year / 30) + 1;
            let day = (day_of_year % 30) + 1;
            
            format!("{:04}-{:02}-{:02} {:02}:{:02}", year, month.min(12), day.min(31), hours, minutes)
        } else {
            format!("{}", timestamp)
        }
    }

    /// Generate authors summary for XML export
    fn export_authors_xml_summary(&self, xml_content: &mut String, data: &[&crate::scanner::messages::ScanMessage]) {
        use crate::scanner::messages::MessageData;
        use std::collections::HashMap;
        
        // Aggregate ALL commit data by author to get accurate totals
        let mut author_stats: HashMap<String, (u32, u32, u32, Vec<(String, String, u64, u32)>)> = HashMap::new();
        
        // Process ALL data for accurate statistics
        for message in &self.collected_data {
            if let MessageData::CommitInfo { hash, author, message: commit_msg, timestamp, changed_files } = &message.data {
                let total_files = changed_files.len() as u32;
                let total_added: u32 = changed_files.iter().map(|f| f.lines_added as u32).sum();
                let total_removed: u32 = changed_files.iter().map(|f| f.lines_removed as u32).sum();
                
                let entry = author_stats.entry(author.clone()).or_insert((0, 0, 0, Vec::new()));
                entry.0 += 1; // commit count
                entry.1 += total_added; // lines added
                entry.2 += total_removed; // lines removed
                
                // Store commit summary (first line only)
                let first_line = commit_msg.lines().next().unwrap_or("").trim().to_string();
                entry.3.push((hash.clone(), first_line, *timestamp as u64, total_files));
            }
        }
        
        // Sort authors by commit count (descending)
        let mut sorted_authors: Vec<_> = author_stats.iter().collect();
        sorted_authors.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        
        // Determine how many recent commits to show (respecting output limit)
        let recent_limit = if self.export_config.output_all {
            usize::MAX
        } else {
            3.min(self.export_config.max_entries.unwrap_or(3)) // Default to 3 recent commits per author
        };
        
        xml_content.push_str("  <authors>\n");
        for (author, (commits, lines_added, lines_removed, recent_commits)) in sorted_authors {
            xml_content.push_str("    <author>\n");
            xml_content.push_str(&format!("      <name>{}</name>\n", self.escape_xml(author)));
            xml_content.push_str(&format!("      <total_commits>{}</total_commits>\n", commits));
            xml_content.push_str(&format!("      <lines_added>{}</lines_added>\n", lines_added));
            xml_content.push_str(&format!("      <lines_removed>{}</lines_removed>\n", lines_removed));
            
            // Add recent commits respecting output limit
            xml_content.push_str("      <recent_commits>\n");
            for (hash, message, timestamp, files) in recent_commits.iter().take(recent_limit) {
                xml_content.push_str("        <commit>\n");
                // Only include first 8 characters of hash for brevity
                let short_hash = if hash.len() > 8 { &hash[..8] } else { hash };
                xml_content.push_str(&format!("          <hash>{}</hash>\n", self.escape_xml(short_hash)));
                xml_content.push_str(&format!("          <message>{}</message>\n", self.escape_xml(message)));
                
                // Format timestamp as human-readable date
                let datetime = chrono::DateTime::from_timestamp(*timestamp as i64, 0)
                    .unwrap_or_else(|| chrono::Utc::now());
                let formatted_date = datetime.format("%Y-%m-%d %H:%M").to_string();
                xml_content.push_str(&format!("          <date>{}</date>\n", formatted_date));
                xml_content.push_str(&format!("          <files_changed>{}</files_changed>\n", files));
                xml_content.push_str("        </commit>\n");
            }
            xml_content.push_str("      </recent_commits>\n");
            xml_content.push_str("    </author>\n");
        }
        xml_content.push_str("  </authors>\n");
    }

    /// Generate authors summary for JSON export
    fn generate_authors_json_summary(&self, data: &[&crate::scanner::messages::ScanMessage]) -> serde_json::Value {
        use crate::scanner::messages::MessageData;
        use std::collections::HashMap;
        
        // First, aggregate ALL commit data by author to get accurate totals
        let mut author_stats: HashMap<String, (u32, u32, u32, Vec<(String, String, u64, u32)>)> = HashMap::new();
        
        // Process ALL data for accurate statistics
        for message in &self.collected_data {
            if let MessageData::CommitInfo { hash, author, message: commit_msg, timestamp, changed_files } = &message.data {
                let total_files = changed_files.len() as u32;
                let total_added: u32 = changed_files.iter().map(|f| f.lines_added as u32).sum();
                let total_removed: u32 = changed_files.iter().map(|f| f.lines_removed as u32).sum();
                
                let entry = author_stats.entry(author.clone()).or_insert((0, 0, 0, Vec::new()));
                entry.0 += 1; // commit count
                entry.1 += total_added; // lines added
                entry.2 += total_removed; // lines removed
                
                // Store commit summary (first line only)
                let first_line = commit_msg.lines().next().unwrap_or("").trim().to_string();
                entry.3.push((hash.clone(), first_line, *timestamp as u64, total_files));
            }
        }
        
        // Sort authors by commit count (descending)
        let mut sorted_authors: Vec<_> = author_stats.iter().collect();
        sorted_authors.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        
        // Determine how many recent commits to show (respecting output limit)
        let recent_limit = if self.export_config.output_all {
            usize::MAX
        } else {
            3.min(self.export_config.max_entries.unwrap_or(3)) // Default to 3 recent commits per author
        };
        
        let authors_json: Vec<serde_json::Value> = sorted_authors.into_iter().map(|(author, (commits, lines_added, lines_removed, recent_commits))| {
            let recent_commits_json: Vec<serde_json::Value> = recent_commits.iter().take(recent_limit).map(|(hash, message, timestamp, files)| {
                // Only include first 8 characters of hash for brevity
                let short_hash = if hash.len() > 8 { &hash[..8] } else { hash };
                
                // Format timestamp as human-readable date
                let datetime = chrono::DateTime::from_timestamp(*timestamp as i64, 0)
                    .unwrap_or_else(|| chrono::Utc::now());
                let formatted_date = datetime.format("%Y-%m-%d %H:%M").to_string();
                
                json!({
                    "hash": short_hash,
                    "message": message,
                    "date": formatted_date,
                    "files_changed": files
                })
            }).collect();
            
            json!({
                "name": author,
                "total_commits": commits,
                "lines_added": lines_added,
                "lines_removed": lines_removed,
                "recent_commits": recent_commits_json
            })
        }).collect();
        
        json!(authors_json)
    }

    /// Generate authors summary for Markdown export
    fn export_authors_markdown_summary(&self, md_content: &mut String) {
        use crate::scanner::messages::MessageData;
        use std::collections::HashMap;
        
        // Aggregate ALL commit data by author to get accurate totals
        let mut author_stats: HashMap<String, (u32, u32, u32, Vec<(String, String, u64, u32)>)> = HashMap::new();
        
        // Process ALL data for accurate statistics
        for message in &self.collected_data {
            if let MessageData::CommitInfo { hash, author, message: commit_msg, timestamp, changed_files } = &message.data {
                let total_files = changed_files.len() as u32;
                let total_added: u32 = changed_files.iter().map(|f| f.lines_added as u32).sum();
                let total_removed: u32 = changed_files.iter().map(|f| f.lines_removed as u32).sum();
                
                let entry = author_stats.entry(author.clone()).or_insert((0, 0, 0, Vec::new()));
                entry.0 += 1; // commit count
                entry.1 += total_added; // lines added
                entry.2 += total_removed; // lines removed
                
                // Store commit summary (first line only)
                let first_line = commit_msg.lines().next().unwrap_or("").trim().to_string();
                entry.3.push((hash.clone(), first_line, *timestamp as u64, total_files));
            }
        }
        
        // Sort authors by commit count (descending)
        let mut sorted_authors: Vec<_> = author_stats.iter().collect();
        sorted_authors.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        
        // Determine how many recent commits to show (respecting output limit)
        let recent_limit = if self.export_config.output_all {
            usize::MAX
        } else {
            3.min(self.export_config.max_entries.unwrap_or(3)) // Default to 3 recent commits per author
        };
        
        md_content.push_str("## Authors Summary\n\n");
        
        for (author, (commits, lines_added, lines_removed, recent_commits)) in sorted_authors {
            md_content.push_str(&format!("### {}\n\n", author));
            md_content.push_str(&format!("- **Total Commits:** {}\n", commits));
            md_content.push_str(&format!("- **Lines Added:** +{}\n", lines_added));
            md_content.push_str(&format!("- **Lines Removed:** -{}\n", lines_removed));
            
            // Add recent commits respecting output limit
            md_content.push_str("\n#### Recent Activity\n\n");
            for (hash, message, timestamp, files) in recent_commits.iter().take(recent_limit) {
                // Only include first 8 characters of hash for brevity
                let short_hash = if hash.len() > 8 { &hash[..8] } else { hash };
                
                // Format timestamp as human-readable date
                let datetime = chrono::DateTime::from_timestamp(*timestamp as i64, 0)
                    .unwrap_or_else(|| chrono::Utc::now());
                let formatted_date = datetime.format("%Y-%m-%d %H:%M").to_string();
                
                md_content.push_str(&format!("- **{}** | {} | {} files | {}\n", 
                    short_hash, formatted_date, files, message));
            }
            
            md_content.push_str("\n");
        }
    }

    /// Generate authors summary for CSV export
    fn export_authors_csv_summary(&self, csv_content: &mut String, delimiter: &str, quote_char: &str) -> PluginResult<()> {
        use crate::scanner::messages::MessageData;
        use std::collections::HashMap;
        
        // Aggregate ALL commit data by author to get accurate totals
        let mut author_stats: HashMap<String, (u32, u32, u32, Vec<(String, String, u64, u32)>)> = HashMap::new();
        
        // Process ALL data for accurate statistics
        for message in &self.collected_data {
            if let MessageData::CommitInfo { hash, author, message: commit_msg, timestamp, changed_files } = &message.data {
                let total_files = changed_files.len() as u32;
                let total_added: u32 = changed_files.iter().map(|f| f.lines_added as u32).sum();
                let total_removed: u32 = changed_files.iter().map(|f| f.lines_removed as u32).sum();
                
                let entry = author_stats.entry(author.clone()).or_insert((0, 0, 0, Vec::new()));
                entry.0 += 1; // commit count
                entry.1 += total_added; // lines added
                entry.2 += total_removed; // lines removed
                
                // Store commit summary (first line only)
                let first_line = commit_msg.lines().next().unwrap_or("").trim().to_string();
                entry.3.push((hash.clone(), first_line, *timestamp as u64, total_files));
            }
        }
        
        // Sort authors by commit count (descending)
        let mut sorted_authors: Vec<_> = author_stats.iter().collect();
        sorted_authors.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        
        // Determine how many recent commits to show (respecting output limit)
        let recent_limit = if self.export_config.output_all {
            usize::MAX
        } else {
            3.min(self.export_config.max_entries.unwrap_or(3)) // Default to 3 recent commits per author
        };
        
        // CSV header for authors summary
        csv_content.push_str(&format!("author{}total_commits{}lines_added{}lines_removed{}recent_activity\n", 
            delimiter, delimiter, delimiter, delimiter));
        
        // CSV rows - one per author
        for (author, (commits, lines_added, lines_removed, recent_commits)) in sorted_authors {
            // Escape author name if needed
            let escaped_author = if author.contains(delimiter) || author.contains(quote_char) {
                if quote_char == "\"" {
                    format!("\"{}\"", author.replace('"', "\"\""))
                } else {
                    format!("{}{}{}", quote_char, author.replace(quote_char, &format!("{}{}", quote_char, quote_char)), quote_char)
                }
            } else {
                author.clone()
            };
            
            // Format recent activity as a summary string (not individual commits to keep CSV clean)
            let recent_activity = if recent_commits.is_empty() {
                "No recent activity".to_string()
            } else {
                let latest_commit = &recent_commits[0];
                let datetime = chrono::DateTime::from_timestamp(latest_commit.2 as i64, 0)
                    .unwrap_or_else(|| chrono::Utc::now());
                let formatted_date = datetime.format("%Y-%m-%d").to_string();
                
                format!("Last: {} ({} commits shown)", formatted_date, recent_commits.len().min(recent_limit))
            };
            
            // Escape recent activity if needed
            let escaped_activity = if recent_activity.contains(delimiter) || recent_activity.contains(quote_char) {
                if quote_char == "\"" {
                    format!("\"{}\"", recent_activity.replace('"', "\"\""))
                } else {
                    format!("{}{}{}", quote_char, recent_activity.replace(quote_char, &format!("{}{}", quote_char, quote_char)), quote_char)
                }
            } else {
                recent_activity
            };
            
            csv_content.push_str(&format!(
                "{}{}{}{}{}{}{}{}{}\n", 
                escaped_author, delimiter,
                commits, delimiter,
                lines_added, delimiter,
                lines_removed, delimiter,
                escaped_activity
            ));
        }
        
        Ok(())
    }

    /// Export data as Markdown
    fn export_markdown(&self) -> PluginResult<String> {
        let mut md_content = String::new();
        
        // Markdown header
        md_content.push_str("# Git Analytics Report\n\n");

        if self.export_config.include_metadata {
            md_content.push_str("## Report Metadata\n\n");
            md_content.push_str(&format!("- **Generated:** {}\n", chrono::Utc::now().to_rfc3339()));
            md_content.push_str(&format!("- **Total Entries:** {}\n", self.collected_data.len()));
            md_content.push_str(&format!("- **Plugin Version:** {}\n\n", self.info.version));
        }

        let data_to_export = self.get_data_to_export();
        
        // Check if this is primarily commit data (for authors reports)
        let is_commit_data = data_to_export.iter()
            .any(|msg| matches!(msg.data, crate::scanner::messages::MessageData::CommitInfo { .. }));

        if is_commit_data {
            // Generate authors summary for Markdown
            self.export_authors_markdown_summary(&mut md_content);
        } else {

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
        } // Close the else block
        
        Ok(md_content)
    }

    /// Export data using a Handlebars template
    fn export_template(&self) -> PluginResult<String> {
        if self.export_config.template_file.is_none() {
            return Err(PluginError::configuration_error("Template file not specified. Use --template to specify a template file."));
        }
        
        // Prepare the template context with all available data
        let context = self.prepare_template_context()?;
        
        // Render the template with the context
        self.template_engine.render(&context)
    }
    
    /// Prepare comprehensive template data context
    fn prepare_template_context(&self) -> PluginResult<serde_json::Value> {
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
        context.insert("scan_config".to_string(), json!({
            "total_items_scanned": self.collected_data.len(),
            "output_all": self.export_config.output_all,
            "output_limit": self.export_config.max_entries,
        }));
        
        // Prepare statistics summary
        let statistics = self.prepare_statistics_summary();
        context.insert("statistics".to_string(), statistics);
        
        // Prepare authors data
        let authors = self.prepare_authors_data();
        context.insert("authors".to_string(), authors);
        
        // Prepare files data
        let files = self.prepare_files_data();
        context.insert("files".to_string(), files);
        
        // Prepare commits data
        let commits = self.prepare_commits_data();
        context.insert("commits".to_string(), commits);
        
        // Add raw scan messages if needed (for advanced templates)
        let data_to_export = self.get_data_to_export();
        let raw_data: Vec<serde_json::Value> = data_to_export
            .iter()
            .filter_map(|msg| serde_json::to_value(msg).ok())
            .collect();
        context.insert("raw_data".to_string(), json!(raw_data));
        
        // Add template variables passed via --template-var
        for (key, value) in &self.template_engine.template_vars {
            context.insert(key.clone(), json!(value));
        }
        
        Ok(serde_json::Value::Object(context))
    }
    
    /// Prepare statistics summary for templates
    fn prepare_statistics_summary(&self) -> serde_json::Value {
        let total_items = self.collected_data.len();
        
        // Count different message types
        let mut commit_count = 0;
        let mut file_count = 0;
        let mut metric_count = 0;
        
        for message in &self.collected_data {
            use crate::scanner::messages::MessageData;
            match &message.data {
                MessageData::CommitInfo { .. } => commit_count += 1,
                MessageData::FileInfo { .. } => file_count += 1,
                MessageData::MetricInfo { .. } => metric_count += 1,
                _ => {}
            }
        }
        
        json!({
            "total_items": total_items,
            "total_commits": commit_count,
            "total_files": file_count,
            "total_metrics": metric_count,
        })
    }
    
    /// Prepare authors data for templates
    fn prepare_authors_data(&self) -> serde_json::Value {
        let mut authors: HashMap<String, usize> = HashMap::new();
        
        for message in &self.collected_data {
            use crate::scanner::messages::MessageData;
            if let MessageData::CommitInfo { author, .. } = &message.data {
                *authors.entry(author.clone()).or_insert(0) += 1;
            }
        }
        
        let total_commits: usize = authors.values().sum();
        let mut author_list: Vec<serde_json::Value> = authors
            .iter()
            .map(|(name, count)| {
                let percentage = if total_commits > 0 {
                    (*count as f64 / total_commits as f64 * 100.0).round()
                } else {
                    0.0
                };
                json!({
                    "name": name,
                    "commits": count,
                    "percentage": percentage,
                })
            })
            .collect();
        
        // Sort by commit count descending
        author_list.sort_by(|a, b| {
            b["commits"].as_u64().unwrap_or(0)
                .cmp(&a["commits"].as_u64().unwrap_or(0))
        });
        
        json!({
            "total_authors": authors.len(),
            "list": author_list,
            "top_author": author_list.first(),
        })
    }
    
    /// Prepare files data for templates
    fn prepare_files_data(&self) -> serde_json::Value {
        let mut file_stats: HashMap<String, (usize, usize, usize)> = HashMap::new(); // (commits, added, removed)
        
        for message in &self.collected_data {
            use crate::scanner::messages::MessageData;
            if let MessageData::CommitInfo { changed_files, .. } = &message.data {
                for file in changed_files {
                    let entry = file_stats.entry(file.path.clone()).or_insert((0, 0, 0));
                    entry.0 += 1; // commits
                    entry.1 += file.lines_added;
                    entry.2 += file.lines_removed;
                }
            }
        }
        
        let mut file_list: Vec<serde_json::Value> = file_stats
            .iter()
            .map(|(path, (commits, added, removed))| {
                json!({
                    "path": path,
                    "commits": commits,
                    "lines_added": added,
                    "lines_removed": removed,
                    "net_change": *added as i32 - *removed as i32,
                })
            })
            .collect();
        
        // Sort by commit count descending
        file_list.sort_by(|a, b| {
            b["commits"].as_u64().unwrap_or(0)
                .cmp(&a["commits"].as_u64().unwrap_or(0))
        });
        
        // Separate lists for top files by different metrics
        let top_by_commits = file_list.clone();
        
        let mut top_by_changes = file_list.clone();
        top_by_changes.sort_by(|a, b| {
            let b_change = b["net_change"].as_i64().unwrap_or(0).abs();
            let a_change = a["net_change"].as_i64().unwrap_or(0).abs();
            b_change.cmp(&a_change)
        });
        
        json!({
            "total_files": file_stats.len(),
            "top_by_commits": top_by_commits,
            "top_by_changes": top_by_changes,
        })
    }
    
    /// Prepare commits data for templates
    fn prepare_commits_data(&self) -> serde_json::Value {
        let mut commits: Vec<serde_json::Value> = Vec::new();
        
        for message in &self.collected_data {
            use crate::scanner::messages::MessageData;
            if let MessageData::CommitInfo { hash, author, message: commit_msg, timestamp, changed_files } = &message.data {
                commits.push(json!({
                    "hash": hash,
                    "author": author,
                    "message": commit_msg,
                    "timestamp": timestamp,
                    "files_changed": changed_files.len(),
                    "lines_added": changed_files.iter().map(|f| f.lines_added).sum::<usize>(),
                    "lines_removed": changed_files.iter().map(|f| f.lines_removed).sum::<usize>(),
                }));
            }
        }
        
        json!({
            "total_commits": commits.len(),
            "list": commits,
        })
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
    .file-change { margin-left: 20px; font-size: 0.9em; color: #666; background-color: #f8f9fa; padding: 3px 8px; border-radius: 3px; margin: 2px 0; }
    
    /* Authors table styles */
    .authors-table { width: 100%; border-collapse: collapse; margin-top: 20px; }
    .authors-table th { background-color: #007bff; color: white; padding: 12px; text-align: left; }
    .authors-table td { padding: 12px; border-bottom: 1px solid #ddd; vertical-align: top; }
    .authors-table tr:nth-child(even) { background-color: #f8f9fa; }
    .authors-table tr:hover { background-color: #e9ecef; }
    .author-name { font-weight: bold; color: #333; }
    .commits-count { text-align: center; font-weight: bold; color: #007bff; }
    .lines-added { text-align: center; color: #28a745; font-weight: bold; }
    .lines-removed { text-align: center; color: #dc3545; font-weight: bold; }
    .recent-activity { max-width: 300px; }
    .commit-summary { margin-bottom: 8px; padding: 6px; background-color: #f1f3f4; border-radius: 4px; font-size: 0.85em; }
    .commit-hash { font-family: monospace; color: #666; font-size: 0.8em; }
    .commit-message { font-weight: 500; margin: 2px 0; }
    .commit-details { color: #666; font-size: 0.8em; }
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
        
        // Write to file if output path is not empty and not default
        if !self.export_config.output_path.is_empty() && self.export_config.output_path != "gstats_export.json" {
            use std::fs;
            fs::write(&self.export_config.output_path, &exported_data)
                .map_err(|e| PluginError::execution_failed(
                    format!("Failed to write output file '{}': {}", self.export_config.output_path, e)
                ))?;
            
            println!("Export written to: {}", self.export_config.output_path);
        } else {
            // Print to stdout if no output file specified
            println!("{}", exported_data);
        }
        
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
            template_file: None,
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
                self.execute_data_export().await
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

impl ExportPlugin {
    fn display_template_help(&self) -> PluginResult<()> {
        println!("# gstats Template System Help\n");
        
        println!("## Tera (Jinja2-like) Syntax Basics\n");
        println!("Templates use Tera syntax for variable substitution and logic:\n");
        println!("- **Simple variables**: `{{{{ variable_name }}}}`");
        println!("- **Nested properties**: `{{{{ object.property }}}}`");
        println!("- **Array iteration**: `{{% for item in array %}}{{{{ item }}}}{{% endfor %}}`");
        println!("- **Conditionals**: `{{% if condition %}}content{{% endif %}}`");
        println!("- **Filters**: `{{{{ value | filter_name }}}}`\n");
        
        println!("## Available Custom Filters\n");
        println!("- `{{{{ number | number_format }}}}` - Add thousands separators to numbers");
        println!("- `{{{{ value | percentage }}}}` - Format as percentage (e.g., 0.25 → 25.0%)");
        println!("- `{{{{ value | percentage(precision=0) }}}}` - Control decimal places\n");
        
        println!("## Template Variables Available\n");
        println!("### Repository Information");
        println!("- `repository.name` - Repository name extracted from path");
        println!("- `repository.path` - Full path to repository");
        println!("- `repository.scan_timestamp` - ISO 8601 scan timestamp\n");
        
        println!("### Statistics");
        println!("- `statistics.total_commits` - Total number of commits");
        println!("- `statistics.total_authors` - Number of unique authors");
        println!("- `statistics.total_files` - Number of files analyzed\n");
        
        println!("### Authors Data");
        println!("- `authors.total_authors` - Total number of contributors");
        println!("- `authors.list` - Array of author objects with commit counts");
        println!("  - Each author has: `name`, `commits`, `percentage`\n");
        
        println!("### Files Data");
        println!("- `files.total_files` - Total number of files");
        println!("- `files.list` - Array of file objects with modification stats");
        println!("  - Each file has: `path`, `commits`, `lines_changed`\n");
        
        println!("### Commits Data");
        println!("- `commits.total_commits` - Total number of commits");
        println!("- `commits.list` - Array of commit objects");
        println!("  - Each commit has: `hash`, `message`, `author`, `timestamp`\n");
        
        println!("### Custom Template Variables");
        println!("Any variables passed via `--template-var key=value` are available as `{{{{ key }}}}`\n");
        
        println!("## Usage Examples\n");
        println!("```bash");
        println!("# Basic template usage");
        println!("gstats commits --template report.j2 --output report.html");
        println!("");
        println!("# With custom variables");
        println!("gstats authors --template team-report.j2 \\\\");
        println!("    --template-var project=\\\"My Project\\\" \\\\");
        println!("    --template-var date=\\\"$(date +%Y-%m-%d)\\\"");
        println!("```\n");
        
        println!("## Simple Template Example\n");
        println!("```jinja2");
        println!("# {{{{ repository.name }}}} Analysis");
        println!("");
        println!("**Total Commits:** {{{{ statistics.total_commits | number_format }}}}");
        println!("**Contributors:** {{{{ authors.total_authors | number_format }}}}");
        println!("");
        println!("## Top Contributors");
        println!("{{% for author in authors.list %}}");
        println!("{{{{ loop.index }}}}. {{{{ author.name }}}} - {{{{ author.commits }}}} commits ({{{{ author.percentage | percentage }}}})");
        println!("{{% endfor %}}");
        println!("");
        println!("## Most Active Files");
        println!("{{% for file in files.list %}}");
        println!("- `{{{{ file.path }}}}` ({{{{ file.commits }}}} commits)");
        println!("{{% endfor %}}");
        println!("");
        println!("---");
        println!("*Generated with gstats on {{{{ repository.scan_timestamp }}}}*");
        println!("```\n");
        
        Ok(())
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
                        
                        // Infer format from file extension if format hasn't been explicitly set
                        if self.export_config.output_format == ExportFormat::Json {
                            if let Some(extension) = std::path::Path::new(&args[i + 1])
                                .extension()
                                .and_then(|ext| ext.to_str())
                            {
                                self.export_config.output_format = match extension.to_lowercase().as_str() {
                                    "json" => ExportFormat::Json,
                                    "csv" => ExportFormat::Csv,
                                    "xml" => ExportFormat::Xml,
                                    "yaml" | "yml" => ExportFormat::Yaml,
                                    "html" | "htm" => ExportFormat::Html,
                                    "md" | "markdown" => ExportFormat::Markdown,
                                    "j2" => ExportFormat::Template,
                                    _ => ExportFormat::Json, // Default fallback
                                };
                            }
                        }
                        
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
                            "template" => ExportFormat::Template,
                            _ => return Err(PluginError::execution_failed(
                                format!("Unknown format: {}. Supported formats: json, csv, xml, yaml, html, markdown, template", args[i + 1])
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
                "--template" | "-t" => {
                    if i + 1 < args.len() {
                        let template_path = PathBuf::from(&args[i + 1]);
                        if !template_path.exists() {
                            return Err(PluginError::execution_failed(
                                format!("Template file not found: {:?}", template_path)
                            ));
                        }
                        
                        self.template_engine.load_template(&template_path)?;
                        self.export_config.output_format = ExportFormat::Template;
                        self.export_config.template_file = Some(template_path);
                        i += 2;
                    } else {
                        return Err(PluginError::execution_failed("--template requires a value"));
                    }
                }
                "--template-var" => {
                    if i + 1 < args.len() {
                        if let Some((key, value)) = args[i + 1].split_once('=') {
                            self.template_engine.add_template_var(key.to_string(), value.to_string());
                        } else {
                            return Err(PluginError::execution_failed(
                                format!("Invalid template variable format: '{}'. Use key=value format.", args[i + 1])
                            ));
                        }
                        i += 2;
                    } else {
                        return Err(PluginError::execution_failed("--template-var requires a value"));
                    }
                }
                "--template-help" => {
                    self.display_template_help()?;
                    return Ok(()); // Early return to show help and exit
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
                description: "Output format (json, csv, xml, yaml, html, markdown, template)".to_string(),
                required: false,
                default_value: Some("json".to_string()),
                arg_type: "string".to_string(),
                examples: vec!["json".to_string(), "csv".to_string(), "html".to_string(), "template".to_string()],
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
            PluginArgDefinition {
                name: "--template, -t".to_string(),
                description: "Template file for custom output formatting (uses Tera/Jinja2 syntax)".to_string(),
                required: false,
                default_value: None,
                arg_type: "string".to_string(),
                examples: vec!["report.j2".to_string(), "summary.html.j2".to_string()],
            },
            PluginArgDefinition {
                name: "--template-var".to_string(),
                description: "Template variables in key=value format (can be used multiple times)".to_string(),
                required: false,
                default_value: None,
                arg_type: "string".to_string(),
                examples: vec!["project=MyApp".to_string(), "date=2025-08-07".to_string()],
            },
            PluginArgDefinition {
                name: "--template-help".to_string(),
                description: "Display template syntax help and available variables".to_string(),
                required: false,
                default_value: None,
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
                assert_eq!(caps.len(), 5);
                assert!(caps.iter().any(|c| c.name == "json_export"));
                assert!(caps.iter().any(|c| c.name == "template_export"));
            }
            _ => panic!("Unexpected response type"),
        }

        // Test export
        let response = plugin.execute(PluginRequest::Export).await.unwrap();
        match response {
            PluginResponse::Execute { data, status, .. } => {
                assert_eq!(status, crate::plugin::context::ExecutionStatus::Success);
                assert!(data.get("exported_data").is_some());
                let exported_data = data.get("exported_data").unwrap().as_str().unwrap();
                assert!(exported_data.contains("scan_results"));
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
        assert!(schema.iter().any(|arg| arg.name.contains("--template")));
        assert!(schema.iter().any(|arg| arg.name.contains("--template-var")));
        assert!(schema.iter().any(|arg| arg.name.contains("--template-help")));
    }

    #[tokio::test]
    async fn test_template_engine_creation() {
        let engine = TemplateEngine::new();
        assert!(engine.template_path.is_none());
        assert!(engine.template_vars.is_empty());
    }

    #[tokio::test]
    async fn test_template_engine_add_var() {
        let mut engine = TemplateEngine::new();
        engine.add_template_var("project".to_string(), "TestProject".to_string());
        
        assert_eq!(engine.template_vars.get("project"), Some(&"TestProject".to_string()));
    }

    #[tokio::test]
    async fn test_template_argument_parsing() {
        let mut plugin = ExportPlugin::new();
        let args = vec![
            "--template-var".to_string(),
            "project=MyApp".to_string(),
            "--template-var".to_string(),
            "version=1.0".to_string(),
        ];
        
        assert!(plugin.parse_plugin_args(&args).await.is_ok());
        assert_eq!(plugin.template_engine.template_vars.get("project"), Some(&"MyApp".to_string()));
        assert_eq!(plugin.template_engine.template_vars.get("version"), Some(&"1.0".to_string()));
    }

    #[tokio::test]
    async fn test_template_argument_parsing_invalid_format() {
        let mut plugin = ExportPlugin::new();
        let args = vec![
            "--template-var".to_string(),
            "invalid-format".to_string(), // Missing '='
        ];
        
        let result = plugin.parse_plugin_args(&args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid template variable format"));
    }

    #[tokio::test]
    async fn test_prepare_template_context() {
        let mut plugin = ExportPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        // Add some test data
        let commit_data = MessageData::CommitInfo {
            hash: "abc123".to_string(),
            author: "Test Author".to_string(),
            message: "Test commit".to_string(),
            timestamp: 1234567890,
            changed_files: vec![],
        };
        let test_message = create_test_message(crate::scanner::modes::ScanMode::HISTORY, commit_data);
        plugin.collected_data.push(test_message);
        
        // Add template variables
        plugin.template_engine.add_template_var("project".to_string(), "TestProject".to_string());
        
        let template_context = plugin.prepare_template_context().unwrap();
        
        // Verify repository information
        assert!(template_context.get("repository").is_some());
        let repo = template_context.get("repository").unwrap();
        assert!(repo.get("name").is_some());
        assert!(repo.get("path").is_some());
        assert!(repo.get("scan_timestamp").is_some());
        
        // Verify statistics
        assert!(template_context.get("statistics").is_some());
        let stats = template_context.get("statistics").unwrap();
        assert_eq!(stats.get("total_commits").unwrap().as_u64().unwrap(), 1);
        
        // Verify authors data
        assert!(template_context.get("authors").is_some());
        let authors = template_context.get("authors").unwrap();
        assert_eq!(authors.get("total_authors").unwrap().as_u64().unwrap(), 1);
        assert!(authors.get("list").unwrap().as_array().is_some());
        
        // Verify template variables are included
        assert_eq!(template_context.get("project").unwrap().as_str().unwrap(), "TestProject");
    }

    #[tokio::test]
    async fn test_format_detection_with_template() {
        let mut plugin = ExportPlugin::new();
        let args = vec![
            "--format".to_string(),
            "template".to_string(),
        ];
        
        assert!(plugin.parse_plugin_args(&args).await.is_ok());
        assert_eq!(plugin.export_config.output_format, ExportFormat::Template);
    }

    #[test]
    fn test_template_filter_number_format() {
        let mut engine = TemplateEngine::new();
        engine.register_custom_filters();
        
        let result = engine.tera.render_str("{{ 1234567 | number_format }}", &tera::Context::new());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "1,234,567");
    }

    #[test]
    fn test_template_filter_percentage() {
        let mut engine = TemplateEngine::new();
        engine.register_custom_filters();
        
        let mut context = tera::Context::new();
        context.insert("value", &23.456);
        
        let result = engine.tera.render_str("{{ value | percentage }}", &context);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "23.5%");
    }
}