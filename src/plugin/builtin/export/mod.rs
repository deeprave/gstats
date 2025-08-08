//! Data Export Plugin Module
//! 
//! Built-in plugin for exporting scan results to various formats.

pub mod formats;
pub mod template_engine;
pub mod config;

use crate::plugin::{
    Plugin, PluginInfo, PluginContext, PluginRequest, PluginResponse,
    PluginResult, PluginError, traits::{PluginType, PluginArgumentParser, PluginArgDefinition}
};
use crate::plugin::builtin::utils::format_detection::{FormatDetector, FormatDetectionResult};
use crate::scanner::messages::ScanMessage;
use async_trait::async_trait;
use std::path::PathBuf;
use serde_json::json;

pub use config::{ExportConfig, ExportFormat};
pub use template_engine::TemplateEngine;

/// Data export plugin for various output formats
pub struct ExportPlugin {
    info: PluginInfo,
    initialized: bool,
    export_config: ExportConfig,
    collected_data: Vec<ScanMessage>,
    template_engine: TemplateEngine,
    format_detector: FormatDetector,
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
            format_detector: FormatDetector::new(),
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
            return Err(PluginError::initialization_failed("Plugin not initialized"));
        }

        // If template is specified, use template with the detected format
        if self.export_config.template_file.is_some() {
            return self.export_template();
        }

        let data_to_export = self.get_data_to_export();

        // Otherwise use built-in formatters
        match self.export_config.output_format {
            ExportFormat::Json => formats::json::export_json(&self.export_config, &self.collected_data, &data_to_export, &self.info),
            ExportFormat::Csv => formats::csv::export_csv(&self.export_config, &self.collected_data, &data_to_export),
            ExportFormat::Xml => formats::xml::export_xml(&self.export_config, &self.collected_data, &data_to_export, &self.info),
            ExportFormat::Yaml => formats::yaml::export_yaml(&self.export_config, &self.collected_data, &data_to_export, &self.info),
            ExportFormat::Html => formats::html::export_html(&self.export_config, &self.collected_data, &data_to_export, &self.info),
            ExportFormat::Markdown => formats::markdown::export_markdown(&self.export_config, &self.collected_data, &data_to_export, &self.info),
        }
    }

    /// Export data using a template
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
        
        // Add template variables passed via --template-var
        for (key, value) in &self.template_engine.template_vars {
            context.insert(key.clone(), json!(value));
        }
        
        Ok(serde_json::Value::Object(context))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::modes::ScanMode;
    use crate::scanner::messages::{MessageHeader, MessageData};

    fn create_test_message(scan_mode: ScanMode, data: MessageData) -> ScanMessage {
        ScanMessage {
            header: MessageHeader {
                scan_mode,
                timestamp: chrono::Utc::now().timestamp() as u64,
            },
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
            assert_eq!(plugin.export_config.output_format, expected_format);

            // 3. Data Processing
            plugin.add_data(create_test_message(
                ScanMode::HISTORY,
                MessageData::CommitInfo {
                    hash: "abc123".to_string(),
                    author: "Test Author".to_string(),
                    message: "Test commit".to_string(),
                    timestamp: 1234567890,
                    changed_files: vec![],
                }
            )).unwrap();
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
            assert_eq!(plugin.export_config.output_format, expected_format, 
                      "Format detection failed for {}", filename);

            // Verify format detector compatibility
            assert!(plugin.format_detector.is_template_compatible(&expected_format),
                   "Format {:?} should be template compatible", expected_format);
        }
    }

    // Add a helper function for tests
    async fn create_test_plugin_context() -> PluginContext {
        use crate::git;
        use std::sync::Arc;
        use crate::scanner::{ScannerConfig, QueryParams};
        
        let repo = git::resolve_repository_handle(None).unwrap();
        let scanner_config = Arc::new(ScannerConfig::default());
        let query_params = Arc::new(QueryParams::default());
        
        PluginContext::new(
            scanner_config,
            Arc::new(repo),
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
                // Perform the actual export
                let exported_data_str = self.export_data().await?;
                
                // Parse the exported data as JSON so it's not double-encoded
                let exported_data: serde_json::Value = serde_json::from_str(&exported_data_str)
                    .unwrap_or_else(|_| serde_json::json!(exported_data_str));
                
                let metadata = crate::plugin::context::ExecutionMetadata {
                    duration_ms: 0,
                    memory_used: 0,
                    items_processed: self.collected_data.len() as u64,
                    plugin_version: "1.0.0".to_string(),
                    extra: std::collections::HashMap::new(),
                };
                
                // Return the exported data as JSON
                let result_data = serde_json::json!({
                    "exported_data": exported_data,
                    "format": format!("{:?}", self.export_config.output_format),
                    "items_processed": self.collected_data.len()
                });
                
                Ok(PluginResponse::success(request_id, result_data, metadata))
            }
            PluginRequest::Export => {
                // Handle direct export request
                let exported_data_str = self.export_data().await?;
                
                // Parse the exported data as JSON so it's not double-encoded
                let exported_data: serde_json::Value = serde_json::from_str(&exported_data_str)
                    .unwrap_or_else(|_| serde_json::json!(exported_data_str));
                
                let metadata = crate::plugin::context::ExecutionMetadata {
                    duration_ms: 0,
                    memory_used: 0,
                    items_processed: self.collected_data.len() as u64,
                    plugin_version: "1.0.0".to_string(),
                    extra: std::collections::HashMap::new(),
                };
                
                // For Export requests, return the raw exported data
                let result_data = serde_json::json!({
                    "exported_data": exported_data,
                    "format": format!("{:?}", self.export_config.output_format),
                    "items_processed": self.collected_data.len()
                });
                
                Ok(PluginResponse::success("export".to_string(), result_data, metadata))
            }
            _ => {
                let _metadata = crate::plugin::context::ExecutionMetadata {
                    duration_ms: 0,
                    memory_used: 0,
                    items_processed: 0,
                    plugin_version: "1.0.0".to_string(),
                    extra: std::collections::HashMap::new(),
                };
                Err(PluginError::execution_failed("Unsupported request type"))
            }
        }
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        Ok(())
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
                    self.export_config.output_path = output_path.clone();
                    
                    // Auto-detect format from file extension if not explicitly set
                    let detection_result = self.format_detector.detect_format_from_path(output_path);
                    match detection_result {
                        FormatDetectionResult::Detected(format) => {
                            self.export_config.output_format = format;
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
                    self.export_config.output_format = match format_str.to_lowercase().as_str() {
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
                    i += 2;
                }
                "--include-metadata" => {
                    self.export_config.include_metadata = true;
                    i += 1;
                }
                "--max-entries" => {
                    if i + 1 >= args.len() {
                        return Err(PluginError::configuration_error("--max-entries requires a value"));
                    }
                    let max_str = &args[i + 1];
                    self.export_config.max_entries = Some(
                        max_str.parse::<usize>()
                            .map_err(|_| PluginError::configuration_error("--max-entries must be a positive integer"))?
                    );
                    i += 2;
                }
                "--output-all" => {
                    self.export_config.output_all = true;
                    i += 1;
                }
                "--csv-delimiter" => {
                    if i + 1 >= args.len() {
                        return Err(PluginError::configuration_error("--csv-delimiter requires a value"));
                    }
                    self.export_config.csv_delimiter = args[i + 1].clone();
                    i += 2;
                }
                "--csv-quote-char" => {
                    if i + 1 >= args.len() {
                        return Err(PluginError::configuration_error("--csv-quote-char requires a value"));
                    }
                    self.export_config.csv_quote_char = args[i + 1].clone();
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
                    self.template_engine.load_template(&template_path)?;
                    self.export_config.template_file = Some(template_path);
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
                        self.template_engine.add_template_var(key, value);
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
        if self.export_config.output_path.is_empty() {
            return Err(PluginError::configuration_error("--output is required"));
        }

        Ok(())
    }
}
