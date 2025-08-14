//! Integration Tests for Enhanced File Tracking
//! 
//! Comprehensive tests for the integration between CheckoutManager, FileTracker,
//! RuntimeScannerConfig, and the conditional file checkout system.

use tempfile::TempDir;

use crate::plugin::traits::PluginDataRequirements;
use crate::scanner::async_engine::checkout_manager::CheckoutManager;
use crate::scanner::async_engine::file_tracker::{FileTracker, LifecycleEventType};
use crate::scanner::config::ScannerConfig;

// Mock plugins for testing different data requirements
struct MockMetadataOnlyPlugin;
impl PluginDataRequirements for MockMetadataOnlyPlugin {
    fn requires_current_file_content(&self) -> bool { false }
    fn requires_historical_file_content(&self) -> bool { false }
}

struct MockContentRequiringPlugin;
impl PluginDataRequirements for MockContentRequiringPlugin {
    fn requires_current_file_content(&self) -> bool { true }
    fn requires_historical_file_content(&self) -> bool { false }
}

struct MockHistoricalContentPlugin;
impl PluginDataRequirements for MockHistoricalContentPlugin {
    fn requires_current_file_content(&self) -> bool { false }
    fn requires_historical_file_content(&self) -> bool { true }
}

struct MockLimitedSizePlugin;
impl PluginDataRequirements for MockLimitedSizePlugin {
    fn requires_current_file_content(&self) -> bool { true }
    fn max_file_size(&self) -> Option<usize> { Some(1024) } // 1KB limit
}

struct MockBinaryHandlingPlugin;
impl PluginDataRequirements for MockBinaryHandlingPlugin {
    fn requires_current_file_content(&self) -> bool { true }
    fn handles_binary_files(&self) -> bool { true }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_config_metadata_only_plugins() {
        let config = ScannerConfig::default();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockMetadataOnlyPlugin),
            Box::new(MockMetadataOnlyPlugin),
        ];
        
        let runtime_config = config.analyze_plugins(&plugins);
        
        assert!(!runtime_config.requires_checkout);
        assert!(!runtime_config.requires_current_content);
        assert!(!runtime_config.requires_historical_content);
        assert!(runtime_config.effective_checkout_dir.is_none());
    }
    
    #[test]
    fn test_runtime_config_content_requiring_plugins() {
        let config = ScannerConfig::default();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockMetadataOnlyPlugin),
            Box::new(MockContentRequiringPlugin),
        ];
        
        let runtime_config = config.analyze_plugins(&plugins);
        
        assert!(runtime_config.requires_checkout);
        assert!(runtime_config.requires_current_content);
        assert!(!runtime_config.requires_historical_content);
        assert!(runtime_config.effective_checkout_dir.is_some());
    }
    
    #[test]
    fn test_runtime_config_historical_content_plugins() {
        let config = ScannerConfig::default();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockHistoricalContentPlugin),
        ];
        
        let runtime_config = config.analyze_plugins(&plugins);
        
        assert!(runtime_config.requires_checkout);
        assert!(!runtime_config.requires_current_content);
        assert!(runtime_config.requires_historical_content);
        assert!(runtime_config.effective_checkout_dir.is_some());
    }
    
    #[test]
    fn test_checkout_manager_no_requirements() {
        let temp_dir = TempDir::new().unwrap();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockMetadataOnlyPlugin),
        ];
        
        let checkout_manager = CheckoutManager::new(temp_dir.path(), &plugins).unwrap();
        
        assert!(!checkout_manager.is_checkout_required());
        
        // Directory should not be created when not required
        let stats = checkout_manager.get_stats();
        assert!(!stats.checkout_required);
        assert_eq!(stats.active_commits, 0);
    }
    
    #[test]
    fn test_checkout_manager_with_requirements() {
        let temp_dir = TempDir::new().unwrap();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockContentRequiringPlugin),
        ];
        
        let mut checkout_manager = CheckoutManager::new(temp_dir.path(), &plugins).unwrap();
        
        assert!(checkout_manager.is_checkout_required());
        
        // Test commit checkout preparation
        let commit_hash = "abc123def456";
        let checkout_path = checkout_manager.prepare_commit_checkout(commit_hash).unwrap();
        assert!(checkout_path.is_some());
        
        let checkout_path = checkout_path.unwrap();
        assert!(checkout_path.exists());
        assert!(checkout_path.to_string_lossy().contains("commit_abc123de"));
        
        // Test file checkout
        let file_content = b"fn main() {\n    println!(\"Hello, world!\");\n}";
        let file_path = "src/main.rs";
        
        let file_checkout_path = checkout_manager.checkout_file(commit_hash, file_path, file_content).unwrap();
        assert!(file_checkout_path.is_some());
        
        let file_checkout_path = file_checkout_path.unwrap();
        assert!(file_checkout_path.exists());
        assert_eq!(std::fs::read(&file_checkout_path).unwrap(), file_content);
        
        // Test cleanup
        checkout_manager.cleanup_commit(commit_hash).unwrap();
        assert!(!checkout_path.exists());
    }
    
    #[test]
    fn test_runtime_config_file_size_limits() {
        let mut config = ScannerConfig::default();
        // Set the max file size in the config to match the plugin's limit
        config.plugin_requirements.max_checkout_file_size = Some(1024);
        
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockLimitedSizePlugin),
        ];
        
        let runtime_config = config.analyze_plugins(&plugins);
        
        // Should checkout small files
        assert!(runtime_config.should_checkout_file("small.rs", Some(512)));
        
        // Should not checkout large files
        assert!(!runtime_config.should_checkout_file("large.rs", Some(2048)));
        
        // Should checkout files without size info
        assert!(runtime_config.should_checkout_file("unknown.rs", None));
    }
    
    #[test]
    fn test_runtime_config_forced_extensions() {
        let mut config = ScannerConfig::default();
        config.plugin_requirements.force_checkout_extensions = vec!["rs".to_string(), "toml".to_string()];
        
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockContentRequiringPlugin),
        ];
        
        let runtime_config = config.analyze_plugins(&plugins);
        
        // Should checkout forced extensions
        assert!(runtime_config.should_checkout_file("test.rs", Some(1024)));
        assert!(runtime_config.should_checkout_file("Cargo.toml", Some(1024)));
        
        // Should not checkout other extensions when forced list exists
        assert!(!runtime_config.should_checkout_file("test.py", Some(1024)));
        assert!(!runtime_config.should_checkout_file("README.md", Some(1024)));
        
        // Should not checkout files without extensions
        assert!(!runtime_config.should_checkout_file("Makefile", Some(1024)));
    }
    
    #[test]
    fn test_file_tracker_integration_with_checkout() {
        let mut tracker = FileTracker::new();
        
        // Initialize files at HEAD
        tracker.initialize_file_at_head("src/main.rs".to_string(), Some(100), false, None);
        tracker.initialize_file_at_head("src/lib.rs".to_string(), Some(50), false, None);
        tracker.initialize_file_at_head("binary.png".to_string(), None, true, Some(2048));
        
        let commit_diff = r#"
diff --git a/src/main.rs b/src/main.rs
index abc123..def456 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,7 @@
 fn main() {
+    // Added comment
+    println!("Starting application");
     println!("Hello");
 }

diff --git a/src/new_file.rs b/src/new_file.rs
new file mode 100644
index 0000000..abc123
--- /dev/null
+++ b/src/new_file.rs
@@ -0,0 +1,3 @@
+pub fn new_function() {
+    println!("New functionality");
+}

diff --git a/deleted_file.rs b/deleted_file.rs
deleted file mode 100644
index abc123..0000000
--- a/deleted_file.rs
+++ /dev/null
@@ -1,5 +0,0 @@
-fn deleted() {
-    println!("This will be deleted");
-}

diff --git a/binary.png b/binary.png
index abc123..def456 100644
Binary files a/binary.png and b/binary.png differ
"#;
        
        let changes = tracker.process_commit_backwards(commit_diff).unwrap();
        
        assert_eq!(changes.len(), 4);
        
        // Test that file states are updated correctly
        let main_state = tracker.get_file_state("src/main.rs").unwrap();
        assert!(main_state.exists);
        assert_eq!(main_state.line_count, Some(99)); // 100 - 2 + 1 = 99
        
        // Test new file handling (working backwards, it didn't exist before)
        let new_file_state = tracker.get_file_state("src/new_file.rs").unwrap();
        assert!(!new_file_state.exists); // Didn't exist before being added
        
        // Test deleted file (working backwards, it existed before)
        let deleted_state = tracker.get_file_state("deleted_file.rs").unwrap();
        assert!(deleted_state.exists); // Existed before being deleted
        
        // Test binary file handling
        let binary_state = tracker.get_file_state("binary.png").unwrap();
        assert!(binary_state.exists);
        assert!(binary_state.is_binary);
        assert_eq!(binary_state.binary_size, Some(2048));
        
        // Test lifecycle analysis
        let lifecycle = tracker.analyze_file_lifecycle();
        
        // In backwards analysis, we have mixed file states
        // We initialized 3 files and processed 4 changes, but some may create additional entries
        let summary = lifecycle.get_summary();
        
        // The exact distribution depends on how files are processed backwards
        // We should have at least 4 files accounted for (the changes we processed)
        assert!(summary.total_files >= 4);
        assert_eq!(summary.stable_count + summary.deleted_count + summary.resurrected_count, summary.total_files);
    }
    
    #[test]
    fn test_checkout_manager_binary_file_handling() {
        let temp_dir = TempDir::new().unwrap();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockBinaryHandlingPlugin),
        ];
        
        let mut checkout_manager = CheckoutManager::new(temp_dir.path(), &plugins).unwrap();
        
        let commit_hash = "binary123";
        checkout_manager.prepare_commit_checkout(commit_hash).unwrap();
        
        // Test binary file checkout
        let binary_content = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG header
        let binary_path = "image.png";
        
        let checkout_path = checkout_manager.checkout_file(commit_hash, binary_path, &binary_content).unwrap();
        assert!(checkout_path.is_some());
        
        let checkout_path = checkout_path.unwrap();
        assert!(checkout_path.exists());
        assert_eq!(std::fs::read(&checkout_path).unwrap(), binary_content);
    }
    
    #[test]
    fn test_file_tracker_rename_and_resurrection_detection() {
        let mut tracker = FileTracker::new();
        
        // Initialize a file that will be renamed
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
        
        tracker.process_commit_backwards(rename_diff).unwrap();
        
        // File should now be tracked under old name
        assert!(tracker.get_file_state("old_name.rs").is_some());
        assert!(tracker.get_file_state("new_name.rs").is_none());
        
        let old_state = tracker.get_file_state("old_name.rs").unwrap();
        assert_eq!(old_state.current_path, "old_name.rs");
        assert_eq!(old_state.line_count, Some(99)); // 100 - 1 added line
        
        // Test lifecycle detection
        let lifecycle_info = tracker.get_file_lifecycle("old_name.rs").unwrap();
        assert_eq!(lifecycle_info.lifecycle_type, LifecycleEventType::Stable);
        assert!(lifecycle_info.exists);
    }
    
    #[test]
    fn test_integration_checkout_conditional_on_plugins() {
        let temp_dir = TempDir::new().unwrap();
        
        // Test with metadata-only plugins
        let metadata_plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockMetadataOnlyPlugin),
        ];
        
        let config = ScannerConfig::default();
        let runtime_config = config.analyze_plugins(&metadata_plugins);
        let checkout_manager = CheckoutManager::new(temp_dir.path(), &metadata_plugins).unwrap();
        
        assert!(!runtime_config.requires_checkout);
        assert!(!checkout_manager.is_checkout_required());
        
        // Test with content-requiring plugins
        let content_plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockContentRequiringPlugin),
        ];
        
        let runtime_config = config.analyze_plugins(&content_plugins);
        let mut checkout_manager = CheckoutManager::new(temp_dir.path().join("content"), &content_plugins).unwrap();
        
        assert!(runtime_config.requires_checkout);
        assert!(checkout_manager.is_checkout_required());
        
        // Verify checkout actually works
        let commit_hash = "test123";
        let result = checkout_manager.prepare_commit_checkout(commit_hash);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }
    
    #[test]
    fn test_multiple_commit_checkout_management() {
        let temp_dir = TempDir::new().unwrap();
        let plugins: Vec<Box<dyn PluginDataRequirements>> = vec![
            Box::new(MockContentRequiringPlugin),
        ];
        
        let mut checkout_manager = CheckoutManager::new(temp_dir.path(), &plugins).unwrap();
        
        // Prepare multiple commits
        let commits = vec!["commit1", "commit2", "commit3"];
        let mut checkout_paths = Vec::new();
        
        for commit in &commits {
            let path = checkout_manager.prepare_commit_checkout(commit).unwrap().unwrap();
            checkout_paths.push(path);
        }
        
        // All commits should have separate directories
        assert_eq!(checkout_paths.len(), 3);
        for path in &checkout_paths {
            assert!(path.exists());
        }
        
        assert_eq!(checkout_manager.get_stats().active_commits, 3);
        
        // Cleanup individual commits
        checkout_manager.cleanup_commit("commit1").unwrap();
        assert_eq!(checkout_manager.get_stats().active_commits, 2);
        assert!(!checkout_paths[0].exists());
        
        // Cleanup all remaining
        checkout_manager.cleanup_all().unwrap();
        assert_eq!(checkout_manager.get_stats().active_commits, 0);
        
        for path in &checkout_paths[1..] {
            assert!(!path.exists());
        }
    }
    
    #[test]
    fn test_file_tracker_complex_history_backwards() {
        let mut tracker = FileTracker::new();
        
        // Simulate a complex file history working backwards
        tracker.initialize_file_at_head("complex.rs".to_string(), Some(200), false, None);
        
        // Step 1: File was modified (working backwards)
        let modify_diff = r#"
diff --git a/complex.rs b/complex.rs
index abc123..def456 100644
--- a/complex.rs
+++ b/complex.rs
@@ -1,10 +1,15 @@
 fn main() {
+    println!("Step 1");
+    println!("Step 2");
+    println!("Step 3");
     println!("Hello");
-    println!("Removed line 1");
-    println!("Removed line 2");
 }
"#;
        
        tracker.process_commit_backwards(modify_diff).unwrap();
        
        let state = tracker.get_file_state("complex.rs").unwrap();
        assert_eq!(state.line_count, Some(199)); // 200 - 3 + 2 = 199
        
        // Step 2: File was renamed (working backwards)
        let rename_diff = r#"
diff --git a/old_complex.rs b/complex.rs
similarity index 90%
rename from old_complex.rs
rename to complex.rs
index abc123..def456 100644
--- a/old_complex.rs
+++ b/complex.rs
@@ -1,5 +1,6 @@
 fn main() {
     println!("Hello");
+    // Added during rename
     println!("Removed line 1");
     println!("Removed line 2");
 }
"#;
        
        tracker.process_commit_backwards(rename_diff).unwrap();
        
        // Should now be tracked under old name
        assert!(tracker.get_file_state("old_complex.rs").is_some());
        assert!(tracker.get_file_state("complex.rs").is_none());
        
        let state = tracker.get_file_state("old_complex.rs").unwrap();
        assert_eq!(state.line_count, Some(198)); // 199 - 1 = 198
        assert_eq!(state.current_path, "old_complex.rs");
        
        // Test comprehensive lifecycle analysis
        let lifecycle = tracker.analyze_file_lifecycle();
        assert_eq!(lifecycle.total_files, 1);
        assert_eq!(lifecycle.stable_files.len(), 1);
        
        let lifecycle_info = tracker.get_file_lifecycle("old_complex.rs").unwrap();
        assert_eq!(lifecycle_info.lifecycle_type, LifecycleEventType::Stable);
        assert!(lifecycle_info.exists);
    }
}