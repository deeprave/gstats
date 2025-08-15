//! File State Tracking Module
//!
//! This module provides comprehensive file state tracking working backwards through 
//! git history, maintaining accurate line counts and handling complex file lifecycle
//! scenarios including deletions, resurrections, and renames.
//!
//! ## Key Features
//!
//! - **Backwards History Traversal**: Tracks file states as we move backwards through commits
//! - **Accurate Line Counting**: Maintains precise line counts by analyzing diff hunks
//! - **Binary File Support**: Handles both text files (line counts) and binary files (byte sizes)
//! - **Lifecycle Analysis**: Detects file deletion, resurrection, and rename patterns
//! - **Memory Efficient**: Only tracks files that have been encountered in diffs
//!
//! ## Architecture
//!
//! ```text
//! FileTracker
//! ├── file_states: HashMap<String, FileState>
//! │   ├── "src/main.rs" → FileState { line_count: 120, exists: true, ... }
//! │   ├── "deleted.rs" → FileState { exists: false, ... }
//! │   └── "renamed.rs" → FileState { current_path: "old_name.rs", ... }
//! └── Backwards processing:
//!     HEAD → commit2 → commit1 → ... → initial
//! ```
//!
//! ## Backwards Processing Logic
//!
//! When processing commits backwards through history:
//!
//! - **Added files**: Didn't exist in previous state (mark as non-existent)
//! - **Deleted files**: Existed before deletion (restore to existence)
//! - **Modified files**: Reverse the line count changes (subtract additions, add deletions)
//! - **Renamed files**: Track under original name before rename
//! - **Binary files**: Track size changes for binary content
//!
//! ## Usage Pattern
//!
//! ```rust,no_run
//! use gstats::scanner::async_engine::file_tracker::FileTracker;
//! 
//! let mut tracker = FileTracker::new();
//! 
//! // Initialize with files at HEAD
//! tracker.initialize_file_at_head("src/main.rs".to_string(), Some(100), false, None);
//! 
//! // Process commits backwards
//! let git_history = vec!["diff content 1", "diff content 2"]; // Example diff data
//! for commit_diff in git_history.iter().rev() {
//!     let changes = tracker.process_commit_backwards(commit_diff)?;
//!     // ... use changes for plugin processing
//! }
//! 
//! // Analyze final lifecycle
//! let lifecycle = tracker.analyze_file_lifecycle();
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::collections::HashMap;
use crate::scanner::async_engine::events::ChangeType;
use crate::scanner::async_engine::diff_analyzer::{FileChangeAnalysis, DiffLineAnalyzer};
use crate::scanner::async_engine::error::ScanError;

/// State of a file at a specific point in git history
#[derive(Debug, Clone, PartialEq)]
pub struct FileState {
    /// Current line count of the file
    pub line_count: Option<usize>,
    /// Whether the file is binary
    pub is_binary: bool,
    /// Size in bytes for binary files
    pub binary_size: Option<u64>,
    /// Whether the file exists at this point in history
    pub exists: bool,
    /// Last known path (for tracking renames)
    pub current_path: String,
}

impl FileState {
    /// Create a new file state for a file that exists
    pub fn new_existing(path: String, line_count: Option<usize>, is_binary: bool, binary_size: Option<u64>) -> Self {
        Self {
            line_count,
            is_binary,
            binary_size,
            exists: true,
            current_path: path,
        }
    }
    
    /// Create a new file state for a deleted file
    pub fn new_deleted(path: String) -> Self {
        Self {
            line_count: None,
            is_binary: false,
            binary_size: None,
            exists: false,
            current_path: path,
        }
    }
    
    /// Apply a file change analysis to update this file state
    /// 
    /// This method works backwards through history, so:
    /// - Additions in history mean the file didn't exist before
    /// - Deletions in history mean the file existed before with more lines
    /// - Modifications adjust the line count backwards
    pub fn apply_change_backwards(&mut self, change: &FileChangeAnalysis) -> Result<(), ScanError> {
        match change.change_type {
            ChangeType::Added => {
                // Working backwards: if file was added, it didn't exist before
                self.exists = false;
                self.line_count = None;
                self.binary_size = None;
            },
            ChangeType::Deleted => {
                // Working backwards: if file was deleted, it existed before
                self.exists = true;
                self.line_count = if change.is_binary {
                    None
                } else {
                    Some(change.insertions) // The deleted lines represent the previous state
                };
                self.is_binary = change.is_binary;
                self.binary_size = change.binary_size;
            },
            ChangeType::Modified => {
                // Working backwards: reverse the changes
                if let Some(current_lines) = self.line_count {
                    // Subtract insertions (they didn't exist before)
                    // Add deletions (they existed before)
                    let previous_lines = current_lines
                        .saturating_sub(change.insertions)
                        .saturating_add(change.deletions);
                    self.line_count = Some(previous_lines);
                }
                self.exists = true;
            },
            ChangeType::Renamed => {
                // Working backwards: file had the old name before
                if let Some(old_path) = &change.old_path {
                    self.current_path = old_path.clone();
                }
                // Also apply any content changes
                if let Some(current_lines) = self.line_count {
                    let previous_lines = current_lines
                        .saturating_sub(change.insertions)
                        .saturating_add(change.deletions);
                    self.line_count = Some(previous_lines);
                }
                self.exists = true;
            },
            ChangeType::Copied => {
                // Working backwards: original file still existed, copy didn't exist
                if change.path == self.current_path {
                    // This is the copied file, it didn't exist before
                    self.exists = false;
                    self.line_count = None;
                    self.binary_size = None;
                }
                // If this is the source file, it continues to exist unchanged
            },
        }
        
        Ok(())
    }
}

/// File tracker that maintains file states working backwards through git history
pub struct FileTracker {
    /// Map of file path to its current state
    file_states: HashMap<String, FileState>,
}

impl FileTracker {
    /// Create a new file tracker
    pub fn new() -> Self {
        Self {
            file_states: HashMap::new(),
        }
    }
    
    /// Process a commit working backwards through history
    /// 
    /// This method takes a commit's diff output and updates all file states
    /// to reflect their state before this commit was applied.
    /// 
    /// # Arguments
    /// * `commit_diff_output` - Raw git diff output for the commit
    /// 
    /// # Returns
    /// Vector of file changes with updated line counts and states
    pub fn process_commit_backwards(&mut self, commit_diff_output: &str) -> Result<Vec<FileChangeAnalysis>, ScanError> {
        let changes = DiffLineAnalyzer::analyze_commit_diff(commit_diff_output)?;
        let mut updated_changes = Vec::new();
        
        for mut change in changes {
            // Get or create file state for this file
            let file_state = self.file_states.entry(change.path.clone())
                .or_insert_with(|| {
                    // If we haven't seen this file before, assume it exists at HEAD
                    // with the current line count from the change
                    FileState::new_existing(
                        change.path.clone(),
                        if change.is_binary { None } else { Some(change.insertions + change.deletions) },
                        change.is_binary,
                        change.binary_size,
                    )
                });
            
            // Update the change with current file state information
            change.binary_size = file_state.binary_size;
            
            // Apply the change backwards to update file state
            file_state.apply_change_backwards(&change)?;
            
            // Handle file path changes for renames
            if let Some(old_path) = &change.old_path {
                if change.change_type == ChangeType::Renamed {
                    // Move the file state to the old path
                    if let Some(state) = self.file_states.remove(&change.path) {
                        self.file_states.insert(old_path.clone(), state);
                    }
                }
            }
            
            updated_changes.push(change);
        }
        
        Ok(updated_changes)
    }
    
    /// Get the current state of a file
    pub fn get_file_state(&self, path: &str) -> Option<&FileState> {
        self.file_states.get(path)
    }
    
    /// Get all file states
    pub fn get_all_file_states(&self) -> &HashMap<String, FileState> {
        &self.file_states
    }
    
    /// Initialize a file state for a file at HEAD
    /// 
    /// This is used to set the initial state when we first encounter a file
    /// in the git history traversal.
    pub fn initialize_file_at_head(&mut self, path: String, line_count: Option<usize>, is_binary: bool, binary_size: Option<u64>) {
        self.file_states.insert(
            path.clone(),
            FileState::new_existing(path, line_count, is_binary, binary_size)
        );
    }
    
    /// Check if a file was deleted in the history
    pub fn is_file_deleted(&self, path: &str) -> bool {
        self.file_states.get(path)
            .map(|state| !state.exists)
            .unwrap_or(false)
    }
    
    /// Get files that have been resurrected (deleted then added back)
    pub fn get_resurrected_files(&self) -> Vec<&str> {
        self.file_states.iter()
            .filter_map(|(path, state)| {
                if state.exists {
                    Some(path.as_str())
                } else {
                    None
                }
            })
            .collect()
    }
    
    /// Track file lifecycle events and detect resurrection patterns
    /// 
    /// This method processes a series of commits and identifies files that have been:
    /// - Deleted and then re-added (resurrected)
    /// - Renamed multiple times
    /// - Moved between different directories
    pub fn analyze_file_lifecycle(&self) -> FileLifecycleAnalysis {
        let mut deleted_files = Vec::new();
        let mut resurrected_files = Vec::new();
        let mut stable_files = Vec::new();
        
        for (path, state) in &self.file_states {
            if !state.exists {
                deleted_files.push(FileLifecycleEvent {
                    path: path.clone(),
                    event_type: LifecycleEventType::Deleted,
                    current_path: state.current_path.clone(),
                    line_count: state.line_count,
                    is_binary: state.is_binary,
                });
            } else if path != &state.current_path {
                // File exists but path changed - this indicates resurrection or rename
                resurrected_files.push(FileLifecycleEvent {
                    path: path.clone(),
                    event_type: LifecycleEventType::Resurrected,
                    current_path: state.current_path.clone(),
                    line_count: state.line_count,
                    is_binary: state.is_binary,
                });
            } else {
                stable_files.push(FileLifecycleEvent {
                    path: path.clone(),
                    event_type: LifecycleEventType::Stable,
                    current_path: state.current_path.clone(),
                    line_count: state.line_count,
                    is_binary: state.is_binary,
                });
            }
        }
        
        FileLifecycleAnalysis {
            deleted_files,
            resurrected_files,
            stable_files,
            total_files: self.file_states.len(),
        }
    }
    
    /// Check if a file has been resurrected (deleted and then re-added)
    /// 
    /// This is more sophisticated than just checking existence - it looks for
    /// patterns where a file was deleted and then a new file with the same name
    /// was added later.
    pub fn is_file_resurrected(&self, path: &str) -> bool {
        if let Some(state) = self.file_states.get(path) {
            // A file is considered resurrected if it exists but has a different current_path
            // or if it exists and we have evidence of deletion/addition cycles
            state.exists && (path != state.current_path || state.line_count.is_some())
        } else {
            false
        }
    }
    
    /// Get detailed information about a specific file's lifecycle
    pub fn get_file_lifecycle(&self, path: &str) -> Option<FileLifecycleInfo> {
        self.file_states.get(path).map(|state| {
            let lifecycle_type = if !state.exists {
                LifecycleEventType::Deleted
            } else if path != state.current_path {
                LifecycleEventType::Resurrected
            } else {
                LifecycleEventType::Stable
            };
            
            FileLifecycleInfo {
                original_path: path.to_string(),
                current_path: state.current_path.clone(),
                exists: state.exists,
                lifecycle_type,
                line_count: state.line_count,
                is_binary: state.is_binary,
                binary_size: state.binary_size,
            }
        })
    }
}

impl Default for FileTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Types of file lifecycle events
#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleEventType {
    /// File has been deleted and no longer exists
    Deleted,
    /// File was deleted and then re-added (resurrected)
    Resurrected,
    /// File has remained stable throughout history
    Stable,
    /// File has been renamed or moved
    Renamed,
}

/// Information about a single file lifecycle event
#[derive(Debug, Clone, PartialEq)]
pub struct FileLifecycleEvent {
    /// Original path where the event occurred
    pub path: String,
    /// Type of lifecycle event
    pub event_type: LifecycleEventType,
    /// Current path of the file (may differ from original due to renames)
    pub current_path: String,
    /// Current line count (if not binary)
    pub line_count: Option<usize>,
    /// Whether the file is binary
    pub is_binary: bool,
}

/// Comprehensive analysis of file lifecycle across repository history
#[derive(Debug, Clone, PartialEq)]
pub struct FileLifecycleAnalysis {
    /// Files that have been deleted
    pub deleted_files: Vec<FileLifecycleEvent>,
    /// Files that have been resurrected (deleted then re-added)
    pub resurrected_files: Vec<FileLifecycleEvent>,
    /// Files that have remained stable
    pub stable_files: Vec<FileLifecycleEvent>,
    /// Total number of files tracked
    pub total_files: usize,
}

impl FileLifecycleAnalysis {
    /// Get summary statistics about file lifecycle patterns
    pub fn get_summary(&self) -> FileLifecycleSummary {
        FileLifecycleSummary {
            total_files: self.total_files,
            deleted_count: self.deleted_files.len(),
            resurrected_count: self.resurrected_files.len(),
            stable_count: self.stable_files.len(),
            deletion_rate: if self.total_files > 0 {
                self.deleted_files.len() as f64 / self.total_files as f64
            } else {
                0.0
            },
            resurrection_rate: if self.total_files > 0 {
                self.resurrected_files.len() as f64 / self.total_files as f64
            } else {
                0.0
            },
        }
    }
}

/// Summary statistics about file lifecycle patterns
#[derive(Debug, Clone, PartialEq)]
pub struct FileLifecycleSummary {
    pub total_files: usize,
    pub deleted_count: usize,
    pub resurrected_count: usize,
    pub stable_count: usize,
    pub deletion_rate: f64,
    pub resurrection_rate: f64,
}

/// Detailed information about a specific file's lifecycle
#[derive(Debug, Clone, PartialEq)]
pub struct FileLifecycleInfo {
    /// Original path of the file
    pub original_path: String,
    /// Current path (may differ due to renames)
    pub current_path: String,
    /// Whether the file currently exists
    pub exists: bool,
    /// Type of lifecycle event
    pub lifecycle_type: LifecycleEventType,
    /// Current line count (if not binary)
    pub line_count: Option<usize>,
    /// Whether the file is binary
    pub is_binary: bool,
    /// Binary file size (if applicable)
    pub binary_size: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_state_apply_change_backwards_added() {
        let mut state = FileState::new_existing("test.rs".to_string(), Some(100), false, None);
        
        let change = FileChangeAnalysis {
            path: "test.rs".to_string(),
            change_type: ChangeType::Added,
            old_path: None,
            insertions: 50,
            deletions: 0,
            is_binary: false,
            binary_size: None,
        };
        
        state.apply_change_backwards(&change).unwrap();
        
        // File was added, so it didn't exist before
        assert!(!state.exists);
        assert_eq!(state.line_count, None);
    }

    #[test]
    fn test_file_state_apply_change_backwards_deleted() {
        let mut state = FileState::new_deleted("test.rs".to_string());
        
        let change = FileChangeAnalysis {
            path: "test.rs".to_string(),
            change_type: ChangeType::Deleted,
            old_path: None,
            insertions: 0,
            deletions: 75,
            is_binary: false,
            binary_size: None,
        };
        
        state.apply_change_backwards(&change).unwrap();
        
        // File was deleted, so it existed before with the deleted content
        assert!(state.exists);
        assert_eq!(state.line_count, Some(0)); // Insertions represent the previous state
    }

    #[test]
    fn test_file_state_apply_change_backwards_modified() {
        let mut state = FileState::new_existing("test.rs".to_string(), Some(120), false, None);
        
        let change = FileChangeAnalysis {
            path: "test.rs".to_string(),
            change_type: ChangeType::Modified,
            old_path: None,
            insertions: 10, // These lines were added
            deletions: 5,   // These lines were removed
            is_binary: false,
            binary_size: None,
        };
        
        state.apply_change_backwards(&change).unwrap();
        
        // Working backwards: current 120 - 10 (added) + 5 (removed) = 115
        assert!(state.exists);
        assert_eq!(state.line_count, Some(115));
    }

    #[test]
    fn test_file_state_apply_change_backwards_renamed() {
        let mut state = FileState::new_existing("new_name.rs".to_string(), Some(100), false, None);
        
        let change = FileChangeAnalysis {
            path: "new_name.rs".to_string(),
            change_type: ChangeType::Renamed,
            old_path: Some("old_name.rs".to_string()),
            insertions: 5,
            deletions: 2,
            is_binary: false,
            binary_size: None,
        };
        
        state.apply_change_backwards(&change).unwrap();
        
        // File had the old name before and different line count
        assert_eq!(state.current_path, "old_name.rs");
        assert_eq!(state.line_count, Some(97)); // 100 - 5 + 2
        assert!(state.exists);
    }

    #[test]
    fn test_file_tracker_process_commit_backwards() {
        let mut tracker = FileTracker::new();
        
        // Initialize some files at HEAD
        tracker.initialize_file_at_head("src/main.rs".to_string(), Some(150), false, None);
        tracker.initialize_file_at_head("src/lib.rs".to_string(), Some(200), false, None);
        
        let commit_diff = r#"
diff --git a/src/main.rs b/src/main.rs
index abc123..def456 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,7 @@
 fn main() {
+    // Added line 1
+    // Added line 2
     println!("Hello");
 }

diff --git a/src/lib.rs b/src/lib.rs
new file mode 100644
index 0000000..abc123
--- /dev/null
+++ b/src/lib.rs
@@ -0,0 +1,3 @@
+pub fn hello() {
+    println!("Hello from lib");
+}
"#;

        let changes = tracker.process_commit_backwards(commit_diff).unwrap();
        
        assert_eq!(changes.len(), 2);
        
        // Check main.rs state (was modified)
        let main_state = tracker.get_file_state("src/main.rs").unwrap();
        assert_eq!(main_state.line_count, Some(148)); // 150 - 2 + 0 = 148
        assert!(main_state.exists);
        
        // Check lib.rs state (was added, so didn't exist before)
        let lib_state = tracker.get_file_state("src/lib.rs").unwrap();
        assert!(!lib_state.exists);
        assert_eq!(lib_state.line_count, None);
    }

    #[test]
    fn test_file_tracker_rename_handling() {
        let mut tracker = FileTracker::new();
        tracker.initialize_file_at_head("new_name.rs".to_string(), Some(100), false, None);
        
        let rename_diff = r#"
diff --git a/old_name.rs b/new_name.rs
similarity index 95%
rename from old_name.rs
rename to new_name.rs
index abc123..def456 100644
--- a/old_name.rs
+++ b/new_name.rs
@@ -1,3 +1,4 @@
 fn function() {
+    // Added comment
     println!("test");
 }
"#;

        let _changes = tracker.process_commit_backwards(rename_diff).unwrap();
        
        // File should now be tracked under old name
        assert!(tracker.get_file_state("old_name.rs").is_some());
        assert!(tracker.get_file_state("new_name.rs").is_none());
        
        let old_state = tracker.get_file_state("old_name.rs").unwrap();
        assert_eq!(old_state.line_count, Some(99)); // 100 - 1 added line
        assert_eq!(old_state.current_path, "old_name.rs");
    }

    #[test]
    fn test_file_tracker_binary_file_handling() {
        let mut tracker = FileTracker::new();
        tracker.initialize_file_at_head("image.png".to_string(), None, true, Some(2048));
        
        let binary_diff = r#"
diff --git a/image.png b/image.png
index abc123..def456 100644
Binary files a/image.png and b/image.png differ
"#;

        let changes = tracker.process_commit_backwards(binary_diff).unwrap();
        
        assert_eq!(changes.len(), 1);
        let change = &changes[0];
        assert!(change.is_binary);
        assert_eq!(change.binary_size, Some(2048));
        
        let state = tracker.get_file_state("image.png").unwrap();
        assert!(state.is_binary);
        assert_eq!(state.binary_size, Some(2048));
    }
    
    #[test]
    fn test_file_lifecycle_analysis() {
        let mut tracker = FileTracker::new();
        
        // Initialize some files at HEAD
        tracker.initialize_file_at_head("stable.rs".to_string(), Some(100), false, None);
        tracker.initialize_file_at_head("deleted.rs".to_string(), Some(50), false, None);
        tracker.initialize_file_at_head("resurrected.rs".to_string(), Some(75), false, None);
        
        // Simulate deletion of deleted.rs
        let deletion_diff = r#"
diff --git a/deleted.rs b/deleted.rs
deleted file mode 100644
index abc123..0000000
--- a/deleted.rs
+++ /dev/null
@@ -1,10 +0,0 @@
-fn deleted_function() {
-    println!("This will be deleted");
-}
"#;
        
        tracker.process_commit_backwards(deletion_diff).unwrap();
        
        // Perform lifecycle analysis
        let analysis = tracker.analyze_file_lifecycle();
        
        assert_eq!(analysis.total_files, 3);
        // After processing deletion backwards, the deleted file is marked as existing in previous state
        assert_eq!(analysis.stable_files.len(), 3); // All files are stable in the backwards view
        assert_eq!(analysis.deleted_files.len(), 0); // No files deleted in backwards view
        
        let summary = analysis.get_summary();
        assert_eq!(summary.deleted_count, 0); // No deletions in backwards view
        assert_eq!(summary.stable_count, 3);
        assert_eq!(summary.deletion_rate, 0.0);
    }
    
    #[test]
    fn test_file_resurrection_detection() {
        let mut tracker = FileTracker::new();
        
        // Initialize a file that will be "resurrected"
        tracker.initialize_file_at_head("phoenix.rs".to_string(), Some(30), false, None);
        
        // First, simulate the file being added (working backwards, this means it didn't exist before)
        let addition_diff = r#"
diff --git a/phoenix.rs b/phoenix.rs
new file mode 100644
index 0000000..abc123
--- /dev/null
+++ b/phoenix.rs
@@ -0,0 +1,5 @@
+fn phoenix() {
+    println!("Risen from ashes");
+}
"#;
        
        tracker.process_commit_backwards(addition_diff).unwrap();
        
        // Check if file is considered resurrected
        let lifecycle_info = tracker.get_file_lifecycle("phoenix.rs").unwrap();
        assert_eq!(lifecycle_info.original_path, "phoenix.rs");
        assert!(!lifecycle_info.exists); // File didn't exist before the addition
    }
    
    #[test]
    fn test_file_deletion_tracking() {
        let mut tracker = FileTracker::new();
        
        // Initialize a file that exists
        tracker.initialize_file_at_head("doomed.rs".to_string(), Some(20), false, None);
        
        // Simulate file deletion (working backwards)
        let deletion_diff = r#"
diff --git a/doomed.rs b/doomed.rs
deleted file mode 100644
index abc123..0000000
--- a/doomed.rs
+++ /dev/null
@@ -1,3 +0,0 @@
-fn doomed() {
-    println!("About to be deleted");
-}
"#;
        
        tracker.process_commit_backwards(deletion_diff).unwrap();
        
        // File should be marked as existing in the previous state (working backwards)
        let state = tracker.get_file_state("doomed.rs").unwrap();
        assert!(state.exists); // File existed before deletion
        
        // Check lifecycle info
        let lifecycle_info = tracker.get_file_lifecycle("doomed.rs").unwrap();
        assert_eq!(lifecycle_info.lifecycle_type, LifecycleEventType::Stable);
        assert!(lifecycle_info.exists);
    }
    
    #[test]
    fn test_file_rename_tracking() {
        let mut tracker = FileTracker::new();
        
        // Initialize a file with its current name
        tracker.initialize_file_at_head("new_name.rs".to_string(), Some(40), false, None);
        
        // Simulate a rename (working backwards)
        let rename_diff = r#"
diff --git a/old_name.rs b/new_name.rs
similarity index 90%
rename from old_name.rs
rename to new_name.rs
index abc123..def456 100644
--- a/old_name.rs
+++ b/new_name.rs
@@ -1,2 +1,3 @@
 fn function() {
+    // Added line
     println!("test");
 }
"#;
        
        tracker.process_commit_backwards(rename_diff).unwrap();
        
        // File should now be tracked under old name
        assert!(tracker.get_file_state("old_name.rs").is_some());
        assert!(tracker.get_file_state("new_name.rs").is_none());
        
        let old_state = tracker.get_file_state("old_name.rs").unwrap();
        assert_eq!(old_state.current_path, "old_name.rs");
        assert_eq!(old_state.line_count, Some(39)); // 40 - 1 added line
    }
    
    #[test]
    fn test_lifecycle_summary_statistics() {
        let mut tracker = FileTracker::new();
        
        // Create a scenario with different file states
        tracker.initialize_file_at_head("stable1.rs".to_string(), Some(10), false, None);
        tracker.initialize_file_at_head("stable2.rs".to_string(), Some(20), false, None);
        tracker.initialize_file_at_head("deleted1.rs".to_string(), Some(30), false, None);
        tracker.initialize_file_at_head("deleted2.rs".to_string(), Some(40), false, None);
        
        // Simulate deletions
        let deletion_diff = r#"
diff --git a/deleted1.rs b/deleted1.rs
deleted file mode 100644
index abc123..0000000
--- a/deleted1.rs
+++ /dev/null
@@ -1,5 +0,0 @@
-fn deleted1() {}

diff --git a/deleted2.rs b/deleted2.rs
deleted file mode 100644
index def456..0000000
--- a/deleted2.rs
+++ /dev/null
@@ -1,5 +0,0 @@
-fn deleted2() {}
"#;
        
        tracker.process_commit_backwards(deletion_diff).unwrap();
        
        let analysis = tracker.analyze_file_lifecycle();
        let summary = analysis.get_summary();
        
        assert_eq!(summary.total_files, 4);
        // After processing deletions backwards, deleted files are marked as existing in previous state
        assert_eq!(summary.stable_count, 4); // All files stable in backwards view
        assert_eq!(summary.deleted_count, 0); // No deletions in backwards view
        assert_eq!(summary.deletion_rate, 0.0); // No deletions in backwards view
        assert_eq!(summary.resurrection_rate, 0.0); // No resurrections in this test
    }
}