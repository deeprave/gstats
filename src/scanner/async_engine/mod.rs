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

// Re-export core types
pub use error::{ScanError, ScanResult};
pub use engine::{AsyncScannerEngine, AsyncScannerEngineBuilder, EngineStats};

// Re-export statistics types for convenience

// Module metadata
pub const MODULE_NAME: &str = "Async Scanner Engine";
pub const MODULE_VERSION: &str = "1.0.0";

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