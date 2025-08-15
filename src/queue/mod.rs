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
//! use gstats::queue::SharedMessageQueue;
//! use gstats::scanner::messages::{ScanMessage, MessageHeader, MessageData};
//!
//! # tokio_test::block_on(async {
//! // Create a queue for a scanning session
//! let queue = SharedMessageQueue::new("scan-001".to_string());
//!
//! // Producer: Start scanning and add messages
//! queue.start().await.unwrap();
//!
//! // Create a sample message
//! let header = MessageHeader::new(0);
//! let data = MessageData::CommitInfo {
//!     hash: "abc123".to_string(),
//!     author: "John Doe".to_string(),
//!     message: "Fix bug".to_string(),
//!     timestamp: 1234567890,
//!     changed_files: vec![],
//! };
//! let scan_message = ScanMessage::new(header, data);
//!
//! queue.enqueue(scan_message).await.unwrap();
//! queue.stop().await.unwrap();
//!
//! // Consumer: Register consumer and process messages
//! let consumer = queue.register_consumer("debug".to_string()).await.unwrap();
//! // In a real scenario, you would process messages with consumer.read_next()
//! // This example just shows the API structure
//! # });
//! ```

pub mod error;
pub mod notifications;
pub mod shared_queue;
pub mod memory;
pub mod multi_consumer;
pub mod queue_consumer;

// Re-export main types for convenience
pub use error::{QueueError, QueueResult};
pub use notifications::{QueueEvent, QueueEventNotifier};
pub use shared_queue::SharedMessageQueue;
pub use memory::MemoryMonitor;
pub use multi_consumer::{
    MultiConsumerQueue, MultiConsumerConfig, QueueStatistics, ConsumerSummary, 
    BackpressureReason
};
pub use queue_consumer::QueueConsumer;

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
