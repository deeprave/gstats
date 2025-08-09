//! Queue Error Types
//!
//! Defines error types specific to the queue system operations.

use thiserror::Error;

/// Result type for queue operations
pub type QueueResult<T> = Result<T, QueueError>;

/// Errors that can occur during queue operations
#[derive(Debug, Error, Clone)]
pub enum QueueError {
    /// Queue has reached its capacity limit
    #[error("Queue is full - cannot add more messages")]
    QueueFull,

    /// Queue is empty when a message was expected
    #[error("Queue is empty - no messages available")]
    QueueEmpty,

    /// Scan has already been started for this queue
    #[error("Scan already started for queue: {scan_id}")]
    ScanAlreadyStarted { scan_id: String },

    /// Scan has already been completed for this queue
    #[error("Scan already completed for queue: {scan_id}")]
    ScanAlreadyCompleted { scan_id: String },

    /// Scan has not been started yet
    #[error("Scan not started for queue: {scan_id}")]
    ScanNotStarted { scan_id: String },

    /// Event notification system error
    #[error("Event notification error: {message}")]
    NotificationError { message: String },

    /// Memory monitoring error
    #[error("Memory monitoring error: {message}")]
    MemoryError { message: String },

    /// Consumer registration error
    #[error("Consumer registration error: {message}")]
    ConsumerError { message: String },

    /// Generic queue operation error
    #[error("Queue operation failed: {message}")]
    OperationFailed { message: String },
}

impl QueueError {
    /// Create a notification error
    pub fn notification_error(message: impl Into<String>) -> Self {
        Self::NotificationError {
            message: message.into(),
        }
    }

    /// Create a memory error
    pub fn memory_error(message: impl Into<String>) -> Self {
        Self::MemoryError {
            message: message.into(),
        }
    }

    /// Create a consumer error
    pub fn consumer_error(message: impl Into<String>) -> Self {
        Self::ConsumerError {
            message: message.into(),
        }
    }

    /// Create an operation failed error
    pub fn operation_failed(message: impl Into<String>) -> Self {
        Self::OperationFailed {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_error_creation() {
        let error = QueueError::QueueFull;
        assert_eq!(error.to_string(), "Queue is full - cannot add more messages");

        let error = QueueError::notification_error("test error");
        assert_eq!(error.to_string(), "Event notification error: test error");
    }

    #[test]
    fn test_queue_error_with_scan_id() {
        let error = QueueError::ScanAlreadyStarted {
            scan_id: "test-scan".to_string(),
        };
        assert_eq!(error.to_string(), "Scan already started for queue: test-scan");
    }
}
