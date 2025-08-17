//! Configuration for Debug Plugin
//!
//! This module defines the configuration structure and options for the
//! debug plugin's message display behavior.

use serde::{Serialize, Deserialize};

/// Configuration for the debug plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugConfig {
    /// Enable verbose output
    pub verbose: bool,
    
    /// Show complete commit messages (not truncated)
    pub full_commit_message: bool,
    
    /// Display file diffs if available
    pub file_diff: bool,
    
    /// Show all raw message fields
    pub raw_data: bool,
    
    /// Display message sequence numbers
    pub message_index: bool,
    
    
    /// Use compact display mode
    pub compact_mode: bool,
    
    /// Maximum lines to display per message
    pub max_display_lines: usize,
    
    /// Truncate long file paths
    pub truncate_paths: bool,
    
    /// Maximum path length before truncation
    pub max_path_length: usize,
    
    /// Show timestamps in output
    pub show_timestamps: bool,
    
    /// Timestamp format (strftime format)
    pub timestamp_format: String,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            verbose: false,
            full_commit_message: false,
            file_diff: false,
            raw_data: false,
            message_index: false,
            compact_mode: false,
            max_display_lines: 100,
            truncate_paths: true,
            max_path_length: 80,
            show_timestamps: true,
            timestamp_format: "%Y-%m-%d %H:%M:%S".to_string(),
        }
    }
}

impl DebugConfig {
    /// Create a verbose configuration
    pub fn verbose() -> Self {
        Self {
            verbose: true,
            full_commit_message: true,
            message_index: true,
            show_timestamps: true,
            ..Self::default()
        }
    }
    
    /// Create a compact configuration
    pub fn compact() -> Self {
        Self {
            compact_mode: true,
            truncate_paths: true,
            max_path_length: 60,
            max_display_lines: 50,
            show_timestamps: false,
            ..Self::default()
        }
    }
    
    /// Create a raw data configuration
    pub fn raw() -> Self {
        Self {
            raw_data: true,
            full_commit_message: true,
            message_index: true,
            ..Self::default()
        }
    }
    
    /// Check if any special display mode is enabled
    pub fn has_special_display(&self) -> bool {
        self.full_commit_message || self.file_diff || self.raw_data || self.message_index
    }
    
    /// Get effective max lines (considering compact mode)
    pub fn effective_max_lines(&self) -> usize {
        if self.compact_mode {
            self.max_display_lines.min(50)
        } else {
            self.max_display_lines
        }
    }
    
    /// Truncate a path if needed
    pub fn truncate_path(&self, path: &str) -> String {
        if !self.truncate_paths || path.len() <= self.max_path_length {
            return path.to_string();
        }
        
        // Try to keep the filename and truncate the directory path
        if let Some(filename) = path.split('/').last() {
            if filename.len() < self.max_path_length - 10 {
                let remaining = self.max_path_length - filename.len() - 4; // ".../"
                if remaining > 0 {
                    let prefix = &path[..remaining.min(path.len() - filename.len() - 1)];
                    return format!("{}.../{}", prefix, filename);
                }
            }
        }
        
        // Fallback to simple truncation
        format!("...{}", &path[path.len() - self.max_path_length + 3..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = DebugConfig::default();
        
        assert!(!config.verbose);
        assert!(!config.full_commit_message);
        assert!(!config.raw_data);
        assert_eq!(config.max_display_lines, 100);
    }
    
    #[test]
    fn test_verbose_config() {
        let config = DebugConfig::verbose();
        
        assert!(config.verbose);
        assert!(config.full_commit_message);
        assert!(config.message_index);
        assert!(config.show_timestamps);
    }
    
    #[test]
    fn test_compact_config() {
        let config = DebugConfig::compact();
        
        assert!(config.compact_mode);
        assert!(config.truncate_paths);
        assert!(!config.show_timestamps);
        assert_eq!(config.max_display_lines, 50);
    }
    
    #[test]
    fn test_raw_config() {
        let config = DebugConfig::raw();
        
        assert!(config.raw_data);
        assert!(config.full_commit_message);
        assert!(config.message_index);
    }
    
    #[test]
    fn test_path_truncation() {
        let config = DebugConfig {
            truncate_paths: true,
            max_path_length: 30,
            ..Default::default()
        };
        
        // Short path - no truncation
        let short_path = "src/main.rs";
        assert_eq!(config.truncate_path(short_path), short_path);
        
        // Long path - should truncate but keep filename
        let long_path = "src/very/long/directory/structure/file.rs";
        let truncated = config.truncate_path(long_path);
        assert!(truncated.contains("file.rs"));
        assert!(truncated.contains("..."));
        assert!(truncated.len() <= 30);
        
        // Very long filename - fallback truncation
        let long_filename = "a/very_very_very_very_very_long_filename.rs";
        let truncated = config.truncate_path(long_filename);
        assert!(truncated.starts_with("..."));
        assert_eq!(truncated.len(), 30);
    }
    
    #[test]
    fn test_effective_max_lines() {
        let mut config = DebugConfig {
            max_display_lines: 100,
            compact_mode: false,
            ..Default::default()
        };
        
        assert_eq!(config.effective_max_lines(), 100);
        
        config.compact_mode = true;
        assert_eq!(config.effective_max_lines(), 50);
        
        config.max_display_lines = 30;
        assert_eq!(config.effective_max_lines(), 30);
    }
}