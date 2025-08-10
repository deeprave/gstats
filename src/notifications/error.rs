//! Notification System Error Types

use std::fmt;

/// Result type for notification operations
pub type NotificationResult<T> = Result<T, NotificationError>;

/// Errors that can occur in the notification system
#[derive(Debug, Clone)]
pub enum NotificationError {
    /// Subscriber already exists
    SubscriberAlreadyExists(String),
    
    /// Subscriber not found
    SubscriberNotFound(String),
    
    /// Event delivery failed
    DeliveryFailed {
        subscriber_id: String,
        error: String,
    },
    
    /// Rate limit exceeded
    RateLimitExceeded {
        subscriber_id: String,
        limit: u32,
    },
    
    /// System shutdown in progress
    SystemShutdown,
    
    /// Invalid event type
    InvalidEventType(String),
    
    /// Timeout occurred
    Timeout {
        operation: String,
        duration_ms: u64,
    },
    
    /// Generic error
    Generic(String),
}

impl fmt::Display for NotificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotificationError::SubscriberAlreadyExists(id) => {
                write!(f, "Subscriber '{}' already exists", id)
            }
            NotificationError::SubscriberNotFound(id) => {
                write!(f, "Subscriber '{}' not found", id)
            }
            NotificationError::DeliveryFailed { subscriber_id, error } => {
                write!(f, "Failed to deliver event to '{}': {}", subscriber_id, error)
            }
            NotificationError::RateLimitExceeded { subscriber_id, limit } => {
                write!(f, "Rate limit ({}/sec) exceeded for subscriber '{}'", limit, subscriber_id)
            }
            NotificationError::SystemShutdown => {
                write!(f, "Notification system is shutting down")
            }
            NotificationError::InvalidEventType(event_type) => {
                write!(f, "Invalid event type: {}", event_type)
            }
            NotificationError::Timeout { operation, duration_ms } => {
                write!(f, "Operation '{}' timed out after {}ms", operation, duration_ms)
            }
            NotificationError::Generic(msg) => {
                write!(f, "Notification error: {}", msg)
            }
        }
    }
}

impl std::error::Error for NotificationError {}

impl NotificationError {
    /// Create a subscriber already exists error
    pub fn subscriber_already_exists<S: Into<String>>(id: S) -> Self {
        Self::SubscriberAlreadyExists(id.into())
    }
    
    /// Create a subscriber not found error
    pub fn subscriber_not_found<S: Into<String>>(id: S) -> Self {
        Self::SubscriberNotFound(id.into())
    }
    
    /// Create a delivery failed error
    pub fn delivery_failed<S: Into<String>>(subscriber_id: S, error: S) -> Self {
        Self::DeliveryFailed {
            subscriber_id: subscriber_id.into(),
            error: error.into(),
        }
    }
    
    /// Create a rate limit exceeded error
    pub fn rate_limit_exceeded<S: Into<String>>(subscriber_id: S, limit: u32) -> Self {
        Self::RateLimitExceeded {
            subscriber_id: subscriber_id.into(),
            limit,
        }
    }
    
    /// Create a timeout error
    pub fn timeout<S: Into<String>>(operation: S, duration_ms: u64) -> Self {
        Self::Timeout {
            operation: operation.into(),
            duration_ms,
        }
    }
    
    /// Create a generic error
    pub fn generic<S: Into<String>>(message: S) -> Self {
        Self::Generic(message.into())
    }
}
