//! Queue Event Notification System
//!
//! Provides a generic event system for coordinating between queue producers
//! and consumers. Events are broadcast to all subscribers using tokio's
//! broadcast channel for efficient async coordination.

use serde::{Deserialize, Serialize};

/// Events emitted by the queue system for coordination
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueueEvent {
    /// Scanning has started
    ScanStarted {
        scan_id: String,
        timestamp: u64,
    },

    /// One or more messages have been added to the queue
    MessageAdded {
        scan_id: String,
        count: usize,
        queue_size: usize,
        timestamp: u64,
    },

    /// Scanning has completed - no more messages will be added
    ScanComplete {
        scan_id: String,
        total_messages: u64,
        timestamp: u64,
    },

    /// Queue has been drained - all messages processed
    QueueDrained {
        scan_id: String,
        timestamp: u64,
    },

    /// Memory usage warning
    MemoryWarning {
        scan_id: String,
        current_size: usize,
        threshold: usize,
        timestamp: u64,
    },
}

impl QueueEvent {
    /// Get the scan ID associated with this event
    pub fn scan_id(&self) -> &str {
        match self {
            QueueEvent::ScanStarted { scan_id, .. } => scan_id,
            QueueEvent::MessageAdded { scan_id, .. } => scan_id,
            QueueEvent::ScanComplete { scan_id, .. } => scan_id,
            QueueEvent::QueueDrained { scan_id, .. } => scan_id,
            QueueEvent::MemoryWarning { scan_id, .. } => scan_id,
        }
    }

    /// Get the timestamp of this event
    pub fn timestamp(&self) -> u64 {
        match self {
            QueueEvent::ScanStarted { timestamp, .. } => *timestamp,
            QueueEvent::MessageAdded { timestamp, .. } => *timestamp,
            QueueEvent::ScanComplete { timestamp, .. } => *timestamp,
            QueueEvent::QueueDrained { timestamp, .. } => *timestamp,
            QueueEvent::MemoryWarning { timestamp, .. } => *timestamp,
        }
    }

    /// Create a scan started event
    pub fn scan_started(scan_id: String) -> Self {
        Self::ScanStarted {
            scan_id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }

    /// Create a message added event
    pub fn message_added(scan_id: String, count: usize, queue_size: usize) -> Self {
        Self::MessageAdded {
            scan_id,
            count,
            queue_size,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }

    /// Create a scan complete event
    pub fn scan_complete(scan_id: String, total_messages: u64) -> Self {
        Self::ScanComplete {
            scan_id,
            total_messages,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }

}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_event_creation() {
        let event = QueueEvent::scan_started("test-scan".to_string());
        assert_eq!(event.scan_id(), "test-scan");
        assert!(event.timestamp() > 0);

        if let QueueEvent::ScanStarted { .. } = event {
            // ScanStarted event created successfully
        } else {
            panic!("Expected ScanStarted event");
        }
    }

}
