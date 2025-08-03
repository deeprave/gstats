//! Code Metrics Plugin
//! 
//! Built-in plugin for analyzing code quality metrics and statistics.

use crate::plugin::{
    Plugin, ScannerPlugin, PluginInfo, PluginContext, PluginRequest, PluginResponse,
    PluginResult, PluginError, traits::{PluginType, PluginCapability}
};
use crate::scanner::{modes::ScanMode, messages::{ScanMessage, MessageData, MessageHeader}};
use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;

/// Code metrics analysis plugin
pub struct MetricsPlugin {
    info: PluginInfo,
    initialized: bool,
    file_metrics: HashMap<String, FileMetrics>,
    total_lines: usize,
    total_files: usize,
}

#[derive(Debug, Clone)]
struct FileMetrics {
    lines_of_code: usize,
    comment_lines: usize,
    blank_lines: usize,
    cyclomatic_complexity: usize,
    file_size_bytes: usize,
    file_extension: String,
}

impl MetricsPlugin {
    /// Create a new metrics plugin
    pub fn new() -> Self {
        let info = PluginInfo::new(
            "metrics".to_string(),
            "1.0.0".to_string(),
            crate::scanner::version::get_api_version() as u32,
            "Analyzes code quality metrics including complexity, lines of code, and file statistics".to_string(),
            "gstats built-in".to_string(),
            PluginType::Processing,
        )
        .with_capability(
            "code_analysis".to_string(),
            "Calculates lines of code, complexity, and other code metrics".to_string(),
            "1.0.0".to_string(),
        )
        .with_capability(
            "file_statistics".to_string(),
            "Provides file-level statistics and aggregations".to_string(),
            "1.0.0".to_string(),
        )
        .with_capability(
            "quality_metrics".to_string(),
            "Evaluates code quality indicators and trends".to_string(),
            "1.0.0".to_string(),
        );

        Self {
            info,
            initialized: false,
            file_metrics: HashMap::new(),
            total_lines: 0,
            total_files: 0,
        }
    }

    /// Process a file and calculate metrics
    fn process_file(&mut self, message: &ScanMessage) -> PluginResult<()> {
        if let MessageData::FileInfo { path, size, lines } = &message.data {
            // For now, we'll use the provided lines count and create simplified metrics
            let metrics = FileMetrics {
                lines_of_code: *lines as usize,
                comment_lines: 0, // We don't have access to content, so estimate
                blank_lines: 0,
                cyclomatic_complexity: 1, // Basic default complexity
                file_size_bytes: *size as usize,
                file_extension: std::path::Path::new(path)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("")
                    .to_string(),
            };
            
            self.total_lines += metrics.lines_of_code;
            self.total_files += 1;
            self.file_metrics.insert(path.clone(), metrics);
        }
        Ok(())
    }

    /// Calculate metrics for a single file
    fn calculate_file_metrics(&self, file_path: &str, content: &str) -> FileMetrics {
        let lines: Vec<&str> = content.lines().collect();
        let mut loc = 0;
        let mut comment_lines = 0;
        let mut blank_lines = 0;

        // Get file extension
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string();

        // Analyze each line
        for line in &lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                blank_lines += 1;
            } else if self.is_comment_line(trimmed, &extension) {
                comment_lines += 1;
            } else {
                loc += 1;
            }
        }

        // Calculate basic cyclomatic complexity (simplified)
        let complexity = self.calculate_complexity(content, &extension);

        FileMetrics {
            lines_of_code: loc,
            comment_lines,
            blank_lines,
            cyclomatic_complexity: complexity,
            file_size_bytes: content.len(),
            file_extension: extension,
        }
    }

    /// Determine if a line is a comment based on file extension
    fn is_comment_line(&self, line: &str, extension: &str) -> bool {
        match extension {
            "rs" | "java" | "c" | "cpp" | "js" | "ts" => {
                line.starts_with("//") || line.starts_with("/*") || line.starts_with("*")
            }
            "py" => line.starts_with("#"),
            "sh" | "bash" => line.starts_with("#"),
            "html" | "xml" => line.contains("<!--") || line.contains("-->"),
            _ => line.starts_with("#") || line.starts_with("//"),
        }
    }

    /// Calculate basic cyclomatic complexity
    fn calculate_complexity(&self, content: &str, extension: &str) -> usize {
        let complexity_keywords = match extension {
            "rs" => vec!["if", "while", "for", "match", "loop"],
            "java" | "c" | "cpp" => vec!["if", "while", "for", "switch", "case"],
            "py" => vec!["if", "while", "for", "elif", "except"],
            "js" | "ts" => vec!["if", "while", "for", "switch", "case"],
            _ => vec!["if", "while", "for"],
        };

        let mut complexity = 1; // Base complexity
        for keyword in complexity_keywords {
            complexity += content.matches(keyword).count();
        }

        complexity
    }

    /// Generate comprehensive metrics summary
    fn generate_metrics_summary(&self) -> PluginResult<ScanMessage> {
        // Calculate aggregated metrics
        let total_complexity: usize = self.file_metrics.values().map(|m| m.cyclomatic_complexity).sum();
        
        // Calculate average complexity
        let avg_complexity = if self.total_files > 0 {
            total_complexity as f64 / self.total_files as f64
        } else {
            0.0
        };

        let data = MessageData::MetricInfo {
            file_count: self.total_files as u32,
            line_count: self.total_lines as u64,
            complexity: avg_complexity,
        };

        let header = MessageHeader::new(
            ScanMode::FILES,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );

        Ok(ScanMessage::new(header, data))
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
            return Err(PluginError::initialization_failed("Plugin already initialized"));
        }

        // Reset metrics
        self.file_metrics.clear();
        self.total_lines = 0;
        self.total_files = 0;
        self.initialized = true;

        Ok(())
    }

    async fn execute(&self, request: PluginRequest) -> PluginResult<PluginResponse> {
        if !self.initialized {
            return Err(PluginError::invalid_state("Plugin not initialized"));
        }

        match request {
            PluginRequest::GetStatistics => {
                let summary = self.generate_metrics_summary()?;
                Ok(PluginResponse::Statistics(summary))
            }
            PluginRequest::GetCapabilities => {
                Ok(PluginResponse::Capabilities(self.info.capabilities.clone()))
            }
            _ => Err(PluginError::execution_failed("Unsupported request type")),
        }
    }

    async fn cleanup(&mut self) -> PluginResult<()> {
        self.initialized = false;
        self.file_metrics.clear();
        self.total_lines = 0;
        self.total_files = 0;
        Ok(())
    }
}

#[async_trait]
impl ScannerPlugin for MetricsPlugin {
    fn supported_modes(&self) -> ScanMode {
        ScanMode::FILES | ScanMode::SECURITY
    }

    async fn process_scan_data(&self, data: &ScanMessage) -> PluginResult<Vec<ScanMessage>> {
        if !self.initialized {
            return Err(PluginError::invalid_state("Plugin not initialized"));
        }

        // Create a mutable copy for processing
        let mut processor = self.clone_for_processing();
        
        let mut results = vec![data.clone()];

        // Process file data if available
        if matches!(data.data, MessageData::FileInfo { .. }) {
            processor.process_file(data)?;
            
            // Generate file-specific metrics
            if let MessageData::FileInfo { path, .. } = &data.data {
                if let Some(metrics) = processor.file_metrics.get(path) {
                    let metrics_message = processor.generate_file_metrics_message(path, metrics)?;
                    results.push(metrics_message);
                }
            }
        }

        Ok(results)
    }

    async fn aggregate_results(&self, results: Vec<ScanMessage>) -> PluginResult<ScanMessage> {
        if !self.initialized {
            return Err(PluginError::invalid_state("Plugin not initialized"));
        }

        // Aggregate metrics from multiple scan messages
        let mut aggregated = MetricsPlugin::new();
        aggregated.initialized = true;

        for message in &results {
            if matches!(message.data, MessageData::FileInfo { .. }) {
                aggregated.process_file(message)?;
            }
        }

        aggregated.generate_metrics_summary()
    }

    fn estimate_processing_time(&self, modes: ScanMode, item_count: usize) -> Option<std::time::Duration> {
        if !modes.intersects(self.supported_modes()) {
            return None;
        }

        // Estimate ~2ms per file for metrics calculation
        let processing_time_ms = item_count * 2;
        Some(std::time::Duration::from_millis(processing_time_ms as u64))
    }

    fn config_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "max_files": {
                    "type": "integer",
                    "description": "Maximum number of files to analyze",
                    "default": 10000,
                    "minimum": 1
                },
                "file_size_limit_bytes": {
                    "type": "integer",
                    "description": "Skip files larger than this size in bytes",
                    "default": 1048576,
                    "minimum": 1024
                },
                "supported_extensions": {
                    "type": "array",
                    "description": "File extensions to include in analysis",
                    "items": {"type": "string"},
                    "default": ["rs", "py", "js", "ts", "java", "c", "cpp", "go"]
                },
                "calculate_complexity": {
                    "type": "boolean",
                    "description": "Whether to calculate cyclomatic complexity",
                    "default": true
                }
            }
        })
    }
}

impl MetricsPlugin {
    /// Clone for processing (workaround for mutable operations)
    fn clone_for_processing(&self) -> Self {
        Self {
            info: self.info.clone(),
            initialized: self.initialized,
            file_metrics: self.file_metrics.clone(),
            total_lines: self.total_lines,
            total_files: self.total_files,
        }
    }

    /// Generate metrics message for a specific file
    fn generate_file_metrics_message(&self, file_path: &str, metrics: &FileMetrics) -> PluginResult<ScanMessage> {
        // Calculate complexity per lines of code
        let complexity_per_loc = if metrics.lines_of_code > 0 {
            metrics.cyclomatic_complexity as f64 / metrics.lines_of_code as f64
        } else {
            0.0
        };

        let data = MessageData::MetricInfo {
            file_count: 1, // Single file
            line_count: metrics.lines_of_code as u64,
            complexity: complexity_per_loc,
        };

        let header = MessageHeader::new(
            ScanMode::FILES,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );

        Ok(ScanMessage::new(header, data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::context::PluginContext;
    use crate::scanner::messages::MessageHeader;

    fn create_test_file_message(file_path: &str, content: &str) -> ScanMessage {
        let data = MessageData::FileInfo {
            path: file_path.to_string(),
            size: content.len() as u64,
            lines: content.lines().count() as u32,
        };

        let header = MessageHeader::new(
            ScanMode::FILES,
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
    async fn test_metrics_plugin_creation() {
        let plugin = MetricsPlugin::new();
        assert_eq!(plugin.plugin_info().name, "metrics");
        assert_eq!(plugin.plugin_info().plugin_type, PluginType::Processing);
        assert!(!plugin.initialized);
    }

    #[tokio::test]
    async fn test_metrics_plugin_initialization() {
        let mut plugin = MetricsPlugin::new();
        let context = create_test_context();

        assert!(plugin.initialize(&context).await.is_ok());
        assert!(plugin.initialized);
    }

    #[tokio::test]
    async fn test_metrics_plugin_supported_modes() {
        let plugin = MetricsPlugin::new();
        let modes = plugin.supported_modes();
        
        assert!(modes.contains(ScanMode::FILES));
        assert!(modes.contains(ScanMode::SECURITY));
        assert!(!modes.contains(ScanMode::HISTORY));
    }

    #[tokio::test]
    async fn test_metrics_plugin_file_metrics_calculation() {
        let plugin = MetricsPlugin::new();
        let content = r#"
// This is a comment
fn main() {
    if true {
        println!("Hello");
    }
    
    // Another comment
    for i in 0..10 {
        if i % 2 == 0 {
            println!("{}", i);
        }
    }
}
"#;
        let metrics = plugin.calculate_file_metrics("test.rs", content);
        
        assert_eq!(metrics.file_extension, "rs");
        assert!(metrics.lines_of_code > 0);
        assert!(metrics.comment_lines > 0);
        assert!(metrics.blank_lines > 0);
        assert!(metrics.cyclomatic_complexity > 1); // Should detect if and for loops
    }

    #[tokio::test]
    async fn test_metrics_plugin_process_scan_data() {
        let mut plugin = MetricsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        let file_message = create_test_file_message(
            "test.rs",
            "fn main() {\n    println!(\"Hello\");\n}"
        );

        let results = plugin.process_scan_data(&file_message).await.unwrap();
        assert_eq!(results.len(), 2); // Original + metrics
        assert_eq!(results[0], file_message);
        assert!(matches!(results[1].data, MessageData::MetricInfo { .. }));
    }

    #[tokio::test]
    async fn test_metrics_plugin_aggregate_results() {
        let mut plugin = MetricsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        let files = vec![
            create_test_file_message("file1.rs", "fn main() {}"),
            create_test_file_message("file2.py", "def main():\n    pass"),
            create_test_file_message("file3.js", "function main() {}"),
        ];

        let summary = plugin.aggregate_results(files).await.unwrap();
        assert!(matches!(summary.data, MessageData::MetricInfo { .. }));
        
        if let MessageData::MetricInfo { file_count, .. } = summary.data {
            assert_eq!(file_count, 3);
        } else {
            panic!("Expected MetricInfo data");
        }
    }

    #[tokio::test]
    async fn test_metrics_plugin_comment_detection() {
        let plugin = MetricsPlugin::new();
        
        // Test Rust comments
        assert!(plugin.is_comment_line("// This is a comment", "rs"));
        assert!(plugin.is_comment_line("/* Block comment */", "rs"));
        assert!(!plugin.is_comment_line("fn main() {}", "rs"));
        
        // Test Python comments
        assert!(plugin.is_comment_line("# This is a comment", "py"));
        assert!(!plugin.is_comment_line("def main():", "py"));
        
        // Test JavaScript comments
        assert!(plugin.is_comment_line("// This is a comment", "js"));
        assert!(!plugin.is_comment_line("function main() {}", "js"));
    }

    #[tokio::test]
    async fn test_metrics_plugin_complexity_calculation() {
        let plugin = MetricsPlugin::new();
        
        let simple_code = "fn main() { println!(\"Hello\"); }";
        let complex_code = r#"
fn complex_function() {
    if condition1 {
        for i in 0..10 {
            if i % 2 == 0 {
                while some_condition {
                    match value {
                        Some(x) => {},
                        None => {},
                    }
                }
            }
        }
    }
}
"#;
        
        let simple_complexity = plugin.calculate_complexity(simple_code, "rs");
        let complex_complexity = plugin.calculate_complexity(complex_code, "rs");
        
        assert!(simple_complexity < complex_complexity);
        assert!(complex_complexity > 1);
    }
}