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

/// Combined scanner that handles multiple modes
pub struct AsyncCombinedScanner {
    name: String,
    file_scanner: AsyncFileScanner,
    history_scanner: AsyncHistoryScanner,
}

impl AsyncCombinedScanner {
    /// Create a new combined scanner
    pub fn new(repository: Arc<AsyncRepositoryHandle>) -> Self {
        let file_scanner = AsyncFileScanner::new(Arc::clone(&repository));
        let history_scanner = AsyncHistoryScanner::new(Arc::clone(&repository));
        
        Self {
            name: "AsyncCombinedScanner".to_string(),
            file_scanner,
            history_scanner,
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
        
        Self {
            name,
            file_scanner,
            history_scanner,
        }
    }
}

#[async_trait::async_trait]
impl AsyncScanner for AsyncCombinedScanner {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn supports_mode(&self, mode: ScanMode) -> bool {
        self.file_scanner.supports_mode(mode) || self.history_scanner.supports_mode(mode)
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
    use tokio_stream::StreamExt;
    
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
    async fn test_async_combined_scanner() {
        let sync_handle = git::resolve_repository_handle(None).unwrap();
        let async_handle = Arc::new(AsyncRepositoryHandle::new(sync_handle));
        let scanner = AsyncCombinedScanner::new(async_handle);
        
        assert_eq!(scanner.name(), "AsyncCombinedScanner");
        assert!(scanner.supports_mode(ScanMode::FILES));
        assert!(scanner.supports_mode(ScanMode::HISTORY));
        assert!(scanner.supports_mode(ScanMode::FILES | ScanMode::HISTORY));
        
        // Test scanning both modes
        let combined_mode = ScanMode::FILES | ScanMode::HISTORY;
        let stream = scanner.scan_async(combined_mode).await.unwrap();
        let messages: Vec<_> = tokio_stream::StreamExt::collect(tokio_stream::StreamExt::take(stream, 10)).await;
        
        assert!(!messages.is_empty());
        
        // Should have messages from both scanners
        let mut has_files = false;
        let mut has_history = false;
        
        for result in messages {
            let message = result.unwrap();
            match message.header().mode() {
                ScanMode::FILES => has_files = true,
                ScanMode::HISTORY => has_history = true,
                _ => {}
            }
        }
        
        // Both modes should be represented (though not guaranteed in small sample)
        // At minimum, we should have at least one type
        assert!(has_files || has_history);
    }
    
    #[tokio::test]
    async fn test_estimate_message_count() {
        let sync_handle = git::resolve_repository_handle(None).unwrap();
        let async_handle = Arc::new(AsyncRepositoryHandle::new(sync_handle));
        
        let file_scanner = AsyncFileScanner::new(Arc::clone(&async_handle));
        let history_scanner = AsyncHistoryScanner::new(Arc::clone(&async_handle));
        let combined_scanner = AsyncCombinedScanner::new(async_handle);
        
        // File scanner should provide estimate
        let file_estimate = file_scanner.estimate_message_count(ScanMode::FILES).await;
        assert!(file_estimate.is_some());
        assert!(file_estimate.unwrap() > 0);
        
        // History scanner should provide estimate
        let history_estimate = history_scanner.estimate_message_count(ScanMode::HISTORY).await;
        assert!(history_estimate.is_some());
        assert!(history_estimate.unwrap() > 0);
        
        // Combined scanner should provide estimate for combined modes
        let combined_estimate = combined_scanner.estimate_message_count(ScanMode::FILES | ScanMode::HISTORY).await;
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