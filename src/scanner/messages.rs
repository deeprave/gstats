//! Message Structures
//! 
//! Compact message structures for memory-efficient queue operations.

use crate::scanner::modes::ScanMode;
use serde::{Serialize, Deserialize};

/// Compact message structure with fixed header and variable data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanMessage {
    /// Fixed header with scanning metadata
    pub header: MessageHeader,
    /// Variable data specific to scanning modes
    pub data: MessageData,
}

/// Fixed header containing scanning metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    /// Scanning mode used
    pub scan_mode: ScanMode,
    /// Timestamp when message was created
    pub timestamp: u64,
}

/// Variable data types for different scanning modes
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Empty data placeholder
    None,
}

impl MessageHeader {
    /// Create a new message header
    pub fn new(scan_mode: ScanMode, timestamp: u64) -> Self {
        Self {
            scan_mode,
            timestamp,
        }
    }
    
    /// Get the scan mode
    pub fn mode(&self) -> ScanMode {
        self.scan_mode
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
            MessageData::CommitInfo { hash, author, message, .. } => {
                hash.len() + author.len() + message.len()
            },
            MessageData::DependencyInfo { name, version, license } => {
                name.len() + version.len() + license.as_ref().map_or(0, |l| l.len())
            },
            MessageData::SecurityInfo { vulnerability, severity, location } => {
                vulnerability.len() + severity.len() + location.len()
            },
            MessageData::PerformanceInfo { function, .. } => function.len(),
            MessageData::MetricInfo { .. } => 0, // No string fields in MetricInfo
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
        let header = MessageHeader::new(ScanMode::FILES, 12345);
        let data = MessageData::FileInfo {
            path: "test.rs".to_string(),
            size: 1024,
            lines: 50,
        };
        let message = ScanMessage::new(header, data);

        assert_eq!(message.header.scan_mode, ScanMode::FILES);
        assert_eq!(message.header.timestamp, 12345);
    }

    #[test]
    fn test_message_serialization() {
        let message = ScanMessage::new(
            MessageHeader::new(ScanMode::HISTORY, 67890),
            MessageData::CommitInfo {
                hash: "abc123".to_string(),
                author: "developer".to_string(),
                message: "Fix bug".to_string(),
                timestamp: 1234567890,
            }
        );

        let bytes = message.to_bytes();
        let deserialized = ScanMessage::from_bytes(&bytes).unwrap();

        assert_eq!(deserialized.header.scan_mode, message.header.scan_mode);
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
}
