//! Memory-Conscious Message Queue System
//! 
//! Provides a memory-monitored MPSC queue with listener/observer pattern for
//! efficient message distribution to interested consumers.

pub mod versioned_message;
pub mod memory_queue;
pub mod memory_tracker;
pub mod backoff;
pub mod listener;
pub mod consumer;
pub mod processor_bridge;
pub mod debug;
pub mod scanner_integration;
pub mod config;
pub mod cli;

#[cfg(test)]
mod tests;

// Re-export core types for easier access
pub use versioned_message::{QueueMessage, MessageType, MessagePayload};
pub use memory_queue::{MemoryQueue, VersionedMemoryQueue, QueueMemoryStatistics, PressureResponseConfig, PressureResponseStatus, PressureResponseMetrics};
pub use memory_tracker::{MemoryTracker, MemoryStatistics, MemoryPressureLevel, LeakInformation, MemoryHistorySample};
pub use backoff::{BackoffAlgorithm, BackoffConfig, BackoffStrategy, BackoffMetrics};
pub use listener::{MessageListener, ListenerRegistry, DefaultListenerRegistry};
pub use consumer::{MessageConsumer, ConsumerConfig, ConsumerMetrics};
pub use processor_bridge::{ScanProcessorBridge, ScanProcessorBridgeBuilder};
pub use debug::{QueueDebug, QueueDebugStatus, QueueStatusLogger, format_consumer_metrics};
pub use scanner_integration::{QueueMessageProducer, ScannerQueueIntegration, ScannerQueueBuilder};
pub use config::{QueueConfig, QueuePreset, load_queue_config, save_queue_config};
pub use cli::{QueueArgs, QueueCommand, handle_queue_command};

use anyhow::Result;

/// Module metadata
pub const MODULE_NAME: &str = "Memory-Conscious Message Queue";
pub const MODULE_VERSION: &str = "1.0.0";

/// Queue configuration error types
#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Queue is full, cannot enqueue message")]
    QueueFull,
    #[error("Queue is empty, cannot dequeue message")]
    QueueEmpty,
    #[error("Memory limit exceeded")]
    MemoryLimitExceeded,
    #[error("Invalid queue configuration: {0}")]
    InvalidConfiguration(String),
    #[error("Listener registration error: {0}")]
    ListenerError(String),
    #[error("Message versioning error: {0}")]
    VersioningError(String),
    #[error("Message dropped due to extreme memory pressure: {0}")]
    MessageDropped(String),
}

/// Basic queue interface trait
pub trait Queue<T> {
    /// Enqueue a message
    fn enqueue(&self, message: T) -> Result<(), QueueError>;
    
    /// Dequeue a message
    fn dequeue(&self) -> Result<Option<T>, QueueError>;
    
    /// Get current queue size
    fn size(&self) -> usize;
    
    /// Check if queue is empty
    fn is_empty(&self) -> bool;
    
    /// Get queue capacity
    fn capacity(&self) -> usize;
}

