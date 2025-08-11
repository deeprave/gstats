//! Change Frequency Processor
//! 
//! Event-driven processor that analyzes file change frequency patterns
//! by processing repository events. This processor can be used by any plugin
//! that needs change frequency analysis.

use crate::scanner::async_engine::events::{RepositoryEvent, CommitInfo, FileChangeData};
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

/// Time window for change frequency analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeWindow {
    Week,
    Month,
    Quarter,
    Year,
    All,
}

impl TimeWindow {
    pub fn cutoff_timestamp(&self) -> Option<i64> {
        match self {
            TimeWindow::All => None,
            _ => {
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .ok()?
                    .as_secs() as i64;
                
                let days_back = match self {
                    TimeWindow::Week => 7,
                    TimeWindow::Month => 30,
                    TimeWindow::Quarter => 90,
                    TimeWindow::Year => 365,
                    TimeWindow::All => return None,
                };
                
                Some(now - (days_back * 24 * 60 * 60))
            }
        }
    }
}

/// File change statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeStats {
    pub file_path: String,
    pub change_count: u32,
    pub last_changed: i64,
    pub first_changed: i64,
    pub authors: Vec<String>,
    pub author_count: usize,
}

impl FileChangeStats {
    pub fn new(file_path: String) -> Self {
        Self {
            file_path,
            change_count: 0,
            last_changed: 0,
            first_changed: 0,
            authors: vec![],
            author_count: 0,
        }
    }
    
    pub fn add_change(&mut self, timestamp: i64, author: String, _commit_hash: String) {
        self.change_count += 1;
        
        if self.first_changed == 0 || timestamp < self.first_changed {
            self.first_changed = timestamp;
        }
        if timestamp > self.last_changed {
            self.last_changed = timestamp;
        }
        
        if !self.authors.contains(&author) {
            self.authors.push(author);
            self.author_count = self.authors.len();
        }
    }
    
    pub fn frequency_score(&self, _time_window: TimeWindow) -> f64 {
        if self.change_count == 0 {
            return 0.0;
        }
        
        let base_score = self.change_count as f64;
        let recency = self.recency_weight();
        base_score * (1.0 + recency)
    }
    
    pub fn recency_weight(&self) -> f64 {
        if self.last_changed == 0 {
            return 0.0;
        }
        
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        
        let days_since_change = (now - self.last_changed) / (24 * 60 * 60);
        
        if days_since_change <= 7 {
            1.0
        } else if days_since_change <= 30 {
            0.7
        } else if days_since_change <= 90 {
            0.4
        } else {
            0.1
        }
    }
}

/// Change Frequency Processor - can be used by any plugin
pub struct ChangeFrequencyProcessor {
    change_stats: HashMap<String, FileChangeStats>,
    time_window: TimeWindow,
    total_changes: usize,
    stats: ProcessorStats,
    shared_state: Option<Arc<SharedProcessorState>>,
}

impl ChangeFrequencyProcessor {
    pub fn new() -> Self {
        Self {
            change_stats: HashMap::new(),
            time_window: TimeWindow::Month,
            total_changes: 0,
            stats: ProcessorStats::default(),
            shared_state: None,
        }
    }

    pub fn with_time_window(time_window: TimeWindow) -> Self {
        Self {
            change_stats: HashMap::new(),
            time_window,
            total_changes: 0,
            stats: ProcessorStats::default(),
            shared_state: None,
        }
    }

    fn process_file_change(&mut self, file_path: &str, _change_data: &FileChangeData, commit: &CommitInfo) {
        let stats = self.change_stats
            .entry(file_path.to_string())
            .or_insert_with(|| FileChangeStats::new(file_path.to_string()));

        let timestamp = commit.timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        stats.add_change(timestamp, commit.author_name.clone(), commit.hash.clone());
        self.total_changes += 1;
    }

    fn create_change_frequency_messages(&self) -> Vec<ScanMessage> {
        let mut messages = Vec::new();

        for (file_path, stats) in &self.change_stats {
            let frequency_score = stats.frequency_score(self.time_window);
            
            if frequency_score > 0.0 {
                let header = MessageHeader::new(
                    ScanMode::CHANGE_FREQUENCY,
                    SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                );

                let change_data = MessageData::ChangeFrequencyInfo {
                    file_path: file_path.clone(),
                    change_count: stats.change_count as u32,
                    author_count: stats.author_count as u32,
                    last_changed: stats.last_changed,
                    first_changed: stats.first_changed,
                    frequency_score,
                    recency_weight: stats.recency_weight(),
                    authors: stats.authors.clone(),
                };

                messages.push(ScanMessage::new(header, change_data));
            }
        }

        messages
    }

    /// Get the collected change statistics (for use by other processors)
    pub fn get_change_stats(&self) -> &HashMap<String, FileChangeStats> {
        &self.change_stats
    }
}

#[async_trait]
impl EventProcessor for ChangeFrequencyProcessor {
    fn supported_modes(&self) -> ScanMode {
        ScanMode::CHANGE_FREQUENCY
    }

    fn name(&self) -> &'static str {
        "change_frequency"
    }

    fn set_shared_state(&mut self, shared_state: Arc<SharedProcessorState>) {
        self.shared_state = Some(shared_state);
    }

    fn shared_state(&self) -> Option<&Arc<SharedProcessorState>> {
        self.shared_state.as_ref()
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        debug!("Initialized ChangeFrequencyProcessor with time window: {:?}", self.time_window);
        Ok(())
    }

    async fn on_repository_metadata(&mut self, metadata: &RepositoryMetadata) -> PluginResult<()> {
        debug!(
            "ChangeFrequencyProcessor received repository metadata: {} commits expected",
            metadata.total_commits.unwrap_or(0)
        );
        Ok(())
    }

    async fn process_event(&mut self, event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
        match event {
            RepositoryEvent::FileChanged { file_path, change_data, commit_context } => {
                self.process_file_change(file_path, change_data, commit_context);
            }
            _ => {}
        }
        self.stats.events_processed += 1;
        Ok(vec![])
    }

    async fn finalize(&mut self) -> PluginResult<Vec<ScanMessage>> {
        let messages = self.create_change_frequency_messages();
        self.stats.messages_generated = messages.len();
        Ok(messages)
    }

    fn get_stats(&self) -> ProcessorStats {
        self.stats.clone()
    }
}

impl Default for ChangeFrequencyProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::async_engine::events::ChangeType;

    fn create_test_commit() -> CommitInfo {
        CommitInfo {
            hash: "abc123def456".to_string(),
            short_hash: "abc123d".to_string(),
            author_name: "Test Author".to_string(),
            author_email: "test@example.com".to_string(),
            committer_name: "Test Author".to_string(),
            committer_email: "test@example.com".to_string(),
            timestamp: SystemTime::now(),
            message: "Test commit message".to_string(),
            parent_hashes: vec!["parent123".to_string()],
            changed_files: vec!["test.rs".to_string()],
            insertions: 10,
            deletions: 2,
        }
    }

    fn create_test_file_change() -> FileChangeData {
        FileChangeData {
            change_type: ChangeType::Modified,
            old_path: Some("test.rs".to_string()),
            new_path: "test.rs".to_string(),
            insertions: 10,
            deletions: 2,
            is_binary: false,
        }
    }

    #[tokio::test]
    async fn test_change_frequency_processor_creation() {
        let processor = ChangeFrequencyProcessor::new();
        assert_eq!(processor.name(), "change_frequency");
        assert_eq!(processor.supported_modes(), ScanMode::CHANGE_FREQUENCY);
        assert_eq!(processor.total_changes, 0);
        assert!(processor.change_stats.is_empty());
    }

    #[tokio::test]
    async fn test_change_frequency_processor_with_time_window() {
        let processor = ChangeFrequencyProcessor::with_time_window(TimeWindow::Week);
        assert_eq!(processor.time_window, TimeWindow::Week);
    }

    #[tokio::test]
    async fn test_file_change_processing() {
        let mut processor = ChangeFrequencyProcessor::new();
        processor.initialize().await.unwrap();

        let commit = create_test_commit();
        let change_data = create_test_file_change();
        let event = RepositoryEvent::FileChanged {
            file_path: "test.rs".to_string(),
            change_data,
            commit_context: commit,
        };

        let messages = processor.process_event(&event).await.unwrap();
        assert_eq!(messages.len(), 0); // No messages during processing

        assert_eq!(processor.total_changes, 1);
        assert_eq!(processor.change_stats.len(), 1);
        assert!(processor.change_stats.contains_key("test.rs"));
    }

    #[tokio::test]
    async fn test_finalization_generates_messages() {
        let mut processor = ChangeFrequencyProcessor::new();
        processor.initialize().await.unwrap();

        let commit = create_test_commit();
        let change_data = create_test_file_change();
        let event = RepositoryEvent::FileChanged {
            file_path: "test.rs".to_string(),
            change_data,
            commit_context: commit,
        };

        processor.process_event(&event).await.unwrap();

        let messages = processor.finalize().await.unwrap();
        assert!(!messages.is_empty());
        
        let message = &messages[0];
        match &message.data {
            MessageData::ChangeFrequencyInfo { file_path, change_count, .. } => {
                assert_eq!(file_path, "test.rs");
                assert_eq!(*change_count, 1);
            }
            _ => panic!("Expected change frequency message data"),
        }
    }
}
