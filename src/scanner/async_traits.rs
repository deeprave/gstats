//! Async Scanner Traits
//! 
//! Trait definitions for async scanning operations with streaming support.

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use crate::scanner::modes::ScanMode;
use crate::scanner::messages::ScanMessage;
use crate::scanner::async_engine::error::{ScanError, ScanResult};

/// Type alias for a boxed stream of scan messages
pub type ScanMessageStream = Pin<Box<dyn Stream<Item = ScanResult<ScanMessage>> + Send>>;

/// Async scanner trait with streaming interface and repository-owning pattern
#[async_trait]
pub trait AsyncScanner: Send + Sync {
    /// Get the name of this scanner
    fn name(&self) -> &str;
    
    /// Check if this scanner supports the given mode
    fn supports_mode(&self, mode: ScanMode) -> bool;
    
    /// Perform an async scan with the specified modes on the given repository path
    /// Returns a stream of scan messages
    /// 
    /// # Arguments
    /// * `repository_path` - Path to the git repository to scan
    /// * `modes` - Scan modes to execute
    /// 
    /// # Repository-Owning Pattern
    /// Each scanner creates its own repository access using spawn_blocking,
    /// eliminating the need for async-safe repository sharing.
    async fn scan_async(&self, repository_path: &std::path::Path, modes: ScanMode) -> ScanResult<ScanMessageStream>;
    
    /// Get estimated message count for progress tracking (optional)
    /// 
    /// # Arguments
    /// * `repository_path` - Path to the git repository
    /// * `modes` - Scan modes to estimate for
    async fn estimate_message_count(&self, _repository_path: &std::path::Path, _modes: ScanMode) -> Option<usize> {
        None
    }
}

/// Streaming message producer trait
#[async_trait]
pub trait StreamingMessageProducer: Send + Sync {
    /// Produce a single message asynchronously
    async fn produce_message(&self, message: ScanMessage) -> ScanResult<()>;
    
    /// Produce a batch of messages efficiently
    async fn produce_batch(&self, messages: Vec<ScanMessage>) -> ScanResult<()> {
        for message in messages {
            self.produce_message(message).await?;
        }
        Ok(())
    }
    
    /// Get the producer name
    fn name(&self) -> &str;
}

/// Async scan progress tracker
#[async_trait]
pub trait AsyncScanProgress: Send + Sync {
    /// Update progress with current count
    async fn update_progress(&self, current: usize, total: Option<usize>);
    
    /// Mark scan as completed
    async fn complete(&self);
    
    /// Report an error
    async fn report_error(&self, error: &ScanError);
}

/// Factory trait for creating scanners
#[async_trait]
pub trait AsyncScannerFactory: Send + Sync {
    /// Create a scanner for the specified repository
    async fn create_scanner(&self, repository_path: &str) -> ScanResult<Box<dyn AsyncScanner>>;
    
    /// Get factory name
    fn name(&self) -> &str;
}

/*
// Temporarily disabled during repository-owning pattern migration
#[cfg(test)]
mod tests {
    // ... test code commented out ...
}
*/
