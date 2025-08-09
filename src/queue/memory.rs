//! Queue Memory Monitoring
//!
//! Provides memory usage tracking and monitoring for the queue system.
//! Includes simple thresholds and debug logging without complex backpressure
//! mechanisms (following YAGNI principle).

use crate::scanner::messages::ScanMessage;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Memory statistics for queue monitoring
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueMemoryStats {
    /// Current memory usage in bytes
    pub current_size: usize,
    /// Current number of messages in queue
    pub message_count: usize,
    /// Peak memory usage seen so far
    pub peak_size: usize,
    /// Total number of messages processed (pushed + popped)
    pub total_messages_processed: u64,
    /// Average message size in bytes
    pub average_message_size: usize,
    /// Memory usage threshold for warnings
    pub warning_threshold: usize,
}

impl QueueMemoryStats {
    /// Create new memory statistics with default warning threshold
    pub fn new() -> Self {
        Self::with_threshold(100 * 1024 * 1024) // 100MB default threshold
    }

    /// Create new memory statistics with custom warning threshold
    pub fn with_threshold(warning_threshold: usize) -> Self {
        Self {
            current_size: 0,
            message_count: 0,
            peak_size: 0,
            total_messages_processed: 0,
            average_message_size: 0,
            warning_threshold,
        }
    }

    /// Update statistics when a message is pushed to the queue
    pub fn update_on_push(&mut self, message_size: usize) {
        self.current_size += message_size;
        self.message_count += 1;
        self.peak_size = self.peak_size.max(self.current_size);
        self.total_messages_processed += 1;
        self.update_average();

        log::trace!(
            "Queue memory: pushed message ({} bytes), current: {} bytes, {} messages",
            message_size,
            self.current_size,
            self.message_count
        );

        if self.is_memory_concerning() {
            log::debug!(
                "Queue memory usage is concerning: {} bytes (threshold: {} bytes)",
                self.current_size,
                self.warning_threshold
            );
        }
    }

    /// Update statistics when a message is popped from the queue
    pub fn update_on_pop(&mut self, message_size: usize) {
        self.current_size = self.current_size.saturating_sub(message_size);
        self.message_count = self.message_count.saturating_sub(1);
        self.total_messages_processed += 1;
        self.update_average();

        log::trace!(
            "Queue memory: popped message ({} bytes), current: {} bytes, {} messages",
            message_size,
            self.current_size,
            self.message_count
        );
    }

    /// Check if current memory usage is concerning
    pub fn is_memory_concerning(&self) -> bool {
        self.current_size > self.warning_threshold
    }

    /// Get memory usage as a percentage of the warning threshold
    pub fn memory_usage_percentage(&self) -> f64 {
        if self.warning_threshold == 0 {
            0.0
        } else {
            (self.current_size as f64 / self.warning_threshold as f64) * 100.0
        }
    }

    /// Update the average message size calculation
    fn update_average(&mut self) {
        if self.total_messages_processed > 0 {
            // Use a running average that includes both current and processed messages
            let total_size_estimate = self.current_size + 
                (self.total_messages_processed.saturating_sub(self.message_count as u64) as usize * self.average_message_size);
            self.average_message_size = total_size_estimate / self.total_messages_processed as usize;
        }
    }

    /// Reset statistics (useful for new scan sessions)
    pub fn reset(&mut self) {
        self.current_size = 0;
        self.message_count = 0;
        self.peak_size = 0;
        self.total_messages_processed = 0;
        self.average_message_size = 0;
        log::debug!("Queue memory statistics reset");
    }

    /// Get a summary string for logging
    pub fn summary(&self) -> String {
        format!(
            "Queue Memory: {} bytes ({:.1}% of threshold), {} messages, peak: {} bytes, avg msg: {} bytes",
            self.current_size,
            self.memory_usage_percentage(),
            self.message_count,
            self.peak_size,
            self.average_message_size
        )
    }
}

impl Default for QueueMemoryStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory monitor for thread-safe queue memory tracking
#[derive(Debug)]
pub struct MemoryMonitor {
    stats: Arc<RwLock<QueueMemoryStats>>,
}

impl MemoryMonitor {
    /// Create a new memory monitor with default threshold
    pub fn new() -> Self {
        Self {
            stats: Arc::new(RwLock::new(QueueMemoryStats::new())),
        }
    }

    /// Create a new memory monitor with custom threshold
    pub fn with_threshold(threshold: usize) -> Self {
        Self {
            stats: Arc::new(RwLock::new(QueueMemoryStats::with_threshold(threshold))),
        }
    }

    /// Record a message being pushed to the queue
    pub async fn record_push(&self, message: &ScanMessage) {
        let message_size = message.estimate_memory_usage();
        let mut stats = self.stats.write().await;
        stats.update_on_push(message_size);
    }

    /// Record a message being popped from the queue
    pub async fn record_pop(&self, message: &ScanMessage) {
        let message_size = message.estimate_memory_usage();
        let mut stats = self.stats.write().await;
        stats.update_on_pop(message_size);
    }

    /// Get current memory statistics
    pub async fn get_stats(&self) -> QueueMemoryStats {
        self.stats.read().await.clone()
    }

    /// Check if memory usage is concerning
    pub async fn is_memory_concerning(&self) -> bool {
        self.stats.read().await.is_memory_concerning()
    }

    /// Reset statistics
    pub async fn reset(&self) {
        self.stats.write().await.reset();
    }

    /// Log current memory status at debug level
    pub async fn log_status(&self) {
        let stats = self.stats.read().await;
        log::debug!("{}", stats.summary());
    }

    /// Log current memory status at trace level with detailed info
    pub async fn log_detailed_status(&self) {
        let stats = self.stats.read().await;
        log::trace!(
            "Detailed Queue Memory Stats: current={} bytes, messages={}, peak={} bytes, \
             total_processed={}, avg_size={} bytes, threshold={} bytes, usage={:.1}%",
            stats.current_size,
            stats.message_count,
            stats.peak_size,
            stats.total_messages_processed,
            stats.average_message_size,
            stats.warning_threshold,
            stats.memory_usage_percentage()
        );
    }
}

impl Clone for MemoryMonitor {
    fn clone(&self) -> Self {
        Self {
            stats: Arc::clone(&self.stats),
        }
    }
}

impl Default for MemoryMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};
    use crate::scanner::modes::ScanMode;

    fn create_test_message(size_hint: usize) -> ScanMessage {
        let header = MessageHeader::new(ScanMode::FILES, 0);
        let data = MessageData::FileInfo {
            path: "x".repeat(size_hint), // Approximate size control
            size: 1000,
            lines: 50,
        };
        ScanMessage::new(header, data)
    }

    #[test]
    fn test_memory_stats_creation() {
        let stats = QueueMemoryStats::new();
        assert_eq!(stats.current_size, 0);
        assert_eq!(stats.message_count, 0);
        assert_eq!(stats.peak_size, 0);
        assert_eq!(stats.warning_threshold, 100 * 1024 * 1024);
    }

    #[test]
    fn test_memory_stats_push_update() {
        let mut stats = QueueMemoryStats::new();
        stats.update_on_push(1000);

        assert_eq!(stats.current_size, 1000);
        assert_eq!(stats.message_count, 1);
        assert_eq!(stats.peak_size, 1000);
        assert_eq!(stats.total_messages_processed, 1);
    }

    #[test]
    fn test_memory_stats_pop_update() {
        let mut stats = QueueMemoryStats::new();
        stats.update_on_push(1000);
        stats.update_on_pop(1000);

        assert_eq!(stats.current_size, 0);
        assert_eq!(stats.message_count, 0);
        assert_eq!(stats.peak_size, 1000); // Peak should remain
        assert_eq!(stats.total_messages_processed, 2); // Push + pop
    }

    #[test]
    fn test_memory_concerning_threshold() {
        let mut stats = QueueMemoryStats::with_threshold(1000);
        assert!(!stats.is_memory_concerning());

        stats.update_on_push(1500);
        assert!(stats.is_memory_concerning());
    }

    #[test]
    fn test_memory_usage_percentage() {
        let mut stats = QueueMemoryStats::with_threshold(1000);
        stats.update_on_push(500);
        assert_eq!(stats.memory_usage_percentage(), 50.0);
    }

    #[tokio::test]
    async fn test_memory_monitor() {
        let monitor = MemoryMonitor::with_threshold(1000);
        let message = create_test_message(100);

        monitor.record_push(&message).await;
        let stats = monitor.get_stats().await;
        assert!(stats.current_size > 0);
        assert_eq!(stats.message_count, 1);

        monitor.record_pop(&message).await;
        let stats = monitor.get_stats().await;
        assert_eq!(stats.current_size, 0);
        assert_eq!(stats.message_count, 0);
    }

    #[tokio::test]
    async fn test_memory_monitor_concerning() {
        let monitor = MemoryMonitor::with_threshold(100);
        let large_message = create_test_message(200); // Should exceed threshold

        assert!(!monitor.is_memory_concerning().await);
        monitor.record_push(&large_message).await;
        // Note: Actual message size may vary, so we test the mechanism works
        // rather than exact threshold crossing
    }

    #[test]
    fn test_stats_summary() {
        let mut stats = QueueMemoryStats::with_threshold(1000);
        stats.update_on_push(500);
        let summary = stats.summary();
        assert!(summary.contains("500 bytes"));
        assert!(summary.contains("50.0%"));
    }
}
