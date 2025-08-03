//! Async Scanner Error Types
//! 
//! Error types specific to async scanning operations.

use thiserror::Error;
use std::sync::Arc;
use crate::scanner::modes::ScanMode;

/// Errors that can occur during async scanning operations
#[derive(Debug, Error)]
pub enum ScanError {
    /// Repository access error
    #[error("Repository error: {0}")]
    Repository(String),
    
    /// Task spawning or execution error
    #[error("Task error: {0}")]
    Task(String),
    
    /// Stream processing error
    #[error("Stream error: {0}")]
    Stream(String),
    
    /// Cancellation was requested
    #[error("Scan cancelled")]
    Cancelled,
    
    /// Resource limit exceeded
    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),
    
    /// Invalid scan mode combination
    #[error("Invalid scan modes: {0:?}")]
    InvalidMode(ScanMode),
    
    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    /// Generic async operation error
    #[error("Async operation failed: {0}")]
    AsyncOperation(String),
    
    /// Wrapped errors from other sources
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ScanError {
    /// Create a repository error
    pub fn repository(msg: impl Into<String>) -> Self {
        Self::Repository(msg.into())
    }
    
    /// Create a task error
    pub fn task(msg: impl Into<String>) -> Self {
        Self::Task(msg.into())
    }
    
    /// Create a stream error
    pub fn stream(msg: impl Into<String>) -> Self {
        Self::Stream(msg.into())
    }
    
    /// Create a resource limit error
    pub fn resource_limit(msg: impl Into<String>) -> Self {
        Self::ResourceLimit(msg.into())
    }
    
    /// Create a configuration error
    pub fn configuration(msg: impl Into<String>) -> Self {
        Self::Configuration(msg.into())
    }
    
    /// Create an async operation error
    pub fn async_operation(msg: impl Into<String>) -> Self {
        Self::AsyncOperation(msg.into())
    }
}

/// Result type for async scanning operations
pub type ScanResult<T> = Result<T, ScanError>;

/// Convert from std::io::Error
impl From<std::io::Error> for ScanError {
    fn from(error: std::io::Error) -> Self {
        Self::Other(error.into())
    }
}

/// Convert from tokio::task::JoinError
impl From<tokio::task::JoinError> for ScanError {
    fn from(error: tokio::task::JoinError) -> Self {
        if error.is_cancelled() {
            Self::Cancelled
        } else {
            Self::Task(error.to_string())
        }
    }
}

/// Task-specific error information
#[derive(Debug, Clone)]
pub struct TaskError {
    pub task_id: String,
    pub mode: ScanMode,
    pub error: Arc<ScanError>,
}

impl TaskError {
    /// Create a new task error
    pub fn new(task_id: impl Into<String>, mode: ScanMode, error: ScanError) -> Self {
        Self {
            task_id: task_id.into(),
            mode,
            error: Arc::new(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_creation() {
        let err = ScanError::repository("test error");
        assert!(matches!(err, ScanError::Repository(_)));
        
        let err = ScanError::task("task failed");
        assert!(matches!(err, ScanError::Task(_)));
        
        let err = ScanError::Cancelled;
        assert!(matches!(err, ScanError::Cancelled));
    }
    
    #[test]
    fn test_error_display() {
        let err = ScanError::repository("connection failed");
        assert_eq!(err.to_string(), "Repository error: connection failed");
        
        let err = ScanError::InvalidMode(ScanMode::FILES | ScanMode::HISTORY);
        assert!(err.to_string().contains("Invalid scan modes"));
    }
    
    #[test]
    fn test_task_error() {
        let scan_err = ScanError::repository("test");
        let task_err = TaskError::new("task-1", ScanMode::FILES, scan_err);
        
        assert_eq!(task_err.task_id, "task-1");
        assert_eq!(task_err.mode, ScanMode::FILES);
        assert!(matches!(&*task_err.error, ScanError::Repository(_)));
    }
}