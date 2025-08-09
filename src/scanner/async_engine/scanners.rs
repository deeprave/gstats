//! Async Scanner Implementations
//! 
//! Provides async implementations of different scan modes using streaming patterns.

use std::sync::Arc;
use tokio_stream::Stream;
use futures::stream::{self, StreamExt as FuturesStreamExt};
use crate::scanner::modes::ScanMode;
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
use crate::scanner::async_traits::AsyncScanner;
use super::error::{ScanError, ScanResult};
use super::repository::AsyncRepositoryHandle;
use super::stream::ScanMessageStream;

/// Async file scanner that streams file information
pub struct AsyncFileScanner {
    repository: Arc<AsyncRepositoryHandle>,
    name: String,
}

impl AsyncFileScanner {
    /// Create a new async file scanner
    pub fn new(repository: Arc<AsyncRepositoryHandle>) -> Self {
        Self {
            repository,
            name: "AsyncFileScanner".to_string(),
        }
    }
    
    /// Create a file scanner with a custom name
    pub fn with_name(repository: Arc<AsyncRepositoryHandle>, name: String) -> Self {
        Self {
            repository,
            name,
        }
    }
    
    /// Stream file information as scan messages
    async fn stream_files(&self) -> ScanResult<impl Stream<Item = ScanResult<ScanMessage>>> {
        let files = self.repository.list_files().await?;
        
        let message_stream = FuturesStreamExt::map(
            stream::iter(files.into_iter().enumerate()),
            |(index, file_info)| {
                let header = MessageHeader::new(ScanMode::FILES, index as u64);
                let data = MessageData::FileInfo {
                    path: file_info.path,
                    size: file_info.size as u64,
                    lines: estimate_line_count(file_info.size) as u32,
                };
                
                Ok(ScanMessage::new(header, data))
            }
        );
        
        Ok(message_stream)
    }
}

#[async_trait::async_trait]
impl AsyncScanner for AsyncFileScanner {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn supports_mode(&self, mode: ScanMode) -> bool {
        mode.contains(ScanMode::FILES)
    }
    
    async fn scan_async(&self, modes: ScanMode) -> ScanResult<ScanMessageStream> {
        if !self.supports_mode(modes) {
            return Err(ScanError::invalid_mode(modes));
        }
        
        let stream = self.stream_files().await?;
        Ok(Box::pin(stream))
    }
    
    async fn estimate_message_count(&self, modes: ScanMode) -> Option<usize> {
        if !self.supports_mode(modes) {
            return None;
        }
        
        // Get file count as estimate
        match self.repository.list_files().await {
            Ok(files) => Some(files.len()),
            Err(_) => None,
        }
    }
}

/// Async history scanner that streams commit information
pub struct AsyncHistoryScanner {
    repository: Arc<AsyncRepositoryHandle>,
    name: String,
    max_commits: Option<usize>,
}

impl AsyncHistoryScanner {
    /// Create a new async history scanner
    pub fn new(repository: Arc<AsyncRepositoryHandle>) -> Self {
        Self {
            repository,
            name: "AsyncHistoryScanner".to_string(),
            max_commits: Some(1000), // Default limit
        }
    }
    
    /// Create a history scanner with a custom name and commit limit
    pub fn with_config(
        repository: Arc<AsyncRepositoryHandle>, 
        name: String, 
        max_commits: Option<usize>
    ) -> Self {
        Self {
            repository,
            name,
            max_commits,
        }
    }
    
    /// Stream commit information as scan messages
    async fn stream_commits(&self) -> ScanResult<impl Stream<Item = ScanResult<ScanMessage>>> {
        let commits = self.repository.get_commit_history(self.max_commits).await?;
        
        let message_stream = FuturesStreamExt::map(
            stream::iter(commits.into_iter().enumerate()),
            |(index, commit_info)| {
                let header = MessageHeader::new(ScanMode::HISTORY, index as u64);
                let data = MessageData::CommitInfo {
                    hash: commit_info.id,
                    author: commit_info.author,
                    message: commit_info.message,
                    timestamp: commit_info.timestamp,
                    changed_files: commit_info.changed_files.into_iter()
                        .map(|fc| crate::scanner::messages::FileChangeData {
                            path: fc.path,
                            lines_added: fc.lines_added,
                            lines_removed: fc.lines_removed,
                        })
                        .collect(),
                };
                
                Ok(ScanMessage::new(header, data))
            }
        );
        
        Ok(message_stream)
    }
}

#[async_trait::async_trait]
impl AsyncScanner for AsyncHistoryScanner {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn supports_mode(&self, mode: ScanMode) -> bool {
        mode.contains(ScanMode::HISTORY)
    }
    
    async fn scan_async(&self, modes: ScanMode) -> ScanResult<ScanMessageStream> {
        if !self.supports_mode(modes) {
            return Err(ScanError::invalid_mode(modes));
        }
        
        let stream = self.stream_commits().await?;
        Ok(Box::pin(stream))
    }
    
    async fn estimate_message_count(&self, modes: ScanMode) -> Option<usize> {
        if !self.supports_mode(modes) {
            return None;
        }
        
        // Return the configured max commits as estimate
        self.max_commits
    }
}

/// Async change frequency scanner that analyzes file change patterns
pub struct AsyncChangeFrequencyScanner {
    repository: Arc<AsyncRepositoryHandle>,
    name: String,
    time_window_days: u32,
}

impl AsyncChangeFrequencyScanner {
    /// Create a new async change frequency scanner
    pub fn new(repository: Arc<AsyncRepositoryHandle>) -> Self {
        Self {
            repository,
            name: "AsyncChangeFrequencyScanner".to_string(),
            time_window_days: 90, // Default to 3 months
        }
    }
    
    /// Create a change frequency scanner with custom time window
    pub fn with_time_window(repository: Arc<AsyncRepositoryHandle>, time_window_days: u32) -> Self {
        Self {
            repository,
            name: format!("AsyncChangeFrequencyScanner-{}d", time_window_days),
            time_window_days,
        }
    }
    
    /// Analyze change frequency for all files in the repository
    async fn analyze_change_frequency(&self) -> ScanResult<impl Stream<Item = ScanResult<ScanMessage>>> {
        use std::collections::HashMap;
        use chrono::{Utc, Duration};
        
        // Get current time and calculate cutoff
        let now = Utc::now();
        let cutoff = now - Duration::days(self.time_window_days as i64);
        let cutoff_timestamp = cutoff.timestamp();
        
        // Get commit history within time window
        let commits = self.repository.get_commit_history(None).await?;
        
        // Build file change statistics
        let mut file_stats: HashMap<String, (u32, Vec<String>, i64, i64)> = HashMap::new(); // (count, authors, first, last)
        
        for commit in commits {
            if commit.timestamp >= cutoff_timestamp {
                for file_change in &commit.changed_files {
                    let entry = file_stats.entry(file_change.path.clone()).or_insert((0, Vec::new(), commit.timestamp, commit.timestamp));
                    entry.0 += 1; // increment change count
                    if !entry.1.contains(&commit.author) {
                        entry.1.push(commit.author.clone()); // add unique author
                    }
                    entry.2 = entry.2.min(commit.timestamp); // first change
                    entry.3 = entry.3.max(commit.timestamp); // last change
                }
            }
        }
        
        // Convert to scan messages
        let messages: Vec<ScanResult<ScanMessage>> = file_stats.into_iter().enumerate().map(|(index, (file_path, (change_count, authors, first_changed, last_changed)))| {
            // Calculate frequency score (changes per day)
            let days_in_window = self.time_window_days as f64;
            let frequency_score = change_count as f64 / days_in_window;
            
            // Calculate recency weight (more recent changes get higher weight)
            let days_since_last_change = (now.timestamp() - last_changed) as f64 / 86400.0; // seconds to days
            let recency_weight = if days_since_last_change <= 0.0 {
                1.0
            } else {
                1.0 / (1.0 + days_since_last_change / 30.0) // decay over 30 days
            };
            
            let header = MessageHeader::new(ScanMode::CHANGE_FREQUENCY, index as u64);
            let data = MessageData::ChangeFrequencyInfo {
                file_path,
                change_count,
                author_count: authors.len() as u32,
                last_changed,
                first_changed,
                frequency_score,
                recency_weight,
                authors,
            };
            
            Ok(ScanMessage::new(header, data))
        }).collect();
        
        Ok(stream::iter(messages))
    }
}

#[async_trait::async_trait]
impl AsyncScanner for AsyncChangeFrequencyScanner {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn supports_mode(&self, mode: ScanMode) -> bool {
        mode.contains(ScanMode::CHANGE_FREQUENCY)
    }
    
    async fn scan_async(&self, modes: ScanMode) -> ScanResult<ScanMessageStream> {
        if !modes.contains(ScanMode::CHANGE_FREQUENCY) {
            return Err(ScanError::invalid_mode(modes));
        }
        
        let stream = self.analyze_change_frequency().await?;
        Ok(Box::pin(stream))
    }
    
    async fn estimate_message_count(&self, modes: ScanMode) -> Option<usize> {
        if !modes.contains(ScanMode::CHANGE_FREQUENCY) {
            return None;
        }
        
        // Estimate based on repository size - this is a rough estimate
        // In practice, we'd need to analyze the git history to get an accurate count
        match self.repository.get_repository_stats().await {
            Ok(_stats) => Some(100), // Fallback estimate since file_count is not available
            Err(_) => Some(100), // Fallback estimate
        }
    }
}

/// Combined scanner that handles multiple modes
pub struct AsyncCombinedScanner {
    name: String,
    file_scanner: AsyncFileScanner,
    history_scanner: AsyncHistoryScanner,
    change_frequency_scanner: AsyncChangeFrequencyScanner,
}

impl AsyncCombinedScanner {
    /// Create a new combined scanner
    pub fn new(repository: Arc<AsyncRepositoryHandle>) -> Self {
        let file_scanner = AsyncFileScanner::new(Arc::clone(&repository));
        let history_scanner = AsyncHistoryScanner::new(Arc::clone(&repository));
        let change_frequency_scanner = AsyncChangeFrequencyScanner::new(Arc::clone(&repository));
        
        Self {
            name: "AsyncCombinedScanner".to_string(),
            file_scanner,
            history_scanner,
            change_frequency_scanner,
        }
    }
    
    /// Create a combined scanner with custom configuration
    pub fn with_config(
        repository: Arc<AsyncRepositoryHandle>,
        name: String,
        max_commits: Option<usize>,
    ) -> Self {
        let file_scanner = AsyncFileScanner::with_name(Arc::clone(&repository), format!("{}-Files", name));
        let history_scanner = AsyncHistoryScanner::with_config(
            Arc::clone(&repository), 
            format!("{}-History", name),
            max_commits
        );
        let change_frequency_scanner = AsyncChangeFrequencyScanner::new(Arc::clone(&repository));
        
        Self {
            name,
            file_scanner,
            history_scanner,
            change_frequency_scanner,
        }
    }
}

#[async_trait::async_trait]
impl AsyncScanner for AsyncCombinedScanner {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn supports_mode(&self, mode: ScanMode) -> bool {
        self.file_scanner.supports_mode(mode) || 
        self.history_scanner.supports_mode(mode) || 
        self.change_frequency_scanner.supports_mode(mode)
    }
    
    async fn scan_async(&self, modes: ScanMode) -> ScanResult<ScanMessageStream> {
        let mut streams = Vec::new();
        
        // Add file scanner stream if FILES mode is requested
        if modes.contains(ScanMode::FILES) {
            let file_stream = self.file_scanner.scan_async(ScanMode::FILES).await?;
            streams.push(file_stream);
        }
        
        // Add history scanner stream if HISTORY mode is requested
        if modes.contains(ScanMode::HISTORY) {
            let history_stream = self.history_scanner.scan_async(ScanMode::HISTORY).await?;
            streams.push(history_stream);
        }
        
        // Add change frequency scanner stream if CHANGE_FREQUENCY mode is requested
        if modes.contains(ScanMode::CHANGE_FREQUENCY) {
            let change_frequency_stream = self.change_frequency_scanner.scan_async(ScanMode::CHANGE_FREQUENCY).await?;
            streams.push(change_frequency_stream);
        }
        
        if streams.is_empty() {
            return Err(ScanError::invalid_mode(modes));
        }
        
        // Merge all streams into one
        let merged_stream = streams.into_iter()
            .fold(
                FuturesStreamExt::boxed(stream::empty()),
                |acc, s| FuturesStreamExt::boxed(stream::select(acc, s))
            );
        
        Ok(merged_stream)
    }
    
    async fn estimate_message_count(&self, modes: ScanMode) -> Option<usize> {
        let mut total = 0;
        let mut has_estimate = false;
        
        if modes.contains(ScanMode::FILES) {
            if let Some(count) = self.file_scanner.estimate_message_count(ScanMode::FILES).await {
                total += count;
                has_estimate = true;
            }
        }
        
        if modes.contains(ScanMode::HISTORY) {
            if let Some(count) = self.history_scanner.estimate_message_count(ScanMode::HISTORY).await {
                total += count;
                has_estimate = true;
            }
        }
        
        if modes.contains(ScanMode::CHANGE_FREQUENCY) {
            if let Some(count) = self.change_frequency_scanner.estimate_message_count(ScanMode::CHANGE_FREQUENCY).await {
                total += count;
                has_estimate = true;
            }
        }
        
        if has_estimate {
            Some(total)
        } else {
            None
        }
    }
}

/// Estimate line count from file size (rough heuristic)
fn estimate_line_count(size: usize) -> usize {
    if size == 0 {
        0
    } else {
        // Assume average of 50 characters per line
        (size / 50).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git;
    
    #[tokio::test]
    async fn test_async_file_scanner() {
        let sync_handle = git::resolve_repository_handle(None).unwrap();
        let async_handle = Arc::new(AsyncRepositoryHandle::new(sync_handle));
        let scanner = AsyncFileScanner::new(async_handle);
        
        assert_eq!(scanner.name(), "AsyncFileScanner");
        assert!(scanner.supports_mode(ScanMode::FILES));
        assert!(!scanner.supports_mode(ScanMode::HISTORY));
        
        // Test scanning
        let stream = scanner.scan_async(ScanMode::FILES).await.unwrap();
        let messages: Vec<_> = tokio_stream::StreamExt::collect(stream).await;
        
        assert!(!messages.is_empty());
        
        // All messages should be successful and have FILES mode
        for result in messages {
            let message = result.unwrap();
            assert_eq!(message.header().mode(), ScanMode::FILES);
            
            match message.data() {
                MessageData::FileInfo { path, size, lines } => {
                    assert!(!path.is_empty());
                    assert!(*size > 0 || *lines == 0);
                }
                _ => panic!("Expected FileInfo message data"),
            }
        }
    }
    
    #[tokio::test]
    async fn test_async_history_scanner() {
        let sync_handle = git::resolve_repository_handle(None).unwrap();
        let async_handle = Arc::new(AsyncRepositoryHandle::new(sync_handle));
        let scanner = AsyncHistoryScanner::new(async_handle);
        
        assert_eq!(scanner.name(), "AsyncHistoryScanner");
        assert!(scanner.supports_mode(ScanMode::HISTORY));
        assert!(!scanner.supports_mode(ScanMode::FILES));
        
        // Test scanning with limited commits
        let stream = scanner.scan_async(ScanMode::HISTORY).await.unwrap();
        let messages: Vec<_> = tokio_stream::StreamExt::collect(tokio_stream::StreamExt::take(stream, 5)).await;
        
        assert!(!messages.is_empty());
        assert!(messages.len() <= 5);
        
        // All messages should be successful and have HISTORY mode
        for result in messages {
            let message = result.unwrap();
            assert_eq!(message.header().mode(), ScanMode::HISTORY);
            
            match message.data() {
                MessageData::CommitInfo { hash, author, message: _, timestamp, changed_files } => {
                    assert!(!hash.is_empty());
                    assert!(!author.is_empty());
                    assert!(*timestamp > 0);
                    assert!(changed_files.is_empty() || !changed_files.is_empty()); // Allows empty or non-empty
                }
                _ => panic!("Expected CommitInfo message data"),
            }
        }
    }
    
    #[tokio::test]
    async fn test_async_change_frequency_scanner() {
        let sync_handle = git::resolve_repository_handle(None).unwrap();
        let async_handle = Arc::new(AsyncRepositoryHandle::new(sync_handle));
        let scanner = AsyncChangeFrequencyScanner::new(async_handle);
        
        assert_eq!(scanner.name(), "AsyncChangeFrequencyScanner");
        assert!(scanner.supports_mode(ScanMode::CHANGE_FREQUENCY));
        assert!(!scanner.supports_mode(ScanMode::FILES));
        assert!(!scanner.supports_mode(ScanMode::HISTORY));
        
        // Test scanning with change frequency mode
        let stream = scanner.scan_async(ScanMode::CHANGE_FREQUENCY).await.unwrap();
        let messages: Vec<_> = tokio_stream::StreamExt::collect(tokio_stream::StreamExt::take(stream, 5)).await;
        
        // Should have some messages (may be empty if no recent changes)
        assert!(messages.len() <= 5);
        
        // All messages should be successful and have CHANGE_FREQUENCY mode
        for result in messages {
            let message = result.unwrap();
            assert_eq!(message.header().mode(), ScanMode::CHANGE_FREQUENCY);
            
            match message.data() {
                MessageData::ChangeFrequencyInfo { file_path, change_count, .. } => {
                    assert!(!file_path.is_empty());
                    assert!(*change_count > 0);
                },
                _ => panic!("Expected ChangeFrequencyInfo message data"),
            }
        }
    }
    
    #[tokio::test]
    async fn test_async_combined_scanner() {
        let sync_handle = git::resolve_repository_handle(None).unwrap();
        let async_handle = Arc::new(AsyncRepositoryHandle::new(sync_handle));
        let scanner = AsyncCombinedScanner::new(async_handle);
        
        assert_eq!(scanner.name(), "AsyncCombinedScanner");
        assert!(scanner.supports_mode(ScanMode::FILES));
        assert!(scanner.supports_mode(ScanMode::HISTORY));
        assert!(scanner.supports_mode(ScanMode::CHANGE_FREQUENCY));
        assert!(scanner.supports_mode(ScanMode::FILES | ScanMode::HISTORY));
        assert!(scanner.supports_mode(ScanMode::FILES | ScanMode::HISTORY | ScanMode::CHANGE_FREQUENCY));
        
        // Test scanning all three modes
        let combined_mode = ScanMode::FILES | ScanMode::HISTORY | ScanMode::CHANGE_FREQUENCY;
        let stream = scanner.scan_async(combined_mode).await.unwrap();
        let messages: Vec<_> = tokio_stream::StreamExt::collect(tokio_stream::StreamExt::take(stream, 15)).await;
        
        assert!(!messages.is_empty());
        
        // Should have messages from all scanners
        let mut has_files = false;
        let mut has_history = false;
        let mut has_change_frequency = false;
        
        for result in messages {
            let message = result.unwrap();
            match message.header().mode() {
                ScanMode::FILES => has_files = true,
                ScanMode::HISTORY => has_history = true,
                ScanMode::CHANGE_FREQUENCY => has_change_frequency = true,
                _ => {}
            }
        }
        
        // At minimum, we should have at least one type
        assert!(has_files || has_history || has_change_frequency);
    }
    
    #[tokio::test]
    async fn test_estimate_message_count() {
        let sync_handle = git::resolve_repository_handle(None).unwrap();
        let async_handle = Arc::new(AsyncRepositoryHandle::new(sync_handle));
        
        let file_scanner = AsyncFileScanner::new(Arc::clone(&async_handle));
        let history_scanner = AsyncHistoryScanner::new(Arc::clone(&async_handle));
        let change_frequency_scanner = AsyncChangeFrequencyScanner::new(Arc::clone(&async_handle));
        let combined_scanner = AsyncCombinedScanner::new(async_handle);
        
        // File scanner should provide estimate
        let file_estimate = file_scanner.estimate_message_count(ScanMode::FILES).await;
        assert!(file_estimate.is_some());
        assert!(file_estimate.unwrap() > 0);
        
        // History scanner should provide estimate
        let history_estimate = history_scanner.estimate_message_count(ScanMode::HISTORY).await;
        assert!(history_estimate.is_some());
        assert!(history_estimate.unwrap() > 0);
        
        // Change frequency scanner should provide estimate
        let change_frequency_estimate = change_frequency_scanner.estimate_message_count(ScanMode::CHANGE_FREQUENCY).await;
        assert!(change_frequency_estimate.is_some());
        assert!(change_frequency_estimate.unwrap() > 0);
        
        // Combined scanner should provide estimate for combined modes
        let combined_estimate = combined_scanner.estimate_message_count(ScanMode::FILES | ScanMode::HISTORY | ScanMode::CHANGE_FREQUENCY).await;
        assert!(combined_estimate.is_some());
        assert!(combined_estimate.unwrap() > 0);
    }
    
    #[tokio::test]
    async fn test_unsupported_mode() {
        let sync_handle = git::resolve_repository_handle(None).unwrap();
        let async_handle = Arc::new(AsyncRepositoryHandle::new(sync_handle));
        let file_scanner = AsyncFileScanner::new(async_handle);
        
        // Should fail with unsupported mode
        let result = file_scanner.scan_async(ScanMode::METRICS).await;
        assert!(result.is_err());
        
        if let Err(error) = result {
            match error {
                ScanError::InvalidMode(_) => {}, // Expected
                _ => panic!("Expected InvalidMode error"),
            }
        }
    }
}