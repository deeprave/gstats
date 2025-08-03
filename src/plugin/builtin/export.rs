//! Data Export Plugin
//! 
//! Built-in plugin for exporting scan results to various formats.

use crate::plugin::{
    Plugin, PluginInfo, PluginContext, PluginRequest, PluginResponse,
    PluginResult, PluginError, traits::{PluginType, PluginCapability}
};
use crate::scanner::{modes::ScanMode, messages::{ScanMessage, MessageData, MessageHeader}};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
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
    compress_output: bool,
    max_entries: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
enum ExportFormat {
    Json,
    Csv,
    Xml,
    Yaml,
    Html,
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
        }
    }

    /// Export data as JSON
    fn export_json(&self) -> PluginResult<String> {
        let mut export_data = HashMap::new();
        
        if self.export_config.include_metadata {
            export_data.insert("metadata", json!({
                "export_timestamp": std::time::SystemTime::now(),
                "total_entries": self.collected_data.len(),
                "format": "json",
                "plugin_version": self.info.version,
            }));
        }

        let data: Vec<serde_json::Value> = self.collected_data.iter()
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

        // CSV header
        csv_content.push_str("timestamp,scan_mode,data_json\n");

        // CSV rows
        for message in &self.collected_data {
            let timestamp = message.header.timestamp;
            let scan_mode = format!("{:?}", message.header.scan_mode);
            let data_json = serde_json::to_string(&message.data)
                .map_err(|e| PluginError::execution_failed(format!("JSON serialization failed: {}", e)))?;

            // Escape CSV values
            let escaped_json = data_json.replace('"', "\"\"");
            csv_content.push_str(&format!("{},{},\"{}\"\n", timestamp, scan_mode, escaped_json));
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

        xml_content.push_str("  <entries>\n");
        for message in &self.collected_data {
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
        
        if self.export_config.include_metadata {
            export_data.insert("metadata", json!({
                "export_timestamp": format!("{:?}", std::time::SystemTime::now()),
                "total_entries": self.collected_data.len(),
                "format": "yaml",
                "plugin_version": self.info.version,
            }));
        }

        let data: Vec<serde_json::Value> = self.collected_data.iter()
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

        // Group data by scan mode for better presentation
        let mut grouped_data: HashMap<String, Vec<&ScanMessage>> = HashMap::new();
        for message in &self.collected_data {
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
            compress_output: false,
            max_entries: None,
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
}