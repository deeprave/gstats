//! Shared Message Queue Implementation
//!
//! Multi-consumer queue implementation providing producer and consumer interfaces
//! for scanner-to-plugin message coordination. Built on MultiConsumerQueue for
//! efficient parallel message processing.

use crate::queue::error::QueueResult;
use crate::queue::{MultiConsumerQueue, MultiConsumerConfig, QueueConsumer, ConsumerSummary};
use crate::scanner::messages::ScanMessage;
use std::sync::Arc;

/// Shared message queue for multi-consumer scanner-to-plugin coordination
/// 
/// This is a clean wrapper around MultiConsumerQueue providing a unified
/// interface for both producers (scanners) and consumers (plugins).
#[derive(Debug, Clone)]
pub struct SharedMessageQueue {
    /// The underlying multi-consumer queue implementation
    queue: Arc<MultiConsumerQueue>,
}

impl SharedMessageQueue {
    /// Create a new shared message queue for the given scan session
    pub fn new(scan_id: String) -> Self {
        Self {
            queue: Arc::new(MultiConsumerQueue::new(scan_id)),
        }
    }

    /// Create a new shared message queue with custom configuration
    pub fn with_config(scan_id: String, config: MultiConsumerConfig) -> Self {
        Self {
            queue: Arc::new(MultiConsumerQueue::with_config(scan_id, config)),
        }
    }

    /// Get the scan ID for this queue
    pub fn scan_id(&self) -> &str {
        self.queue.scan_id()
    }

    // Producer Interface

    /// Start the queue for message processing
    pub async fn start(&self) -> QueueResult<()> {
        self.queue.start().await
    }

    /// Add a message to the queue
    pub async fn enqueue(&self, message: ScanMessage) -> QueueResult<u64> {
        self.queue.enqueue(message).await
    }

    /// Stop the queue and complete processing
    pub async fn stop(&self) -> QueueResult<()> {
        self.queue.stop().await
    }

    // Consumer Interface

    /// Register a new consumer with the queue
    pub async fn register_consumer(&self, plugin_name: String) -> QueueResult<QueueConsumer> {
        self.queue.register_consumer(plugin_name).await
    }

    /// Register a new consumer with priority
    pub async fn register_consumer_with_priority(&self, plugin_name: String, priority: i32) -> QueueResult<QueueConsumer> {
        self.queue.register_consumer_with_priority(plugin_name, priority).await
    }

    /// Deregister a consumer from the queue
    pub async fn deregister_consumer(&self, consumer: &QueueConsumer) -> QueueResult<()> {
        self.queue.deregister_consumer(consumer).await
    }

    // Status and Monitoring

    /// Check if the queue is active
    pub async fn is_active(&self) -> bool {
        self.queue.is_active().await
    }

    /// Get queue statistics
    pub async fn get_statistics(&self) -> crate::queue::QueueStatistics {
        self.queue.get_statistics().await
    }

    /// Get memory statistics
    pub async fn get_memory_stats(&self) -> crate::queue::memory::QueueMemoryStats {
        self.queue.get_memory_stats().await
    }

    /// Get consumer summary
    pub async fn get_consumer_summary(&self) -> ConsumerSummary {
        self.queue.get_consumer_summary().await
    }

    /// Force backpressure evaluation
    pub async fn force_backpressure_evaluation(&self) -> Option<crate::queue::BackpressureReason> {
        self.queue.force_backpressure_evaluation().await
    }

}

// Clone is automatically derived since SharedMessageQueue only contains Arc<MultiConsumerQueue>

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};

    fn create_test_message() -> ScanMessage {
        let header = MessageHeader::new(0);
        let data = MessageData::FileInfo {
            path: "test.rs".to_string(),
            size: 1000,
            lines: 50,
        };
        ScanMessage::new(header, data)
    }

    #[tokio::test]
    async fn test_queue_creation() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        assert_eq!(queue.scan_id(), "test-scan");
        assert!(!queue.is_active().await);
        
        let stats = queue.get_statistics().await;
        assert_eq!(stats.queue_size, 0);
        assert_eq!(stats.active_consumers, 0);
    }

    #[tokio::test]
    async fn test_queue_lifecycle() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        
        // Start queue
        queue.start().await.unwrap();
        assert!(queue.is_active().await);

        // Stop queue
        queue.stop().await.unwrap();
        assert!(!queue.is_active().await);
    }

    #[tokio::test]
    async fn test_message_enqueue_and_consume() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        queue.start().await.unwrap();

        let message = create_test_message();
        let _sequence = queue.enqueue(message.clone()).await.unwrap();
        // Sequence numbers start at 0 for first message
        
        let stats = queue.get_statistics().await;
        assert_eq!(stats.queue_size, 1);

        // Register consumer and read message
        let consumer = queue.register_consumer("test-plugin".to_string()).await.unwrap();
        let consumed_arc = consumer.read_next().await.unwrap().unwrap();
        let consumed = (*consumed_arc).clone();
        
        assert_eq!(consumed.data(), message.data());
        
        // Acknowledge the message
        consumer.acknowledge(consumed_arc.header().sequence()).await.unwrap();
        
        // Clean up
        queue.deregister_consumer(&consumer).await.unwrap();
        queue.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_multiple_consumers() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        queue.start().await.unwrap();

        // Add messages
        for i in 0..5 {
            let header = MessageHeader::new(0);
            let data = MessageData::FileInfo {
                path: format!("test{}.rs", i),
                size: 1000,
                lines: 50,
            };
            let message = ScanMessage::new(header, data);
            queue.enqueue(message).await.unwrap();
        }

        // Register two consumers
        let consumer1 = queue.register_consumer("plugin1".to_string()).await.unwrap();
        let consumer2 = queue.register_consumer("plugin2".to_string()).await.unwrap();
        
        // Both consumers should be able to read all messages
        let messages1 = consumer1.read_batch(5).await.unwrap();
        let messages2 = consumer2.read_batch(5).await.unwrap();
        
        assert_eq!(messages1.len(), 5);
        assert_eq!(messages2.len(), 5);
        
        // Acknowledge all messages for both consumers
        for msg in &messages1 {
            consumer1.acknowledge(msg.header().sequence()).await.unwrap();
        }
        for msg in &messages2 {
            consumer2.acknowledge(msg.header().sequence()).await.unwrap();
        }
        
        // Clean up
        queue.deregister_consumer(&consumer1).await.unwrap();
        queue.deregister_consumer(&consumer2).await.unwrap();
        queue.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_consumer_summary() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        queue.start().await.unwrap();

        // Add some messages
        for _ in 0..3 {
            queue.enqueue(create_test_message()).await.unwrap();
        }

        // Register consumer
        let consumer = queue.register_consumer("test-plugin".to_string()).await.unwrap();
        
        // Get summary
        let summary = queue.get_consumer_summary().await;
        assert_eq!(summary.total_consumers, 1);
        assert_eq!(summary.active_consumers, 1);
        
        // Clean up
        queue.deregister_consumer(&consumer).await.unwrap();
        queue.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_with_config() {
        let config = MultiConsumerConfig {
            max_queue_size: 100,
            memory_threshold: 1024 * 1024, // 1MB
            ..Default::default()
        };
        
        let queue = SharedMessageQueue::with_config("test-scan".to_string(), config);
        assert_eq!(queue.scan_id(), "test-scan");
        
        // Start and add some messages
        queue.start().await.unwrap();
        
        for _ in 0..5 {
            queue.enqueue(create_test_message()).await.unwrap();
        }
        
        let stats = queue.get_statistics().await;
        assert_eq!(stats.queue_size, 5);
        
        queue.stop().await.unwrap();
    }
}
