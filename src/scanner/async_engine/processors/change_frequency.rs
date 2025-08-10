use crate::scanner::async_engine::events::{RepositoryEvent, CommitInfo, FileChangeData};
use crate::scanner::async_engine::processors::{EventProcessor, ProcessorStats};
use crate::scanner::async_engine::shared_state::{SharedProcessorState, RepositoryMetadata, SharedStateAccess};
use crate::scanner::messages::{ScanMessage, MessageData, MessageHeader};
use crate::scanner::modes::ScanMode;
use crate::plugin::builtin::utils::change_frequency::{FileChangeStats, TimeWindow};
use crate::plugin::PluginResult;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use log::{debug, info};

/// Event processor for handling file change frequency analysis
pub struct ChangeFrequencyEventProcessor {
    change_stats: HashMap<String, FileChangeStats>,
    time_window: TimeWindow,
    total_changes: usize,
    processing_start_time: Option<Instant>,
    stats: ProcessorStats,
    shared_state: Option<Arc<SharedProcessorState>>,
}

impl ChangeFrequencyEventProcessor {
    /// Create a new change frequency event processor
    pub fn new() -> Self {
        Self {
            change_stats: HashMap::new(),
            time_window: TimeWindow::Month, // Default time window
            total_changes: 0,
            processing_start_time: None,
            stats: ProcessorStats::default(),
            shared_state: None,
        }
    }

    /// Create a new change frequency event processor with custom time window
    pub fn with_time_window(time_window: TimeWindow) -> Self {
        Self {
            change_stats: HashMap::new(),
            time_window,
            total_changes: 0,
            processing_start_time: None,
            stats: ProcessorStats::default(),
            shared_state: None,
        }
    }

    /// Process a file change and update statistics
    fn process_file_change(&mut self, file_path: &str, _change_data: &FileChangeData, commit: &CommitInfo) {
        let stats = self.change_stats
            .entry(file_path.to_string())
            .or_insert_with(|| FileChangeStats::new(file_path.to_string()));

        // Add the change to the file's statistics
        let timestamp = commit.timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        stats.add_change(timestamp, commit.author_name.clone(), commit.hash.clone());
        self.total_changes += 1;

        debug!(
            "Recorded change for file '{}' by {} at timestamp {}",
            file_path, commit.author_name, timestamp
        );
    }

    /// Create change frequency messages for files with significant activity
    fn create_change_frequency_messages(&self) -> Vec<ScanMessage> {
        let mut messages = Vec::new();

        for (file_path, stats) in &self.change_stats {
            let frequency_score = stats.frequency_score(self.time_window);
            
            // Only create messages for files with meaningful change frequency
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

    /// Check if a file change should be processed based on time window
    fn should_process_change(&self, commit: &CommitInfo) -> bool {
        if let Some(cutoff) = self.time_window.cutoff_timestamp() {
            let commit_timestamp = commit.timestamp
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            
            commit_timestamp >= cutoff
        } else {
            true // No time limit
        }
    }
}

#[async_trait]
impl EventProcessor for ChangeFrequencyEventProcessor {
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
        self.processing_start_time = Some(Instant::now());
        debug!("Initialized ChangeFrequencyEventProcessor with time window: {:?}", self.time_window);
        Ok(())
    }

    async fn on_repository_metadata(&mut self, metadata: &RepositoryMetadata) -> PluginResult<()> {
        debug!(
            "ChangeFrequencyEventProcessor received repository metadata: {} commits expected",
            metadata.total_commits.unwrap_or(0)
        );
        Ok(())
    }

    async fn process_event(&mut self, event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
        let start_time = Instant::now();

        match event {
            RepositoryEvent::FileChanged { file_path, change_data, commit_context } => {
                if self.should_process_change(commit_context) {
                    self.process_file_change(file_path, change_data, commit_context);
                }
            }
            RepositoryEvent::RepositoryStarted { .. } => {
                info!("Starting change frequency analysis with time window: {:?}", self.time_window);
            }
            RepositoryEvent::RepositoryCompleted { stats: _ } => {
                info!(
                    "Change frequency analysis completed: {} changes processed across {} files",
                    self.total_changes, self.change_stats.len()
                );
            }
            _ => {
                // Ignore other event types
            }
        }

        // Update statistics
        self.stats.events_processed += 1;
        self.stats.processing_time += start_time.elapsed();

        // Don't generate messages during event processing - wait for finalization
        Ok(vec![])
    }

    async fn finalize(&mut self) -> PluginResult<Vec<ScanMessage>> {
        let messages = self.create_change_frequency_messages();
        
        self.stats.messages_generated = messages.len();

        if let Some(start_time) = self.processing_start_time {
            let total_duration = start_time.elapsed();
            info!(
                "ChangeFrequencyEventProcessor finalized: {} files analyzed, {} messages generated in {:?}",
                self.change_stats.len(), messages.len(), total_duration
            );
        }

        Ok(messages)
    }

    fn get_stats(&self) -> ProcessorStats {
        self.stats.clone()
    }
}

impl SharedStateAccess for ChangeFrequencyEventProcessor {
    fn shared_state(&self) -> &SharedProcessorState {
        self.shared_state.as_ref()
            .expect("SharedProcessorState not initialized")
    }
}

impl Default for ChangeFrequencyEventProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::async_engine::events::{ChangeType, RepositoryStats};
    use std::time::Duration;

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
        let processor = ChangeFrequencyEventProcessor::new();
        assert_eq!(processor.name(), "change_frequency");
        assert_eq!(processor.supported_modes(), ScanMode::CHANGE_FREQUENCY);
        assert_eq!(processor.total_changes, 0);
        assert!(processor.change_stats.is_empty());
    }

    #[tokio::test]
    async fn test_change_frequency_processor_with_time_window() {
        let processor = ChangeFrequencyEventProcessor::with_time_window(TimeWindow::Week);
        assert_eq!(processor.time_window, TimeWindow::Week);
    }

    #[tokio::test]
    async fn test_file_change_processing() {
        let mut processor = ChangeFrequencyEventProcessor::new();
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
    async fn test_multiple_changes_same_file() {
        let mut processor = ChangeFrequencyEventProcessor::new();
        processor.initialize().await.unwrap();

        // Process multiple changes to the same file
        for i in 0..3 {
            let mut commit = create_test_commit();
            commit.hash = format!("commit{}", i);
            
            let change_data = create_test_file_change();
            let event = RepositoryEvent::FileChanged {
                file_path: "test.rs".to_string(),
                change_data,
                commit_context: commit,
            };

            processor.process_event(&event).await.unwrap();
        }

        assert_eq!(processor.total_changes, 3);
        assert_eq!(processor.change_stats.len(), 1);
        
        let stats = processor.change_stats.get("test.rs").unwrap();
        assert_eq!(stats.change_count, 3);
    }

    #[tokio::test]
    async fn test_multiple_files() {
        let mut processor = ChangeFrequencyEventProcessor::new();
        processor.initialize().await.unwrap();

        let files = ["test.rs", "lib.rs", "main.rs"];
        
        for file in &files {
            let commit = create_test_commit();
            let change_data = FileChangeData {
                new_path: file.to_string(),
                ..create_test_file_change()
            };
            
            let event = RepositoryEvent::FileChanged {
                file_path: file.to_string(),
                change_data,
                commit_context: commit,
            };

            processor.process_event(&event).await.unwrap();
        }

        assert_eq!(processor.total_changes, 3);
        assert_eq!(processor.change_stats.len(), 3);
        
        for file in &files {
            assert!(processor.change_stats.contains_key(*file));
        }
    }

    #[tokio::test]
    async fn test_finalization_generates_messages() {
        let mut processor = ChangeFrequencyEventProcessor::new();
        processor.initialize().await.unwrap();

        // Process some changes
        let commit = create_test_commit();
        let change_data = create_test_file_change();
        let event = RepositoryEvent::FileChanged {
            file_path: "test.rs".to_string(),
            change_data,
            commit_context: commit,
        };

        processor.process_event(&event).await.unwrap();

        // Finalize and check messages
        let messages = processor.finalize().await.unwrap();
        
        // Should generate at least one message for the changed file
        assert!(!messages.is_empty());
        
        // Verify message content
        let message = &messages[0];
        match &message.data {
            MessageData::ChangeFrequencyInfo { file_path, change_count, .. } => {
                assert_eq!(file_path, "test.rs");
                assert_eq!(*change_count, 1);
            }
            _ => panic!("Expected change frequency message data"),
        }
    }

    #[tokio::test]
    async fn test_repository_lifecycle_events() {
        let mut processor = ChangeFrequencyEventProcessor::new();
        processor.initialize().await.unwrap();

        // Test repository started event
        let start_event = RepositoryEvent::RepositoryStarted {
            total_commits: Some(100),
            total_files: Some(50),
            scan_modes: ScanMode::CHANGE_FREQUENCY,
        };
        let messages = processor.process_event(&start_event).await.unwrap();
        assert_eq!(messages.len(), 0);

        // Test repository completed event
        let stats = RepositoryStats {
            total_commits: 100,
            total_files: 50,
            total_changes: 200,
            scan_duration: Duration::from_secs(5),
            events_emitted: 150,
        };
        let complete_event = RepositoryEvent::RepositoryCompleted { stats };
        let messages = processor.process_event(&complete_event).await.unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[tokio::test]
    async fn test_processor_statistics() {
        let mut processor = ChangeFrequencyEventProcessor::new();
        processor.initialize().await.unwrap();

        let commit = create_test_commit();
        let change_data = create_test_file_change();
        let event = RepositoryEvent::FileChanged {
            file_path: "test.rs".to_string(),
            change_data,
            commit_context: commit,
        };

        processor.process_event(&event).await.unwrap();

        let stats = processor.get_stats();
        assert_eq!(stats.events_processed, 1);
        assert!(stats.processing_time > Duration::from_nanos(0));
    }
}
