//! Queue Consumer Interface
//!
//! Provides abstraction layer for plugins to consume messages from the queue
//! without direct queue access. This module prototypes both polling and
//! event-driven consumer APIs to determine the best approach.

use crate::queue::error::{QueueError, QueueResult};
use crate::queue::notifications::{QueueEvent, QueueEventNotifier};
use crate::queue::shared_queue::SharedMessageQueue;
use crate::scanner::messages::ScanMessage;
use async_trait::async_trait;
use std::time::Duration;
use tokio::sync::broadcast;

/// Consumer API approach - polling with optional timeout
#[async_trait]
pub trait PollingConsumer: Send + Sync {
    /// Pop a single message from the queue
    /// - `timeout`: None = wait forever, Some(0) = non-blocking, Some(duration) = wait up to duration
    /// - Returns: Some(message) if available, None if timeout or queue empty
    async fn pop(&self, timeout: Option<Duration>) -> QueueResult<Option<ScanMessage>>;
    
    /// Pop multiple messages in a batch (up to max_count)
    async fn pop_batch(&self, max_count: usize, timeout: Option<Duration>) -> QueueResult<Vec<ScanMessage>>;
    
    /// Check if scanning is complete
    async fn is_scan_complete(&self) -> bool;
    
    /// Check if queue is empty
    async fn is_empty(&self) -> bool;
    
    /// Wait for scan completion and queue drain
    async fn wait_for_completion(&self) -> QueueResult<()>;
}

/// Consumer API approach - event-driven callbacks
#[async_trait]
pub trait EventDrivenConsumer: Send + Sync {
    /// Called when messages are available in the queue
    async fn on_messages_available(&mut self, count: usize) -> QueueResult<()>;
    
    /// Called when scanning is complete
    async fn on_scan_complete(&mut self, total_messages: u64) -> QueueResult<()>;
    
    /// Called when queue is drained (empty and scan complete)
    async fn on_queue_drained(&mut self) -> QueueResult<()>;
    
    /// Called on memory warnings
    async fn on_memory_warning(&mut self, current_size: usize, threshold: usize) -> QueueResult<()> {
        log::warn!("Queue memory warning: {} bytes (threshold: {} bytes)", current_size, threshold);
        Ok(())
    }
    
    /// Start consuming events from the queue
    async fn start_consuming(&mut self, queue: &SharedMessageQueue) -> QueueResult<()>;
}

/// Polling-based queue consumer implementation
#[derive(Debug)]
pub struct QueuePollingConsumer {
    queue: SharedMessageQueue,
}

impl QueuePollingConsumer {
    /// Create a new polling consumer for the given queue
    pub fn new(queue: SharedMessageQueue) -> Self {
        Self { queue }
    }
}

#[async_trait]
impl PollingConsumer for QueuePollingConsumer {
    async fn pop(&self, timeout: Option<Duration>) -> QueueResult<Option<ScanMessage>> {
        match timeout {
            None => {
                // Wait forever - keep polling until message available or scan complete
                loop {
                    if let Some(message) = self.queue.pop().await {
                        return Ok(Some(message));
                    }
                    
                    if self.queue.is_scan_complete().await && self.queue.is_empty().await {
                        return Ok(None);
                    }
                    
                    // Brief pause to avoid busy waiting
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
            Some(duration) if duration.is_zero() => {
                // Non-blocking
                Ok(self.queue.pop().await)
            }
            Some(duration) => {
                // Wait with timeout
                let start = tokio::time::Instant::now();
                loop {
                    if let Some(message) = self.queue.pop().await {
                        return Ok(Some(message));
                    }
                    
                    if self.queue.is_scan_complete().await && self.queue.is_empty().await {
                        return Ok(None);
                    }
                    
                    if start.elapsed() >= duration {
                        return Ok(None); // Timeout
                    }
                    
                    // Brief pause to avoid busy waiting
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        }
    }
    
    async fn pop_batch(&self, max_count: usize, timeout: Option<Duration>) -> QueueResult<Vec<ScanMessage>> {
        match timeout {
            None => {
                // Wait forever for at least one message
                loop {
                    let messages = self.queue.pop_batch(max_count).await;
                    if !messages.is_empty() {
                        return Ok(messages);
                    }
                    
                    if self.queue.is_scan_complete().await && self.queue.is_empty().await {
                        return Ok(Vec::new());
                    }
                    
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
            Some(duration) if duration.is_zero() => {
                // Non-blocking
                Ok(self.queue.pop_batch(max_count).await)
            }
            Some(duration) => {
                // Wait with timeout
                let start = tokio::time::Instant::now();
                loop {
                    let messages = self.queue.pop_batch(max_count).await;
                    if !messages.is_empty() {
                        return Ok(messages);
                    }
                    
                    if self.queue.is_scan_complete().await && self.queue.is_empty().await {
                        return Ok(Vec::new());
                    }
                    
                    if start.elapsed() >= duration {
                        return Ok(Vec::new()); // Timeout
                    }
                    
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        }
    }
    
    async fn is_scan_complete(&self) -> bool {
        self.queue.is_scan_complete().await
    }
    
    async fn is_empty(&self) -> bool {
        self.queue.is_empty().await
    }
    
    async fn wait_for_completion(&self) -> QueueResult<()> {
        self.queue.wait_for_drain().await
    }
}

/// Event-driven queue consumer implementation
#[derive(Debug)]
pub struct QueueEventConsumer {
    queue: SharedMessageQueue,
    event_receiver: Option<broadcast::Receiver<QueueEvent>>,
}

impl QueueEventConsumer {
    /// Create a new event-driven consumer for the given queue
    pub fn new(queue: SharedMessageQueue) -> Self {
        Self {
            queue,
            event_receiver: None,
        }
    }
    
    /// Get a message from the queue (used by event handlers)
    pub async fn get_message(&self) -> Option<ScanMessage> {
        self.queue.pop().await
    }
    
    /// Get multiple messages from the queue (used by event handlers)
    pub async fn get_messages(&self, max_count: usize) -> Vec<ScanMessage> {
        self.queue.pop_batch(max_count).await
    }
}

#[async_trait]
impl EventDrivenConsumer for QueueEventConsumer {
    async fn on_messages_available(&mut self, count: usize) -> QueueResult<()> {
        log::trace!("Messages available: {}", count);
        // Default implementation - subclasses should override
        Ok(())
    }
    
    async fn on_scan_complete(&mut self, total_messages: u64) -> QueueResult<()> {
        log::info!("Scan complete: {} total messages", total_messages);
        // Default implementation - subclasses should override
        Ok(())
    }
    
    async fn on_queue_drained(&mut self) -> QueueResult<()> {
        log::info!("Queue drained - all messages processed");
        // Default implementation - subclasses should override
        Ok(())
    }
    
    async fn start_consuming(&mut self, queue: &SharedMessageQueue) -> QueueResult<()> {
        let mut event_receiver = queue.subscribe_events();
        
        while let Ok(event) = event_receiver.recv().await {
            match event {
                QueueEvent::MessageAdded { count, .. } => {
                    self.on_messages_available(count).await?;
                }
                QueueEvent::ScanComplete { total_messages, .. } => {
                    self.on_scan_complete(total_messages).await?;
                }
                QueueEvent::QueueDrained { .. } => {
                    self.on_queue_drained().await?;
                    break; // Exit consumption loop
                }
                QueueEvent::MemoryWarning { current_size, threshold, .. } => {
                    self.on_memory_warning(current_size, threshold).await?;
                }
                _ => {} // Ignore other events
            }
        }
        
        Ok(())
    }
}

/// Plugin consumer abstraction - hides queue implementation from plugins
#[async_trait]
pub trait PluginConsumer: Send + Sync {
    /// Process a single message from the queue
    async fn process_message(&mut self, message: ScanMessage) -> QueueResult<()>;
    
    /// Called when scanning is complete and queue is empty
    async fn on_scan_complete(&mut self) -> QueueResult<()>;
    
    /// Start consuming messages from the queue
    async fn consume_messages(&mut self, consumer: &dyn PollingConsumer) -> QueueResult<()> {
        // Default implementation using polling API
        loop {
            // Try to get a message with a reasonable timeout
            match consumer.pop(Some(Duration::from_millis(100))).await? {
                Some(message) => {
                    self.process_message(message).await?;
                }
                None => {
                    // Check if we're done
                    if consumer.is_scan_complete().await && consumer.is_empty().await {
                        break;
                    }
                    // Continue polling
                }
            }
        }
        
        // Scan is complete and queue is empty
        self.on_scan_complete().await?;
        Ok(())
    }
    
    /// Batch processing variant for efficiency
    async fn consume_messages_batch(&mut self, consumer: &dyn PollingConsumer, batch_size: usize) -> QueueResult<()> {
        loop {
            // Try to get a batch of messages
            let messages = consumer.pop_batch(batch_size, Some(Duration::from_millis(100))).await?;
            
            if messages.is_empty() {
                // Check if we're done
                if consumer.is_scan_complete().await && consumer.is_empty().await {
                    break;
                }
                // Continue polling
            } else {
                // Process all messages in the batch
                for message in messages {
                    self.process_message(message).await?;
                }
            }
        }
        
        // Scan is complete and queue is empty
        self.on_scan_complete().await?;
        Ok(())
    }
}
pub mod api_analysis {
    //! Analysis of polling vs event-driven consumer APIs
    
    /// Pros and cons of polling API
    pub const POLLING_PROS: &[&str] = &[
        "Simple and predictable",
        "Easy to understand and debug",
        "Direct control over timing",
        "Works well with existing async patterns",
        "No callback complexity",
        "Easy to test"
    ];
    
    pub const POLLING_CONS: &[&str] = &[
        "Potential for busy waiting",
        "Less efficient for sparse messages",
        "Requires timeout management",
        "May miss optimal processing opportunities"
    ];
    
    /// Pros and cons of event-driven API
    pub const EVENT_DRIVEN_PROS: &[&str] = &[
        "More reactive and efficient",
        "No busy waiting",
        "Immediate response to events",
        "Better for sparse message scenarios",
        "Natural backpressure handling"
    ];
    
    pub const EVENT_DRIVEN_CONS: &[&str] = &[
        "More complex callback management",
        "Harder to debug event flows",
        "Potential for callback hell",
        "More complex error handling",
        "Requires careful state management"
    ];
    
    /// Recommendation based on analysis
    pub const RECOMMENDATION: &str = 
        "For the initial implementation, polling API is recommended due to its simplicity \
         and predictability. Event-driven API can be added later as an advanced option \
         for performance-critical plugins.";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::modes::ScanMode;
    use crate::scanner::messages::{MessageHeader, MessageData};
    use tokio::time::timeout;

    fn create_test_message() -> ScanMessage {
        let header = MessageHeader::new(ScanMode::FILES, 0);
        let data = MessageData::FileInfo {
            path: "test.rs".to_string(),
            size: 1000,
            lines: 50,
        };
        ScanMessage::new(header, data)
    }

    #[tokio::test]
    async fn test_polling_consumer_non_blocking() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        let consumer = QueuePollingConsumer::new(queue.clone());
        
        // Non-blocking pop on empty queue should return None
        let result = consumer.pop(Some(Duration::ZERO)).await.unwrap();
        assert!(result.is_none());
        
        // Add a message and try again
        queue.start_scan(ScanMode::FILES).await.unwrap();
        queue.push(create_test_message()).await.unwrap();
        
        let result = consumer.pop(Some(Duration::ZERO)).await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_polling_consumer_timeout() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        let consumer = QueuePollingConsumer::new(queue.clone());
        
        queue.start_scan(ScanMode::FILES).await.unwrap();
        
        // Should timeout after 50ms
        let start = tokio::time::Instant::now();
        let result = consumer.pop(Some(Duration::from_millis(50))).await.unwrap();
        let elapsed = start.elapsed();
        
        assert!(result.is_none());
        assert!(elapsed >= Duration::from_millis(45)); // Allow some variance
        assert!(elapsed <= Duration::from_millis(100)); // But not too much
    }

    #[tokio::test]
    async fn test_polling_consumer_batch() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        let consumer = QueuePollingConsumer::new(queue.clone());
        
        queue.start_scan(ScanMode::FILES).await.unwrap();
        
        // Add multiple messages
        for _ in 0..5 {
            queue.push(create_test_message()).await.unwrap();
        }
        
        // Pop batch
        let messages = consumer.pop_batch(3, Some(Duration::ZERO)).await.unwrap();
        assert_eq!(messages.len(), 3);
        
        // Pop remaining
        let messages = consumer.pop_batch(10, Some(Duration::ZERO)).await.unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[tokio::test]
    async fn test_event_driven_consumer_basic() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        let mut consumer = QueueEventConsumer::new(queue.clone());
        
        // Start scan and add message in background
        let queue_clone = queue.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            queue_clone.start_scan(ScanMode::FILES).await.unwrap();
            queue_clone.push(create_test_message()).await.unwrap();
            queue_clone.complete_scan().await.unwrap();
            
            // Consume the message to trigger drain event
            tokio::time::sleep(Duration::from_millis(10)).await;
            queue_clone.pop().await;
        });
        
        // This should complete when queue is drained
        let result = timeout(Duration::from_millis(500), consumer.start_consuming(&queue)).await;
        assert!(result.is_ok(), "Consumer should complete when queue is drained");
    }

    #[tokio::test]
    async fn test_consumer_wait_for_completion() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        let consumer = QueuePollingConsumer::new(queue.clone());
        
        queue.start_scan(ScanMode::FILES).await.unwrap();
        queue.push(create_test_message()).await.unwrap();
        queue.complete_scan().await.unwrap();
        
        // Consume the message in background
        let queue_clone = queue.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            queue_clone.pop().await;
        });
        
        // Should complete when queue is drained
        let result = timeout(Duration::from_millis(200), consumer.wait_for_completion()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_plugin_consumer_trait() {
        struct TestPluginConsumer {
            messages_processed: usize,
            scan_completed: bool,
        }
        
        #[async_trait]
        impl PluginConsumer for TestPluginConsumer {
            async fn process_message(&mut self, _message: ScanMessage) -> QueueResult<()> {
                self.messages_processed += 1;
                Ok(())
            }
            
            async fn on_scan_complete(&mut self) -> QueueResult<()> {
                self.scan_completed = true;
                Ok(())
            }
        }
        
        let queue = SharedMessageQueue::new("test-scan".to_string());
        let consumer = QueuePollingConsumer::new(queue.clone());
        let mut plugin_consumer = TestPluginConsumer {
            messages_processed: 0,
            scan_completed: false,
        };
        
        // Start scan and add messages
        queue.start_scan(ScanMode::FILES).await.unwrap();
        for _ in 0..3 {
            queue.push(create_test_message()).await.unwrap();
        }
        queue.complete_scan().await.unwrap();
        
        // Consume messages using plugin consumer trait
        plugin_consumer.consume_messages(&consumer).await.unwrap();
        
        assert_eq!(plugin_consumer.messages_processed, 3);
        assert!(plugin_consumer.scan_completed);
    }
}
