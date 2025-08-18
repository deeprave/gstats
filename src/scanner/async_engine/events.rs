use crate::scanner::query::QueryParams;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

/// Repository events emitted during single-pass scanning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RepositoryEvent {
    /// Emitted when scanning starts
    RepositoryStarted {
        total_commits: Option<usize>,
        total_files: Option<usize>,
    },

    /// Emitted when a commit is discovered during history traversal
    CommitDiscovered {
        commit: CommitInfo,
        index: usize,
    },

    /// Emitted when a file change is detected in a commit
    FileChanged {
        file_path: String,
        change_data: FileChangeData,
        commit_context: CommitInfo,
    },

    /// Emitted when a file is scanned in the working directory
    FileScanned {
        file_info: FileInfo,
    },

    /// Emitted when scanning completes
    RepositoryCompleted {
        stats: RepositoryStats,
    },

    /// Emitted for error conditions that don't stop scanning
    ScanError {
        error: String,
        context: String,
    },
}

/// Comprehensive commit information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub author_name: String,
    pub author_email: String,
    pub committer_name: String,
    pub committer_email: String,
    pub timestamp: SystemTime,
    pub message: String,
    pub parent_hashes: Vec<String>,
    pub changed_files: Vec<String>,
    pub insertions: usize,
    pub deletions: usize,
}

/// File change information within a commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeData {
    pub change_type: ChangeType,
    pub old_path: Option<String>,
    pub new_path: String,
    pub insertions: usize,
    pub deletions: usize,
    pub is_binary: bool,
}

/// Type of file change
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

/// File information from working directory scan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: PathBuf,
    pub relative_path: String,
    pub size: u64,
    pub extension: Option<String>,
    pub is_binary: bool,
    pub line_count: Option<usize>,
    pub last_modified: Option<SystemTime>,
}

/// Repository scanning statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryStats {
    pub total_commits: usize,
    pub total_files: usize,
    pub total_changes: usize,
    pub scan_duration: std::time::Duration,
    pub events_emitted: usize,
}

/// Event filter for optimization
#[derive(Debug, Clone)]
pub struct EventFilter {
    pub query_params: QueryParams,
    pub include_binary_files: bool,
    pub max_file_size: Option<u64>,
}

impl EventFilter {
    /// Create a new event filter from query parameters
    pub fn from_query_params(query_params: QueryParams) -> Self {
        Self {
            query_params,
            include_binary_files: false,
            max_file_size: Some(10 * 1024 * 1024), // 10MB default limit
        }
    }

    /// Check if a commit should be included based on filters
    pub fn should_include_commit(&self, commit: &CommitInfo) -> bool {
        // Apply date filters
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

        // Apply author filters
        if !self.query_params.authors.include.is_empty() {
            let author_match = self.query_params.authors.include.iter().any(|author| {
                commit.author_name.contains(author) || commit.author_email.contains(author)
            });
            if !author_match {
                return false;
            }
        }

        // Apply author exclusion filters
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

    /// Check if a file should be included based on filters
    pub fn should_include_file(&self, file_info: &FileInfo) -> bool {
        // Skip binary files if not included
        if file_info.is_binary && !self.include_binary_files {
            return false;
        }

        // Apply file size limits
        if let Some(max_size) = self.max_file_size {
            if file_info.size > max_size {
                return false;
            }
        }

        // Apply file path filters
        if !self.query_params.file_paths.include.is_empty() {
            let path_match = self.query_params.file_paths.include.iter().any(|pattern| {
                file_info.relative_path.contains(pattern.to_string_lossy().as_ref())
            });
            if !path_match {
                return false;
            }
        }

        // Apply file path exclusion filters
        if !self.query_params.file_paths.exclude.is_empty() {
            let path_excluded = self.query_params.file_paths.exclude.iter().any(|pattern| {
                file_info.relative_path.contains(pattern.to_string_lossy().as_ref())
            });
            if path_excluded {
                return false;
            }
        }

        true
    }

    /// Check if a file change should be included
    pub fn should_include_file_change(&self, change: &FileChangeData, commit: &CommitInfo) -> bool {
        // First check if the commit should be included
        if !self.should_include_commit(commit) {
            return false;
        }

        // Skip binary files if not included
        if change.is_binary && !self.include_binary_files {
            return false;
        }

        // Apply file path filters to the changed file
        if !self.query_params.file_paths.include.is_empty() {
            let path_match = self.query_params.file_paths.include.iter().any(|pattern| {
                let pattern_str = pattern.to_string_lossy();
                change.new_path.contains(pattern_str.as_ref()) || 
                change.old_path.as_ref().is_some_and(|old| old.contains(pattern_str.as_ref()))
            });
            if !path_match {
                return false;
            }
        }

        // Apply file path exclusion filters
        if !self.query_params.file_paths.exclude.is_empty() {
            let path_excluded = self.query_params.file_paths.exclude.iter().any(|pattern| {
                let pattern_str = pattern.to_string_lossy();
                change.new_path.contains(pattern_str.as_ref()) || 
                change.old_path.as_ref().is_some_and(|old| old.contains(pattern_str.as_ref()))
            });
            if path_excluded {
                return false;
            }
        }

        true
    }
}

impl RepositoryEvent {
    /// Get the event type as a string for debugging
    pub fn event_type(&self) -> &'static str {
        match self {
            RepositoryEvent::RepositoryStarted { .. } => "RepositoryStarted",
            RepositoryEvent::CommitDiscovered { .. } => "CommitDiscovered",
            RepositoryEvent::FileChanged { .. } => "FileChanged",
            RepositoryEvent::FileScanned { .. } => "FileScanned",
            RepositoryEvent::RepositoryCompleted { .. } => "RepositoryCompleted",
            RepositoryEvent::ScanError { .. } => "ScanError",
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn test_event_type_identification() {
        let event = RepositoryEvent::CommitDiscovered {
            commit: create_test_commit(),
            index: 0,
        };
        assert_eq!(event.event_type(), "CommitDiscovered");
    }

    #[test]
    fn test_event_processing_without_modes() {
        let _commit_event = RepositoryEvent::CommitDiscovered {
            commit: create_test_commit(),
            index: 0,
        };
        
        // Events are now processed without mode filtering
        let _file_event = RepositoryEvent::FileScanned {
            file_info: create_test_file_info(),
        };
    }

    #[test]
    fn test_event_filter_commit_date_filtering() {
        use crate::scanner::query::DateRange;
        
        let mut query_params = QueryParams::default();
        query_params.date_range = Some(DateRange {
            start: Some(UNIX_EPOCH + Duration::from_secs(1000)),
            end: None,
        });
        
        let filter = EventFilter::from_query_params(query_params);
        
        let old_commit = CommitInfo {
            timestamp: UNIX_EPOCH + Duration::from_secs(500),
            ..create_test_commit()
        };
        
        let new_commit = CommitInfo {
            timestamp: UNIX_EPOCH + Duration::from_secs(1500),
            ..create_test_commit()
        };
        
        assert!(!filter.should_include_commit(&old_commit));
        assert!(filter.should_include_commit(&new_commit));
    }

    #[test]
    fn test_event_filter_author_filtering() {
        use crate::scanner::query::AuthorFilter;
        
        let mut query_params = QueryParams::default();
        query_params.authors = AuthorFilter {
            include: vec!["john".to_string()],
            exclude: vec![],
        };
        
        let filter = EventFilter::from_query_params(query_params);
        
        let matching_commit = CommitInfo {
            author_name: "john.doe".to_string(),
            ..create_test_commit()
        };
        
        let non_matching_commit = CommitInfo {
            author_name: "jane.smith".to_string(),
            ..create_test_commit()
        };
        
        assert!(filter.should_include_commit(&matching_commit));
        assert!(!filter.should_include_commit(&non_matching_commit));
    }

    #[test]
    fn test_event_filter_file_size_filtering() {
        let filter = EventFilter::from_query_params(QueryParams::default());
        
        let small_file = FileInfo {
            size: 1024,
            ..create_test_file_info()
        };
        
        let large_file = FileInfo {
            size: 20 * 1024 * 1024, // 20MB
            ..create_test_file_info()
        };
        
        assert!(filter.should_include_file(&small_file));
        assert!(!filter.should_include_file(&large_file));
    }

    fn create_test_commit() -> CommitInfo {
        CommitInfo {
            hash: "abc123".to_string(),
            short_hash: "abc123".to_string(),
            author_name: "Test Author".to_string(),
            author_email: "test@example.com".to_string(),
            committer_name: "Test Author".to_string(),
            committer_email: "test@example.com".to_string(),
            timestamp: UNIX_EPOCH + Duration::from_secs(1000),
            message: "Test commit".to_string(),
            parent_hashes: vec![],
            changed_files: vec!["test.rs".to_string()],
            insertions: 10,
            deletions: 5,
        }
    }

    fn create_test_file_info() -> FileInfo {
        FileInfo {
            path: PathBuf::from("test.rs"),
            relative_path: "test.rs".to_string(),
            size: 1024,
            extension: Some("rs".to_string()),
            is_binary: false,
            line_count: Some(50),
            last_modified: Some(UNIX_EPOCH + Duration::from_secs(1000)),
        }
    }
}
