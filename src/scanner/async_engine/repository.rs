//! Async Repository Handle
//! 
//! Provides async-friendly, thread-safe access to git repository operations
//! with support for concurrent scanning operations.

use std::sync::Arc;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;
use tokio::task;
use anyhow::{Result, Context};
use git2::{Repository, ObjectType, TreeWalkMode, TreeWalkResult};
use crate::git::RepositoryHandle;
use crate::scanner::modes::ScanMode;
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
use super::error::{ScanError, ScanResult};

/// Async wrapper around RepositoryHandle for concurrent access
#[derive(Clone)]
pub struct AsyncRepositoryHandle {
    /// Underlying repository handle
    handle: Arc<RepositoryHandle>,
    
    /// Read-write lock for coordinating access
    /// Most git operations are read-only and can be concurrent,
    /// but we use this for safety and future extensibility
    _lock: Arc<RwLock<()>>,
}

impl AsyncRepositoryHandle {
    /// Create a new async repository handle from a synchronous handle
    pub fn new(handle: RepositoryHandle) -> Self {
        Self {
            handle: Arc::new(handle),
            _lock: Arc::new(RwLock::new(())),
        }
    }
    
    /// Open a repository from a path asynchronously
    pub async fn open<P: AsRef<Path>>(path: P) -> ScanResult<Self> {
        let path = path.as_ref().to_path_buf();
        
        let handle = task::spawn_blocking(move || {
            RepositoryHandle::open(path)
        }).await
            .map_err(|e| ScanError::async_operation(format!("Task failed: {}", e)))?
            .map_err(|e| ScanError::repository(format!("Failed to open repository: {}", e)))?;
        
        Ok(Self::new(handle))
    }
    
    /// Get the repository path
    pub fn path(&self) -> String {
        self.handle.path()
    }
    
    /// Get the repository working directory
    pub fn workdir(&self) -> Option<PathBuf> {
        self.handle.workdir().map(|p| p.to_path_buf())
    }
    
    /// Check if this is a bare repository
    pub fn is_bare(&self) -> bool {
        self.handle.is_bare()
    }
    
    /// Get the underlying repository handle for sync operations
    pub fn sync_handle(&self) -> &RepositoryHandle {
        &self.handle
    }
    
    /// Perform a thread-safe read operation on the repository
    async fn with_repository<F, T>(&self, operation: F) -> ScanResult<T>
    where
        F: FnOnce(&Repository) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let handle = Arc::clone(&self.handle);
        
        task::spawn_blocking(move || {
            operation(handle.repository())
        }).await
            .map_err(|e| ScanError::async_operation(format!("Repository operation failed: {}", e)))?
            .map_err(|e| ScanError::repository(e.to_string()))
    }
    
    /// Get repository statistics asynchronously
    pub async fn get_repository_stats(&self) -> ScanResult<RepositoryStats> {
        self.with_repository(|repo| {
            let mut stats = RepositoryStats::default();
            
            // Count references
            let refs = repo.references()?;
            for reference in refs {
                let reference = reference?;
                match reference.target() {
                    Some(_) => stats.branch_count += 1,
                    None => stats.tag_count += 1,
                }
            }
            
            // Get HEAD commit if available
            if let Ok(head) = repo.head() {
                if let Some(oid) = head.target() {
                    stats.head_commit = Some(oid.to_string());
                }
            }
            
            // Count objects in the database
            let odb = repo.odb()?;
            odb.foreach(|_oid| {
                stats.object_count += 1;
                true
            })?;
            
            Ok(stats)
        }).await
    }
    
    /// List all files in the repository asynchronously
    pub async fn list_files(&self) -> ScanResult<Vec<FileInfo>> {
        self.with_repository(|repo| {
            let mut files = Vec::new();
            
            let head = repo.head()
                .context("Failed to get HEAD reference")?;
            let tree = head.peel_to_tree()
                .context("Failed to peel HEAD to tree")?;
            
            tree.walk(TreeWalkMode::PreOrder, |root, entry| {
                if entry.kind() == Some(ObjectType::Blob) {
                    let path = if root.is_empty() {
                        entry.name().unwrap_or("unknown").to_string()
                    } else {
                        format!("{}/{}", root, entry.name().unwrap_or("unknown"))
                    };
                    
                    let size = entry.to_object(repo)
                        .ok()
                        .and_then(|obj| {
                            if obj.kind() == Some(ObjectType::Blob) {
                                obj.as_blob().map(|blob| blob.size())
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    
                    let file_info = FileInfo {
                        path: path.clone(),
                        size,
                        executable: entry.filemode() == 0o100755,
                    };
                    
                    files.push(file_info);
                }
                TreeWalkResult::Ok
            })?;
            
            Ok(files)
        }).await
    }
    
    /// Get commit history asynchronously with a limit
    pub async fn get_commit_history(&self, max_commits: Option<usize>) -> ScanResult<Vec<CommitInfo>> {
        self.with_repository(move |repo| {
            let mut commits = Vec::new();
            let mut walk = repo.revwalk()?;
            
            // Start from HEAD
            walk.push_head()?;
            walk.set_sorting(git2::Sort::TIME)?;
            
            let limit = max_commits.unwrap_or(1000);
            
            for (i, oid_result) in walk.enumerate() {
                if i >= limit {
                    break;
                }
                
                let oid = oid_result?;
                let commit = repo.find_commit(oid)?;
                
                let commit_info = CommitInfo {
                    id: oid.to_string(),
                    message: commit.message().unwrap_or("").to_string(),
                    author: commit.author().name().unwrap_or("").to_string(),
                    author_email: commit.author().email().unwrap_or("").to_string(),
                    timestamp: commit.time().seconds(),
                    parent_count: commit.parent_count(),
                };
                
                commits.push(commit_info);
            }
            
            Ok(commits)
        }).await
    }
    
    /// Stream file scan messages
    pub async fn scan_files_to_messages(&self, modes: ScanMode) -> ScanResult<Vec<ScanMessage>> {
        if !modes.contains(ScanMode::FILES) {
            return Ok(Vec::new());
        }
        
        let files = self.list_files().await?;
        let mut messages = Vec::new();
        
        for (index, file_info) in files.into_iter().enumerate() {
            let header = MessageHeader::new(ScanMode::FILES, index as u64);
            let data = MessageData::FileInfo {
                path: file_info.path,
                size: file_info.size as u64,
                lines: estimate_line_count(file_info.size) as u32,
            };
            
            messages.push(ScanMessage::new(header, data));
        }
        
        Ok(messages)
    }
    
    /// Stream history scan messages
    pub async fn scan_history_to_messages(&self, modes: ScanMode, max_commits: Option<usize>) -> ScanResult<Vec<ScanMessage>> {
        if !modes.contains(ScanMode::HISTORY) {
            return Ok(Vec::new());
        }
        
        let commits = self.get_commit_history(max_commits).await?;
        let mut messages = Vec::new();
        
        for (index, commit_info) in commits.into_iter().enumerate() {
            let header = MessageHeader::new(ScanMode::HISTORY, index as u64);
            let data = MessageData::CommitInfo {
                hash: commit_info.id,
                message: commit_info.message,
                author: commit_info.author,
                timestamp: commit_info.timestamp,
            };
            
            messages.push(ScanMessage::new(header, data));
        }
        
        Ok(messages)
    }
}

/// Repository statistics
#[derive(Debug, Clone, Default)]
pub struct RepositoryStats {
    pub branch_count: usize,
    pub tag_count: usize,
    pub object_count: usize,
    pub head_commit: Option<String>,
}

/// File information from repository
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: String,
    pub size: usize,
    pub executable: bool,
}

/// Commit information from repository
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    pub author: String,
    pub author_email: String,
    pub timestamp: i64,
    pub parent_count: usize,
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

// Ensure thread safety
unsafe impl Send for AsyncRepositoryHandle {}
unsafe impl Sync for AsyncRepositoryHandle {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git;
    
    #[tokio::test]
    async fn test_async_repository_creation() {
        let sync_handle = git::resolve_repository_handle(None).unwrap();
        let async_handle = AsyncRepositoryHandle::new(sync_handle);
        
        assert!(!async_handle.path().is_empty());
        assert!(!async_handle.is_bare());
    }
    
    #[tokio::test]
    async fn test_async_repository_open() {
        let async_handle = AsyncRepositoryHandle::open(".").await.unwrap();
        assert!(async_handle.path().contains("gstats"));
    }
    
    #[tokio::test]
    async fn test_repository_stats() {
        let sync_handle = git::resolve_repository_handle(None).unwrap();
        let async_handle = AsyncRepositoryHandle::new(sync_handle);
        
        let stats = async_handle.get_repository_stats().await.unwrap();
        assert!(stats.object_count > 0);
        assert!(stats.head_commit.is_some());
    }
    
    #[tokio::test]
    async fn test_list_files() {
        let sync_handle = git::resolve_repository_handle(None).unwrap();
        let async_handle = AsyncRepositoryHandle::new(sync_handle);
        
        let files = async_handle.list_files().await.unwrap();
        assert!(!files.is_empty());
        
        // Should contain some Rust files
        let rust_files: Vec<_> = files.iter()
            .filter(|f| f.path.ends_with(".rs"))
            .collect();
        assert!(!rust_files.is_empty());
    }
    
    #[tokio::test]
    async fn test_commit_history() {
        let sync_handle = git::resolve_repository_handle(None).unwrap();
        let async_handle = AsyncRepositoryHandle::new(sync_handle);
        
        let commits = async_handle.get_commit_history(Some(10)).await.unwrap();
        assert!(!commits.is_empty());
        assert!(commits.len() <= 10);
        
        // All commits should have valid data
        for commit in &commits {
            assert!(!commit.id.is_empty());
            assert!(!commit.author.is_empty());
        }
    }
    
    #[tokio::test]
    async fn test_scan_files_to_messages() {
        let sync_handle = git::resolve_repository_handle(None).unwrap();
        let async_handle = AsyncRepositoryHandle::new(sync_handle);
        
        let messages = async_handle.scan_files_to_messages(ScanMode::FILES).await.unwrap();
        assert!(!messages.is_empty());
        
        // All messages should be file type
        for message in &messages {
            assert_eq!(message.header().mode(), ScanMode::FILES);
            match message.data() {
                MessageData::FileInfo { path, size, lines } => {
                    assert!(!path.is_empty());
                    assert!(*size > 0 || *lines == 0); // Empty files can have 0 size but should have 0 lines
                }
                _ => panic!("Expected FileInfo message data"),
            }
        }
    }
    
    #[tokio::test]
    async fn test_scan_history_to_messages() {
        let sync_handle = git::resolve_repository_handle(None).unwrap();
        let async_handle = AsyncRepositoryHandle::new(sync_handle);
        
        let messages = async_handle.scan_history_to_messages(ScanMode::HISTORY, Some(5)).await.unwrap();
        assert!(!messages.is_empty());
        assert!(messages.len() <= 5);
        
        // All messages should be commit type
        for message in &messages {
            assert_eq!(message.header().mode(), ScanMode::HISTORY);
            match message.data() {
                MessageData::CommitInfo { hash, author, .. } => {
                    assert!(!hash.is_empty());
                    assert!(!author.is_empty());
                }
                _ => panic!("Expected CommitInfo message data"),
            }
        }
    }
    
    #[tokio::test]
    async fn test_concurrent_access() {
        let sync_handle = git::resolve_repository_handle(None).unwrap();
        let async_handle = AsyncRepositoryHandle::new(sync_handle);
        
        // Spawn multiple concurrent operations
        let handle1 = async_handle.clone();
        let handle2 = async_handle.clone();
        let handle3 = async_handle.clone();
        
        let task1 = tokio::spawn(async move {
            handle1.get_repository_stats().await
        });
        
        let task2 = tokio::spawn(async move {
            handle2.list_files().await
        });
        
        let task3 = tokio::spawn(async move {
            handle3.get_commit_history(Some(5)).await
        });
        
        // All should complete successfully
        let (result1, result2, result3) = tokio::join!(task1, task2, task3);
        
        assert!(result1.unwrap().is_ok());
        assert!(result2.unwrap().is_ok());
        assert!(result3.unwrap().is_ok());
    }
}