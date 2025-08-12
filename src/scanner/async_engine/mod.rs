//! Async Scanner Engine Module
//! 
//! Provides asynchronous scanning capabilities with streaming results
//! and concurrent task coordination.

pub mod error;
pub mod task_manager;
pub mod engine;
pub mod stream;
pub mod streaming_producer;
pub mod scanners;
pub mod events;
// pub mod event_engine; // Temporarily disabled during AsyncRepositoryHandler removal
pub mod processors;
pub mod shared_state;
pub mod event_filtering;

#[cfg(test)]
mod tests;

// Re-export core types
// pub use error::ScanResult;
pub use engine::AsyncScannerEngineBuilder;
// Removed unused exports: RepositoryEvent, CommitInfo, FileChangeData, FileInfo, RepositoryStats, EventFilter, ChangeType
// Removed unused export: RepositoryEventEngine
// Removed unused exports: EventProcessor, ProcessorRegistry, ProcessorFactory, EventProcessingCoordinator
// Removed unused exports: SharedProcessorState, RepositoryMetadata, ProcessorSharedData, CacheStats, SharedStateAccess
// Removed unused exports: AdvancedEventFilter, FilterDecision, ProcessorRouting, EventBatchingConfig, MemoryPressureMonitor

// Re-export statistics types for convenience

// Module metadata
const MODULE_NAME: &str = "Async Scanner Engine";
const MODULE_VERSION: &str = "1.0.0";

/// Check if async engine is available
pub fn is_async_available() -> bool {
    // Could check for specific runtime features here
    true
}

/// Get async engine information
pub fn get_engine_info() -> String {
    format!(
        "{} v{} - Async scanning with streaming support",
        MODULE_NAME,
        MODULE_VERSION
    )
}