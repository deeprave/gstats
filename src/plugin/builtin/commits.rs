//! Commits Analysis Plugin
//! 
//! Built-in plugin for analyzing git commit history and statistics.

use crate::plugin::{
    Plugin, ScannerPlugin, PluginInfo, PluginContext, PluginRequest, PluginResponse,
    PluginResult, PluginError, traits::{PluginType, PluginFunction}
};
use crate::scanner::{modes::ScanMode, messages::{ScanMessage, MessageData, MessageHeader}};
use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;

/// Commits analysis plugin
pub struct CommitsPlugin {
    info: PluginInfo,
    initialized: bool,
    commit_count: usize,
    author_stats: HashMap<String, usize>,
}

impl CommitsPlugin {
    /// Create a new commits plugin
    pub fn new() -> Self {
        let info = PluginInfo::new(
            "commits".to_string(),
            "1.0.0".to_string(),
            crate::scanner::version::get_api_version() as u32,
            "Analyzes git commit history and provides commit statistics".to_string(),
            "gstats built-in".to_string(),
            PluginType::Scanner,
        )
        .with_capability(
            "commit_analysis".to_string(),
            "Processes git commits and generates statistics".to_string(),
            "1.0.0".to_string(),
        )
        .with_capability(
            "author_tracking".to_string(),
            "Tracks commits by author for contributor analysis".to_string(),
            "1.0.0".to_string(),
        );

        Self {
            info,
            initialized: false,
            commit_count: 0,
            author_stats: HashMap::new(),
        }
    }

    /// Process a commit message and extract statistics
    fn process_commit(&mut self, message: &ScanMessage) -> PluginResult<()> {
        // Extract commit information from scan message
        if let MessageData::CommitInfo { author, .. } = &message.data {
            self.commit_count += 1;
            *self.author_stats.entry(author.clone()).or_insert(0) += 1;
        }
        Ok(())
    }

    /// Generate commit summary statistics
    fn generate_summary(&self) -> PluginResult<ScanMessage> {
        // Create a metric info message containing our summary statistics
        let data = MessageData::MetricInfo {
            file_count: self.author_stats.len() as u32,
            line_count: self.commit_count as u64,
            complexity: if self.author_stats.is_empty() {
                0.0
            } else {
                self.commit_count as f64 / self.author_stats.len() as f64
            },
        };

        let header = MessageHeader::new(
            ScanMode::HISTORY,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );

        Ok(ScanMessage::new(header, data))
    }
    
    /// Execute commits analysis function
    async fn execute_commits_analysis(&self) -> PluginResult<PluginResponse> {
        let data = json!({
            "total_commits": self.commit_count,
            "unique_authors": self.author_stats.len(),
            "avg_commits_per_author": if self.author_stats.is_empty() { 
                0.0 
            } else { 
                self.commit_count as f64 / self.author_stats.len() as f64 
            },
            "function": "commits"
        });
        
        Ok(PluginResponse::Execute {
            request_id: "commits_analysis".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_ms: 0,
                memory_used: 0,
                items_processed: self.commit_count as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
    
    /// Execute author analysis function  
    async fn execute_author_analysis(&self) -> PluginResult<PluginResponse> {
        let mut authors: Vec<_> = self.author_stats.iter().collect();
        authors.sort_by(|a, b| b.1.cmp(a.1)); // Sort by commit count descending
        
        let data = json!({
            "total_authors": self.author_stats.len(),
            "top_authors": authors.iter().take(10).map(|(name, count)| {
                json!({ "name": name, "commits": count })
            }).collect::<Vec<_>>(),
            "author_stats": self.author_stats,
            "function": "authors"
        });
        
        Ok(PluginResponse::Execute {
            request_id: "author_analysis".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_ms: 0,
                memory_used: 0,
                items_processed: self.author_stats.len() as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
}

impl Default for CommitsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for CommitsPlugin {
    fn plugin_info(&self) -> &PluginInfo {
        &self.info
    }

    async fn initialize(&mut self, _context: &PluginContext) -> PluginResult<()> {
        if self.initialized {
            return Err(PluginError::initialization_failed("Plugin already initialized"));
        }

        // Reset statistics
        self.commit_count = 0;
        self.author_stats.clear();
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
                    crate::plugin::InvocationType::Direct => self.default_function().unwrap_or("commits"),
                    crate::plugin::InvocationType::Default => "commits",
                };
                
                // Route to appropriate function
                match function_name {
                    "commits" | "commit" | "history" => {
                        self.execute_commits_analysis().await
                    }
                    "authors" | "contributors" | "committers" => {
                        self.execute_author_analysis().await
                    }
                    _ => Err(PluginError::execution_failed(
                        format!("Unknown function: {}", function_name)
                    )),
                }
            }
            PluginRequest::GetStatistics => {
                let summary = self.generate_summary()?;
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
        self.commit_count = 0;
        self.author_stats.clear();
        Ok(())
    }
    
    /// Get all functions this plugin can handle
    fn advertised_functions(&self) -> Vec<PluginFunction> {
        vec![
            PluginFunction {
                name: "commits".to_string(),
                aliases: vec!["commit".to_string(), "history".to_string()],
                description: "Analyze git commit history and generate commit statistics".to_string(),
                is_default: true,
            },
            PluginFunction {
                name: "authors".to_string(),
                aliases: vec!["contributors".to_string(), "committers".to_string()],
                description: "Analyze commit authors and contributor statistics".to_string(),
                is_default: false,
            },
        ]
    }
    
    /// Get the default function name
    fn default_function(&self) -> Option<&str> {
        Some("commits")
    }
}

#[async_trait]
impl ScannerPlugin for CommitsPlugin {
    fn supported_modes(&self) -> ScanMode {
        ScanMode::HISTORY
    }

    async fn process_scan_data(&self, data: &ScanMessage) -> PluginResult<Vec<ScanMessage>> {
        if !self.initialized {
            return Err(PluginError::invalid_state("Plugin not initialized"));
        }

        // For this implementation, we'll create a mutable copy for processing
        let mut processor = self.clone_for_processing();
        processor.process_commit(data)?;

        // Return the original message plus any derived messages
        let mut results = vec![data.clone()];
        
        // Add commit analysis result if this is a commit message
        if matches!(data.data, MessageData::CommitInfo { .. }) {
            let analysis = processor.generate_commit_analysis(data)?;
            results.push(analysis);
        }

        Ok(results)
    }

    async fn aggregate_results(&self, results: Vec<ScanMessage>) -> PluginResult<ScanMessage> {
        if !self.initialized {
            return Err(PluginError::invalid_state("Plugin not initialized"));
        }

        // Aggregate commit statistics from multiple scan messages
        let mut aggregated = CommitsPlugin::new();
        aggregated.initialized = true;

        for message in &results {
            if matches!(message.data, MessageData::CommitInfo { .. }) {
                aggregated.process_commit(message)?;
            }
        }

        aggregated.generate_summary()
    }

    fn estimate_processing_time(&self, modes: ScanMode, item_count: usize) -> Option<std::time::Duration> {
        if !modes.intersects(self.supported_modes()) {
            return None;
        }

        // Estimate ~1ms per commit for processing
        let processing_time_ms = if modes.contains(ScanMode::HISTORY) {
            item_count
        } else {
            item_count / 2 // Less time for author-only analysis
        };

        Some(std::time::Duration::from_millis(processing_time_ms as u64))
    }

    fn config_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "max_authors": {
                    "type": "integer",
                    "description": "Maximum number of authors to track",
                    "default": 1000,
                    "minimum": 1
                },
                "include_merge_commits": {
                    "type": "boolean",
                    "description": "Whether to include merge commits in statistics",
                    "default": true
                },
                "author_email_domains": {
                    "type": "array",
                    "description": "Filter authors by email domains",
                    "items": {"type": "string"},
                    "default": []
                }
            }
        })
    }
}

impl CommitsPlugin {
    /// Clone for processing (workaround for mutable operations in immutable context)
    fn clone_for_processing(&self) -> Self {
        Self {
            info: self.info.clone(),
            initialized: self.initialized,
            commit_count: self.commit_count,
            author_stats: self.author_stats.clone(),
        }
    }

    /// Generate analysis for a single commit
    fn generate_commit_analysis(&self, commit: &ScanMessage) -> PluginResult<ScanMessage> {
        if let MessageData::CommitInfo {  author, message,  .. } = &commit.data {
            // Get author commit count
            let count = self.author_stats.get(author).unwrap_or(&0);
            
            // Analyze commit message
            let is_merge_commit = message.contains("Merge");
            let _issue_refs_count = message
                .split_whitespace()
                .filter(|word| word.starts_with('#') || word.contains("GS-"))
                .count();

            // Create a metric info that represents the commit analysis
            let data = MessageData::MetricInfo {
                file_count: *count as u32, // Author's commit count
                line_count: message.len() as u64, // Message length
                complexity: if is_merge_commit { 1.0 } else { 0.0 }, // Is merge commit indicator
            };

            let header = MessageHeader::new(
                ScanMode::HISTORY,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );

            Ok(ScanMessage::new(header, data))
        } else {
            Err(PluginError::execution_failed("Expected CommitInfo data"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::context::PluginContext;
    use crate::scanner::messages::MessageHeader;

    fn create_test_commit_message(author: &str, hash: &str, message: &str) -> ScanMessage {
        let data = MessageData::CommitInfo {
            author: author.to_string(),
            hash: hash.to_string(),
            message: message.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            changed_files: vec![crate::scanner::messages::FileChangeData {
                path: "src/main.rs".to_string(),
                lines_added: 10,
                lines_removed: 2,
            }], // Add test file
        };

        let header = MessageHeader::new(
            ScanMode::HISTORY,
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
    async fn test_commits_plugin_creation() {
        let plugin = CommitsPlugin::new();
        assert_eq!(plugin.plugin_info().name, "commits");
        assert_eq!(plugin.plugin_info().plugin_type, PluginType::Scanner);
        assert!(!plugin.initialized);
    }

    #[tokio::test]
    async fn test_commits_plugin_initialization() {
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();

        assert!(plugin.initialize(&context).await.is_ok());
        assert!(plugin.initialized);

        // Test double initialization fails
        assert!(plugin.initialize(&context).await.is_err());
    }

    #[tokio::test]
    async fn test_commits_plugin_supported_modes() {
        let plugin = CommitsPlugin::new();
        let modes = plugin.supported_modes();
        
        assert!(modes.contains(ScanMode::HISTORY));
        assert!(!modes.contains(ScanMode::FILES));
    }

    #[tokio::test]
    async fn test_commits_plugin_process_scan_data() {
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        let commit = create_test_commit_message(
            "alice@example.com",
            "abc123",
            "Fix bug in scanner module"
        );

        let results = plugin.process_scan_data(&commit).await.unwrap();
        assert_eq!(results.len(), 2); // Original + analysis
        assert_eq!(results[0], commit);
        assert!(matches!(results[1].data, MessageData::MetricInfo { .. }));
    }

    #[tokio::test]
    async fn test_commits_plugin_aggregate_results() {
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        let commits = vec![
            create_test_commit_message("alice@example.com", "abc123", "Fix bug"),
            create_test_commit_message("bob@example.com", "def456", "Add feature"),
            create_test_commit_message("alice@example.com", "ghi789", "Update docs"),
        ];

        let summary = plugin.aggregate_results(commits).await.unwrap();
        assert!(matches!(summary.data, MessageData::MetricInfo { .. }));
        
        if let MessageData::MetricInfo { file_count, line_count, .. } = summary.data {
            assert_eq!(line_count, 3); // Total commits
            assert_eq!(file_count, 2); // Total authors
        } else {
            panic!("Expected MetricInfo data");
        }
    }

    #[tokio::test]
    async fn test_commits_plugin_execute() {
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        // Test get capabilities
        let response = plugin.execute(PluginRequest::GetCapabilities).await.unwrap();
        match response {
            PluginResponse::Capabilities(caps) => {
                assert_eq!(caps.len(), 2);
                assert!(caps.iter().any(|c| c.name == "commit_analysis"));
            }
            _ => panic!("Unexpected response type"),
        }

        // Test get statistics
        let response = plugin.execute(PluginRequest::GetStatistics).await.unwrap();
        match response {
            PluginResponse::Statistics(stats) => {
                assert!(matches!(stats.data, MessageData::MetricInfo { .. }));
            }
            _ => panic!("Unexpected response type"),
        }
    }

    #[tokio::test]
    async fn test_commits_plugin_cleanup() {
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        assert!(plugin.cleanup().await.is_ok());
        assert!(!plugin.initialized);
        assert_eq!(plugin.commit_count, 0);
        assert!(plugin.author_stats.is_empty());
    }

    #[tokio::test]
    async fn test_commits_plugin_processing_time_estimation() {
        let plugin = CommitsPlugin::new();
        
        // Test with supported modes
        let time = plugin.estimate_processing_time(ScanMode::HISTORY, 100);
        assert!(time.is_some());
        assert_eq!(time.unwrap().as_millis(), 100);

        // Test with unsupported modes
        let time = plugin.estimate_processing_time(ScanMode::FILES, 100);
        assert!(time.is_none());
    }

    #[tokio::test]
    async fn test_commits_plugin_config_schema() {
        let plugin = CommitsPlugin::new();
        let schema = plugin.config_schema();
        
        assert!(schema.is_object());
        assert!(schema.get("properties").is_some());
        assert!(schema["properties"].get("max_authors").is_some());
        assert!(schema["properties"].get("include_merge_commits").is_some());
    }
}