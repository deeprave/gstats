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


    /// Generic queue operation error
    #[error("Queue operation failed: {message}")]
    OperationFailed { message: String },
}

impl QueueError {

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

        let error = QueueError::operation_failed("test error");
        assert_eq!(error.to_string(), "Queue operation failed: test error");
    }
}
