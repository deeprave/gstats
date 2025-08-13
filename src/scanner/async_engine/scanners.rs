//! Async Scanner Implementations
//! 
//! This module contains the event-driven scanner architecture that provides:
//! - Single-pass repository traversal
//! - Repository-owning pattern with spawn_blocking
//! - Memory-efficient processing with Send+Sync data extraction
//! - Better performance for multi-mode scans

use crate::scanner::async_traits::AsyncScanner;
use crate::scanner::query::QueryParams;
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
use super::error::{ScanError, ScanResult};
use super::stream::ScanMessageStream;
use super::events::{EventFilter, CommitInfo, FileInfo};
use log::debug;
use std::path::{Path, PathBuf};
use std::time::{UNIX_EPOCH, Duration};
use async_trait::async_trait;

/// Event-driven scanner that provides single-pass repository traversal
/// with repository-owning pattern
/// 
/// This scanner creates its own repository access using spawn_blocking,
/// eliminating Send/Sync issues and enabling proper async operation.
pub struct EventDrivenScanner {
    query_params: QueryParams,
    name: String,
    event_filter: EventFilter,
}

impl EventDrivenScanner {
    /// Create a new event-driven scanner
    pub fn new(query_params: QueryParams) -> Self {
        let event_filter = EventFilter::from_query_params(query_params.clone());
        Self {
            query_params,
            name: "EventDrivenScanner".to_string(),
            event_filter,
        }
    }
    
    /// Create an event-driven scanner with custom name
    pub fn with_name(query_params: QueryParams, name: String) -> Self {
        let event_filter = EventFilter::from_query_params(query_params.clone());
        Self {
            query_params,
            name,
            event_filter,
        }
    }
}

#[async_trait]
impl AsyncScanner for EventDrivenScanner {
    fn name(&self) -> &str {
        &self.name
    }


    async fn scan_async(&self, repository_path: &Path) -> ScanResult<ScanMessageStream> {
        debug!("EventDrivenScanner: Starting scan for path: {:?}", repository_path);
        
        // Repository-owning pattern: extract Send+Sync data immediately using spawn_blocking
        let repo_path = repository_path.to_path_buf();
        let event_filter = self.event_filter.clone();
        
        // Extract all required data in spawn_blocking to ensure Send+Sync compliance
        let scan_data = tokio::task::spawn_blocking(move || -> ScanResult<Vec<ScanMessage>> {
            let repo = gix::discover(&repo_path)
                .map_err(|e| ScanError::Repository(format!("Invalid repository at {}: {}", repo_path.display(), e)))?;
            
            let mut messages = Vec::new();
            let mut message_index = 0u64;

            let head = repo.head_commit()
                .map_err(|e| ScanError::Repository(format!("Failed to get head: {}", e)))?;
            let head_id = head.id;

            // Extract commit data - scanner now emits ALL repository data
            {
                let walk = repo.rev_walk([head_id]);
                let commits = walk.all()
                    .map_err(|e| ScanError::Repository(format!("Commit walk error: {}", e)))?;
                
                for commit_info in commits.take(100) { // Limit to 100 commits for now
                    let commit_info = commit_info
                        .map_err(|e| ScanError::Repository(format!("Failed to get commit info: {}", e)))?;
                    
                    let commit_id = commit_info.id;
                    let commit = repo.find_object(commit_id)
                        .map_err(|e| ScanError::Repository(format!("Failed to find commit: {}", e)))?
                        .try_into_commit()
                        .map_err(|e| ScanError::Repository(format!("Failed to convert to commit: {}", e)))?;
                    
                    // Extract Send+Sync data immediately
                    let hash = commit_id.to_string();
                    let commit_message = commit.message()
                        .map_err(|e| ScanError::Repository(format!("Failed to get commit message: {}", e)))?
                        .title.to_string();
                    let author_info = commit.author()
                        .map_err(|e| ScanError::Repository(format!("Failed to get commit author: {}", e)))?;
                    let author_name = author_info.name.to_string();
                    let author_email = author_info.email.to_string();
                    let timestamp_seconds = commit.time()
                        .map_err(|e| ScanError::Repository(format!("Failed to get commit time: {}", e)))?
                        .seconds;
                    let timestamp = UNIX_EPOCH + Duration::from_secs(timestamp_seconds as u64);
                    
                    // Create CommitInfo for filtering
                    let commit_info = CommitInfo {
                        hash: hash.clone(),
                        short_hash: hash.chars().take(8).collect(),
                        author_name: author_name.clone(),
                        author_email: author_email.clone(),
                        committer_name: author_name.clone(), // Use author as committer for simplicity
                        committer_email: author_email.clone(),
                        timestamp,
                        message: commit_message.clone(),
                        parent_hashes: vec![], // TODO: Extract parent hashes if needed
                        changed_files: vec![], // TODO: Extract changed files if needed
                        insertions: 0, // TODO: Calculate insertions if needed
                        deletions: 0,  // TODO: Calculate deletions if needed
                    };
                    
                    // Apply pre-filtering before creating event
                    if event_filter.should_include_commit(&commit_info) {
                        let scan_message = ScanMessage::new(
                            MessageHeader::new(message_index),
                            MessageData::CommitInfo {
                                hash,
                                message: commit_message,
                                author: author_name,
                                timestamp: timestamp_seconds as i64,
                                changed_files: vec![], // TODO: Extract file changes if needed
                            },
                        );
                        
                        messages.push(scan_message);
                        message_index += 1;
                    }
                }
            }
            
            // Extract file data - scanner now emits ALL repository data
            {
                let tree = head.tree()
                    .map_err(|e| ScanError::Repository(format!("Failed to get tree: {}", e)))?;
                
                let traverse = tree.traverse();
                let files = traverse.breadthfirst.files()
                    .map_err(|e| ScanError::Repository(format!("Failed to traverse files: {}", e)))?;
                
                for entry in files {
                    // Extract Send+Sync data immediately
                    let path = entry.filepath.to_string();
                    // For now, estimate size based on path length (actual blob reading would require more complex logic)
                    let size = (path.len() * 50) as u64; // Rough estimate
                    let lines = if size == 0 { 0 } else { ((size / 50).max(1)) as u32 };
                    
                    // Create FileInfo for filtering
                    let file_info = FileInfo {
                        path: PathBuf::from(&path),
                        relative_path: path.clone(),
                        size,
                        extension: PathBuf::from(&path).extension().map(|s| s.to_string_lossy().to_string()),
                        is_binary: false, // Estimate as non-binary for now
                        line_count: Some(lines as usize),
                        last_modified: None, // TODO: Extract last modified if needed
                    };
                    
                    // Apply pre-filtering before creating event
                    if event_filter.should_include_file(&file_info) {
                        let scan_message = ScanMessage::new(
                            MessageHeader::new(message_index),
                            MessageData::FileInfo {
                                path,
                                size,
                                lines,
                            },
                        );
                        
                        messages.push(scan_message);
                        message_index += 1;
                    }
                }
            }
            
            Ok(messages)
        }).await
        .map_err(|e| ScanError::Repository(format!("Spawn blocking failed: {}", e)))??;
        
        debug!("EventDrivenScanner: Extracted {} messages", scan_data.len());
        
        // Convert to stream with correct Result type
        let stream = futures::stream::iter(scan_data.into_iter().map(Ok));
        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::query::QueryParams;
    use futures::StreamExt;
    use std::path::Path;

    #[tokio::test]
    async fn test_event_driven_scanner_creation() {
        let query_params = QueryParams::default();
        let scanner = EventDrivenScanner::new(query_params);
        
        assert_eq!(scanner.name(), "EventDrivenScanner");
    }


    #[tokio::test]
    async fn test_event_driven_scanner_with_invalid_path() {
        let query_params = QueryParams::default();
        let scanner = EventDrivenScanner::new(query_params);
        let invalid_path = Path::new("/nonexistent/path");
        
        let result = scanner.scan_async(invalid_path).await;
        assert!(result.is_err());
        
        if let Err(ScanError::Repository(msg)) = result {
            assert!(msg.contains("Invalid repository"));
        } else {
            panic!("Expected Repository error");
        }
    }

    #[tokio::test]
    async fn test_event_driven_scanner_with_current_directory() {
        let query_params = QueryParams::default();
        let scanner = EventDrivenScanner::new(query_params);
        let current_path = Path::new(".");
        
        // This should work if current directory is a git repository
        let result = scanner.scan_async(current_path).await;
        
        // Result depends on whether current directory is a git repo
        // If it's a git repo, should succeed; if not, should fail with Repository error
        match result {
            Ok(mut stream) => {
                // If successful, should be able to read from stream
                let first_message = stream.next().await;
                if let Some(Ok(message)) = first_message {
                    println!("✅ EventDrivenScanner produced message: {:?}", message.header());
                }
                // Don't assert on content since it depends on actual repository state
            }
            Err(ScanError::Repository(_)) => {
                // Expected if current directory is not a git repository
            }
            Err(e) => {
                panic!("Unexpected error type: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_event_driven_scanner_history_mode() {
        let query_params = QueryParams::default();
        let scanner = EventDrivenScanner::new(query_params);
        let current_path = Path::new(".");
        
        // Test scanner with current directory
        match scanner.scan_async(current_path).await {
            Ok(mut stream) => {
                let mut commit_count = 0;
                while let Some(message_result) = stream.next().await {
                    match message_result {
                        Ok(message) => {
                            if let MessageData::CommitInfo { hash, author, message: msg, .. } = &message.data {
                                println!("✅ Commit: {} by {} - {}", hash, author, msg);
                                commit_count += 1;
                                if commit_count >= 3 { // Limit output for test
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            println!("⚠️  Message error: {}", e);
                            break;
                        }
                    }
                }
                println!("✅ EventDrivenScanner processed {} commits", commit_count);
            }
            Err(e) => {
                println!("⚠️  EventDrivenScanner failed (expected if not in git repo): {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_event_driven_scanner_files_mode() {
        let query_params = QueryParams::default();
        let scanner = EventDrivenScanner::new(query_params);
        let current_path = Path::new(".");
        
        // Test scanner with current directory
        match scanner.scan_async(current_path).await {
            Ok(mut stream) => {
                let mut file_count = 0;
                while let Some(message_result) = stream.next().await {
                    match message_result {
                        Ok(message) => {
                            if let MessageData::FileInfo { path, size, lines } = &message.data {
                                println!("✅ File: {} ({} bytes, {} lines)", path, size, lines);
                                file_count += 1;
                                if file_count >= 5 { // Limit output for test
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            println!("⚠️  Message error: {}", e);
                            break;
                        }
                    }
                }
                println!("✅ EventDrivenScanner processed {} files", file_count);
            }
            Err(e) => {
                println!("⚠️  EventDrivenScanner failed (expected if not in git repo): {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_scanner_date_range_prefiltering() {
        use crate::scanner::query::DateRange;
        use std::time::{Duration, UNIX_EPOCH};
        
        // Create scanner with date range that excludes all commits (future date)
        let mut query_params = QueryParams::default();
        query_params.date_range = Some(DateRange {
            start: Some(UNIX_EPOCH + Duration::from_secs(9999999999)), // Far future
            end: None,
        });
        
        let scanner = EventDrivenScanner::new(query_params);
        let current_path = Path::new(".");
        
        // This test should fail until pre-filtering is implemented
        match scanner.scan_async(current_path).await {
            Ok(mut stream) => {
                let mut commit_count = 0;
                while let Some(message_result) = stream.next().await {
                    if let Ok(message) = message_result {
                        if let MessageData::CommitInfo { .. } = &message.data {
                            commit_count += 1;
                        }
                    }
                    if commit_count >= 10 { break; } // Prevent infinite loop
                }
                // Should be 0 commits due to date filtering, but will fail until implemented
                assert_eq!(commit_count, 0, "Scanner should filter commits by date range before creating events");
            }
            Err(_) => {
                // Skip test if not in git repo
            }
        }
    }

    #[tokio::test]
    async fn test_scanner_author_prefiltering() {
        use crate::scanner::query::AuthorFilter;
        
        // Create scanner with author filter that excludes all authors
        let mut query_params = QueryParams::default();
        query_params.authors = AuthorFilter {
            include: vec!["nonexistent_author_12345".to_string()],
            exclude: vec![],
        };
        
        let scanner = EventDrivenScanner::new(query_params);
        let current_path = Path::new(".");
        
        // This test should fail until pre-filtering is implemented
        match scanner.scan_async(current_path).await {
            Ok(mut stream) => {
                let mut commit_count = 0;
                while let Some(message_result) = stream.next().await {
                    if let Ok(message) = message_result {
                        if let MessageData::CommitInfo { .. } = &message.data {
                            commit_count += 1;
                        }
                    }
                    if commit_count >= 10 { break; } // Prevent infinite loop
                }
                // Should be 0 commits due to author filtering, but will fail until implemented
                assert_eq!(commit_count, 0, "Scanner should filter commits by author before creating events");
            }
            Err(_) => {
                // Skip test if not in git repo
            }
        }
    }

    #[tokio::test]
    async fn test_scanner_file_path_prefiltering() {
        use crate::scanner::query::FilePathFilter;
        
        // Create scanner with file path filter that excludes all files
        let mut query_params = QueryParams::default();
        query_params.file_paths = FilePathFilter {
            include: vec!["nonexistent_pattern_12345".into()],
            exclude: vec![],
        };
        
        let scanner = EventDrivenScanner::new(query_params);
        let current_path = Path::new(".");
        
        // This test should fail until pre-filtering is implemented
        match scanner.scan_async(current_path).await {
            Ok(mut stream) => {
                let mut file_count = 0;
                while let Some(message_result) = stream.next().await {
                    if let Ok(message) = message_result {
                        if let MessageData::FileInfo { .. } = &message.data {
                            file_count += 1;
                        }
                    }
                    if file_count >= 10 { break; } // Prevent infinite loop
                }
                // Should be 0 files due to path filtering, but will fail until implemented
                assert_eq!(file_count, 0, "Scanner should filter files by path before creating events");
            }
            Err(_) => {
                // Skip test if not in git repo
            }
        }
    }
}
