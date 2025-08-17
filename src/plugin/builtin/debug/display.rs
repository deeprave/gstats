//! Message Display and Formatting for Debug Plugin
//!
//! This module handles the formatting and display of scan messages
//! according to the debug plugin's configuration.

use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{Utc, TimeZone};
use colored::Colorize;

use crate::scanner::messages::{ScanMessage, MessageData};
use crate::scanner::async_engine::events::ChangeType;
use crate::plugin::error::PluginResult;
use crate::display::ColourManager;
use super::config::DebugConfig;

/// Message formatter for debug plugin display
pub struct MessageFormatter {
    /// Configuration for formatting
    config: Arc<RwLock<DebugConfig>>,
    /// Color manager for consistent output formatting
    colour_manager: Option<Arc<ColourManager>>,
}

impl MessageFormatter {
    /// Create a new message formatter
    pub fn new(config: Arc<RwLock<DebugConfig>>) -> Self {
        Self { 
            config,
            colour_manager: None,
        }
    }
    
    /// Create a new message formatter with color manager
    pub fn with_colour_manager(config: Arc<RwLock<DebugConfig>>, colour_manager: Option<Arc<ColourManager>>) -> Self {
        Self { 
            config,
            colour_manager,
        }
    }
    
    /// Format and display a scan message
    pub async fn format_message(&self, message: &ScanMessage) -> PluginResult<()> {
        let config = self.config.read().await;
        
        // Display raw data if enabled (takes precedence)
        if config.raw_data {
            if config.message_index {
                self.display_message_index(message, &config);
            }
            self.display_raw_data(message);
            return Ok(());
        }
        
        // Format based on message type
        match message.data() {
            MessageData::CommitInfo { hash, author, message: commit_msg, timestamp, changed_files } => {
                let file_paths: Vec<String> = changed_files.iter()
                    .map(|f| f.path.clone())
                    .collect();
                self.format_commit_info(message, hash, author, commit_msg, *timestamp, &file_paths, &config);
            }
            MessageData::FileChange { path, change_type, old_path, insertions, deletions, is_binary, commit_hash, commit_timestamp, .. } => {
                self.format_file_change(
                    message, path, change_type, old_path.as_deref(), 
                    *insertions, *deletions, *is_binary,
                    commit_hash, *commit_timestamp, &config
                );
            }
            MessageData::FileInfo { path, size, lines } => {
                self.format_file_info(message, path, *size, *lines as u64, &config);
            }
            _ => {
                if config.verbose {
                    if config.message_index {
                        self.display_message_index(message, &config);
                    }
                    println!("Unknown message type: {:?}", message.data());
                }
            }
        }
        
        Ok(())
    }
    
    /// Display message index/sequence number
    fn display_message_index(&self, message: &ScanMessage, _config: &DebugConfig) {
        let header = message.header();
        let index_str = format!("[MSG #{}]", header.sequence);
        
        if let Some(ref colour_manager) = self.colour_manager {
            if colour_manager.colours_enabled() {
                print!("{} ", index_str.bright_blue());
            } else {
                print!("{} ", index_str);
            }
        } else {
            print!("{} ", index_str);
        }
    }
    
    /// Get message index as string for compact mode
    fn get_message_index_string(&self, message: &ScanMessage) -> String {
        let header = message.header();
        format!("[MSG #{}]", header.sequence)
    }
    
    /// Display raw message data
    fn display_raw_data(&self, message: &ScanMessage) {
        println!("=== RAW MESSAGE DATA ===");
        println!("Header: {:?}", message.header());
        println!("Data: {:#?}", message.data());
        println!("========================\n");
    }
    
    /// Format commit info message
    fn format_commit_info(
        &self,
        scan_message: &ScanMessage,
        hash: &str,
        author: &str,
        message: &str,
        timestamp: i64,
        changed_files: &[String],
        config: &DebugConfig,
    ) {
        if config.compact_mode {
            // Ultra-compact format: [MSG #N] hash, author, date, message on one line
            let first_line = message.lines().next().unwrap_or("");
            
            // Format timestamp as YYYY-MM-DD HH:MM:SS
            let dt = match Utc.timestamp_opt(timestamp, 0) {
                chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
                _ => "Invalid Date".to_string(),
            };
            
            // Get message index prefix if enabled
            let index_prefix = if config.message_index {
                format!("{} ", self.get_message_index_string(scan_message))
            } else {
                String::new()
            };
            
            if let Some(ref colour_manager) = self.colour_manager {
                if colour_manager.colours_enabled() {
                    println!("{}{}, {}, {}, {}",
                        if config.message_index { index_prefix.bright_blue() } else { index_prefix.into() },
                        hash.yellow(),
                        author.cyan(),
                        dt.blue(),
                        first_line
                    );
                } else {
                    println!("{}{}, {}, {}, {}", index_prefix, hash, author, dt, first_line);
                }
            } else {
                println!("{}{}, {}, {}, {}", index_prefix, hash, author, dt, first_line);
            }
            
            // Show file count summary
            let file_count = changed_files.len();
            if file_count > 0 {
                if let Some(ref colour_manager) = self.colour_manager {
                    if colour_manager.colours_enabled() {
                        println!("{} files changed:", file_count.to_string().green());
                    } else {
                        println!("{} files changed:", file_count);
                    }
                } else {
                    println!("{} files changed:", file_count);
                }
            }
        } else {
            // Full format
            // Display message index if enabled
            if config.message_index {
                self.display_message_index(scan_message, config);
            }
            
            if let Some(ref colour_manager) = self.colour_manager {
                if colour_manager.colours_enabled() {
                    println!("{}: {}", "CommitInfo".green().bold(), hash.yellow());
                } else {
                    println!("CommitInfo: {}", hash);
                }
            } else {
                println!("CommitInfo: {}", hash);
            }
            
            println!("├─ Author: {}", author);
            
            if config.show_timestamps {
                let dt = Utc.timestamp_opt(timestamp, 0).unwrap();
                let formatted = dt.format(&config.timestamp_format).to_string();
                println!("├─ Date: {}", formatted);
            }
            
            // Display commit message
            if config.full_commit_message {
                println!("├─ Message:");
                for line in message.lines() {
                    println!("│  {}", line);
                }
            } else {
                let first_line = message.lines().next().unwrap_or("");
                println!("├─ Message: {}", first_line);
            }
            
            println!("└─ Files: {} changed", changed_files.len());
            
            if config.verbose && !changed_files.is_empty() {
                for (i, file) in changed_files.iter().enumerate() {
                    let is_last = i == changed_files.len() - 1;
                    let prefix = if is_last { "   └─" } else { "   ├─" };
                    let truncated = config.truncate_path(file);
                    println!("{} {}", prefix, truncated);
                }
            }
            
            println!(); // Empty line for separation
        }
    }
    
    /// Format file change message
    fn format_file_change(
        &self,
        scan_message: &ScanMessage,
        path: &str,
        change_type: &ChangeType,
        old_path: Option<&str>,
        insertions: usize,
        deletions: usize,
        is_binary: bool,
        commit_hash: &str,
        _commit_timestamp: i64,
        config: &DebugConfig,
    ) {
        let truncated_path = config.truncate_path(path);
        
        if config.compact_mode {
            // Ultra-compact format: simple single line per file  
            let change_symbol = match change_type {
                ChangeType::Added => "Added",
                ChangeType::Modified => "Modified", 
                ChangeType::Deleted => "Deleted",
                ChangeType::Renamed => "Renamed",
                ChangeType::Copied => "Copied",
            };
            
            if let Some(ref colour_manager) = self.colour_manager {
                if colour_manager.colours_enabled() {
                    if is_binary {
                        println!("  {}, {}, {}", 
                            truncated_path.white(), 
                            change_symbol.yellow(), 
                            "binary".blue()
                        );
                    } else {
                        println!("  {}, {}, +{}, -{}, {}", 
                            truncated_path.white(),
                            change_symbol.yellow(),
                            insertions.to_string().green(),
                            deletions.to_string().red(),
                            "false".blue()
                        );
                    }
                } else {
                    if is_binary {
                        println!("  {}, {}, true", truncated_path, change_symbol);
                    } else {
                        println!("  {}, {}, +{}, -{}, false", 
                            truncated_path, change_symbol, insertions, deletions
                        );
                    }
                }
            } else {
                if is_binary {
                    println!("  {}, {}, true", truncated_path, change_symbol);
                } else {
                    println!("  {}, {}, +{}, -{}, false", 
                        truncated_path, change_symbol, insertions, deletions
                    );
                }
            }
        } else {
            // Full format
            // Display message index if enabled
            if config.message_index {
                self.display_message_index(scan_message, config);
            }
            
            if let Some(ref colour_manager) = self.colour_manager {
                if colour_manager.colours_enabled() {
                    println!("{}: {}", "FileChange".blue().bold(), truncated_path.cyan());
                } else {
                    println!("FileChange: {}", truncated_path);
                }
            } else {
                println!("FileChange: {}", truncated_path);
            }
            
            let type_str = match change_type {
                ChangeType::Added => "Added",
                ChangeType::Modified => "Modified",
                ChangeType::Deleted => "Deleted",
                ChangeType::Renamed => "Renamed",
                ChangeType::Copied => "Copied",
            };
            
            println!("├─ Type: {}", type_str);
            
            if let Some(old) = old_path {
                let truncated_old = config.truncate_path(old);
                println!("├─ From: {}", truncated_old);
            }
            
            if !is_binary {
                if let Some(ref colour_manager) = self.colour_manager {
                    if colour_manager.colours_enabled() {
                        println!("├─ Insertions: {}", format!("+{}", insertions).green());
                        println!("├─ Deletions: {}", format!("-{}", deletions).red());
                    } else {
                        println!("├─ Insertions: +{}", insertions);
                        println!("├─ Deletions: -{}", deletions);
                    }
                } else {
                    println!("├─ Insertions: +{}", insertions);
                    println!("├─ Deletions: -{}", deletions);
                }
            }
            
            println!("├─ Binary: {}", is_binary);
            println!("└─ Commit: {}", &commit_hash[..8.min(commit_hash.len())]);
            
            println!(); // Empty line for separation
        }
    }
    
    /// Format file info message
    fn format_file_info(&self, scan_message: &ScanMessage, path: &str, size: u64, lines: u64, config: &DebugConfig) {
        let truncated_path = config.truncate_path(path);
        
        if config.compact_mode {
            println!("FILE {} ({} bytes, {} lines)", truncated_path, size, lines);
        } else {
            // Display message index if enabled
            if config.message_index {
                self.display_message_index(scan_message, config);
            }
            
            if let Some(ref colour_manager) = self.colour_manager {
                if colour_manager.colours_enabled() {
                    println!("{}: {}", "FileInfo".magenta().bold(), truncated_path);
                } else {
                    println!("FileInfo: {}", truncated_path);
                }
            } else {
                println!("FileInfo: {}", truncated_path);
            }
            
            println!("├─ Size: {} bytes", size);
            println!("└─ Lines: {}", lines);
            println!();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, FileChangeData};
    
    #[tokio::test]
    async fn test_message_formatter_creation() {
        let config = Arc::new(RwLock::new(DebugConfig::default()));
        let formatter = MessageFormatter::new(config);
        
        // Formatter should be created successfully
        assert!(std::ptr::eq(
            formatter.config.as_ref() as *const _,
            formatter.config.as_ref() as *const _
        ));
    }
    
    #[tokio::test]
    async fn test_format_commit_info_message() {
        let config = Arc::new(RwLock::new(DebugConfig::default()));
        let formatter = MessageFormatter::new(config);
        
        let header = MessageHeader::new(1);
        let data = MessageData::CommitInfo {
            hash: "abc123def456".to_string(),
            author: "Test Author".to_string(),
            message: "Test commit message".to_string(),
            timestamp: 1234567890,
            changed_files: vec![
                FileChangeData {
                    path: "file1.rs".to_string(),
                    lines_added: 5,
                    lines_removed: 2,
                },
                FileChangeData {
                    path: "file2.rs".to_string(),
                    lines_added: 3,
                    lines_removed: 1,
                },
            ],
        };
        let message = ScanMessage::new(header, data);
        
        // Should format without error
        formatter.format_message(&message).await.unwrap();
    }
    
    #[tokio::test]
    async fn test_format_file_change_message() {
        let config = Arc::new(RwLock::new(DebugConfig::default()));
        let formatter = MessageFormatter::new(config);
        
        let header = MessageHeader::new(2);
        let data = MessageData::FileChange {
            path: "src/main.rs".to_string(),
            change_type: ChangeType::Modified,
            old_path: None,
            insertions: 10,
            deletions: 5,
            is_binary: false,
            binary_size: None,
            line_count: Some(50),
            commit_hash: "abc123".to_string(),
            commit_timestamp: 1234567890,
            checkout_path: None,
        };
        let message = ScanMessage::new(header, data);
        
        formatter.format_message(&message).await.unwrap();
    }
    
    #[tokio::test]
    async fn test_format_with_message_index() {
        let config = Arc::new(RwLock::new(DebugConfig {
            message_index: true,
            ..Default::default()
        }));
        let formatter = MessageFormatter::new(config);
        
        let header = MessageHeader::new(42);
        let data = MessageData::FileInfo {
            path: "test.rs".to_string(),
            size: 1000,
            lines: 50,
        };
        let message = ScanMessage::new(header, data);
        
        formatter.format_message(&message).await.unwrap();
    }
    
    #[tokio::test]
    async fn test_format_raw_data() {
        let config = Arc::new(RwLock::new(DebugConfig {
            raw_data: true,
            ..Default::default()
        }));
        let formatter = MessageFormatter::new(config);
        
        let header = MessageHeader::new(1);
        let data = MessageData::FileInfo {
            path: "test.rs".to_string(),
            size: 1000,
            lines: 50,
        };
        let message = ScanMessage::new(header, data);
        
        formatter.format_message(&message).await.unwrap();
    }
    
    #[tokio::test]
    async fn test_format_compact_mode() {
        let config = Arc::new(RwLock::new(DebugConfig {
            compact_mode: true,
            ..Default::default()
        }));
        let formatter = MessageFormatter::new(config);
        
        let header = MessageHeader::new(1);
        let data = MessageData::CommitInfo {
            hash: "abc123def456".to_string(),
            author: "Test Author".to_string(),
            message: "Test commit message that is very long and should be truncated in compact mode".to_string(),
            timestamp: 1234567890,
            changed_files: vec![
                FileChangeData {
                    path: "file1.rs".to_string(),
                    lines_added: 10,
                    lines_removed: 5,
                },
            ],
        };
        let message = ScanMessage::new(header, data);
        
        formatter.format_message(&message).await.unwrap();
    }
}