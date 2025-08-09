use crate::scanner::async_engine::events::{RepositoryEvent, CommitInfo};
use crate::scanner::async_engine::processors::{EventProcessor, ProcessorStats};
use crate::scanner::messages::{ScanMessage, MessageData, MessageHeader};
use crate::scanner::modes::ScanMode;
use crate::scanner::query::QueryParams;
use crate::plugin::PluginResult;
use async_trait::async_trait;
use std::time::{Instant, SystemTime};
use log::{debug, info};

/// Event processor for handling git history events
pub struct HistoryEventProcessor {
    query_params: QueryParams,
    commit_count: usize,
    filtered_commits: Vec<CommitInfo>,
    processing_start_time: Option<Instant>,
    stats: ProcessorStats,
}

impl HistoryEventProcessor {
    /// Create a new history event processor
    pub fn new() -> Self {
        Self {
            query_params: QueryParams::default(),
            commit_count: 0,
            filtered_commits: Vec::new(),
            processing_start_time: None,
            stats: ProcessorStats::default(),
        }
    }

    /// Create a new history event processor with query parameters
    pub fn with_query_params(query_params: QueryParams) -> Self {
        Self {
            query_params,
            commit_count: 0,
            filtered_commits: Vec::new(),
            processing_start_time: None,
            stats: ProcessorStats::default(),
        }
    }

    /// Convert CommitInfo to ScanMessage
    fn create_commit_message(&self, commit: &CommitInfo, index: usize) -> ScanMessage {
        let header = MessageHeader::new(
            ScanMode::HISTORY,
            commit.timestamp
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );

        let commit_data = MessageData::CommitInfo {
            hash: commit.hash.clone(),
            author: commit.author_name.clone(),
            message: commit.message.clone(),
            timestamp: commit.timestamp
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            changed_files: vec![], // We'll populate this when we have the data structure
        };

        ScanMessage::new(header, commit_data)
    }

    /// Check if commit should be included based on query parameters
    fn should_include_commit(&self, commit: &CommitInfo) -> bool {
        // Apply date range filter
        if let Some(date_range) = &self.query_params.date_range {
            if let Some(start) = date_range.start {
                if commit.timestamp < start {
                    return false;
                }
            }
            if let Some(end) = date_range.end {
                if commit.timestamp > end {
                    return false;
                }
            }
        }

        // Apply author filter
        if !self.query_params.authors.include.is_empty() {
            let author_match = self.query_params.authors.include.iter().any(|author| {
                commit.author_name.contains(author) || commit.author_email.contains(author)
            });
            if !author_match {
                return false;
            }
        }

        // Apply author exclusion filter
        if !self.query_params.authors.exclude.is_empty() {
            let author_excluded = self.query_params.authors.exclude.iter().any(|author| {
                commit.author_name.contains(author) || commit.author_email.contains(author)
            });
            if author_excluded {
                return false;
            }
        }

        true
    }
}

#[async_trait]
impl EventProcessor for HistoryEventProcessor {
    fn supported_modes(&self) -> ScanMode {
        ScanMode::HISTORY
    }

    fn name(&self) -> &'static str {
        "history"
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        self.processing_start_time = Some(Instant::now());
        debug!("Initialized HistoryEventProcessor");
        Ok(())
    }

    async fn process_event(&mut self, event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
        let start_time = Instant::now();
        let mut messages = Vec::new();

        match event {
            RepositoryEvent::CommitDiscovered { commit, index } => {
                if self.should_include_commit(commit) {
                    let message = self.create_commit_message(commit, *index);
                    messages.push(message);
                    self.filtered_commits.push(commit.clone());
                    self.commit_count += 1;
                    
                    debug!("Processed commit {} ({})", commit.short_hash, commit.author_name);
                }
            }
            RepositoryEvent::RepositoryStarted { total_commits, .. } => {
                if let Some(total) = total_commits {
                    info!("Starting history processing for {} commits", total);
                }
            }
            RepositoryEvent::RepositoryCompleted { stats } => {
                info!(
                    "History processing completed: {} commits processed from {} total",
                    self.commit_count, stats.total_commits
                );
            }
            _ => {
                // Ignore other event types
            }
        }

        // Update statistics
        self.stats.events_processed += 1;
        self.stats.messages_generated += messages.len();
        self.stats.processing_time += start_time.elapsed();

        Ok(messages)
    }

    async fn finalize(&mut self) -> PluginResult<Vec<ScanMessage>> {
        if let Some(start_time) = self.processing_start_time {
            let total_duration = start_time.elapsed();
            info!(
                "HistoryEventProcessor finalized: {} commits processed in {:?}",
                self.commit_count, total_duration
            );
        }

        // No additional messages to generate during finalization
        Ok(vec![])
    }

    fn get_stats(&self) -> ProcessorStats {
        self.stats.clone()
    }
}

impl Default for HistoryEventProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::async_engine::events::RepositoryStats;
    use std::time::Duration;

    fn create_test_commit() -> CommitInfo {
        CommitInfo {
            hash: "abc123def456".to_string(),
            short_hash: "abc123d".to_string(),
            author_name: "Test Author".to_string(),
            author_email: "test@example.com".to_string(),
            committer_name: "Test Author".to_string(),
            committer_email: "test@example.com".to_string(),
            timestamp: SystemTime::UNIX_EPOCH + Duration::from_secs(1000),
            message: "Test commit message".to_string(),
            parent_hashes: vec!["parent123".to_string()],
            changed_files: vec!["test.rs".to_string(), "lib.rs".to_string()],
            insertions: 15,
            deletions: 3,
        }
    }

    #[tokio::test]
    async fn test_history_processor_creation() {
        let processor = HistoryEventProcessor::new();
        assert_eq!(processor.name(), "history");
        assert_eq!(processor.supported_modes(), ScanMode::HISTORY);
        assert_eq!(processor.commit_count, 0);
    }

    #[tokio::test]
    async fn test_history_processor_with_query_params() {
        let query_params = QueryParams::default();
        let processor = HistoryEventProcessor::with_query_params(query_params);
        assert_eq!(processor.name(), "history");
        assert_eq!(processor.supported_modes(), ScanMode::HISTORY);
    }

    #[tokio::test]
    async fn test_commit_processing() {
        let mut processor = HistoryEventProcessor::new();
        processor.initialize().await.unwrap();

        let commit = create_test_commit();
        let event = RepositoryEvent::CommitDiscovered {
            commit: commit.clone(),
            index: 0,
        };

        let messages = processor.process_event(&event).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(processor.commit_count, 1);

        // Verify message content
        let message = &messages[0];
        match &message.data {
            MessageData::CommitInfo { hash, author, .. } => {
                assert_eq!(hash, &commit.hash);
                assert_eq!(author, &commit.author_name);
            }
            _ => panic!("Expected commit message data"),
        }
    }

    #[tokio::test]
    async fn test_commit_filtering() {
        let mut query_params = QueryParams::default();
        query_params.authors.include = vec!["john".to_string()];
        
        let mut processor = HistoryEventProcessor::with_query_params(query_params);
        processor.initialize().await.unwrap();

        // Create commits with different authors
        let matching_commit = CommitInfo {
            author_name: "john.doe".to_string(),
            ..create_test_commit()
        };

        let non_matching_commit = CommitInfo {
            author_name: "jane.smith".to_string(),
            ..create_test_commit()
        };

        // Process matching commit
        let event1 = RepositoryEvent::CommitDiscovered {
            commit: matching_commit,
            index: 0,
        };
        let messages1 = processor.process_event(&event1).await.unwrap();
        assert_eq!(messages1.len(), 1);

        // Process non-matching commit
        let event2 = RepositoryEvent::CommitDiscovered {
            commit: non_matching_commit,
            index: 1,
        };
        let messages2 = processor.process_event(&event2).await.unwrap();
        assert_eq!(messages2.len(), 0);

        assert_eq!(processor.commit_count, 1); // Only one commit should be counted
    }

    #[tokio::test]
    async fn test_repository_lifecycle_events() {
        let mut processor = HistoryEventProcessor::new();
        processor.initialize().await.unwrap();

        // Test repository started event
        let start_event = RepositoryEvent::RepositoryStarted {
            total_commits: Some(100),
            total_files: Some(50),
            scan_modes: ScanMode::HISTORY,
        };
        let messages = processor.process_event(&start_event).await.unwrap();
        assert_eq!(messages.len(), 0); // No messages generated for lifecycle events

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
        assert_eq!(messages.len(), 0); // No messages generated for lifecycle events
    }

    #[tokio::test]
    async fn test_processor_statistics() {
        let mut processor = HistoryEventProcessor::new();
        processor.initialize().await.unwrap();

        let commit = create_test_commit();
        let event = RepositoryEvent::CommitDiscovered {
            commit,
            index: 0,
        };

        processor.process_event(&event).await.unwrap();

        let stats = processor.get_stats();
        assert_eq!(stats.events_processed, 1);
        assert_eq!(stats.messages_generated, 1);
        assert!(stats.processing_time > Duration::from_nanos(0));
    }

    #[tokio::test]
    async fn test_finalization() {
        let mut processor = HistoryEventProcessor::new();
        processor.initialize().await.unwrap();

        // Process some commits
        let commit = create_test_commit();
        let event = RepositoryEvent::CommitDiscovered {
            commit,
            index: 0,
        };
        processor.process_event(&event).await.unwrap();

        // Finalize
        let final_messages = processor.finalize().await.unwrap();
        assert_eq!(final_messages.len(), 0); // No additional messages during finalization
    }
}
