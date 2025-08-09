//! Queue System for Plugin Message Processing
//! 
//! This module provides a robust, async message queue system for coordinating
//! between repository scanners and plugin consumers. The queue system is designed
//! to be independent of both scanner and plugin implementations, providing clean
//! abstractions and proper async coordination.
//!
//! # Architecture
//!
//! The queue system consists of several key components:
//!
//! - **SharedMessageQueue**: Core queue implementation with producer interface
//! - **QueueConsumer**: Abstract consumer API for plugins
//! - **QueueEvent System**: Generic notification system for coordination
//! - **Memory Monitoring**: Queue memory usage tracking and reporting
//!
//! # Usage
//!
//! ```rust
//! use gstats::queue::{SharedMessageQueue, QueueEvent};
//! use gstats::scanner::modes::ScanMode;
//! use gstats::scanner::messages::{ScanMessage, MessageHeader, MessageData};
//!
//! # tokio_test::block_on(async {
//! // Create a queue for a scanning session
//! let queue = SharedMessageQueue::new("scan-001".to_string());
//!
//! // Producer: Start scanning and add messages
//! queue.start_scan(ScanMode::HISTORY).await.unwrap();
//!
//! // Create a sample message
//! let header = MessageHeader::new(ScanMode::HISTORY, 0);
//! let data = MessageData::CommitInfo {
//!     hash: "abc123".to_string(),
//!     author: "John Doe".to_string(),
//!     message: "Fix bug".to_string(),
//!     timestamp: 1234567890,
//!     changed_files: vec![],
//! };
//! let scan_message = ScanMessage::new(header, data);
//!
//! queue.push(scan_message).await.unwrap();
//! queue.complete_scan().await.unwrap();
//!
//! // Consumer: Subscribe to events and process messages
//! let mut event_receiver = queue.subscribe_events();
//! // In a real scenario, you would process events in a loop
//! // This example just shows the API structure
//! # });
//! ```

pub mod error;
pub mod notifications;
pub mod shared_queue;
pub mod memory;
pub mod consumer;
pub mod intermediate;

// Re-export main types for convenience
pub use error::{QueueError, QueueResult};
pub use notifications::{QueueEvent, QueueEventNotifier};
pub use shared_queue::SharedMessageQueue;
pub use memory::{QueueMemoryStats, MemoryMonitor};
pub use consumer::{PollingConsumer, EventDrivenConsumer, QueuePollingConsumer, QueueEventConsumer, PluginConsumer};
pub use intermediate::{IntermediateData, DataTransformer, FileChangeData, ChangeType, FrequencyMetrics};

/// Queue system version for compatibility tracking
pub const QUEUE_API_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_module_exports() {
        // Ensure all main types are properly exported
        let _error: QueueError = QueueError::QueueFull;
        // Additional basic tests will be added as components are implemented
    }
}
