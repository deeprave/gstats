//! Generic Event Processor Trait
//! 
//! This trait provides a common interface for event processing that can be used
//! both in the scanner module and in plugin modules.

use crate::scanner::async_engine::events::RepositoryEvent;
use crate::scanner::async_engine::shared_state::{SharedProcessorState, RepositoryMetadata};
use crate::scanner::messages::ScanMessage;
use crate::plugin::PluginResult;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

/// Statistics for event processor performance monitoring
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ProcessorStats {
    pub events_processed: usize,
    pub messages_generated: usize,
    pub processing_time: Duration,
    pub errors_encountered: usize,
}

/// Generic trait for processing repository events and generating scan messages
/// 
/// This trait can be implemented by both scanner processors and plugin processors,
/// providing a unified interface for event-driven analysis.
#[async_trait]
pub trait EventProcessor: Send + Sync {
    /// Get a unique name for this processor
    fn name(&self) -> &'static str;

    /// Set shared state for cross-processor communication (optional)
    fn set_shared_state(&mut self, shared_state: Arc<SharedProcessorState>) {
        // Default implementation - processors can override if they need shared state
        let _ = shared_state; // Suppress unused parameter warning
    }

    /// Get access to shared state (optional)
    fn shared_state(&self) -> Option<&Arc<SharedProcessorState>> {
        // Default implementation - processors can override if they use shared state
        None
    }

    /// Initialize the processor before event processing begins
    async fn initialize(&mut self) -> PluginResult<()> {
        Ok(())
    }

    /// Handle repository metadata (called once at the start of processing)
    async fn on_repository_metadata(&mut self, _metadata: &RepositoryMetadata) -> PluginResult<()> {
        Ok(())
    }

    /// Process a single repository event and generate scan messages
    /// 
    /// This is the core method that processors implement to handle events.
    /// Events can be commits, file changes, repository lifecycle events, etc.
    async fn process_event(&mut self, event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>>;

    /// Finalize processing and generate any accumulated results
    /// 
    /// Called after all events have been processed. Processors can use this
    /// to generate summary messages or perform final calculations.
    async fn finalize(&mut self) -> PluginResult<Vec<ScanMessage>> {
        Ok(vec![])
    }

    /// Get processing statistics (optional)
    fn get_stats(&self) -> ProcessorStats {
        ProcessorStats::default()
    }

    /// Check if this processor should handle the given event (optional)
    /// 
    /// Default implementation accepts all events. Processors can override
    /// to filter events based on their specific needs.
    fn should_process_event(&self, _event: &RepositoryEvent) -> bool {
        true
    }

    /// Get processor configuration (optional)
    /// 
    /// Processors can return configuration information that can be used
    /// by the system for optimization or debugging.
    fn get_config(&self) -> ProcessorConfig {
        ProcessorConfig::default()
    }
}

/// Configuration information for a processor
#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    /// Whether this processor requires shared state
    pub requires_shared_state: bool,
    /// Whether this processor generates messages during processing or only at finalization
    pub generates_streaming_messages: bool,
    /// Estimated memory usage per event (in bytes)
    pub estimated_memory_per_event: usize,
    /// Whether this processor can be run concurrently with others
    pub supports_concurrent_processing: bool,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            requires_shared_state: false,
            generates_streaming_messages: false,
            estimated_memory_per_event: 1024, // 1KB default estimate
            supports_concurrent_processing: true,
        }
    }
}

/// Trait for accessing shared state (helper trait)
pub trait SharedStateAccess {
    fn shared_state(&self) -> &SharedProcessorState;
}

/// Helper macro for implementing EventProcessor with common patterns
#[macro_export]
macro_rules! impl_event_processor {
    ($processor:ty, $name:expr) => {
        #[async_trait]
        impl EventProcessor for $processor {
            fn name(&self) -> &'static str {
                $name
            }

            async fn process_event(&mut self, event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
                self.handle_event(event).await
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::async_engine::events::{RepositoryEvent, CommitInfo};
    use std::time::SystemTime;

    struct TestProcessor {
        name: String,
        events_processed: usize,
    }

    impl TestProcessor {
        fn new(name: String) -> Self {
            Self {
                name,
                events_processed: 0,
            }
        }
    }

    #[async_trait]
    impl EventProcessor for TestProcessor {
        fn name(&self) -> &'static str {
            "test_processor"
        }

        async fn process_event(&mut self, _event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
            self.events_processed += 1;
            Ok(vec![])
        }

        fn get_stats(&self) -> ProcessorStats {
            ProcessorStats {
                events_processed: self.events_processed,
                messages_generated: 0,
                processing_time: Duration::from_millis(10),
                errors_encountered: 0,
            }
        }
    }

    #[tokio::test]
    async fn test_event_processor_trait() {
        let mut processor = TestProcessor::new("test".to_string());
        
        assert_eq!(processor.name(), "test_processor");
        // Event processor no longer uses scan modes
        
        // Test initialization
        processor.initialize().await.unwrap();
        
        // Test event processing
        let _commit = CommitInfo {
            hash: "abc123".to_string(),
            short_hash: "abc123".to_string(),
            author_name: "Test Author".to_string(),
            author_email: "test@example.com".to_string(),
            committer_name: "Test Author".to_string(),
            committer_email: "test@example.com".to_string(),
            timestamp: SystemTime::now(),
            message: "Test commit".to_string(),
            parent_hashes: vec![],
            changed_files: vec![],
            insertions: 0,
            deletions: 0,
        };
        
        let event = RepositoryEvent::RepositoryStarted {
            total_commits: Some(1),
            total_files: Some(1),
        };
        
        let messages = processor.process_event(&event).await.unwrap();
        assert!(messages.is_empty());
        
        // Test finalization
        let final_messages = processor.finalize().await.unwrap();
        assert!(final_messages.is_empty());
        
        // Test stats
        let stats = processor.get_stats();
        assert_eq!(stats.events_processed, 1);
    }

    #[tokio::test]
    async fn test_processor_config() {
        let config = ProcessorConfig::default();
        assert!(!config.requires_shared_state);
        assert!(!config.generates_streaming_messages);
        assert!(config.supports_concurrent_processing);
        assert_eq!(config.estimated_memory_per_event, 1024);
    }
}
