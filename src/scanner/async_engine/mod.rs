//! Async Scanner Manager Module
//! 
//! Manages EventDrivenScanner instances for repository scanning.

pub mod error;
pub mod task_manager;
pub mod engine;
pub mod scanners;
pub mod events;
pub mod processors;
pub mod shared_state;
pub mod diff_analyzer;
pub mod file_tracker;
pub mod checkout_manager;

#[cfg(test)]
mod tests;

// Re-export core types
pub use engine::{AsyncScannerManager, AsyncScannerManagerBuilder};

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
    format!("{MODULE_NAME} v{MODULE_VERSION} - Async scanning with streaming support")
}