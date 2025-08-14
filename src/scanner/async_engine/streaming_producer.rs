//! Queue-Based Streaming Producer
//! 
//! Provides streaming integration with SharedMessageQueue for proper async message flow.

use crate::scanner::traits::QueueMessageProducer;
use crate::queue::SharedMessageQueue;
use super::error::ScanResult;

/// Create a queue-based message producer for streaming operations
pub fn create_queue_producer(
    queue: SharedMessageQueue,
    producer_name: String,
) -> ScanResult<QueueMessageProducer> {
    Ok(QueueMessageProducer::new(queue, producer_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData, ScanMessage};
    use crate::scanner::traits::MessageProducer;
    use crate::queue::SharedMessageQueue;

    fn create_test_message(id: u64) -> ScanMessage {
        ScanMessage::new(
            MessageHeader::new(id),
            MessageData::FileInfo {
                path: format!("test_{}.rs", id),
                size: 1024,
                lines: 50,
            },
        )
    }

    #[tokio::test]
    async fn test_queue_producer_creation() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        let producer = create_queue_producer(queue, "test".to_string()).unwrap();
        assert_eq!(producer.get_producer_name(), "test");
    }

    #[tokio::test]
    async fn test_produce_message_to_queue() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        queue.start().await.unwrap();
        
        let producer = create_queue_producer(queue.clone(), "test".to_string()).unwrap();
        let message = create_test_message(1);
        
        let result = producer.produce_message(message).await;
        assert!(result.is_ok());
        
        // Verify message was added to queue
        let stats = queue.get_statistics().await;
        assert_eq!(stats.queue_size, 1);
    }
}