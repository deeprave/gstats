//! Async Scanner Error Types
//! 
//! Error types specific to async scanning operations.

use thiserror::Error;
use std::sync::Arc;

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
    #[error("Analysis was cancelled")]
    Cancelled,
    
    /// Resource limit exceeded
    #[error("Resource limit exceeded: {0}\n\nTry reducing the scope with filters like --since or --max-files, or increase memory limits.")]
    ResourceLimit(String),
    
    /// Invalid scan mode combination
    #[error("The requested analysis mode is not available: {0}\n\nAvailable options:\n  • 'gstats commits' - Analyze commit history and contributors\n  • 'gstats metrics' - Analyze code metrics (requires metrics plugin)\n\nRun 'gstats --help' to see all available commands.")]
    InvalidMode(String),
    
    /// Configuration error
    #[error("Configuration problem: {0}\n\nCheck your configuration file or command line arguments. Run 'gstats --help' for usage information.")]
    Configuration(String),
    
    /// Generic async operation error
    #[error("Analysis operation failed: {0}")]
    AsyncOperation(String),
    
    /// Wrapped errors from other sources
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ScanError {
    /// Create a repository error with context
    pub fn repository(msg: impl Into<String>) -> Self {
        let msg = msg.into();
        let enhanced_msg = if msg.contains("not a git repository") {
            format!("{msg}\n\nMake sure you're running this command from within a git repository, or specify a repository path with the --repository option.")
        } else if msg.contains("Permission denied") {
            format!("{msg}\n\nCheck that you have read access to the repository directory and files.")
        } else {
            format!("{msg}\n\nVerify the repository path exists and is accessible.")
        };
        Self::Repository(enhanced_msg)
    }
    
    /// Create a repository error for a specific path
    pub fn repository_with_path(msg: impl Into<String>, path: impl AsRef<std::path::Path>) -> Self {
        let path_display = path.as_ref().display();
        let msg = msg.into();
        let enhanced_msg = format!("{msg}\n\nRepository path: {path_display}\n\nSuggestions:\n  • Check that the path exists and is accessible\n  • Ensure it's a valid git repository\n  • Verify you have proper permissions");
        Self::Repository(enhanced_msg)
    }
    
    /// Create a task error
    pub fn task(msg: impl Into<String>) -> Self {
        Self::Task(msg.into())
    }
    
    /// Create a stream error
    pub fn stream(msg: impl Into<String>) -> Self {
        Self::Stream(msg.into())
    }
    
    /// Create a processing error
    pub fn processing(msg: impl Into<String>) -> Self {
        Self::Task(msg.into()) // Map to Task variant for now
    }
    
    /// Create a resource limit error
    pub fn resource_limit(msg: impl Into<String>) -> Self {
        Self::ResourceLimit(msg.into())
    }
    
    /// Create a configuration error with helpful suggestions
    pub fn configuration(msg: impl Into<String>) -> Self {
        Self::Configuration(msg.into())
    }
    
    /// Create a configuration error for missing scanners
    pub fn no_scanners_registered() -> Self {
        Self::Configuration("No analysis modules are available.\n\nThis usually means plugins failed to load. Try:\n  • Running 'gstats commits' for basic commit analysis\n  • Checking your plugin configuration\n  • Reinstalling gstats if the problem persists".to_string())
    }
    
    /// Create an async operation error
    pub fn async_operation(msg: impl Into<String>) -> Self {
        Self::AsyncOperation(msg.into())
    }
    
    pub fn invalid_mode(mode: &str) -> Self {
        Self::InvalidMode(mode.to_string())
    }
}

/// Result type for async scanning operations
pub type ScanResult<T> = Result<T, ScanError>;

/// Convert from std::io::Error
impl From<std::io::Error> for ScanError {
    fn from(error: std::io::Error) -> Self {
        let user_msg = match error.kind() {
            std::io::ErrorKind::NotFound => {
                format!("File or directory not found: {error}\n\nCheck that the path exists and is spelled correctly.")
            },
            std::io::ErrorKind::PermissionDenied => {
                format!("Permission denied: {error}\n\nYou don't have the necessary permissions to access this file or directory.\nTry:\n  • Running with appropriate permissions\n  • Checking file/directory ownership\n  • Ensuring the path is readable")
            },
            std::io::ErrorKind::ConnectionRefused => {
                format!("Connection refused: {error}\n\nThis usually indicates a network or service issue.")
            },
            std::io::ErrorKind::TimedOut => {
                format!("Operation timed out: {error}\n\nThe operation took too long to complete. Try again or check your network connection.")
            },
            _ => {
                format!("File system error: {error}\n\nCheck the file path and your system permissions.")
            }
        };
        Self::Other(anyhow::anyhow!(user_msg))
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
    pub error: Arc<ScanError>,
}

impl TaskError {
    /// Create a new task error
    pub fn new(task_id: impl Into<String>, error: ScanError) -> Self {
        Self {
            task_id: task_id.into(),
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
        assert!(err.to_string().contains("Repository error: connection failed"));
        
        let err = ScanError::InvalidMode("invalid_mode".to_string());
        assert!(err.to_string().contains("The requested analysis mode is not available"));
    }
    
    #[test]
    fn test_task_error() {
        let scan_err = ScanError::repository("test");
        let task_err = TaskError::new("task-1", scan_err);
        
        assert_eq!(task_err.task_id, "task-1");
        assert!(matches!(&*task_err.error, ScanError::Repository(_)));
    }
}