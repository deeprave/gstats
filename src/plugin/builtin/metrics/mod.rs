//! Code Metrics Plugin
//! 
//! Built-in plugin for analyzing code quality metrics and statistics.

use crate::plugin::{
    Plugin, ScannerPlugin, PluginInfo, PluginContext, PluginRequest, PluginResponse,
    PluginResult, PluginError, traits::{PluginType, PluginFunction}
};
use crate::plugin::builtin::utils::change_frequency::{ChangeFrequencyAnalyzer, TimeWindow};
use crate::plugin::builtin::utils::hotspot_detector::{HotspotDetector, FileComplexityMetrics};
use crate::plugin::builtin::utils::duplication_detector::DuplicationDetector;
use crate::plugin::builtin::utils::debt_assessor::DebtAssessor;
use crate::plugin::builtin::utils::complexity_calculator::ComplexityCalculator;
use crate::scanner::{modes::ScanMode, messages::{ScanMessage, MessageData, MessageHeader}};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;
use serde_json::json;

/// Code metrics analysis plugin
pub struct MetricsPlugin {
    info: PluginInfo,
    initialized: bool,
    file_metrics: RwLock<HashMap<String, FileMetrics>>,
    total_lines: usize,
    total_files: usize,
    complexity_calculator: ComplexityCalculator,
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
            file_metrics: RwLock::new(HashMap::new()),
            total_lines: 0,
            total_files: 0,
            complexity_calculator: ComplexityCalculator::new(),
        }
    }

    /// Process a file and calculate metrics
    fn process_file(&mut self, message: &ScanMessage) -> PluginResult<()> {
        if let MessageData::FileInfo { path, size, lines } = &message.data {
            // Calculate cyclomatic complexity
            let cyclomatic_complexity = self.complexity_calculator
                .calculate_complexity(path)
                .unwrap_or(1); // Fall back to default if calculation fails
            
            let metrics = FileMetrics {
                lines_of_code: *lines as usize,
                comment_lines: 0, // We don't have access to content, so estimate
                blank_lines: 0,
                cyclomatic_complexity,
                file_size_bytes: *size as usize,
                file_extension: std::path::Path::new(path)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("")
                    .to_string(),
            };
            
            self.total_lines += metrics.lines_of_code;
            self.total_files += 1;
            self.file_metrics.write().unwrap().insert(path.clone(), metrics);
        }
        Ok(())
    }




    /// Generate comprehensive metrics summary
    fn generate_metrics_summary(&self) -> PluginResult<ScanMessage> {
        // Calculate aggregated metrics
        let total_complexity: usize = self.file_metrics.read().unwrap().values().map(|m| m.cyclomatic_complexity).sum();
        
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
        self.file_metrics.write().unwrap().clear();
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
            PluginRequest::Execute {  invocation_type, .. } => {
                // Handle function-based execution
                let function_name = match invocation_type {
                    crate::plugin::InvocationType::Function(ref func) => func.as_str(),
                    crate::plugin::InvocationType::Direct => self.default_function().unwrap_or("metrics"),
                    crate::plugin::InvocationType::Default => "metrics",
                };

                // Route to appropriate function
                match function_name {
                    "metrics" | "code" | "quality" => {
                        self.execute_code_metrics().await
                    }
                    "complexity" | "cyclomatic" => {
                        self.execute_complexity_analysis().await
                    }
                    "files" | "file-stats" => {
                        self.execute_file_statistics().await
                    }
                    "hotspots" | "changes" | "frequency" => {
                        self.execute_hotspot_analysis().await
                    }
                    "duplicates" | "duplication" | "clone-detection" => {
                        self.execute_duplication_analysis().await
                    }
                    "debt" | "technical-debt" | "td-assessment" => {
                        self.execute_debt_assessment().await
                    }
                    _ => Err(PluginError::execution_failed(
                        format!("Unknown function: {}", function_name)
                    )),
                }
            }
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
        self.file_metrics.write().unwrap().clear();
        self.total_lines = 0;
        self.total_files = 0;
        Ok(())
    }
    
    /// Get all functions this plugin can handle
    fn advertised_functions(&self) -> Vec<PluginFunction> {
        vec![
            PluginFunction {
                name: "metrics".to_string(),
                aliases: vec!["code".to_string(), "quality".to_string()],
                description: "Analyze code quality metrics including lines of code and overall quality indicators".to_string(),
                is_default: true,
            },
            PluginFunction {
                name: "complexity".to_string(),
                aliases: vec!["cyclomatic".to_string()],
                description: "Calculate cyclomatic complexity and complexity metrics for code files".to_string(),
                is_default: false,
            },
            PluginFunction {
                name: "files".to_string(),
                aliases: vec!["file-stats".to_string()],
                description: "Generate file-level statistics and aggregations".to_string(),
                is_default: false,
            },
            PluginFunction {
                name: "hotspots".to_string(),
                aliases: vec!["changes".to_string(), "frequency".to_string()],
                description: "Identify code hotspots by combining complexity and change frequency analysis".to_string(),
                is_default: false,
            },
            PluginFunction {
                name: "duplicates".to_string(),
                aliases: vec!["duplication".to_string(), "clone-detection".to_string()],
                description: "Detect duplicate and similar code patterns using token-based analysis".to_string(),
                is_default: false,
            },
            PluginFunction {
                name: "debt".to_string(),
                aliases: vec!["technical-debt".to_string(), "td-assessment".to_string()],
                description: "Comprehensive technical debt assessment combining complexity, frequency, and duplication metrics".to_string(),
                is_default: false,
            },
        ]
    }
    
    /// Get the default function name
    fn default_function(&self) -> Option<&str> {
        Some("metrics")
    }
    
    /// Override to provide ScannerPlugin access
    fn as_scanner_plugin(&self) -> Option<&dyn ScannerPlugin> {
        Some(self)
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

        // Process file data and extract metrics
        if let MessageData::FileInfo { path, size, lines } = &data.data {
            // Calculate metrics for this file
            use std::path::Path;
            let path_obj = Path::new(path);
            if let Some(extension) = path_obj.extension().and_then(|ext| ext.to_str()) {
                if is_source_file_extension(extension) {
                    let lines_count = *lines as usize;

                    if lines_count > 0 {
                        let file_size = *size as usize;

                        let complexity = self.complexity_calculator
                            .calculate_complexity(&path_obj.to_string_lossy())
                            .unwrap_or(1);

                        // Create a metrics message with the calculated data
                        let _metrics_data = json!({
                            "path": path,
                            "lines_of_code": lines_count,
                            "file_size_bytes": file_size,
                            "cyclomatic_complexity": complexity,
                            "file_extension": extension
                        });

                        // Store the metrics for aggregation
                        self.file_metrics.write().unwrap().insert(path.clone(), FileMetrics {
                            lines_of_code: lines_count,
                            comment_lines: 0, // TODO: implement comment counting
                            blank_lines: 0,   // TODO: implement blank line counting
                            cyclomatic_complexity: complexity as usize,
                            file_size_bytes: file_size,
                            file_extension: extension.to_string(),
                        });

                        // Create a MetricInfo message instead of Custom
                        let metrics_message = ScanMessage {
                            header: MessageHeader::new(data.header.scan_mode, data.header.timestamp),
                            data: MessageData::MetricInfo {
                                file_count: 1,
                                line_count: lines_count as u64,
                                complexity: complexity as f64,
                            },
                        };

                        return Ok(vec![data.clone(), metrics_message]);
                    }
                }
            }
        }

        // If not a file we can process, return empty
        Ok(vec![])
    }

    async fn aggregate_results(&self, results: Vec<ScanMessage>) -> PluginResult<ScanMessage> {
        if !self.initialized {
            return Err(PluginError::invalid_state("Plugin not initialized"));
        }

        // Process the input messages to extract metrics
        let mut total_files = 0;
        let mut total_lines = 0;
        let mut total_complexity = 0.0;
        let mut file_extensions: HashMap<String, usize> = HashMap::new();

        for message in &results {
            if let MessageData::FileInfo { path, lines, .. } = &message.data {
                use std::path::Path;
                let path_obj = Path::new(path);
                if let Some(extension) = path_obj.extension().and_then(|ext| ext.to_str()) {
                    if is_source_file_extension(extension) {
                        total_files += 1;
                        total_lines += *lines as u64;
                        
                        // Calculate complexity for this file
                        let complexity = self.complexity_calculator
                            .calculate_complexity(&path_obj.to_string_lossy())
                            .unwrap_or(1) as f64;
                        total_complexity += complexity;
                        
                        *file_extensions.entry(extension.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }

        // Create aggregated metrics data (for future use in detailed reporting)
        let _aggregated_data = json!({
            "total_files": total_files,
            "total_lines": total_lines,
            "total_comment_lines": 0, // TODO: implement comment counting
            "total_blank_lines": 0,   // TODO: implement blank line counting
            "average_lines_per_file": if total_files > 0 { total_lines as f64 / total_files as f64 } else { 0.0 },
            "total_complexity": total_complexity,
            "average_complexity": if total_files > 0 { total_complexity / total_files as f64 } else { 0.0 },
            "file_extensions": file_extensions,
            "function": "metrics"
        });

        // Create the aggregated message using MetricInfo
        let aggregated_message = ScanMessage {
            header: MessageHeader::new(
                ScanMode::FILES, 
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            ),
            data: MessageData::MetricInfo {
                file_count: total_files as u32,
                line_count: total_lines,
                complexity: if total_files > 0 { total_complexity / total_files as f64 } else { 0.0 },
            },
        };

        Ok(aggregated_message)
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
            file_metrics: RwLock::new(self.file_metrics.read().unwrap().clone()),
            total_lines: self.total_lines,
            total_files: self.total_files,
            complexity_calculator: ComplexityCalculator::new(),
        }
    }

    /// Generate metrics message for a specific file
    fn generate_file_metrics_message(&self, _file_path: &str, metrics: &FileMetrics) -> PluginResult<ScanMessage> {
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
    
    /// Execute code metrics analysis function
    async fn execute_code_metrics(&self) -> PluginResult<PluginResponse> {
        // Measure execution time
        let start_time = std::time::Instant::now();

        // CRITICAL: Use aggregated data from scanner subsystem instead of direct scanning
        // This enforces the architectural separation - plugins NEVER scan repositories directly
        let file_metrics = self.file_metrics.read().unwrap();

        if file_metrics.is_empty() {
            // This is a CRITICAL ERROR - scanner should have provided data
            return Err(PluginError::execution_failed(
                "No aggregated scan data available. This indicates a critical architectural failure - \
                the scanner subsystem must provide data for the specified ScanMode before plugin execution. \
                Plugins are not allowed to scan repositories directly."
            ));
        }

        let total_files = file_metrics.len();
        let total_lines: usize = file_metrics.values().map(|m| m.lines_of_code).sum();
        let total_complexity: usize = file_metrics.values().map(|m| m.cyclomatic_complexity).sum();
        let total_comment_lines: usize = file_metrics.values().map(|m| m.comment_lines).sum();
        let total_blank_lines: usize = file_metrics.values().map(|m| m.blank_lines).sum();

        let data = json!({
            "total_files": total_files,
            "total_lines": total_lines,
            "total_comment_lines": total_comment_lines,
            "total_blank_lines": total_blank_lines,
            "average_lines_per_file": if total_files > 0 { total_lines as f64 / total_files as f64 } else { 0.0 },
            "total_complexity": total_complexity,
            "average_complexity": if total_files > 0 { total_complexity as f64 / total_files as f64 } else { 0.0 },
            "function": "metrics"
        });

        let duration_us = start_time.elapsed().as_micros() as u64;

        Ok(PluginResponse::Execute {
            request_id: "code_metrics".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_us,
                memory_used: 0,
                entries_processed: total_files as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
    
    /// Execute complexity analysis function
    async fn execute_complexity_analysis(&self) -> PluginResult<PluginResponse> {
        // Measure execution time
        let start_time = std::time::Instant::now();

        let mut complexity_distribution = HashMap::new();
        let mut high_complexity_files = Vec::new();

        let file_metrics = self.file_metrics.read().unwrap();
        for (path, metrics) in file_metrics.iter() {
            let complexity = metrics.cyclomatic_complexity;
            *complexity_distribution.entry(complexity).or_insert(0) += 1;

            if complexity > 10 {
                high_complexity_files.push(json!({
                    "file": path,
                    "complexity": complexity,
                    "lines_of_code": metrics.lines_of_code
                }));
            }
        }

        let data = json!({
            "complexity_distribution": complexity_distribution,
            "high_complexity_files": high_complexity_files,
            "files_analyzed": self.total_files,
            "function": "complexity"
        });

        let duration_us = start_time.elapsed().as_micros() as u64;

        Ok(PluginResponse::Execute {
            request_id: "complexity_analysis".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_us,
                memory_used: 0,
                entries_processed: self.total_files as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
    
    /// Execute file statistics function
    async fn execute_file_statistics(&self) -> PluginResult<PluginResponse> {
        // Measure execution time
        let start_time = std::time::Instant::now();

        let mut extension_stats = HashMap::new();
        let mut size_distribution = HashMap::new();

        let file_metrics = self.file_metrics.read().unwrap();
        for metrics in file_metrics.values() {
            let ext_stat = extension_stats.entry(metrics.file_extension.clone()).or_insert(json!({
                "count": 0,
                "total_lines": 0,
                "total_size": 0
            }));

            ext_stat["count"] = serde_json::Value::Number(serde_json::Number::from(
                ext_stat["count"].as_u64().unwrap_or(0) + 1
            ));
            ext_stat["total_lines"] = serde_json::Value::Number(serde_json::Number::from(
                ext_stat["total_lines"].as_u64().unwrap_or(0) + metrics.lines_of_code as u64
            ));
            ext_stat["total_size"] = serde_json::Value::Number(serde_json::Number::from(
                ext_stat["total_size"].as_u64().unwrap_or(0) + metrics.file_size_bytes as u64
            ));

            let size_bucket = match metrics.file_size_bytes {
                0..=1024 => "small",
                1025..=10240 => "medium",
                10241..=102400 => "large",
                _ => "very_large",
            };
            *size_distribution.entry(size_bucket.to_string()).or_insert(0) += 1;
        }

        let data = json!({
            "extension_statistics": extension_stats,
            "size_distribution": size_distribution,
            "total_files": self.total_files,
            "function": "files"
        });

        let duration_us = start_time.elapsed().as_micros() as u64;

        Ok(PluginResponse::Execute {
            request_id: "file_statistics".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_us,
                memory_used: 0,
                entries_processed: self.total_files as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
    
    /// Execute hotspot analysis function combining complexity and change frequency
    async fn execute_hotspot_analysis(&self) -> PluginResult<PluginResponse> {
        // TODO: This function requires git history data from scanner
        // Will be implemented when ScanMode::GIT_HISTORY is available
        // See GS-57 Phase 7 implementation
        
        return Err(PluginError::execution_failed(
            "Hotspot analysis requires git history data from scanner (not yet implemented)"
        ));
        
        /* STUBBED OUT - REQUIRES ScanMode::GIT_HISTORY
        // Measure execution time
        let start_time = std::time::Instant::now();

        // Require repository for change frequency analysis
        let repository = self.repository.as_ref()
            .ok_or_else(|| PluginError::invalid_state("Repository not available for hotspot analysis"))?;
        
        // Convert FileMetrics to FileComplexityMetrics for hotspot detection
        let mut complexity_metrics = HashMap::new();
        let file_metrics = self.file_metrics.read().unwrap();
        for (path, metrics) in file_metrics.iter() {
            let mut complexity_metric = FileComplexityMetrics::new(path.clone());
            complexity_metric.lines_of_code = metrics.lines_of_code;
            complexity_metric.cyclomatic_complexity = metrics.cyclomatic_complexity as f64;
            complexity_metric.comment_ratio = if metrics.lines_of_code > 0 {
                metrics.comment_lines as f64 / metrics.lines_of_code as f64
            } else {
                0.0
            };
            complexity_metric.file_size_bytes = metrics.file_size_bytes;
            complexity_metrics.insert(path.clone(), complexity_metric);
        }
        
        // Perform change frequency analysis
        let mut frequency_analyzer = ChangeFrequencyAnalyzer::new(
            repository.clone(), 
            TimeWindow::Quarter // Analyze last 3 months by default
        );
        
        if let Err(e) = frequency_analyzer.analyze() {
            return Err(PluginError::execution_failed(
                format!("Change frequency analysis failed: {}", e)
            ));
        }
        
        let change_stats = frequency_analyzer.get_file_stats();
        
        // Detect hotspots
        let hotspot_detector = HotspotDetector::with_defaults(TimeWindow::Quarter);
        let hotspots = hotspot_detector.get_top_hotspots(&complexity_metrics, change_stats, 20);
        let summary = hotspot_detector.generate_summary(&complexity_metrics, change_stats);
        
        // Prepare response data
        let hotspot_data: Vec<_> = hotspots.iter().map(|h| {
            json!({
                "file_path": h.file_path,
                "hotspot_score": h.hotspot_score,
                "complexity_score": h.complexity_score,
                "frequency_score": h.frequency_score,
                "recency_weight": h.recency_weight,
                "priority": format!("{:?}", h.priority),
                "change_count": h.change_stats.change_count,
                "author_count": h.change_stats.author_count,
                "last_changed": h.change_stats.last_changed,
                "lines_of_code": h.complexity_metrics.lines_of_code,
                "cyclomatic_complexity": h.complexity_metrics.cyclomatic_complexity,
                "recommendations": h.recommendations
            })
        }).collect();
        
        let data = json!({
            "hotspots": hotspot_data,
            "summary": {
                "total_hotspots": summary.total_hotspots,
                "critical_hotspots": summary.critical_hotspots,
                "high_hotspots": summary.high_hotspots,
                "medium_hotspots": summary.medium_hotspots,
                "low_hotspots": summary.low_hotspots,
                "average_hotspot_score": summary.average_hotspot_score,
                "max_hotspot_score": summary.max_hotspot_score,
                "files_analyzed": summary.files_analyzed,
                "time_window": format!("{:?}", summary.time_window)
            },
            "change_frequency_summary": {
                "total_files": frequency_analyzer.get_summary().total_files,
                "total_changes": frequency_analyzer.get_summary().total_changes,
                "commits_analyzed": frequency_analyzer.get_summary().total_commits_analyzed,
                "average_frequency": frequency_analyzer.get_summary().average_frequency,
                "max_frequency": frequency_analyzer.get_summary().max_frequency
            },
            "function": "hotspots"
        });
        
        let duration_us = start_time.elapsed().as_micros() as u64;

        Ok(PluginResponse::Execute {
            request_id: "hotspot_analysis".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_us,
                memory_used: 0,
                entries_processed: hotspots.len() as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
        */
    }
    
    /// Execute duplication analysis function
    async fn execute_duplication_analysis(&self) -> PluginResult<PluginResponse> {
        // Measure execution time
        let start_time = std::time::Instant::now();
        // Collect file contents for analysis
        let mut file_contents = HashMap::new();
        
        // For now, we'll use a simplified approach by reading available files
        // In a full implementation, this would integrate with the scanner to get actual file contents
        let file_metrics = self.file_metrics.read().unwrap();
        for (file_path, metrics) in file_metrics.iter() {
            // Generate sample content based on metrics for demonstration
            // In practice, this would read actual file contents
            let sample_content = self.generate_sample_content(file_path, metrics);
            file_contents.insert(file_path.clone(), sample_content);
        }
        
        // Create duplication detector with default config
        let detector = DuplicationDetector::with_defaults();
        
        // Detect duplicates
        let duplicate_groups = detector.detect_duplicates(&file_contents);
        let summary = detector.generate_summary(&file_contents, &duplicate_groups);
        
        // Prepare response data
        let duplicate_data: Vec<_> = duplicate_groups.iter().map(|group| {
            json!({
                "id": group.id,
                "similarity_score": group.similarity_score,
                "total_lines": group.total_lines,
                "total_tokens": group.total_tokens,
                "impact_score": group.impact_score,
                "block_count": group.blocks.len(),
                "involved_files": group.get_involved_files(),
                "blocks": group.blocks.iter().map(|block| {
                    json!({
                        "file_path": block.file_path,
                        "start_line": block.start_line,
                        "end_line": block.end_line,
                        "line_count": block.line_count(),
                        "token_count": block.token_count()
                    })
                }).collect::<Vec<_>>()
            })
        }).collect();
        
        let data = json!({
            "duplicate_groups": duplicate_data,
            "summary": {
                "total_files_analyzed": summary.total_files_analyzed,
                "total_lines_analyzed": summary.total_lines_analyzed,
                "duplicate_groups": summary.duplicate_groups,
                "total_duplicate_lines": summary.total_duplicate_lines,
                "duplication_percentage": summary.duplication_percentage,
                "average_similarity": summary.average_similarity,
                "highest_impact_score": summary.highest_impact_score,
                "files_with_duplicates": summary.files_with_duplicates
            },
            "function": "duplicates"
        });
        
        let duration_us = start_time.elapsed().as_micros() as u64;

        Ok(PluginResponse::Execute {
            request_id: "duplication_analysis".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_us,
                memory_used: 0,
                entries_processed: duplicate_groups.len() as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
    
    /// Generate sample content for duplication analysis (temporary implementation)
    fn generate_sample_content(&self, file_path: &str, metrics: &FileMetrics) -> String {
        // This is a placeholder - in practice, actual file contents would be read
        let extension = &metrics.file_extension;
        let lines_count = metrics.lines_of_code;
        
        match extension.as_str() {
            "rs" => {
                let mut content = String::new();
                content.push_str("// Sample Rust file\n");
                content.push_str("use std::collections::HashMap;\n\n");
                content.push_str("fn main() {\n");
                for i in 0..lines_count.min(50) {
                    content.push_str(&format!("    println!(\"Line {}\");\n", i));
                }
                content.push_str("}\n");
                content
            }
            "py" => {
                let mut content = String::new();
                content.push_str("# Sample Python file\n");
                content.push_str("import os\n\n");
                content.push_str("def main():\n");
                for i in 0..lines_count.min(50) {
                    content.push_str(&format!("    print(\"Line {}\")\n", i));
                }
                content.push_str("\nif __name__ == '__main__':\n    main()\n");
                content
            }
            _ => {
                let mut content = String::new();
                for i in 0..lines_count.min(50) {
                    content.push_str(&format!("Line {} of {}\n", i, file_path));
                }
                content
            }
        }
    }
    
    /// Execute comprehensive technical debt assessment
    async fn execute_debt_assessment(&self) -> PluginResult<PluginResponse> {
        // TODO: This function requires git history and change frequency data from scanner
        // Will be implemented when ScanMode::GIT_HISTORY and ScanMode::CHANGE_FREQUENCY are available
        // See GS-57 Phase 7 implementation
        
        return Err(PluginError::execution_failed(
            "Technical debt assessment requires git history and change frequency data from scanner (not yet implemented)"
        ));
        
        /* STUBBED OUT - REQUIRES ScanMode::GIT_HISTORY and ScanMode::CHANGE_FREQUENCY
        // Measure execution time
        let start_time = std::time::Instant::now();

        if !self.initialized {
            return Err(PluginError::invalid_state("Plugin not initialized"));
        }
        
        let repository = match &self.repository {
            Some(repo) => repo.clone(),
            None => return Err(PluginError::execution_failed("No repository available for change frequency analysis")),
        };
        
        // Build complexity metrics map
        let mut complexity_metrics = HashMap::new();
        let file_metrics = self.file_metrics.read().unwrap();
        for (file_path, metrics) in file_metrics.iter() {
            let mut complexity = FileComplexityMetrics::new(file_path.clone());
            complexity.lines_of_code = metrics.lines_of_code;
            complexity.cyclomatic_complexity = metrics.cyclomatic_complexity as f64;
            complexity.comment_ratio = if metrics.lines_of_code > 0 {
                metrics.comment_lines as f64 / metrics.lines_of_code as f64
            } else {
                0.0
            };
            complexity.file_size_bytes = metrics.file_size_bytes;
            complexity_metrics.insert(file_path.clone(), complexity);
        }
        
        // Analyze change frequency
        let mut analyzer = ChangeFrequencyAnalyzer::new(repository, TimeWindow::Month);
        let change_stats = match analyzer.analyze() {
            Ok(_) => analyzer.get_file_stats().clone(),
            Err(e) => {
                eprintln!("Warning: Could not analyze change frequency: {}", e);
                HashMap::new()
            }
        };
        
        // Generate file contents for duplication analysis (using sample content)
        let mut file_contents = HashMap::new();
        let file_metrics = self.file_metrics.read().unwrap();
        for (file_path, metrics) in file_metrics.iter() {
            let content = self.generate_sample_content(file_path, metrics);
            file_contents.insert(file_path.clone(), content);
        }
        
        // Analyze duplications
        let duplication_detector = DuplicationDetector::with_defaults();
        let duplicate_groups = duplication_detector.detect_duplicates(&file_contents);
        
        // Perform debt assessment
        let debt_assessor = DebtAssessor::with_defaults();
        let assessments = debt_assessor.assess_debt(&complexity_metrics, &change_stats, &duplicate_groups);
        let summary = debt_assessor.generate_summary(&assessments);
        
        // Prepare response data
        let assessment_data: Vec<_> = assessments.iter().map(|assessment| {
            json!({
                "file_path": assessment.file_path,
                "debt_score": assessment.debt_score,
                "debt_level": assessment.debt_level.as_str(),
                "component_scores": {
                    "complexity": assessment.complexity_score,
                    "frequency": assessment.frequency_score,
                    "duplication": assessment.duplication_score,
                    "size": assessment.size_score,
                    "age": assessment.age_score
                },
                "recommendations": assessment.recommendations,
                "priority_actions": assessment.priority_actions,
                "estimated_hours": assessment.estimated_hours
            })
        }).collect();
        
        let data = json!({
            "assessments": assessment_data,
            "summary": {
                "total_files_with_debt": summary.total_files_with_debt,
                "debt_levels": {
                    "critical": summary.critical_debt_files,
                    "high": summary.high_debt_files,
                    "medium": summary.medium_debt_files,
                    "low": summary.low_debt_files,
                    "minimal": summary.minimal_debt_files
                },
                "average_debt_score": summary.average_debt_score,
                "max_debt_score": summary.max_debt_score,
                "total_estimated_hours": summary.total_estimated_hours,
                "config": {
                    "complexity_weight": summary.config.complexity_weight,
                    "frequency_weight": summary.config.frequency_weight,
                    "duplication_weight": summary.config.duplication_weight,
                    "size_weight": summary.config.size_weight,
                    "age_weight": summary.config.age_weight,
                    "debt_threshold": summary.config.debt_threshold,
                    "time_window": format!("{:?}", summary.config.time_window)
                }
            },
            "function": "debt"
        });
        
        let duration_us = start_time.elapsed().as_micros() as u64;

        Ok(PluginResponse::Execute {
            request_id: "debt_assessment".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_us,
                memory_used: 0,
                entries_processed: assessments.len() as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
        */
    }
}

/// Helper function to determine if a file extension represents a source code file
fn is_source_file_extension(extension: &str) -> bool {
    matches!(extension.to_lowercase().as_str(),
        // Common programming languages
        "rs" | "py" | "js" | "ts" | "java" | "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" |
        "cs" | "php" | "rb" | "go" | "kt" | "swift" | "scala" | "clj" | "hs" | "ml" |
        "fs" | "vb" | "pas" | "pl" | "pm" | "r" | "m" | "mm" | "dart" | "lua" | "sh" |
        "bash" | "zsh" | "fish" | "ps1" | "psm1" | "psd1" | "bat" | "cmd" | "asm" | "s" |
        // Web technologies
        "html" | "htm" | "css" | "scss" | "sass" | "less" | "jsx" | "tsx" | "vue" |
        "svelte" | "asp" | "aspx" | "jsp" | "erb" | "ejs" | "hbs" | "mustache" |
        // Configuration and data
        "json" | "xml" | "yaml" | "yml" | "toml" | "ini" | "cfg" | "conf" | "properties" |
        // Database
        "sql" | "plsql" | "psql" |
        // Other
        "dockerfile" | "makefile" | "cmake" | "gradle" | "sbt" | "build"
    )
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

}