//! Statistics tracking module
//! 
//! Provides data structures and functions for tracking file and author statistics

use std::collections::{BTreeMap, HashSet};

/// File existence status
#[derive(Debug, Clone, PartialEq)]
pub enum FileStatus {
    /// File exists in current repository
    Exists,
    /// File has been deleted from repository
    Deleted,
    /// File status unknown (not yet checked)
    Unknown,
}

impl Default for FileStatus {
    fn default() -> Self {
        FileStatus::Unknown
    }
}

/// Statistics for a single file across all commits
#[derive(Debug, Clone, Default)]
pub struct FileStatistics {
    /// Total number of commits that touched this file
    pub commit_count: usize,
    /// Total lines added to this file
    pub lines_added: usize,
    /// Total lines removed from this file
    pub lines_removed: usize,
    /// Net change (added - removed)
    pub net_change: i64,
    /// Current number of lines in the file (if exists)
    pub current_lines: usize,
    /// File existence status
    pub status: FileStatus,
    /// Set of authors who have modified this file
    pub authors: HashSet<String>,
    /// Timestamp of first commit
    pub first_seen: Option<i64>,
    /// Timestamp of last commit
    pub last_modified: Option<i64>,
}

impl FileStatistics {
    /// Create new empty file statistics
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Update statistics with a new commit
    pub fn update(&mut self, author: &str, lines_added: usize, lines_removed: usize, timestamp: i64) {
        self.commit_count += 1;
        self.lines_added += lines_added;
        self.lines_removed += lines_removed;
        self.net_change = self.lines_added as i64 - self.lines_removed as i64;
        self.authors.insert(author.to_string());
        
        // Update timestamps
        if self.first_seen.is_none() || timestamp < self.first_seen.unwrap() {
            self.first_seen = Some(timestamp);
        }
        if self.last_modified.is_none() || timestamp > self.last_modified.unwrap() {
            self.last_modified = Some(timestamp);
        }
    }
    
    /// Set the current line count for the file
    pub fn set_current_lines(&mut self, lines: usize) {
        self.current_lines = lines;
        self.status = FileStatus::Exists;
    }
    
    /// Mark the file as deleted
    pub fn set_deleted(&mut self) {
        self.status = FileStatus::Deleted;
        self.current_lines = 0; // Reset to 0 for deleted files
    }
    
    /// Get current lines as a display string (shows '-' for deleted files)
    pub fn current_lines_display(&self) -> String {
        match self.status {
            FileStatus::Deleted => "-".to_string(),
            FileStatus::Exists => self.current_lines.to_string(),
            FileStatus::Unknown => "?".to_string(),
        }
    }
    
    /// Get the number of unique authors
    pub fn author_count(&self) -> usize {
        self.authors.len()
    }
}

/// Statistics for all files in the repository
#[derive(Debug, Clone, Default)]
pub struct RepositoryFileStats {
    /// File statistics mapped by file path
    pub files: BTreeMap<String, FileStatistics>,
}

impl RepositoryFileStats {
    /// Create new empty repository statistics
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Update statistics for a file
    pub fn update_file(&mut self, path: &str, author: &str, lines_added: usize, lines_removed: usize, timestamp: i64) {
        let file_stats = self.files.entry(path.to_string()).or_insert_with(FileStatistics::new);
        file_stats.update(author, lines_added, lines_removed, timestamp);
    }
    
    /// Set current line count for a file
    pub fn set_file_current_lines(&mut self, path: &str, lines: usize) {
        if let Some(file_stats) = self.files.get_mut(path) {
            file_stats.set_current_lines(lines);
        }
    }
    
    /// Mark files as deleted if they don't exist in current repository
    pub fn mark_missing_files_as_deleted(&mut self, existing_files: &std::collections::HashSet<String>) {
        for (path, file_stats) in self.files.iter_mut() {
            if file_stats.status == FileStatus::Unknown {
                if existing_files.contains(path) {
                    // File exists but wasn't processed (probably empty)
                    file_stats.status = FileStatus::Exists;
                } else {
                    // File doesn't exist in current repository
                    file_stats.set_deleted();
                }
            }
        }
    }
    
    /// Get all file paths that need existence checking
    pub fn get_unknown_file_paths(&self) -> Vec<String> {
        self.files.iter()
            .filter(|(_, stats)| stats.status == FileStatus::Unknown)
            .map(|(path, _)| path.clone())
            .collect()
    }
    
    /// Get total number of unique files
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
    
    /// Get files sorted by commit count (descending)
    pub fn files_by_commit_count(&self) -> Vec<(&String, &FileStatistics)> {
        let mut files: Vec<_> = self.files.iter().collect();
        files.sort_by(|a, b| b.1.commit_count.cmp(&a.1.commit_count));
        files
    }
    
    /// Get files sorted by net change (descending)
    pub fn files_by_net_change(&self) -> Vec<(&String, &FileStatistics)> {
        let mut files: Vec<_> = self.files.iter().collect();
        files.sort_by(|a, b| b.1.net_change.cmp(&a.1.net_change));
        files
    }
    
    /// Get total commit count across all files
    pub fn total_commits(&self) -> usize {
        self.files.values().map(|f| f.commit_count).sum()
    }
}