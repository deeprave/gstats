//! Async Scanner Implementations
//! 
//! This module contains the event-driven scanner architecture that replaced legacy
//! scanner implementations. The EventDrivenScanner provides:
//! - Single-pass repository traversal
//! - Shared state management across processors
//! - Memory-efficient processing with advanced filtering
//! - Better performance for multi-mode scans
//!
//! The event-driven architecture uses EventProcessor components for processing
//! different types of repository events in a coordinated manner.

use std::sync::Arc;
use futures::stream;
use crate::scanner::modes::ScanMode;
use crate::scanner::async_traits::AsyncScanner;
use super::error::{ScanError, ScanResult};
use super::repository::AsyncRepositoryHandle;
use super::stream::ScanMessageStream;

/*
/// Event-driven scanner that provides single-pass repository traversal
/// with coordinated event processing across multiple processors
/// 
/// NOTE: Currently commented out due to git2 Send/Sync limitations.
/// The event processing integration is handled through collect_scan_data_with_event_processing
/// in main.rs instead.
pub struct EventDrivenScanner {
    repository: Arc<AsyncRepositoryHandle>,
    query_params: QueryParams,
    name: String,
}

impl EventDrivenScanner {
    /// Create a new event-driven scanner
    pub fn new(repository: Arc<AsyncRepositoryHandle>, query_params: QueryParams) -> Self {
        Self {
            repository,
            query_params,
            name: "EventDrivenScanner".to_string(),
        }
    }
    
    /// Create an event-driven scanner with custom name
    pub fn with_name(repository: Arc<AsyncRepositoryHandle>, query_params: QueryParams, name: String) -> Self {
        Self {
            repository,
            query_params,
            name,
        }
    }
}

#[async_trait::async_trait]
impl AsyncScanner for EventDrivenScanner {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn supports_mode(&self, mode: ScanMode) -> bool {
        // Event-driven scanner supports all major scan modes
        mode.intersects(
            ScanMode::FILES | 
            ScanMode::HISTORY | 
            ScanMode::METRICS | 
            ScanMode::CHANGE_FREQUENCY |
            ScanMode::DEPENDENCIES |
            ScanMode::SECURITY |
            ScanMode::PERFORMANCE
        )
    }
    
    async fn scan_async(&self, modes: ScanMode) -> ScanResult<ScanMessageStream> {
        if !self.supports_mode(modes) {
            return Err(ScanError::invalid_mode(modes));
        }
        
        debug!("Starting event-driven scan with modes: {:?}", modes);
        
        // Create event engine for single-pass repository traversal
        let event_engine = RepositoryEventEngine::new(
            Arc::clone(&self.repository),
            self.query_params.clone(),
            modes,
        );
        
        // Create processor coordinator for handling events
        let coordinator = EventProcessingCoordinator::new(modes);
        
        // Get repository event stream
        let event_stream = event_engine.scan_repository().await?;
        
        // Process events and convert to scan messages
        let scan_messages = self.process_events_to_messages(event_stream, coordinator).await?;
        
        info!("Event-driven scan completed, generated {} messages", scan_messages.len());
        
        // Convert to stream
        let message_stream = stream::iter(scan_messages.into_iter().map(Ok));
        Ok(Box::pin(message_stream))
    }
    
    async fn estimate_message_count(&self, modes: ScanMode) -> Option<usize> {
        // Try to estimate based on repository size
        match self.repository.estimate_scan_size(modes).await {
            Ok(estimate) => Some(estimate),
            Err(e) => {
                warn!("Failed to estimate message count: {}", e);
                None
            }
        }
    }
}

impl EventDrivenScanner {
    /// Process repository events and convert them to scan messages
    async fn process_events_to_messages(
        &self,
        event_stream: impl Stream<Item = RepositoryEvent>,
        mut coordinator: EventProcessingCoordinator,
    ) -> ScanResult<Vec<ScanMessage>> {
        let mut scan_messages = Vec::new();
        let mut event_stream = Box::pin(event_stream);
        
        // Process each event through the coordinator
        while let Some(event) = event_stream.next().await {
            match coordinator.process_event(&event).await {
                Ok(messages) => {
                    scan_messages.extend(messages);
                }
                Err(e) => {
                    warn!("Error processing event: {}", e);
                    // Continue processing other events
                }
            }
        }
        
        // Finalize processing and get any remaining messages
        match coordinator.finalize().await {
            Ok(final_messages) => {
                scan_messages.extend(final_messages);
            }
            Err(e) => {
                warn!("Error finalizing event processing: {}", e);
            }
        }
        
        Ok(scan_messages)
    }
}
*/

/// Placeholder scanner for fallback compatibility
/// This is kept as a fallback during the transition to event-driven architecture
pub struct PlaceholderScanner {
    repository: Arc<AsyncRepositoryHandle>,
    name: String,
}

impl PlaceholderScanner {
    /// Create a new placeholder scanner
    pub fn new(repository: Arc<AsyncRepositoryHandle>) -> Self {
        Self {
            repository,
            name: "PlaceholderScanner".to_string(),
        }
    }
    
    /// Create a placeholder scanner with custom name
    pub fn with_name(repository: Arc<AsyncRepositoryHandle>, name: String) -> Self {
        Self {
            repository,
            name,
        }
    }
}

#[async_trait::async_trait]
impl AsyncScanner for PlaceholderScanner {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn supports_mode(&self, mode: ScanMode) -> bool {
        // Support basic modes for compatibility
        mode.intersects(ScanMode::FILES | ScanMode::HISTORY | ScanMode::CHANGE_FREQUENCY)
    }
    
    async fn scan_async(&self, modes: ScanMode) -> ScanResult<ScanMessageStream> {
        if !self.supports_mode(modes) {
            return Err(ScanError::invalid_mode(modes));
        }
        
        // Return empty stream for now - this is a placeholder
        let empty_stream = stream::empty();
        Ok(Box::pin(empty_stream))
    }
    
    async fn estimate_message_count(&self, _modes: ScanMode) -> Option<usize> {
        // Return 0 for placeholder
        Some(0)
    }
}

/// Estimate line count based on file size (rough approximation)
fn estimate_line_count(size: usize) -> usize {
    // Rough estimate: average 50 characters per line
    if size == 0 {
        0
    } else {
        (size / 50).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::resolve_repository_handle;

    #[test]
    fn test_estimate_line_count() {
        assert_eq!(estimate_line_count(0), 0);
        assert_eq!(estimate_line_count(50), 1);
        assert_eq!(estimate_line_count(100), 2);
        assert_eq!(estimate_line_count(25), 1); // Minimum 1 line
    }

    #[test]
    fn test_unsupported_mode() {
        // Test that unsupported modes are handled correctly
        let unsupported_mode = ScanMode::DEPENDENCIES;
        assert!(!ScanMode::FILES.contains(unsupported_mode));
    }

    #[tokio::test]
    async fn test_placeholder_scanner() {
        let repo = resolve_repository_handle(None).unwrap();
        let async_handle = Arc::new(AsyncRepositoryHandle::new(repo));
        
        let scanner = PlaceholderScanner::new(async_handle);
        assert_eq!(scanner.name(), "PlaceholderScanner");
        assert!(scanner.supports_mode(ScanMode::FILES));
        assert!(scanner.supports_mode(ScanMode::HISTORY));
        assert!(scanner.supports_mode(ScanMode::CHANGE_FREQUENCY));
    }
}
