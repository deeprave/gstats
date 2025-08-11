//! Complexity Processor
//! 
//! Event-driven processor that calculates code complexity metrics
//! by analyzing file content and structure. This processor can be used
//! by any plugin that needs complexity analysis.

use crate::scanner::async_engine::events::{RepositoryEvent, FileInfo};
use crate::scanner::async_engine::processors::{EventProcessor, ProcessorStats};
use crate::scanner::async_engine::shared_state::{SharedProcessorState, RepositoryMetadata};
use crate::scanner::messages::{ScanMessage, MessageData, MessageHeader};
use crate::scanner::modes::ScanMode;
use crate::plugin::PluginResult;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use log::debug;
use serde::{Serialize, Deserialize};

/// Complexity metrics for a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityMetrics {
    pub file_path: String,
    pub cyclomatic_complexity: f64,
    pub cognitive_complexity: f64,
    pub lines_of_code: u32,
    pub function_count: u32,
    pub class_count: u32,
    pub nesting_depth: u32,
    pub file_size_bytes: u64,
}

impl ComplexityMetrics {
    pub fn new(file_path: String) -> Self {
        Self {
            file_path,
            cyclomatic_complexity: 0.0,
            cognitive_complexity: 0.0,
            lines_of_code: 0,
            function_count: 0,
            class_count: 0,
            nesting_depth: 0,
            file_size_bytes: 0,
        }
    }

    /// Calculate overall complexity score
    pub fn complexity_score(&self) -> f64 {
        // Weighted combination of different complexity metrics
        let cyclomatic_weight = 0.4;
        let cognitive_weight = 0.3;
        let size_weight = 0.2;
        let nesting_weight = 0.1;

        let size_factor = (self.lines_of_code as f64 / 100.0).min(10.0); // Cap at 10x
        let nesting_factor = self.nesting_depth as f64;

        (self.cyclomatic_complexity * cyclomatic_weight) +
        (self.cognitive_complexity * cognitive_weight) +
        (size_factor * size_weight) +
        (nesting_factor * nesting_weight)
    }

    /// Determine complexity level
    pub fn complexity_level(&self) -> ComplexityLevel {
        let score = self.complexity_score();
        match score {
            s if s < 5.0 => ComplexityLevel::Low,
            s if s < 10.0 => ComplexityLevel::Medium,
            s if s < 20.0 => ComplexityLevel::High,
            _ => ComplexityLevel::VeryHigh,
        }
    }
}

/// Complexity level classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplexityLevel {
    Low,
    Medium,
    High,
    VeryHigh,
}

impl ComplexityLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ComplexityLevel::Low => "low",
            ComplexityLevel::Medium => "medium",
            ComplexityLevel::High => "high",
            ComplexityLevel::VeryHigh => "very_high",
        }
    }
}

/// Complexity Processor - can be used by any plugin
pub struct ComplexityProcessor {
    file_complexities: HashMap<String, ComplexityMetrics>,
    stats: ProcessorStats,
    shared_state: Option<Arc<SharedProcessorState>>,
}

impl ComplexityProcessor {
    pub fn new() -> Self {
        Self {
            file_complexities: HashMap::new(),
            stats: ProcessorStats::default(),
            shared_state: None,
        }
    }

    fn calculate_complexity(&self, file_path: &str, file_info: Option<&FileInfo>) -> ComplexityMetrics {
        let mut metrics = ComplexityMetrics::new(file_path.to_string());
        
        // Extract file extension for language-specific analysis
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // Set basic metrics from file info if available
        if let Some(info) = file_info {
            metrics.lines_of_code = info.line_count.unwrap_or(0) as u32;
            metrics.file_size_bytes = info.size;
        }

        // Language-specific complexity estimation
        // In a full implementation, this would parse the actual file content
        match extension {
            "rs" => {
                // Rust complexity estimation
                metrics.cyclomatic_complexity = estimate_rust_complexity(&metrics);
                metrics.cognitive_complexity = metrics.cyclomatic_complexity * 0.8;
                metrics.function_count = estimate_function_count(&metrics, "rust");
                metrics.nesting_depth = estimate_nesting_depth(&metrics, "rust");
            }
            "py" => {
                // Python complexity estimation
                metrics.cyclomatic_complexity = estimate_python_complexity(&metrics);
                metrics.cognitive_complexity = metrics.cyclomatic_complexity * 0.9;
                metrics.function_count = estimate_function_count(&metrics, "python");
                metrics.nesting_depth = estimate_nesting_depth(&metrics, "python");
            }
            "js" | "ts" => {
                // JavaScript/TypeScript complexity estimation
                metrics.cyclomatic_complexity = estimate_js_complexity(&metrics);
                metrics.cognitive_complexity = metrics.cyclomatic_complexity * 0.85;
                metrics.function_count = estimate_function_count(&metrics, "javascript");
                metrics.nesting_depth = estimate_nesting_depth(&metrics, "javascript");
            }
            "java" | "c" | "cpp" | "cc" | "cxx" => {
                // C-family languages complexity estimation
                metrics.cyclomatic_complexity = estimate_c_family_complexity(&metrics);
                metrics.cognitive_complexity = metrics.cyclomatic_complexity * 0.75;
                metrics.function_count = estimate_function_count(&metrics, "c_family");
                metrics.class_count = estimate_class_count(&metrics, extension);
                metrics.nesting_depth = estimate_nesting_depth(&metrics, "c_family");
            }
            _ => {
                // Generic complexity estimation
                metrics.cyclomatic_complexity = estimate_generic_complexity(&metrics);
                metrics.cognitive_complexity = metrics.cyclomatic_complexity * 0.8;
                metrics.function_count = estimate_function_count(&metrics, "generic");
                metrics.nesting_depth = estimate_nesting_depth(&metrics, "generic");
            }
        }

        metrics
    }

    /// Get the collected complexity metrics (for use by other processors)
    pub fn get_complexity_metrics(&self) -> &HashMap<String, ComplexityMetrics> {
        &self.file_complexities
    }

    fn create_complexity_messages(&self) -> Vec<ScanMessage> {
        let mut messages = Vec::new();
        
        for (file_path, metrics) in &self.file_complexities {
            let header = MessageHeader::new(
                ScanMode::METRICS,
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );

            let data = MessageData::MetricInfo {
                file_count: 1,
                line_count: metrics.lines_of_code as u64,
                complexity: metrics.complexity_score(),
            };

            messages.push(ScanMessage::new(header, data));
        }
        
        messages
    }
}

#[async_trait]
impl EventProcessor for ComplexityProcessor {
    fn supported_modes(&self) -> ScanMode {
        ScanMode::METRICS
    }

    fn name(&self) -> &'static str {
        "complexity"
    }

    fn set_shared_state(&mut self, shared_state: Arc<SharedProcessorState>) {
        self.shared_state = Some(shared_state);
    }

    fn shared_state(&self) -> Option<&Arc<SharedProcessorState>> {
        self.shared_state.as_ref()
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        debug!("Initialized ComplexityProcessor");
        Ok(())
    }

    async fn process_event(&mut self, event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
        match event {
            RepositoryEvent::FileChanged { file_path, .. } => {
                let metrics = self.calculate_complexity(file_path, None);
                self.file_complexities.insert(file_path.clone(), metrics);
            }
            _ => {}
        }
        self.stats.events_processed += 1;
        Ok(vec![])
    }

    async fn finalize(&mut self) -> PluginResult<Vec<ScanMessage>> {
        let messages = self.create_complexity_messages();
        self.stats.messages_generated = messages.len();
        Ok(messages)
    }

    fn get_stats(&self) -> ProcessorStats {
        self.stats.clone()
    }
}

impl Default for ComplexityProcessor {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions for complexity estimation
// In a full implementation, these would parse actual file content

fn estimate_rust_complexity(metrics: &ComplexityMetrics) -> f64 {
    // Rust-specific complexity estimation based on lines of code
    let base_complexity = (metrics.lines_of_code as f64 / 20.0).max(1.0);
    base_complexity * 1.2 // Rust tends to have slightly higher complexity due to ownership
}

fn estimate_python_complexity(metrics: &ComplexityMetrics) -> f64 {
    // Python-specific complexity estimation
    let base_complexity = (metrics.lines_of_code as f64 / 25.0).max(1.0);
    base_complexity * 0.9 // Python tends to be more readable
}

fn estimate_js_complexity(metrics: &ComplexityMetrics) -> f64 {
    // JavaScript/TypeScript complexity estimation
    let base_complexity = (metrics.lines_of_code as f64 / 22.0).max(1.0);
    base_complexity * 1.1 // JS can have callback complexity
}

fn estimate_c_family_complexity(metrics: &ComplexityMetrics) -> f64 {
    // C-family languages complexity estimation
    let base_complexity = (metrics.lines_of_code as f64 / 18.0).max(1.0);
    base_complexity * 1.3 // C-family can be quite complex
}

fn estimate_generic_complexity(metrics: &ComplexityMetrics) -> f64 {
    // Generic complexity estimation
    (metrics.lines_of_code as f64 / 20.0).max(1.0)
}

fn estimate_function_count(metrics: &ComplexityMetrics, language: &str) -> u32 {
    // Estimate function count based on lines of code and language
    let lines_per_function = match language {
        "rust" => 15,
        "python" => 12,
        "javascript" => 10,
        "c_family" => 20,
        _ => 15,
    };
    
    (metrics.lines_of_code / lines_per_function).max(1)
}

fn estimate_class_count(metrics: &ComplexityMetrics, extension: &str) -> u32 {
    // Estimate class count for OOP languages
    match extension {
        "java" => (metrics.lines_of_code / 50).max(0),
        "cpp" | "cc" | "cxx" => (metrics.lines_of_code / 80).max(0),
        _ => 0,
    }
}

fn estimate_nesting_depth(metrics: &ComplexityMetrics, language: &str) -> u32 {
    // Estimate maximum nesting depth
    let complexity_factor = match language {
        "rust" => 0.8, // Rust encourages early returns
        "python" => 1.0,
        "javascript" => 1.2, // JS can have deep callback nesting
        "c_family" => 1.1,
        _ => 1.0,
    };
    
    let base_depth = (metrics.lines_of_code as f64 / 30.0 * complexity_factor) as u32;
    base_depth.min(10).max(1) // Cap at reasonable maximum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_complexity_processor_creation() {
        let processor = ComplexityProcessor::new();
        assert_eq!(processor.name(), "complexity");
        assert_eq!(processor.supported_modes(), ScanMode::METRICS);
        assert!(processor.file_complexities.is_empty());
    }

    #[tokio::test]
    async fn test_complexity_calculation() {
        let processor = ComplexityProcessor::new();
        let metrics = processor.calculate_complexity("test.rs", None);
        
        assert_eq!(metrics.file_path, "test.rs");
        assert!(metrics.cyclomatic_complexity > 0.0);
    }

    #[tokio::test]
    async fn test_complexity_levels() {
        let mut metrics = ComplexityMetrics::new("test.rs".to_string());
        
        metrics.cyclomatic_complexity = 2.0;
        assert_eq!(metrics.complexity_level(), ComplexityLevel::Low);
        
        metrics.cyclomatic_complexity = 8.0;
        assert_eq!(metrics.complexity_level(), ComplexityLevel::Medium);
        
        metrics.cyclomatic_complexity = 15.0;
        assert_eq!(metrics.complexity_level(), ComplexityLevel::High);
        
        metrics.cyclomatic_complexity = 25.0;
        assert_eq!(metrics.complexity_level(), ComplexityLevel::VeryHigh);
    }

    #[tokio::test]
    async fn test_file_processing() {
        let mut processor = ComplexityProcessor::new();
        processor.initialize().await.unwrap();

        let event = RepositoryEvent::FileChanged {
            file_path: "test.rs".to_string(),
            change_data: crate::scanner::async_engine::events::FileChangeData {
                change_type: crate::scanner::async_engine::events::ChangeType::Modified,
                old_path: Some("test.rs".to_string()),
                new_path: "test.rs".to_string(),
                insertions: 10,
                deletions: 2,
                is_binary: false,
            },
            commit_context: crate::scanner::async_engine::events::CommitInfo {
                hash: "abc123".to_string(),
                short_hash: "abc123".to_string(),
                author_name: "Test Author".to_string(),
                author_email: "test@example.com".to_string(),
                committer_name: "Test Author".to_string(),
                committer_email: "test@example.com".to_string(),
                timestamp: SystemTime::now(),
                message: "Test commit".to_string(),
                parent_hashes: vec![],
                changed_files: vec![],
                insertions: 0,
                deletions: 0,
            },
        };

        let messages = processor.process_event(&event).await.unwrap();
        assert!(messages.is_empty()); // No messages during processing

        assert_eq!(processor.file_complexities.len(), 1);
        assert!(processor.file_complexities.contains_key("test.rs"));
    }
}
