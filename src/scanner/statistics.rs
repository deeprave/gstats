//! Repository Statistics Collection
//! 
//! Basic repository statistics for analysis context.

use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::git::RepositoryHandle;

/// Basic repository statistics for analysis context
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepositoryStatistics {
    /// Total number of commits in the repository
    pub total_commits: u64,
    /// Total number of files in the repository
    pub total_files: u64,
    /// Total number of unique authors
    pub total_authors: u64,
    /// Repository size in bytes (working directory)
    pub repository_size: u64,
    /// Timestamp of first commit (Unix timestamp)
    pub first_commit_date: Option<i64>,
    /// Timestamp of last commit (Unix timestamp)
    pub last_commit_date: Option<i64>,
}

impl Default for RepositoryStatistics {
    fn default() -> Self {
        Self {
            total_commits: 0,
            total_files: 0,
            total_authors: 0,
            repository_size: 0,
            first_commit_date: None,
            last_commit_date: None,
        }
    }
}

/// Repository statistics collector
pub struct RepositoryStatsCollector;

impl RepositoryStatsCollector {
    /// Create a new statistics collector
    pub fn new() -> Self {
        Self
    }
    
    /// Collect basic repository statistics
    pub fn collect_statistics(&self, repo: &RepositoryHandle) -> Result<RepositoryStatistics> {
        let mut stats = RepositoryStatistics::default();
        
        // Count commits and collect dates/authors
        self.collect_commit_statistics(repo, &mut stats)?;
        
        // Count files and calculate repository size
        self.collect_file_statistics(repo, &mut stats)?;
        
        Ok(stats)
    }
    
    /// Collect commit-related statistics
    fn collect_commit_statistics(&self, repo: &RepositoryHandle, stats: &mut RepositoryStatistics) -> Result<()> {
        use std::collections::HashSet;
        
        let mut authors = HashSet::new();
        let mut commit_count = 0u64;
        let mut first_date: Option<i64> = None;
        let mut last_date: Option<i64> = None;
        
        // Get access to the underlying git2::Repository
        let git_repo = repo.repository();
        
        // Walk through all commits
        let mut revwalk = git_repo.revwalk()?;
        revwalk.push_head()?;
        
        for oid in revwalk {
            let oid = oid?;
            if let Ok(commit) = git_repo.find_commit(oid) {
                commit_count += 1;
                
                // Collect author information
                if let Some(author) = commit.author().email() {
                    authors.insert(author.to_string());
                }
                
                // Track commit dates
                let commit_time = commit.time().seconds();
                match (first_date, last_date) {
                    (None, None) => {
                        first_date = Some(commit_time);
                        last_date = Some(commit_time);
                    }
                    (Some(first), Some(last)) => {
                        if commit_time < first {
                            first_date = Some(commit_time);
                        }
                        if commit_time > last {
                            last_date = Some(commit_time);
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
        
        stats.total_commits = commit_count;
        stats.total_authors = authors.len() as u64;
        stats.first_commit_date = first_date;
        stats.last_commit_date = last_date;
        
        Ok(())
    }
    
    /// Collect file-related statistics from Git repository (tracked files only)
    fn collect_file_statistics(&self, repo: &RepositoryHandle, stats: &mut RepositoryStatistics) -> Result<()> {
        let git_repo = repo.repository();
        
        let mut file_count = 0u64;
        let mut total_size = 0u64;
        
        // Get the HEAD commit to read the tree
        if let Ok(head) = git_repo.head() {
            if let Ok(commit) = head.peel_to_commit() {
                let tree = commit.tree()?;
                
                // Walk through all files in the Git tree (tracked files only)
                tree.walk(git2::TreeWalkMode::PreOrder, |_, entry| {
                    if entry.kind() == Some(git2::ObjectType::Blob) {
                        file_count += 1;
                        
                        // Try to get the blob size
                        if let Ok(blob) = git_repo.find_blob(entry.id()) {
                            total_size += blob.size() as u64;
                        }
                    }
                    git2::TreeWalkResult::Ok
                })?;
            }
        }
        
        stats.total_files = file_count;
        stats.repository_size = total_size;
        
        Ok(())
    }
}

impl Default for RepositoryStatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_repository_statistics_default() {
        let stats = RepositoryStatistics::default();
        
        assert_eq!(stats.total_commits, 0);
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_authors, 0);
        assert_eq!(stats.repository_size, 0);
        assert!(stats.first_commit_date.is_none());
        assert!(stats.last_commit_date.is_none());
    }
    
    #[test]
    fn test_repository_statistics_serialization() {
        let stats = RepositoryStatistics {
            total_commits: 100,
            total_files: 50,
            total_authors: 5,
            repository_size: 1024 * 1024, // 1MB
            first_commit_date: Some(1609459200), // 2021-01-01
            last_commit_date: Some(1640995200),  // 2022-01-01
        };
        
        // Test JSON serialization
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: RepositoryStatistics = serde_json::from_str(&json).unwrap();
        
        assert_eq!(stats, deserialized);
    }
    
    #[test]
    fn test_stats_collector_creation() {
        let _collector = RepositoryStatsCollector::new();
        // Size is always non-negative (usize type)
        
        let _collector2 = RepositoryStatsCollector::default();
        // Size is always non-negative (usize type)
    }
    
    #[test]
    fn test_real_repository_statistics() {
        use crate::git::RepositoryHandle;
        
        // Test with current repository
        if let Ok(repo) = RepositoryHandle::open(".") {
            let collector = RepositoryStatsCollector::new();
            let stats = collector.collect_statistics(&repo).unwrap();
            
            // Verify we got some reasonable statistics
            assert!(stats.total_commits > 0, "Should have commits");
            assert!(stats.total_files > 0, "Should have files");
            assert!(stats.total_authors > 0, "Should have authors");
            assert!(stats.repository_size > 0, "Should have size");
            assert!(stats.first_commit_date.is_some(), "Should have first commit date");
            assert!(stats.last_commit_date.is_some(), "Should have last commit date");
            
            // Verify date ordering makes sense
            if let (Some(first), Some(last)) = (stats.first_commit_date, stats.last_commit_date) {
                assert!(first <= last, "First commit should be before or equal to last commit");
            }
            
            println!("Repository statistics:");
            println!("  Commits: {}", stats.total_commits);
            println!("  Files: {}", stats.total_files);
            println!("  Authors: {}", stats.total_authors);
            println!("  Size: {} bytes", stats.repository_size);
        }
    }
    
    // Note: Integration tests with actual repositories would require
    // setting up test repositories, which is better done in integration tests
    // rather than unit tests.
}