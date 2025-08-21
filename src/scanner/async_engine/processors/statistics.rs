//! Statistics Event Processor
//! 
//! This processor collects repository statistics from streaming events
//! instead of directly accessing the repository. It runs as a private
//! always-present subprocessor that accumulates statistics data.

use crate::scanner::async_engine::events::{RepositoryEvent, CommitInfo, FileInfo};
use crate::scanner::async_engine::shared_state::{SharedProcessorState, RepositoryMetadata};
use crate::scanner::async_engine::processors::{EventProcessor, ProcessorStats};
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
use serde::{Deserialize, Serialize};
use crate::plugin::PluginResult;
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::SystemTime;
use log::debug;

/// Repository statistics collected from scanner events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepositoryStatistics {
    /// Total number of commits in repository
    pub total_commits: u64,
    
    /// Total number of tracked files
    pub total_files: u64,
    
    /// Total size of tracked files in bytes
    pub total_file_size: u64,
    
    /// Repository size in bytes (for compatibility)
    pub repository_size: u64,
    
    /// Number of unique contributors
    pub total_authors: u64,
    
    /// Repository age in days (from first to last commit)
    pub age_days: u64,
    
    /// Average commits per day
    pub avg_commits_per_day: f64,
    
    /// Date of first commit (Unix timestamp)
    pub first_commit_date: Option<i64>,
    
    /// Date of last commit (Unix timestamp)
    pub last_commit_date: Option<i64>,
}

impl Default for RepositoryStatistics {
    fn default() -> Self {
        Self {
            total_commits: 0,
            total_files: 0,
            total_file_size: 0,
            repository_size: 0,
            total_authors: 0,
            age_days: 0,
            avg_commits_per_day: 0.0,
            first_commit_date: None,
            last_commit_date: None,
        }
    }
}

/// Statistics processor that collects repository statistics from events
/// This processor is always active and collects data for all scan modes
pub struct StatisticsProcessor {
    shared_state: Option<Arc<SharedProcessorState>>,
    stats: RepositoryStatistics,
    unique_authors: HashSet<String>,
    first_commit_time: Option<SystemTime>,
    last_commit_time: Option<SystemTime>,
    total_file_size: u64,
    events_processed: usize,
}

impl StatisticsProcessor {
    /// Create a new statistics processor
    pub fn new() -> Self {
        Self {
            shared_state: None,
            stats: RepositoryStatistics::default(),
            unique_authors: HashSet::new(),
            first_commit_time: None,
            last_commit_time: None,
            total_file_size: 0,
            events_processed: 0,
        }
    }

    /// Get the current statistics
    pub fn get_statistics(&self) -> &RepositoryStatistics {
        &self.stats
    }

    /// Process a commit discovered event
    fn process_commit_discovered(&mut self, commit: &CommitInfo) {
        self.stats.total_commits += 1;
        
        // Track unique authors
        self.unique_authors.insert(commit.author_email.clone());
        
        // Track commit times for age calculation
        if self.first_commit_time.is_none() || commit.timestamp < self.first_commit_time.unwrap() {
            self.first_commit_time = Some(commit.timestamp);
        }
        
        if self.last_commit_time.is_none() || commit.timestamp > self.last_commit_time.unwrap() {
            self.last_commit_time = Some(commit.timestamp);
        }
        
        debug!("Statistics: Processed commit {}, total commits: {}", 
               commit.short_hash, self.stats.total_commits);
    }

    /// Process a file scanned event
    fn process_file_scanned(&mut self, file_info: &FileInfo) {
        self.stats.total_files += 1;
        self.total_file_size += file_info.size;
        
        debug!("Statistics: Processed file {}, total files: {}, size: {}", 
               file_info.relative_path, self.stats.total_files, file_info.size);
    }

    /// Finalize statistics calculations
    fn finalize_statistics(&mut self) {
        // Update author count
        self.stats.total_authors = self.unique_authors.len() as u64;
        
        // Update file size
        self.stats.total_file_size = self.total_file_size;
        self.stats.repository_size = self.total_file_size; // For compatibility
        
        // Calculate age and average commits per day
        if let (Some(first), Some(last)) = (self.first_commit_time, self.last_commit_time) {
            let duration = last.duration_since(first).unwrap_or_default();
            self.stats.age_days = duration.as_secs() / (24 * 60 * 60);
            
            if self.stats.age_days > 0 {
                self.stats.avg_commits_per_day = self.stats.total_commits as f64 / self.stats.age_days as f64;
            }
            
            // Convert to Unix timestamps
            self.stats.first_commit_date = first.duration_since(SystemTime::UNIX_EPOCH)
                .ok().map(|d| d.as_secs() as i64);
            self.stats.last_commit_date = last.duration_since(SystemTime::UNIX_EPOCH)
                .ok().map(|d| d.as_secs() as i64);
        }
        
        debug!("Statistics finalized: {} commits, {} files, {} authors, {} days old", 
               self.stats.total_commits, self.stats.total_files, 
               self.stats.total_authors, self.stats.age_days);
    }
}

#[async_trait]
impl EventProcessor for StatisticsProcessor {
    fn name(&self) -> &'static str {
        "StatisticsProcessor"
    }

    fn set_shared_state(&mut self, shared_state: Arc<SharedProcessorState>) {
        self.shared_state = Some(shared_state);
    }

    fn shared_state(&self) -> Option<&Arc<SharedProcessorState>> {
        self.shared_state.as_ref()
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        debug!("Initializing statistics processor");
        self.stats = RepositoryStatistics::default();
        self.unique_authors.clear();
        self.first_commit_time = None;
        self.last_commit_time = None;
        self.total_file_size = 0;
        self.events_processed = 0;
        Ok(())
    }

    async fn process_event(&mut self, event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
        self.events_processed += 1;
        
        match event {
            RepositoryEvent::CommitDiscovered { commit, .. } => {
                self.process_commit_discovered(commit);
            }
            RepositoryEvent::FileScanned { file_info } => {
                self.process_file_scanned(file_info);
            }
            RepositoryEvent::RepositoryStarted { total_commits, total_files, .. } => {
                debug!("Statistics: Repository scan started, estimated {} commits, {} files", 
                       total_commits.unwrap_or(0), total_files.unwrap_or(0));
            }
            RepositoryEvent::RepositoryCompleted { stats: _ } => {
                debug!("Statistics: Repository scan completed, {} events processed", 
                       self.events_processed);
            }
            _ => {
                // Other events don't affect statistics
            }
        }
        
        // Statistics processor doesn't generate scan messages during processing
        // It only provides statistics through get_statistics()
        Ok(vec![])
    }

    async fn finalize(&mut self) -> PluginResult<Vec<ScanMessage>> {
        self.finalize_statistics();
        
        // Generate a final statistics message
        let message = ScanMessage::new(
            MessageHeader::new(0, "statistics-processor".to_string()),
            MessageData::RepositoryStatistics {
                total_commits: self.stats.total_commits,
                total_files: self.stats.total_files,
                total_authors: self.stats.total_authors,
                repository_size: self.stats.repository_size,
                age_days: self.stats.age_days,
                avg_commits_per_day: self.stats.avg_commits_per_day,
            },
        );
        
        Ok(vec![message])
    }

    fn get_stats(&self) -> ProcessorStats {
        ProcessorStats {
            events_processed: self.events_processed,
            messages_generated: 1, // Only generates one final message
            processing_time: std::time::Duration::from_millis(0), // TODO: Track actual time
            errors_encountered: 0,
        }
    }

    async fn on_repository_metadata(&mut self, metadata: &RepositoryMetadata) -> PluginResult<()> {
        debug!("Statistics processor received repository metadata for: {}", 
               metadata.repository_path.as_deref().unwrap_or("unknown"));
        Ok(())
    }
}

impl Default for StatisticsProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::async_engine::events::{CommitInfo, FileInfo};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_test_commit(hash: &str, author: &str, timestamp_secs: u64) -> CommitInfo {
        CommitInfo {
            hash: hash.to_string(),
            short_hash: hash.chars().take(8).collect::<String>(),
            author_name: author.to_string(),
            author_email: format!("{}@example.com", author.to_lowercase()),
            committer_name: author.to_string(),
            committer_email: format!("{}@example.com", author.to_lowercase()),
            timestamp: UNIX_EPOCH + std::time::Duration::from_secs(timestamp_secs),
            message: format!("Test commit by {}", author),
            parent_hashes: vec![],
            changed_files: vec![],
            insertions: 10,
            deletions: 5,
        }
    }

    fn create_test_file(path: &str, size: u64) -> FileInfo {
        FileInfo {
            path: PathBuf::from(path),
            relative_path: path.to_string(),
            size,
            extension: PathBuf::from(path).extension().map(|s| s.to_string_lossy().to_string()),
            is_binary: false,
            line_count: Some((size / 50) as usize), // Rough estimate
            last_modified: Some(SystemTime::now()),
        }
    }

    #[tokio::test]
    async fn test_statistics_processor_creation() {
        let processor = StatisticsProcessor::new();
        assert_eq!(processor.name(), "StatisticsProcessor");
        // StatisticsProcessor processes all types of events
        assert_eq!(processor.get_statistics().total_commits, 0);
        assert_eq!(processor.get_statistics().total_files, 0);
        assert_eq!(processor.get_statistics().total_authors, 0);
    }

    #[tokio::test]
    async fn test_commit_processing() {
        let mut processor = StatisticsProcessor::new();
        processor.initialize().await.unwrap();

        // Process first commit
        let commit1 = create_test_commit("abc123", "Alice", 1000000);
        let event1 = RepositoryEvent::CommitDiscovered { commit: commit1, index: 0 };
        processor.process_event(&event1).await.unwrap();

        // Process second commit by different author
        let commit2 = create_test_commit("def456", "Bob", 2000000);
        let event2 = RepositoryEvent::CommitDiscovered { commit: commit2, index: 1 };
        processor.process_event(&event2).await.unwrap();

        // Process third commit by first author again
        let commit3 = create_test_commit("ghi789", "Alice", 3000000);
        let event3 = RepositoryEvent::CommitDiscovered { commit: commit3, index: 2 };
        processor.process_event(&event3).await.unwrap();

        processor.finalize().await.unwrap();

        let stats = processor.get_statistics();
        assert_eq!(stats.total_commits, 3);
        assert_eq!(stats.total_authors, 2); // Alice and Bob
        assert!(stats.age_days > 0);
        assert!(stats.avg_commits_per_day > 0.0);
        assert!(stats.first_commit_date.is_some());
        assert!(stats.last_commit_date.is_some());
    }

    #[tokio::test]
    async fn test_file_processing() {
        let mut processor = StatisticsProcessor::new();
        processor.initialize().await.unwrap();

        // Process files
        let file1 = create_test_file("src/main.rs", 1000);
        let event1 = RepositoryEvent::FileScanned { file_info: file1 };
        processor.process_event(&event1).await.unwrap();

        let file2 = create_test_file("src/lib.rs", 2000);
        let event2 = RepositoryEvent::FileScanned { file_info: file2 };
        processor.process_event(&event2).await.unwrap();

        processor.finalize().await.unwrap();

        let stats = processor.get_statistics();
        assert_eq!(stats.total_files, 2);
        assert_eq!(stats.total_file_size, 3000);
        assert_eq!(stats.repository_size, 3000);
    }

    #[tokio::test]
    async fn test_mixed_event_processing() {
        let mut processor = StatisticsProcessor::new();
        processor.initialize().await.unwrap();

        // Process commits and files
        let commit = create_test_commit("abc123", "Alice", 1000000);
        let commit_event = RepositoryEvent::CommitDiscovered { commit, index: 0 };
        processor.process_event(&commit_event).await.unwrap();

        let file = create_test_file("README.md", 500);
        let file_event = RepositoryEvent::FileScanned { file_info: file };
        processor.process_event(&file_event).await.unwrap();

        let messages = processor.finalize().await.unwrap();

        let stats = processor.get_statistics();
        assert_eq!(stats.total_commits, 1);
        assert_eq!(stats.total_files, 1);
        assert_eq!(stats.total_authors, 1);
        assert_eq!(stats.total_file_size, 500);

        // Should generate one statistics message
        assert_eq!(messages.len(), 1);
        if let MessageData::RepositoryStatistics { total_commits, total_files, .. } = &messages[0].data {
            assert_eq!(*total_commits, 1);
            assert_eq!(*total_files, 1);
        } else {
            panic!("Expected RepositoryStatistics message");
        }
    }

    #[tokio::test]
    async fn test_processor_stats() {
        let mut processor = StatisticsProcessor::new();
        processor.initialize().await.unwrap();

        // Process some events
        let commit = create_test_commit("abc123", "Alice", 1000000);
        let event = RepositoryEvent::CommitDiscovered { commit, index: 0 };
        processor.process_event(&event).await.unwrap();

        let stats = processor.get_stats();
        assert_eq!(stats.events_processed, 1);
        assert_eq!(stats.messages_generated, 1);
        assert_eq!(stats.errors_encountered, 0);
    }

    #[tokio::test]
    async fn test_statistics_processor_processes_all_events() {
        // Test that StatisticsProcessor can handle all types of repository events
        let mut processor = StatisticsProcessor::new();
        processor.initialize().await.unwrap();
        
        // Test file event processing
        let file_event = RepositoryEvent::FileScanned {
            file_info: crate::scanner::async_engine::events::FileInfo {
                path: std::path::PathBuf::from("test.rs"),
                relative_path: "test.rs".to_string(),
                size: 1000,
                extension: Some("rs".to_string()),
                is_binary: false,
                line_count: Some(50),
                last_modified: None,
            },
        };
        let result = processor.process_event(&file_event).await;
        assert!(result.is_ok());
        
        // Test commit event processing
        let commit = create_test_commit("abc123", "Alice", 1000000);
        let commit_event = RepositoryEvent::CommitDiscovered { commit, index: 0 };
        let result = processor.process_event(&commit_event).await;
        assert!(result.is_ok());
        
        let stats = processor.get_statistics();
        assert_eq!(stats.total_files, 1);
        assert_eq!(stats.total_commits, 1);
    }
}
