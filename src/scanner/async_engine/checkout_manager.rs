//! Checkout Manager for Plugin Data Requirements
//! 
//! The CheckoutManager provides on-demand file checkout capabilities for plugins
//! that require access to actual file content during scanning. It implements a
//! smart checkout system that only creates temporary files when plugins explicitly
//! require them, optimizing both performance and disk usage.
//!
//! ## Key Features
//!
//! - **Conditional Checkout**: Only creates temporary files when plugins require file content
//! - **Commit-Scoped Directories**: Organizes checkouts by commit hash for isolation
//! - **Binary File Support**: Handles both text and binary files seamlessly
//! - **Automatic Cleanup**: Implements Drop trait for guaranteed cleanup
//! - **Memory Efficient**: Avoids unnecessary disk operations when possible
//!
//! ## Architecture
//!
//! ```text
//! CheckoutManager
//! ├── base_checkout_dir/
//! │   ├── commit_abc123de/         (8-char commit prefix)
//! │   │   ├── src/main.rs
//! │   │   ├── Cargo.toml
//! │   │   └── docs/README.md
//! │   ├── commit_def456ab/
//! │   │   └── modified_files...
//! │   └── ...
//! ├── checkout_dirs (HashMap)      (Tracks active checkouts)
//! └── checkout_required (bool)     (Plugin requirements analysis)
//! ```
//!
//! ## Usage Pattern
//!
//! ```rust,no_run
//! use std::path::Path;
//! use gstats::plugin::traits::PluginDataRequirements;
//! use gstats::scanner::async_engine::checkout_manager::CheckoutManager;
//! 
//! // Create manager based on plugin requirements
//! let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![]; // Plugin list here
//! let temp_dir = Path::new("/tmp/checkout");
//! let mut checkout_manager = CheckoutManager::new(&temp_dir, &plugins)?;
//!
//! if checkout_manager.is_checkout_required() {
//!     // Prepare commit-specific checkout directory
//!     let checkout_path = checkout_manager.prepare_commit_checkout("abc123")?;
//!     
//!     // Checkout specific files as needed
//!     let file_content = b"fn main() {}";
//!     let file_path = checkout_manager.checkout_file(
//!         "abc123", 
//!         "src/main.rs", 
//!         file_content
//!     )?;
//!     
//!     // Cleanup when done
//!     checkout_manager.cleanup_commit("abc123")?;
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Integration with Scanner
//!
//! The CheckoutManager integrates with the scanner's diff processing to provide
//! file content only when plugins require it:
//!
//! - **Metadata-only plugins**: No checkout operations performed
//! - **Content-requiring plugins**: Files are checked out on-demand
//! - **Binary-aware plugins**: Binary files are handled appropriately
//! - **Size-limited plugins**: Large files are skipped based on limits

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::Write;
use crate::scanner::async_engine::error::ScanError;
use crate::plugin::traits::PluginDataRequirements;

/// Manages checkout directories and file content for plugins that require file access
pub struct CheckoutManager {
    /// Base directory for all checkouts
    base_checkout_dir: PathBuf,
    /// Map of commit hash to checkout directory
    checkout_dirs: HashMap<String, PathBuf>,
    /// Whether any plugins require file checkout
    checkout_required: bool,
}

impl CheckoutManager {
    /// Create a new CheckoutManager
    /// 
    /// # Arguments
    /// * `base_dir` - Base directory for creating checkout directories
    /// * `plugins` - Vector of plugins to check for file access requirements
    pub fn new<P: AsRef<Path>>(base_dir: P, plugins: &[Box<dyn PluginDataRequirements>]) -> Result<Self, ScanError> {
        let base_checkout_dir = base_dir.as_ref().to_path_buf();
        
        // Check if any plugins require file checkout
        let checkout_required = plugins.iter().any(|plugin| 
            plugin.requires_current_file_content() || plugin.requires_historical_file_content());
        
        // Only create base directory if checkout is required
        if checkout_required {
            fs::create_dir_all(&base_checkout_dir)
                .map_err(|e| ScanError::Repository(format!("Failed to create checkout directory {}: {}", base_checkout_dir.display(), e)))?;
        }
        
        Ok(Self {
            base_checkout_dir,
            checkout_dirs: HashMap::new(),
            checkout_required,
        })
    }
    
    /// Check if checkout is required (any plugins need file access)
    pub fn is_checkout_required(&self) -> bool {
        self.checkout_required
    }
    
    /// Prepare a checkout directory for a specific commit
    /// 
    /// This method only creates directories if checkout is required.
    /// Returns the path to the checkout directory for the commit.
    /// 
    /// # Arguments
    /// * `commit_hash` - The hash of the commit to prepare checkout for
    /// 
    /// # Returns
    /// Path to the checkout directory for this commit, or None if checkout not required
    pub fn prepare_commit_checkout(&mut self, commit_hash: &str) -> Result<Option<PathBuf>, ScanError> {
        if !self.checkout_required {
            return Ok(None);
        }
        
        let commit_short = if commit_hash.len() >= 8 { &commit_hash[..8] } else { commit_hash };
        let commit_dir = self.base_checkout_dir.join(format!("commit_{commit_short}"));
        
        // Create the directory structure
        fs::create_dir_all(&commit_dir)
            .map_err(|e| ScanError::Repository(format!("Failed to create commit checkout directory {}: {}", commit_dir.display(), e)))?;
        
        self.checkout_dirs.insert(commit_hash.to_string(), commit_dir.clone());
        Ok(Some(commit_dir))
    }
    
    /// Write file content to the checkout directory for a specific commit
    /// 
    /// This method only writes files if checkout is required.
    /// 
    /// # Arguments
    /// * `commit_hash` - The hash of the commit
    /// * `file_path` - Relative path of the file within the repository
    /// * `content` - Content of the file as bytes
    /// 
    /// # Returns
    /// Path to the checked out file, or None if checkout not required
    pub fn checkout_file(&mut self, commit_hash: &str, file_path: &str, content: &[u8]) -> Result<Option<PathBuf>, ScanError> {
        if !self.checkout_required {
            return Ok(None);
        }
        
        let commit_dir = self.checkout_dirs.get(commit_hash)
            .ok_or_else(|| ScanError::Repository(format!("No checkout directory prepared for commit {commit_hash}")))?;
        
        let file_checkout_path = commit_dir.join(file_path);
        
        // Create parent directories if they don't exist
        if let Some(parent_dir) = file_checkout_path.parent() {
            fs::create_dir_all(parent_dir)
                .map_err(|e| ScanError::Repository(format!("Failed to create parent directory {}: {}", parent_dir.display(), e)))?;
        }
        
        // Write the file content
        let mut file = File::create(&file_checkout_path)
            .map_err(|e| ScanError::Repository(format!("Failed to create file {}: {}", file_checkout_path.display(), e)))?;
        
        file.write_all(content)
            .map_err(|e| ScanError::Repository(format!("Failed to write file content {}: {}", file_checkout_path.display(), e)))?;
        
        Ok(Some(file_checkout_path))
    }
    
    /// Get the checkout path for a specific file in a commit
    /// 
    /// Returns None if checkout is not required or the file hasn't been checked out.
    /// 
    /// # Arguments
    /// * `commit_hash` - The hash of the commit
    /// * `file_path` - Relative path of the file within the repository
    pub fn get_checkout_path(&self, commit_hash: &str, file_path: &str) -> Option<PathBuf> {
        if !self.checkout_required {
            return None;
        }
        
        self.checkout_dirs.get(commit_hash)
            .map(|commit_dir| commit_dir.join(file_path))
    }
    
    /// Clean up checkout directory for a specific commit
    /// 
    /// This should be called when processing of a commit is complete
    /// to avoid excessive disk usage.
    /// 
    /// # Arguments
    /// * `commit_hash` - The hash of the commit to clean up
    pub fn cleanup_commit(&mut self, commit_hash: &str) -> Result<(), ScanError> {
        if !self.checkout_required {
            return Ok(());
        }
        
        if let Some(commit_dir) = self.checkout_dirs.remove(commit_hash) {
            if commit_dir.exists() {
                fs::remove_dir_all(&commit_dir)
                    .map_err(|e| ScanError::Repository(format!("Failed to cleanup commit directory {}: {}", commit_dir.display(), e)))?;
            }
        }
        
        Ok(())
    }
    
    /// Clean up all checkout directories
    /// 
    /// This should be called when scanning is complete.
    pub fn cleanup_all(&mut self) -> Result<(), ScanError> {
        if !self.checkout_required {
            return Ok(());
        }
        
        // Clean up individual commit directories
        let commit_hashes: Vec<String> = self.checkout_dirs.keys().cloned().collect();
        for commit_hash in commit_hashes {
            self.cleanup_commit(&commit_hash)?;
        }
        
        // Remove the base checkout directory if it exists and is empty
        if self.base_checkout_dir.exists() {
            if let Ok(entries) = fs::read_dir(&self.base_checkout_dir) {
                if entries.count() == 0 {
                    fs::remove_dir(&self.base_checkout_dir)
                        .map_err(|e| ScanError::Repository(format!("Failed to cleanup base checkout directory {}: {}", self.base_checkout_dir.display(), e)))?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Get statistics about checkout usage
    pub fn get_stats(&self) -> CheckoutStats {
        CheckoutStats {
            checkout_required: self.checkout_required,
            active_commits: self.checkout_dirs.len(),
            base_directory: self.base_checkout_dir.clone(),
        }
    }
}

/// Statistics about checkout manager usage
#[derive(Debug, Clone)]
pub struct CheckoutStats {
    /// Whether checkout is required by any plugins
    pub checkout_required: bool,
    /// Number of commits with active checkout directories
    pub active_commits: usize,
    /// Base directory for checkouts
    pub base_directory: PathBuf,
}

impl Drop for CheckoutManager {
    /// Automatically clean up on drop to prevent leaving temporary files
    fn drop(&mut self) {
        if let Err(e) = self.cleanup_all() {
            eprintln!("Warning: Failed to cleanup CheckoutManager: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::plugin::traits::PluginDataRequirements;
    
    // Mock plugin that requires file checkout
    struct MockFileRequiringPlugin;
    
    impl PluginDataRequirements for MockFileRequiringPlugin {
        fn requires_current_file_content(&self) -> bool {
            true
        }
        
        fn requires_historical_file_content(&self) -> bool {
            false
        }
    }
    
    // Mock plugin that doesn't require file checkout
    struct MockNoFilePlugin;
    
    impl PluginDataRequirements for MockNoFilePlugin {
        fn requires_current_file_content(&self) -> bool {
            false
        }
        
        fn requires_historical_file_content(&self) -> bool {
            false
        }
    }
    
    #[test]
    fn test_checkout_manager_no_plugins_requiring_files() {
        let temp_dir = TempDir::new().unwrap();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockNoFilePlugin),
        ];
        
        let checkout_manager = CheckoutManager::new(temp_dir.path(), &plugins).unwrap();
        
        assert!(!checkout_manager.is_checkout_required());
        assert_eq!(checkout_manager.get_stats().active_commits, 0);
    }
    
    #[test]
    fn test_checkout_manager_with_file_requiring_plugins() {
        let temp_dir = TempDir::new().unwrap();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockFileRequiringPlugin),
            Box::new(MockNoFilePlugin),
        ];
        
        let checkout_manager = CheckoutManager::new(temp_dir.path(), &plugins).unwrap();
        
        assert!(checkout_manager.is_checkout_required());
        assert!(temp_dir.path().exists()); // Base directory should be created
    }
    
    #[test]
    fn test_commit_checkout_preparation() {
        let temp_dir = TempDir::new().unwrap();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockFileRequiringPlugin),
        ];
        
        let mut checkout_manager = CheckoutManager::new(temp_dir.path(), &plugins).unwrap();
        
        let commit_hash = "abcdef123456";
        let checkout_path = checkout_manager.prepare_commit_checkout(commit_hash).unwrap();
        
        assert!(checkout_path.is_some());
        let checkout_path = checkout_path.unwrap();
        assert!(checkout_path.exists());
        assert!(checkout_path.to_string_lossy().contains("commit_abcdef12"));
    }
    
    #[test]
    fn test_file_checkout() {
        let temp_dir = TempDir::new().unwrap();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockFileRequiringPlugin),
        ];
        
        let mut checkout_manager = CheckoutManager::new(temp_dir.path(), &plugins).unwrap();
        
        let commit_hash = "abcdef123456";
        checkout_manager.prepare_commit_checkout(commit_hash).unwrap();
        
        let file_content = b"fn main() { println!(\"Hello\"); }";
        let file_path = "src/main.rs";
        
        let checkout_file_path = checkout_manager.checkout_file(commit_hash, file_path, file_content).unwrap();
        
        assert!(checkout_file_path.is_some());
        let checkout_file_path = checkout_file_path.unwrap();
        assert!(checkout_file_path.exists());
        
        let read_content = fs::read(&checkout_file_path).unwrap();
        assert_eq!(read_content, file_content);
    }
    
    #[test]
    fn test_checkout_path_retrieval() {
        let temp_dir = TempDir::new().unwrap();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockFileRequiringPlugin),
        ];
        
        let mut checkout_manager = CheckoutManager::new(temp_dir.path(), &plugins).unwrap();
        
        let commit_hash = "abcdef123456";
        checkout_manager.prepare_commit_checkout(commit_hash).unwrap();
        
        let file_path = "src/lib.rs";
        let checkout_path = checkout_manager.get_checkout_path(commit_hash, file_path);
        
        assert!(checkout_path.is_some());
        let checkout_path = checkout_path.unwrap();
        assert!(checkout_path.to_string_lossy().contains("src/lib.rs"));
    }
    
    #[test]
    fn test_commit_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockFileRequiringPlugin),
        ];
        
        let mut checkout_manager = CheckoutManager::new(temp_dir.path(), &plugins).unwrap();
        
        let commit_hash = "abcdef123456";
        let checkout_path = checkout_manager.prepare_commit_checkout(commit_hash).unwrap().unwrap();
        
        assert!(checkout_path.exists());
        assert_eq!(checkout_manager.get_stats().active_commits, 1);
        
        checkout_manager.cleanup_commit(commit_hash).unwrap();
        
        assert!(!checkout_path.exists());
        assert_eq!(checkout_manager.get_stats().active_commits, 0);
    }
    
    #[test]
    fn test_no_checkout_when_not_required() {
        let temp_dir = TempDir::new().unwrap();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockNoFilePlugin),
        ];
        
        let mut checkout_manager = CheckoutManager::new(temp_dir.path(), &plugins).unwrap();
        
        let commit_hash = "abcdef123456";
        let checkout_path = checkout_manager.prepare_commit_checkout(commit_hash).unwrap();
        
        assert!(checkout_path.is_none());
        
        let file_path = "src/main.rs";
        let file_content = b"fn main() {}";
        let checkout_file_path = checkout_manager.checkout_file(commit_hash, file_path, file_content).unwrap();
        
        assert!(checkout_file_path.is_none());
    }
    
    #[test]
    fn test_cleanup_all() {
        let temp_dir = TempDir::new().unwrap();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockFileRequiringPlugin),
        ];
        
        let mut checkout_manager = CheckoutManager::new(temp_dir.path(), &plugins).unwrap();
        
        // Create multiple commit checkouts
        let commit1 = "abc123";
        let commit2 = "def456";
        
        let path1 = checkout_manager.prepare_commit_checkout(commit1).unwrap().unwrap();
        let path2 = checkout_manager.prepare_commit_checkout(commit2).unwrap().unwrap();
        
        assert!(path1.exists());
        assert!(path2.exists());
        assert_eq!(checkout_manager.get_stats().active_commits, 2);
        
        checkout_manager.cleanup_all().unwrap();
        
        assert!(!path1.exists());
        assert!(!path2.exists());
        assert_eq!(checkout_manager.get_stats().active_commits, 0);
    }
}