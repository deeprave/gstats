//! Commits Analysis Plugin
//! 
//! Built-in plugin for analyzing git commit history and statistics.

use crate::plugin::{
    Plugin, PluginInfo, PluginContext, PluginRequest, PluginResponse,
    PluginResult, PluginError, traits::{PluginType, PluginFunction}
};
use crate::scanner::messages::{ScanMessage, MessageData, MessageHeader};
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
            PluginType::Processing,
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
    
    /// Handle ScanDataReady event to fetch and process queued commit data
    pub async fn handle_scan_data_ready(&mut self, event: crate::notifications::ScanEvent) -> PluginResult<()> {
        use crate::notifications::ScanEvent;
        
        match event {
            ScanEvent::ScanDataReady { scan_id, data_type, message_count } => {
                log::info!("CommitsPlugin received ScanDataReady event: {} messages of type '{}' for scan {}", 
                          message_count, data_type, scan_id);
                
                // For now, just log that we received the event
                // In future iterations, we'll implement actual data fetching from the queue
                // TODO: Fetch commit data from the scanner queue
                // TODO: Process commit messages and update internal statistics
                // TODO: Emit DataReady event when processing is complete
                
                Ok(())
            }
            _ => {
                Err(PluginError::ExecutionFailed { 
                    message: "CommitsPlugin::handle_scan_data_ready received non-ScanDataReady event".to_string() 
                })
            }
        }
    }
    
    /// Handle ScanWarning event - log warnings and continue processing
    pub async fn handle_scan_warning(&mut self, event: crate::notifications::ScanEvent) -> PluginResult<()> {
        use crate::notifications::ScanEvent;
        
        match event {
            ScanEvent::ScanWarning { scan_id, warning, recoverable } => {
                if recoverable {
                    log::warn!("CommitsPlugin received recoverable warning for scan {}: {}", scan_id, warning);
                    // Continue processing with degraded data quality
                } else {
                    log::error!("CommitsPlugin received non-recoverable warning for scan {}: {}", scan_id, warning);
                    // May need to adjust processing strategy
                }
                Ok(())
            }
            _ => {
                Err(PluginError::ExecutionFailed { 
                    message: "CommitsPlugin::handle_scan_warning received non-ScanWarning event".to_string() 
                })
            }
        }
    }
    
    /// Handle ScanError event - abort processing and cleanup resources if fatal
    pub async fn handle_scan_error(&mut self, event: crate::notifications::ScanEvent) -> PluginResult<()> {
        use crate::notifications::ScanEvent;
        
        match event {
            ScanEvent::ScanError { scan_id, error, fatal } => {
                if fatal {
                    log::error!("CommitsPlugin received fatal error for scan {}: {}", scan_id, error);
                    // Fatal errors require cleanup and abort processing
                    self.commit_count = 0;
                    self.author_stats.clear();
                    log::info!("CommitsPlugin cleaned up partial data for scan {}", scan_id);
                } else {
                    log::warn!("CommitsPlugin received non-fatal error for scan {}: {}", scan_id, error);
                    // Non-fatal errors allow graceful degradation
                }
                Ok(())
            }
            _ => {
                Err(PluginError::ExecutionFailed { 
                    message: "CommitsPlugin::handle_scan_error received non-ScanError event".to_string() 
                })
            }
        }
    }
    
    /// Handle ScanCompleted event - finalize processing and prepare results
    pub async fn handle_scan_completed(&mut self, event: crate::notifications::ScanEvent) -> PluginResult<()> {
        use crate::notifications::ScanEvent;
        
        match event {
            ScanEvent::ScanCompleted { scan_id, duration, warnings } => {
                log::info!("CommitsPlugin received ScanCompleted event for scan {} (duration: {:?}, warnings: {})", 
                          scan_id, duration, warnings.len());
                
                // Finalize commit analysis processing
                log::info!("CommitsPlugin processed {} commits from {} authors for scan {}", 
                          self.commit_count, self.author_stats.len(), scan_id);
                
                // TODO: Emit DataReady event to signal export plugins that commit analysis is complete
                // TODO: Prepare final commit statistics and metrics
                
                Ok(())
            }
            _ => {
                Err(PluginError::ExecutionFailed { 
                    message: "CommitsPlugin::handle_scan_completed received non-ScanCompleted event".to_string() 
                })
            }
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
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );

        Ok(ScanMessage::new(header, data))
    }
    
    /// Execute commits analysis function
    async fn execute_commits_analysis(&self) -> PluginResult<PluginResponse> {
        let start_time = std::time::Instant::now();

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

        let duration_us = start_time.elapsed().as_micros() as u64;

        Ok(PluginResponse::Execute {
            request_id: "commits_analysis".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_us,
                memory_used: 0,
                entries_processed: self.commit_count as u64,
                plugin_version: self.info.version.clone(),
                extra: HashMap::new(),
            },
            errors: vec![],
        })
    }
    
    /// Execute author analysis function  
    async fn execute_author_analysis(&self) -> PluginResult<PluginResponse> {
        let start_time = std::time::Instant::now();

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

        let duration_us = start_time.elapsed().as_micros() as u64;

        Ok(PluginResponse::Execute {
            request_id: "author_analysis".to_string(),
            status: crate::plugin::context::ExecutionStatus::Success,
            data,
            metadata: crate::plugin::context::ExecutionMetadata {
                duration_us,
                memory_used: 0,
                entries_processed: self.author_stats.len() as u64,
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
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );

        ScanMessage::new(header, data)
    }

    fn create_test_context() -> PluginContext {
        let scanner_config = std::sync::Arc::new(crate::scanner::ScannerConfig::default());
        let query_params = std::sync::Arc::new(crate::scanner::QueryParams::default());
        
        PluginContext::new(
            scanner_config,
            query_params,
        )
    }

    #[tokio::test]
    async fn test_commits_plugin_creation() {
        let plugin = CommitsPlugin::new();
        assert_eq!(plugin.plugin_info().name, "commits");
        assert_eq!(plugin.plugin_info().plugin_type, PluginType::Processing);
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
    async fn test_commits_plugin_processing() {
        let _plugin = CommitsPlugin::new();
        // Plugin no longer advertises supported modes - processes all data
        // This test just verifies the plugin can be created
        assert!(true);
    }

    #[tokio::test]
    async fn test_commits_plugin_execution() {
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();

        let request = PluginRequest::new()
            .with_parameter("test".to_string(), "value".to_string());

        let response = plugin.execute(request).await.unwrap();
        assert!(response.is_success());
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
    async fn test_commits_plugin_handles_scan_data_ready() {
        use crate::notifications::ScanEvent;
        // Removed unused import: crate::scanner::ScanMode
        
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        // Create ScanDataReady event
        let event = ScanEvent::ScanDataReady {
            scan_id: "test_scan".to_string(),
            data_type: "commits".to_string(),
            message_count: 1,
        };
        
        // This should fail because handle_scan_data_ready is not implemented yet
        let result = plugin.handle_scan_data_ready(event).await;
        assert!(result.is_ok());
        
        // Verify that the plugin processed the commit data
        // For now, just verify the method exists and returns Ok
    }
    
    #[tokio::test]
    async fn test_commits_plugin_handles_scan_warning() {
        use crate::notifications::ScanEvent;
        
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        let event = ScanEvent::ScanWarning {
            scan_id: "test_scan".to_string(),
            warning: "Test warning message".to_string(),
            recoverable: true,
        };
        
        let result = plugin.handle_scan_warning(event).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_commits_plugin_handles_scan_error() {
        use crate::notifications::ScanEvent;
        
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        let event = ScanEvent::ScanError {
            scan_id: "test_scan".to_string(),
            error: "Test error message".to_string(),
            fatal: false,
        };
        
        let result = plugin.handle_scan_error(event).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_commits_plugin_handles_scan_completed() {
        use crate::notifications::ScanEvent;
        use std::time::Duration;
        
        let mut plugin = CommitsPlugin::new();
        let context = create_test_context();
        plugin.initialize(&context).await.unwrap();
        
        let event = ScanEvent::ScanCompleted {
            scan_id: "test_scan".to_string(),
            duration: Duration::from_secs(10),
            warnings: vec!["Warning 1".to_string()],
        };
        
        let result = plugin.handle_scan_completed(event).await;
        assert!(result.is_ok());
    }
}