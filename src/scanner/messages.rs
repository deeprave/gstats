//! Message Structures
//! 
//! Compact message structures for memory-efficient queue operations.

use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use crate::scanner::async_engine::events::ChangeType;

/// File change data for commits
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileChangeData {
    pub path: String,
    pub lines_added: usize,
    pub lines_removed: usize,
}

/// Compact message structure with fixed header and variable data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanMessage {
    /// Fixed header with scanning metadata
    pub header: MessageHeader,
    /// Variable data specific to scanning modes
    pub data: MessageData,
}

/// Fixed header containing scanning metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageHeader {
    /// Message sequence number
    pub sequence: u64,
    /// Timestamp when message was created
    pub timestamp: u64,
    /// Scan identifier this message belongs to
    pub scan_id: String,
}

/// Variable data types for different scanning modes
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MessageData {
    /// File system scanning data
    FileInfo {
        path: String,
        size: u64,
        lines: u32,
    },
    /// Git history scanning data
    CommitInfo {
        hash: String,
        author: String,
        message: String,
        timestamp: i64,
        changed_files: Vec<FileChangeData>,
    },
    /// Change frequency analysis data
    ChangeFrequencyInfo {
        file_path: String,
        change_count: u32,
        author_count: u32,
        last_changed: i64,
        first_changed: i64,
        frequency_score: f64,
        recency_weight: f64,
        authors: Vec<String>,
    },
    /// Code metrics scanning data
    MetricInfo {
        file_count: u32,
        line_count: u64,
        complexity: f64,
    },
    /// Dependency scanning data
    DependencyInfo {
        name: String,
        version: String,
        license: Option<String>,
    },
    /// Security scanning data
    SecurityInfo {
        vulnerability: String,
        severity: String,
        location: String,
    },
    /// Performance scanning data
    PerformanceInfo {
        function: String,
        execution_time: f64,
        memory_usage: u64,
    },
    /// Repository statistics data
    RepositoryStatistics {
        total_commits: u64,
        total_files: u64,
        total_authors: u64,
        repository_size: u64,
        age_days: u64,
        avg_commits_per_day: f64,
    },
    /// File change data with commit context (GS-76)
    FileChange {
        path: String,
        change_type: ChangeType,
        old_path: Option<String>,
        insertions: usize,
        deletions: usize,
        is_binary: bool,
        binary_size: Option<u64>,
        line_count: Option<usize>,
        commit_hash: String,
        commit_timestamp: i64,
        checkout_path: Option<PathBuf>,
    },
    /// Empty data placeholder
    None,
}

impl MessageHeader {
    /// Create a new message header
    pub fn new(sequence: u64, scan_id: String) -> Self {
        Self {
            sequence,
            scan_id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }
    
    /// Get the sequence number
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    
    /// Get the timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

impl ScanMessage {
    /// Create a new scan message
    pub fn new(header: MessageHeader, data: MessageData) -> Self {
        Self {
            header,
            data,
        }
    }

    /// Serialize message to bytes for queue transmission
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self)
            .expect("Failed to serialize scan message")
    }

    /// Deserialize message from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let message = bincode::deserialize(bytes)?;
        Ok(message)
    }
    
    /// Estimate memory usage of this message in bytes
    pub fn estimate_memory_usage(&self) -> usize {
        let base_size = std::mem::size_of::<Self>();
        let data_size = match &self.data {
            MessageData::FileInfo { path, .. } => path.len(),
            MessageData::CommitInfo { hash, author, message, changed_files, .. } => {
                hash.len() + author.len() + message.len() + 
                changed_files.iter().map(|f| f.path.len() + 16).sum::<usize>() // path + 2 usizes
            },
            MessageData::ChangeFrequencyInfo { file_path, authors, .. } => {
                file_path.len() + authors.iter().map(|a| a.len()).sum::<usize>() + 
                (authors.len() * std::mem::size_of::<String>()) + 32 // other fields
            },
            MessageData::DependencyInfo { name, version, license } => {
                name.len() + version.len() + license.as_ref().map_or(0, |l| l.len())
            },
            MessageData::SecurityInfo { vulnerability, severity, location } => {
                vulnerability.len() + severity.len() + location.len()
            },
            MessageData::PerformanceInfo { function, .. } => function.len(),
            MessageData::MetricInfo { .. } => 0, // No string fields in MetricInfo
            MessageData::RepositoryStatistics { .. } => 0, // No string fields in RepositoryStatistics
            MessageData::FileChange { path, old_path, commit_hash, checkout_path, .. } => {
                path.len() + 
                old_path.as_ref().map_or(0, |p| p.len()) + 
                commit_hash.len() + 
                checkout_path.as_ref().map_or(0, |p| p.to_string_lossy().len()) +
                48 // insertions, deletions, timestamp, binary_size, line_count + other fields
            },
            MessageData::None => 0,
        };
        base_size + data_size
    }
    
    /// Get a reference to the message header
    pub fn header(&self) -> &MessageHeader {
        &self.header
    }
    
    /// Get a reference to the message data
    pub fn data(&self) -> &MessageData {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let header = MessageHeader::new(12345, "test-scan".to_string());
        let data = MessageData::FileInfo {
            path: "test.rs".to_string(),
            size: 1024,
            lines: 50,
        };
        let message = ScanMessage::new(header, data);

        assert_eq!(message.header.sequence, 12345);
    }

    #[test]
    fn test_message_serialization() {
        let message = ScanMessage::new(
            MessageHeader::new(67890, "test-scan".to_string()),
            MessageData::CommitInfo {
                hash: "abc123".to_string(),
                author: "developer".to_string(),
                message: "Fix bug".to_string(),
                timestamp: 1234567890,
                changed_files: vec![FileChangeData {
                    path: "src/main.rs".to_string(),
                    lines_added: 10,
                    lines_removed: 5,
                }],
            }
        );

        let bytes = message.to_bytes();
        let deserialized = ScanMessage::from_bytes(&bytes).unwrap();

        assert_eq!(deserialized.header.sequence, message.header.sequence);
        assert_eq!(deserialized.header.timestamp, message.header.timestamp);
    }

    #[test]
    fn test_message_data_variants() {
        let file_data = MessageData::FileInfo {
            path: "main.rs".to_string(),
            size: 2048,
            lines: 100,
        };

        let commit_data = MessageData::CommitInfo {
            hash: "def456".to_string(),
            author: "contributor".to_string(),
            message: "Add feature".to_string(),
            timestamp: 1234567890,
            changed_files: vec![
                FileChangeData {
                    path: "src/lib.rs".to_string(),
                    lines_added: 25,
                    lines_removed: 3,
                },
                FileChangeData {
                    path: "README.md".to_string(),
                    lines_added: 8,
                    lines_removed: 1,
                }
            ],
        };

        let metric_data = MessageData::MetricInfo {
            file_count: 10,
            line_count: 1000,
            complexity: 5.5,
        };

        // Test that all variants can be created
        assert!(matches!(file_data, MessageData::FileInfo { .. }));
        assert!(matches!(commit_data, MessageData::CommitInfo { .. }));
        assert!(matches!(metric_data, MessageData::MetricInfo { .. }));
    }

    #[test]
    fn test_file_change_message_type() {
        use crate::scanner::async_engine::events::ChangeType;

        let file_change_data = MessageData::FileChange {
            path: "src/main.rs".to_string(),
            change_type: ChangeType::Modified,
            old_path: None,
            insertions: 15,
            deletions: 3,
            is_binary: false,
            binary_size: None,
            line_count: Some(150),
            commit_hash: "abc123def456".to_string(),
            commit_timestamp: 1672531200,
            checkout_path: None,
        };

        let message = ScanMessage::new(
            MessageHeader::new(42, "test-scan".to_string()),
            file_change_data,
        );

        // Test that FileChange variant can be created
        assert!(matches!(message.data, MessageData::FileChange { .. }));
        
        // Test serialization and deserialization
        let bytes = message.to_bytes();
        let deserialized = ScanMessage::from_bytes(&bytes).unwrap();
        assert_eq!(deserialized.header.sequence, message.header.sequence);
        
        // Test memory estimation includes new variant
        let memory_usage = message.estimate_memory_usage();
        assert!(memory_usage > 0);
    }

    #[test]
    fn test_file_change_with_rename() {
        use crate::scanner::async_engine::events::ChangeType;

        let file_change_data = MessageData::FileChange {
            path: "src/new_name.rs".to_string(),
            change_type: ChangeType::Renamed,
            old_path: Some("src/old_name.rs".to_string()),
            insertions: 0,
            deletions: 0,
            is_binary: false,
            binary_size: None,
            line_count: Some(100),
            commit_hash: "rename123".to_string(),
            commit_timestamp: 1672531200,
            checkout_path: None,
        };

        assert!(matches!(file_change_data, MessageData::FileChange { 
            change_type: ChangeType::Renamed,
            old_path: Some(_),
            ..
        }));
    }

    #[test]
    fn test_binary_file_change() {
        use crate::scanner::async_engine::events::ChangeType;

        let binary_file_data = MessageData::FileChange {
            path: "assets/image.png".to_string(),
            change_type: ChangeType::Added,
            old_path: None,
            insertions: 0, // Binary files should have 0 line counts
            deletions: 0,
            is_binary: true,
            binary_size: Some(204800), // 200KB binary file
            line_count: None, // No line count for binary files
            commit_hash: "binary123".to_string(),
            commit_timestamp: 1672531200,
            checkout_path: None,
        };

        if let MessageData::FileChange { is_binary, insertions, deletions, binary_size, line_count, .. } = binary_file_data {
            assert!(is_binary);
            assert_eq!(insertions, 0);
            assert_eq!(deletions, 0);
            assert_eq!(binary_size, Some(204800));
            assert_eq!(line_count, None);
        }
    }

    #[test]
    fn test_file_change_with_checkout_path() {
        use crate::scanner::async_engine::events::ChangeType;
        use std::path::PathBuf;

        let checkout_path = PathBuf::from("/tmp/gstats/checkout/src/main.rs");
        let file_change_data = MessageData::FileChange {
            path: "src/main.rs".to_string(),
            change_type: ChangeType::Modified,
            old_path: None,
            insertions: 20,
            deletions: 5,
            is_binary: false,
            binary_size: None,
            line_count: Some(200),
            commit_hash: "checkout123".to_string(),
            commit_timestamp: 1672531200,
            checkout_path: Some(checkout_path.clone()),
        };

        if let MessageData::FileChange { checkout_path: cp, line_count, binary_size, .. } = file_change_data {
            assert_eq!(cp, Some(checkout_path));
            assert_eq!(line_count, Some(200));
            assert_eq!(binary_size, None);
        }
    }
}
