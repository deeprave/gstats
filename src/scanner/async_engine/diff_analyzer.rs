//! Smart Diff Analysis Module
//!
//! This module provides smart diff line analysis by parsing git diff output directly,
//! eliminating the need to apply diffs or recount lines. It counts additions and deletions
//! by parsing the + and - prefixed lines from git diff output.

use std::io::{BufRead, BufReader, Cursor};
use crate::scanner::async_engine::events::ChangeType;
use crate::scanner::async_engine::error::ScanError;

/// Result of analyzing a file change in a commit
#[derive(Debug, Clone, PartialEq)]
pub struct FileChangeAnalysis {
    /// File path
    pub path: String,
    /// Type of change (Added, Modified, Deleted, Renamed, etc.)
    pub change_type: ChangeType,
    /// Previous path for renamed files
    pub old_path: Option<String>,
    /// Number of lines added (count of + prefixed lines)
    pub insertions: usize,
    /// Number of lines deleted (count of - prefixed lines)
    pub deletions: usize,
    /// Whether the file is binary
    pub is_binary: bool,
    /// Size in bytes for binary files
    pub binary_size: Option<u64>,
}

/// Smart diff line analyzer that parses git diff output directly
pub struct DiffLineAnalyzer;

impl DiffLineAnalyzer {
    /// Analyze a single diff output string and extract file change statistics
    /// 
    /// This method parses git diff output to count:
    /// - Lines with `+` prefix (additions) - excluding diff headers like `+++`
    /// - Lines with `-` prefix (deletions) - excluding diff headers like `---`
    /// - Binary file detection from diff headers
    /// - File status changes (Added/Modified/Deleted/Renamed)
    /// 
    /// # Arguments
    /// * `diff_output` - Raw git diff output for a single file
    /// * `file_path` - Path of the file being analyzed
    /// 
    /// # Returns
    /// `FileChangeAnalysis` with accurate line counts from diff parsing
    pub fn analyze_file_diff(diff_output: &str, file_path: &str) -> Result<FileChangeAnalysis, ScanError> {
        let mut insertions = 0;
        let mut deletions = 0;
        let mut is_binary = false;
        let mut binary_size = None;
        let mut change_type = ChangeType::Modified; // Default
        let mut old_path = None;
        
        let reader = BufReader::new(Cursor::new(diff_output));
        
        for line_result in reader.lines() {
            let line = line_result.map_err(|e| ScanError::Repository(format!("Failed to read diff line: {}", e)))?;
            
            // Detect binary files
            if line.contains("Binary files") || line.contains("GIT binary patch") {
                is_binary = true;
                binary_size = Some(0); // Placeholder - actual size would come from git show
                continue;
            }
            
            // Detect file status from diff headers
            if line.starts_with("new file mode") {
                change_type = ChangeType::Added;
            } else if line.starts_with("deleted file mode") {
                change_type = ChangeType::Deleted;
            } else if line.starts_with("rename from") {
                change_type = ChangeType::Renamed;
                // Extract old path from "rename from old/path"
                if let Some(path) = line.strip_prefix("rename from ") {
                    old_path = Some(path.to_string());
                }
            } else if line.starts_with("copy from") {
                change_type = ChangeType::Copied;
                if let Some(path) = line.strip_prefix("copy from ") {
                    old_path = Some(path.to_string());
                }
            }
            
            // Count actual content changes (skip diff headers)
            if line.starts_with('+') && !line.starts_with("+++") {
                insertions += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                deletions += 1;
            }
            
            // Try to extract binary file size from diff output
            if is_binary && line.contains("differ") {
                // Binary files a/file and b/file differ
                // TODO: Extract actual file size if available in diff output
                binary_size = Some(0); // Placeholder - actual size would come from git show
            }
        }
        
        Ok(FileChangeAnalysis {
            path: file_path.to_string(),
            change_type,
            old_path,
            insertions,
            deletions,
            is_binary,
            binary_size,
        })
    }
    
    /// Analyze multiple file changes from a commit diff output
    /// 
    /// This method splits a multi-file diff output into individual file diffs
    /// and analyzes each one separately.
    /// 
    /// # Arguments
    /// * `commit_diff_output` - Complete git diff output for a commit
    /// 
    /// # Returns
    /// Vector of `FileChangeAnalysis` for all files changed in the commit
    pub fn analyze_commit_diff(commit_diff_output: &str) -> Result<Vec<FileChangeAnalysis>, ScanError> {
        let mut file_changes = Vec::new();
        let mut current_file_diff = String::new();
        let mut current_file_path: Option<String> = None;
        
        let reader = BufReader::new(Cursor::new(commit_diff_output));
        
        for line_result in reader.lines() {
            let line = line_result.map_err(|e| ScanError::Repository(format!("Failed to read commit diff line: {}", e)))?;
            
            // Detect start of new file diff
            if line.starts_with("diff --git") {
                // Process previous file if any
                if let Some(file_path) = current_file_path.take() {
                    let analysis = Self::analyze_file_diff(&current_file_diff, &file_path)?;
                    file_changes.push(analysis);
                    current_file_diff.clear();
                }
                
                // Extract file path from "diff --git a/path b/path"
                if let Some(path) = Self::extract_file_path_from_diff_header(&line) {
                    current_file_path = Some(path);
                }
            }
            
            // Accumulate lines for current file
            if current_file_path.is_some() {
                current_file_diff.push_str(&line);
                current_file_diff.push('\n');
            }
        }
        
        // Process last file
        if let Some(file_path) = current_file_path {
            let analysis = Self::analyze_file_diff(&current_file_diff, &file_path)?;
            file_changes.push(analysis);
        }
        
        Ok(file_changes)
    }
    
    /// Extract file path from git diff header line
    /// 
    /// Parses lines like:
    /// - `diff --git a/src/main.rs b/src/main.rs`
    /// - `diff --git a/old/path b/new/path` (for renames)
    /// 
    /// Returns the "b/" path (destination path) which represents the current file path
    fn extract_file_path_from_diff_header(diff_header: &str) -> Option<String> {
        // Split by whitespace and find b/ prefix
        let parts: Vec<&str> = diff_header.split_whitespace().collect();
        
        for part in parts {
            if let Some(path) = part.strip_prefix("b/") {
                return Some(path.to_string());
            }
        }
        
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_simple_text_file_diff() {
        let diff_output = r#"
diff --git a/src/main.rs b/src/main.rs
index abc123..def456 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,7 @@
 fn main() {
-    println!("Hello, world!");
+    println!("Hello, Rust!");
+    println!("This is a new line");
 }
+// Added comment
"#;

        let analysis = DiffLineAnalyzer::analyze_file_diff(diff_output, "src/main.rs").unwrap();
        
        assert_eq!(analysis.path, "src/main.rs");
        assert_eq!(analysis.change_type, ChangeType::Modified);
        assert_eq!(analysis.old_path, None);
        assert_eq!(analysis.insertions, 3); // +Hello Rust, +new line, +comment
        assert_eq!(analysis.deletions, 1);  // -Hello world
        assert!(!analysis.is_binary);
        assert_eq!(analysis.binary_size, None);
    }

    #[test]
    fn test_analyze_new_file_diff() {
        let diff_output = r#"
diff --git a/src/lib.rs b/src/lib.rs
new file mode 100644
index 0000000..abc123
--- /dev/null
+++ b/src/lib.rs
@@ -0,0 +1,3 @@
+pub fn hello() {
+    println!("Hello from lib!");
+}
"#;

        let analysis = DiffLineAnalyzer::analyze_file_diff(diff_output, "src/lib.rs").unwrap();
        
        assert_eq!(analysis.change_type, ChangeType::Added);
        assert_eq!(analysis.insertions, 3);
        assert_eq!(analysis.deletions, 0);
    }

    #[test]
    fn test_analyze_deleted_file_diff() {
        let diff_output = r#"
diff --git a/src/old.rs b/src/old.rs
deleted file mode 100644
index abc123..0000000
--- a/src/old.rs
+++ /dev/null
@@ -1,2 +0,0 @@
-fn old_function() {
-}
"#;

        let analysis = DiffLineAnalyzer::analyze_file_diff(diff_output, "src/old.rs").unwrap();
        
        assert_eq!(analysis.change_type, ChangeType::Deleted);
        assert_eq!(analysis.insertions, 0);
        assert_eq!(analysis.deletions, 2);
    }

    #[test]
    fn test_analyze_renamed_file_diff() {
        let diff_output = r#"
diff --git a/src/old_name.rs b/src/new_name.rs
similarity index 90%
rename from src/old_name.rs
rename to src/new_name.rs
index abc123..def456 100644
--- a/src/old_name.rs
+++ b/src/new_name.rs
@@ -1,3 +1,4 @@
 fn function() {
+    // Added line
     println!("Hello");
 }
"#;

        let analysis = DiffLineAnalyzer::analyze_file_diff(diff_output, "src/new_name.rs").unwrap();
        
        assert_eq!(analysis.change_type, ChangeType::Renamed);
        assert_eq!(analysis.old_path, Some("src/old_name.rs".to_string()));
        assert_eq!(analysis.insertions, 1);
        assert_eq!(analysis.deletions, 0);
    }

    #[test]
    fn test_analyze_binary_file_diff() {
        let diff_output = r#"
diff --git a/assets/image.png b/assets/image.png
new file mode 100644
index 0000000..abc123
Binary files /dev/null and b/assets/image.png differ
"#;

        let analysis = DiffLineAnalyzer::analyze_file_diff(diff_output, "assets/image.png").unwrap();
        
        assert_eq!(analysis.change_type, ChangeType::Added);
        assert!(analysis.is_binary);
        assert_eq!(analysis.insertions, 0); // Binary files have no line changes
        assert_eq!(analysis.deletions, 0);
        assert_eq!(analysis.binary_size, Some(0)); // Placeholder - would need git show for real size
    }

    #[test]
    fn test_extract_file_path_from_diff_header() {
        let header = "diff --git a/src/main.rs b/src/main.rs";
        let path = DiffLineAnalyzer::extract_file_path_from_diff_header(header);
        assert_eq!(path, Some("src/main.rs".to_string()));
        
        let rename_header = "diff --git a/old/path.rs b/new/path.rs";
        let rename_path = DiffLineAnalyzer::extract_file_path_from_diff_header(rename_header);
        assert_eq!(rename_path, Some("new/path.rs".to_string()));
    }

    #[test]
    fn test_analyze_commit_diff_multiple_files() {
        let commit_diff = r#"
diff --git a/src/main.rs b/src/main.rs
index abc123..def456 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1 +1,2 @@
 fn main() {}
+// Comment

diff --git a/src/lib.rs b/src/lib.rs
new file mode 100644
index 0000000..abc123
--- /dev/null
+++ b/src/lib.rs
@@ -0,0 +1,1 @@
+pub fn lib() {}
"#;

        let analyses = DiffLineAnalyzer::analyze_commit_diff(commit_diff).unwrap();
        
        assert_eq!(analyses.len(), 2);
        
        let main_analysis = &analyses[0];
        assert_eq!(main_analysis.path, "src/main.rs");
        assert_eq!(main_analysis.insertions, 1);
        assert_eq!(main_analysis.deletions, 0);
        
        let lib_analysis = &analyses[1];
        assert_eq!(lib_analysis.path, "src/lib.rs");
        assert_eq!(lib_analysis.change_type, ChangeType::Added);
        assert_eq!(lib_analysis.insertions, 1);
        assert_eq!(lib_analysis.deletions, 0);
    }
}