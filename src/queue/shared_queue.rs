//! Shared Message Queue Implementation
//!
//! Multi-consumer queue implementation providing producer and consumer interfaces
//! for scanner-to-plugin message coordination. Built on MultiConsumerQueue for
//! efficient parallel message processing.

use crate::queue::error::QueueResult;
use crate::queue::{MultiConsumerQueue, QueueConsumer};
use crate::scanner::messages::ScanMessage;
use std::sync::Arc;

/// Shared message queue for multi-consumer scanner-to-plugin coordination
/// 
/// This is a clean wrapper around MultiConsumerQueue providing a unified
/// interface for both producers (scanners) and consumers (plugins).
#[derive(Clone)]
pub struct SharedMessageQueue {
    /// The underlying multi-consumer queue implementation
    queue: Arc<MultiConsumerQueue>,
}

impl SharedMessageQueue {
    /// Create a new shared message queue for the given scan session
    pub fn new(scan_id: String, notification_manager: Arc<crate::notifications::AsyncNotificationManager<crate::notifications::events::QueueEvent>>) -> Self {
        Self {
            queue: Arc::new(MultiConsumerQueue::new(scan_id, notification_manager)),
        }
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


    // Consumer Interface

    /// Register a new consumer with the queue
    pub async fn register_consumer(&self, plugin_name: String) -> QueueResult<QueueConsumer> {
        self.queue.register_consumer(plugin_name).await
    }


    // Status and Monitoring

    /// Get queue statistics
    pub async fn get_statistics(&self) -> crate::queue::QueueStatistics {
        self.queue.get_statistics().await
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
        let notification_manager = Arc::new(crate::notifications::AsyncNotificationManager::new());
        let queue = SharedMessageQueue::new("test-scan".to_string(), notification_manager);
        
        let stats = queue.get_statistics().await;
        assert_eq!(stats.queue_size, 0);
        assert_eq!(stats.active_consumers, 0);
    }

    #[tokio::test]
    async fn test_queue_lifecycle() {
        let notification_manager = Arc::new(crate::notifications::AsyncNotificationManager::new());
        let queue = SharedMessageQueue::new("test-scan".to_string(), notification_manager);
        
        // Start queue
        queue.start().await.unwrap();
    }

    #[tokio::test]
    async fn test_message_enqueue_and_consume() {
        let notification_manager = Arc::new(crate::notifications::AsyncNotificationManager::new());
        let queue = SharedMessageQueue::new("test-scan".to_string(), notification_manager);
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
        
    }

    #[tokio::test]
    async fn test_multiple_consumers() {
        let notification_manager = Arc::new(crate::notifications::AsyncNotificationManager::new());
        let queue = SharedMessageQueue::new("test-scan".to_string(), notification_manager);
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
        
    }


}
