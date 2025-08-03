//! Simple Debug and Logging Support
//! 
//! Provides basic debugging and status reporting for the queue system
//! without the overhead of a full monitoring infrastructure.

use crate::queue::{MemoryQueue, ConsumerMetrics, MemoryPressureLevel};
use std::fmt::Write;

/// Simple status information for debugging
#[derive(Debug, Clone)]
pub struct QueueDebugStatus {
    pub message_count: usize,
    pub memory_usage_mb: f64,
    pub memory_usage_percent: f64,
    pub memory_pressure: MemoryPressureLevel,
    pub capacity_used_percent: f64,
    pub backoff_active: bool,
    pub consumer_running: bool,
}

/// Debug trait for queue components
pub trait QueueDebug {
    /// Get a simple one-line status string
    fn debug_status(&self) -> String;
    
    /// Get detailed debug information
    fn debug_info(&self) -> String;
}

impl QueueDebug for MemoryQueue {
    fn debug_status(&self) -> String {
        format!(
            "Queue: {} msgs, {:.1}MB ({:.1}%), pressure={:?}",
            self.size(),
            self.memory_usage() as f64 / 1024.0 / 1024.0,
            self.memory_usage_percent(),
            self.memory_tracker.as_ref()
                .map(|t| t.get_pressure_level())
                .unwrap_or(MemoryPressureLevel::Normal)
        )
    }
    
    fn debug_info(&self) -> String {
        let mut info = String::new();
        writeln!(info, "=== Memory Queue Debug Info ===").unwrap();
        writeln!(info, "Messages: {} / {} capacity", self.size(), self.capacity()).unwrap();
        writeln!(info, "Memory: {:.2}MB used ({:.1}%)", 
            self.memory_usage() as f64 / 1024.0 / 1024.0,
            self.memory_usage_percent()
        ).unwrap();
        
        if let Some(tracker) = &self.memory_tracker {
            writeln!(info, "Memory Limit: {:.2}MB", 
                tracker.memory_limit() as f64 / 1024.0 / 1024.0
            ).unwrap();
            writeln!(info, "Pressure Level: {:?}", tracker.get_pressure_level()).unwrap();
        }
        
        let backoff_metrics = self.get_backoff_metrics();
        if backoff_metrics.total_backoff_events > 0 {
            writeln!(info, "Backoff Events: {}", backoff_metrics.total_backoff_events).unwrap();
            writeln!(info, "Avg Backoff Delay: {:?}", backoff_metrics.average_backoff_delay).unwrap();
        }
        
        info
    }
}

/// Format consumer metrics for logging
pub fn format_consumer_metrics(metrics: &ConsumerMetrics) -> String {
    format!(
        "Consumer: {} processed, {} notified, {} errors, {:.2} avg batch, {:?} avg latency",
        metrics.messages_processed,
        metrics.notifications_sent,
        metrics.notification_errors,
        metrics.average_batch_size,
        metrics.average_notification_latency
    )
}

/// Simple logging macros for queue operations
#[macro_export]
macro_rules! queue_debug {
    ($($arg:tt)*) => {
        log::debug!(target: "gstats::queue", $($arg)*);
    };
}

#[macro_export]
macro_rules! queue_info {
    ($($arg:tt)*) => {
        log::info!(target: "gstats::queue", $($arg)*);
    };
}

#[macro_export]
macro_rules! queue_warn {
    ($($arg:tt)*) => {
        log::warn!(target: "gstats::queue", $($arg)*);
    };
}

#[macro_export]
macro_rules! queue_error {
    ($($arg:tt)*) => {
        log::error!(target: "gstats::queue", $($arg)*);
    };
}

/// Helper to log queue status periodically
pub struct QueueStatusLogger {
    last_log: std::time::Instant,
    log_interval: std::time::Duration,
}

impl QueueStatusLogger {
    pub fn new(log_interval: std::time::Duration) -> Self {
        Self {
            last_log: std::time::Instant::now(),
            log_interval,
        }
    }
    
    /// Log status if enough time has passed
    pub fn maybe_log_status(&mut self, queue: &MemoryQueue) {
        if self.last_log.elapsed() >= self.log_interval {
            queue_info!("{}", queue.debug_status());
            self.last_log = std::time::Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::memory_tracker::MemoryTracker;
    use std::sync::Arc;
    
    #[test]
    fn test_debug_status() {
        let tracker = Arc::new(MemoryTracker::new(1024 * 1024)); // 1MB
        let queue = MemoryQueue::with_shared_tracker(100, 1024 * 1024, tracker);
        
        let status = queue.debug_status();
        assert!(status.contains("Queue:"));
        assert!(status.contains("msgs"));
        assert!(status.contains("MB"));
        assert!(status.contains("pressure=Normal"));
    }
    
    #[test]
    fn test_debug_info() {
        let queue = MemoryQueue::new(100, 1024 * 1024);
        
        let info = queue.debug_info();
        assert!(info.contains("Memory Queue Debug Info"));
        assert!(info.contains("Messages:"));
        assert!(info.contains("Memory:"));
        assert!(info.contains("capacity"));
    }
    
    #[test]
    fn test_format_consumer_metrics() {
        let metrics = ConsumerMetrics {
            messages_processed: 100,
            notifications_sent: 95,
            notification_errors: 5,
            batches_processed: 10,
            average_batch_size: 10.0,
            average_notification_latency: std::time::Duration::from_millis(5),
        };
        
        let formatted = format_consumer_metrics(&metrics);
        assert!(formatted.contains("100 processed"));
        assert!(formatted.contains("95 notified"));
        assert!(formatted.contains("5 errors"));
        assert!(formatted.contains("10.00 avg batch"));
    }
}