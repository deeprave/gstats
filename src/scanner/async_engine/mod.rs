//! Async Scanner Engine Module
//! 
//! Provides asynchronous scanning capabilities with streaming results
//! and concurrent task coordination.

pub mod error;
pub mod task_manager;
pub mod engine;
pub mod stream;
pub mod streaming_producer;
pub mod repository;
pub mod scanners;
pub mod events;
pub mod event_engine;
pub mod processors;
pub mod shared_state;
pub mod event_filtering;

// Re-export core types
// pub use error::ScanResult;
pub use engine::{AsyncScannerEngineBuilder, EngineStats};
pub use events::{RepositoryEvent, CommitInfo, FileChangeData, FileInfo, RepositoryStats, EventFilter, ChangeType};
pub use event_engine::RepositoryEventEngine;
pub use processors::{EventProcessor, ProcessorRegistry, ProcessorFactory, EventProcessingCoordinator};
pub use shared_state::{SharedProcessorState, RepositoryMetadata, ProcessorSharedData, CacheStats, SharedStateAccess};
pub use event_filtering::{AdvancedEventFilter, FilterDecision, ProcessorRouting, EventBatchingConfig, MemoryPressureMonitor};

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