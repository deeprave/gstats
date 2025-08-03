//! Versioned Message Envelope
//! 
//! Provides versioned message wrapper for backward/forward compatibility

use serde::{Serialize, Deserialize};
use crate::scanner::messages::ScanMessage;

/// Versioned envelope for queue messages with backward/forward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueMessage {
    /// Message format version (semantic versioning as u32: major.minor.patch)
    pub version: u32,
    /// Message type identifier for deserialization routing
    pub message_type: MessageType,
    /// Timestamp when message was enqueued
    pub enqueue_timestamp: u64,
    /// The actual message payload
    pub payload: MessagePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    /// Standard scan message (current format)
    ScanMessage,
    /// Future message types can be added here
    MetricsMessage,
    ControlMessage,
    Unknown(String), // For forward compatibility
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    /// Current ScanMessage format
    Scan(ScanMessage),
    /// Raw bytes for unknown/future message types
    Raw(Vec<u8>),
}

/// Message versioning errors
#[derive(Debug, thiserror::Error)]
pub enum MessageError {
    #[error("Incompatible message type")]
    IncompatibleMessageType,
    #[error("Unsupported message version: {0}")]
    UnsupportedVersion(u32),
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

impl QueueMessage {
    /// Create a new queue message from a ScanMessage
    pub fn from_scan_message(scan_message: ScanMessage) -> Self {
        Self {
            version: 1_00_00, // v1.0.0
            message_type: MessageType::ScanMessage,
            enqueue_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            payload: MessagePayload::Scan(scan_message),
        }
    }
    
    /// Extract ScanMessage if compatible
    pub fn try_extract_scan_message(&self) -> Result<&ScanMessage, MessageError> {
        match (&self.message_type, &self.payload) {
            (MessageType::ScanMessage, MessagePayload::Scan(msg)) => Ok(msg),
            _ => Err(MessageError::IncompatibleMessageType),
        }
    }
    
    /// Check if this message version is compatible with current implementation
    pub fn is_version_compatible(&self) -> bool {
        let major = self.version / 10000;
        let current_major = 1; // Current major version
        major == current_major
    }
    
    /// Serialize message to bytes for queue transmission
    pub fn to_bytes(&self) -> Result<Vec<u8>, MessageError> {
        bincode::serialize(self)
            .map_err(|e| MessageError::SerializationError(e.to_string()))
    }

    /// Deserialize message from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MessageError> {
        bincode::deserialize(bytes)
            .map_err(|e| MessageError::SerializationError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};
    use crate::scanner::modes::ScanMode;

    #[test]
    fn test_queue_message_creation() {
        let scan_message = ScanMessage::new(
            MessageHeader::new(ScanMode::FILES, 12345),
            MessageData::FileInfo {
                path: "test.rs".to_string(),
                size: 1024,
                lines: 50,
            }
        );
        
        let queue_message = QueueMessage::from_scan_message(scan_message);
        
        assert_eq!(queue_message.version, 1_00_00);
        assert!(matches!(queue_message.message_type, MessageType::ScanMessage));
        assert!(queue_message.is_version_compatible());
    }

    #[test]
    fn test_scan_message_extraction() {
        let scan_message = ScanMessage::new(
            MessageHeader::new(ScanMode::FILES, 12345),
            MessageData::FileInfo {
                path: "test.rs".to_string(),
                size: 1024,
                lines: 50,
            }
        );
        
        let queue_message = QueueMessage::from_scan_message(scan_message);
        let extracted = queue_message.try_extract_scan_message().unwrap();
        
        assert_eq!(extracted.header.scan_mode, ScanMode::FILES);
        assert_eq!(extracted.header.timestamp, 12345);
    }

    #[test]
    fn test_message_serialization() {
        let scan_message = ScanMessage::new(
            MessageHeader::new(ScanMode::HISTORY, 67890),
            MessageData::CommitInfo {
                hash: "abc123".to_string(),
                author: "developer".to_string(),
                message: "Fix bug".to_string(),
                timestamp: 1234567890,
            }
        );
        
        let queue_message = QueueMessage::from_scan_message(scan_message);
        let bytes = queue_message.to_bytes().unwrap();
        let deserialized = QueueMessage::from_bytes(&bytes).unwrap();
        
        assert_eq!(deserialized.version, queue_message.version);
        assert_eq!(deserialized.enqueue_timestamp, queue_message.enqueue_timestamp);
        
        let extracted = deserialized.try_extract_scan_message().unwrap();
        assert_eq!(extracted.header.scan_mode, ScanMode::HISTORY);
    }
}