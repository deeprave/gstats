//! Generic Pub/Sub Notification System
//! 
//! A generic, reusable notification system for inter-subsystem communication.
//! Enables loose coupling between publishers and subscribers through event-driven architecture.
//! 
//! # Architecture
//! 
//! - **Publishers**: Components that emit events (scanners, queues, plugins)
//! - **Subscribers**: Components that handle events (plugins, export systems)
//! - **NotificationManager**: Central coordinator for event routing
//! - **Events**: Typed messages that flow through the system
//! 
//! # Example Usage
//! 
//! ```no_run
//! use gstats::notifications::{AsyncNotificationManager, ScanEvent};
//! use gstats::notifications::traits::NotificationManager;
//! use std::sync::Arc;
//! 
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create notification manager
//! let mut manager = AsyncNotificationManager::<ScanEvent>::new();
//! 
//! // Create and publish an event
//! let event = ScanEvent::started("scan_001".to_string());
//! manager.publish(event).await?;
//! # Ok(())
//! # }
//! ```

pub mod traits;
pub mod manager;
pub mod events;
pub mod error;

#[cfg(test)]
mod tests;

// Re-export core types for convenience
pub use manager::AsyncNotificationManager;
pub use events::ScanEvent;
pub use error::NotificationResult;

/// Module metadata
pub const MODULE_NAME: &str = "Generic Notification System";
pub const MODULE_VERSION: &str = "1.0.0";

/// Check if notification system is available
pub fn is_available() -> bool {
    true
}

/// Get notification system information
pub fn get_system_info() -> String {
    format!(
        "{} v{} - Generic pub/sub event system",
        MODULE_NAME,
        MODULE_VERSION
    )
}
