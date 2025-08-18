//! Queue Consumer Handle Implementation
//!
//! This module provides the QueueConsumer handle that plugins use to read
//! messages from the multi-consumer queue. Each consumer maintains its own
//! position using sequence numbers and can read messages independently.
//!
//! # Design
//!
//! The QueueConsumer is a lightweight handle that references the shared
//! MultiConsumerQueue. It tracks its own read position and provides methods
//! for reading messages and acknowledging processing completion.
//!
//! # Usage
//!
//! ```rust
//! use gstats::queue::{MultiConsumerQueue, QueueConsumer};
//! use std::sync::Arc;
//!
//! # tokio_test::block_on(async {
//! let notification_manager = Arc::new(gstats::notifications::AsyncNotificationManager::new());
//! let queue = MultiConsumerQueue::new("scan-001".to_string(), notification_manager);
//! queue.start().await.unwrap();
//!
//! // Register consumer
//! let consumer = queue.register_consumer("debug-plugin".to_string()).await.unwrap();
//!
//! // Read messages
//! if let Some(message) = consumer.read_next().await.unwrap() {
//!     // Process message
//!     println!("Processing: {:?}", message);
//!     
//!     // Acknowledge completion
//!     consumer.acknowledge(message.header().sequence).await.unwrap();
//! }
//! # });
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::queue::{QueueError, QueueResult, MultiConsumerQueue};
use crate::scanner::messages::ScanMessage;

/// Consumer handle for reading messages from the multi-consumer queue
pub struct QueueConsumer {
    /// Unique consumer identifier
    consumer_id: String,
    
    /// Plugin name this consumer belongs to
    plugin_name: String,
    
    /// Reference to the shared queue
    queue: Arc<MultiConsumerQueue>,
    
    /// Current read position (sequence number)
    current_sequence: Arc<RwLock<u64>>,
    
    /// Last acknowledged sequence
    last_acknowledged: Arc<RwLock<u64>>,
    
    /// Whether consumer is currently active
    active: Arc<RwLock<bool>>,
    
    /// Processing statistics
    stats: Arc<RwLock<ConsumerStats>>,
    
    /// Consumer priority
    priority: Arc<RwLock<i32>>,
}

/// Consumer-specific statistics
#[derive(Debug, Clone)]
struct ConsumerStats {
    /// Messages read from queue
    messages_read: u64,
    
    /// Messages acknowledged
    messages_acknowledged: u64,
    
    /// Messages skipped (already processed)
    messages_skipped: u64,
    
    /// Read errors encountered
    read_errors: u64,
    
    /// Acknowledgment errors
    ack_errors: u64,
    
    /// Total read operations
    read_operations: u64,
    
    /// Total time spent reading
    total_read_time: Duration,
    
    /// Last read timestamp
    last_read: Option<Instant>,
}

impl Default for ConsumerStats {
    fn default() -> Self {
        Self {
            messages_read: 0,
            messages_acknowledged: 0,
            messages_skipped: 0,
            read_errors: 0,
            ack_errors: 0,
            read_operations: 0,
            total_read_time: Duration::from_secs(0),
            last_read: None,
        }
    }
}

impl QueueConsumer {
    /// Create a new queue consumer (internal use only)
    pub(crate) fn new(
        consumer_id: String,
        plugin_name: String,
        queue: Arc<MultiConsumerQueue>,
        priority: i32,
    ) -> Self {
        Self {
            consumer_id,
            plugin_name,
            queue,
            current_sequence: Arc::new(RwLock::new(0)),
            last_acknowledged: Arc::new(RwLock::new(0)),
            active: Arc::new(RwLock::new(true)),
            stats: Arc::new(RwLock::new(ConsumerStats::default())),
            priority: Arc::new(RwLock::new(priority)),
        }
    }
    
    /// Get the consumer ID
    pub fn consumer_id(&self) -> &str {
        &self.consumer_id
    }
    
    /// Get the plugin name
    pub fn plugin_name(&self) -> &str {
        &self.plugin_name
    }
    
    
    /// Get consumer priority
    pub async fn priority(&self) -> i32 {
        *self.priority.read().await
    }
    
    /// Set consumer priority
    pub async fn set_priority(&self, priority: i32) -> QueueResult<()> {
        // Update local priority
        *self.priority.write().await = priority;
        
        Ok(())
    }
    
    /// Check if consumer is active
    pub async fn is_active(&self) -> bool {
        *self.active.read().await
    }
    
    /// Activate/deactivate consumer
    pub async fn set_active(&self, active: bool) {
        *self.active.write().await = active;
    }
    
    /// Read the next available message
    pub async fn read_next(&self) -> QueueResult<Option<Arc<ScanMessage>>> {
        if !self.is_active().await {
            return Ok(None);
        }
        
        let start_time = Instant::now();
        let mut stats = self.stats.write().await;
        stats.read_operations += 1;
        drop(stats);
        
        let current_seq = *self.current_sequence.read().await;
        
        // Try to read message at current sequence
        let result = self.read_message_at_sequence(current_seq).await;
        
        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_read_time += start_time.elapsed();
        stats.last_read = Some(Instant::now());
        
        match result {
            Ok(Some(message)) => {
                stats.messages_read += 1;
                
                // Advance read position
                let mut current_seq = self.current_sequence.write().await;
                *current_seq += 1;
                
                Ok(Some(message))
            }
            Ok(None) => {
                // No message available
                Ok(None)
            }
            Err(e) => {
                stats.read_errors += 1;
                Err(e)
            }
        }
    }
    
    /// Read multiple messages up to the specified limit
    pub async fn read_batch(&self, max_count: usize) -> QueueResult<Vec<Arc<ScanMessage>>> {
        if !self.is_active().await || max_count == 0 {
            return Ok(Vec::new());
        }
        
        let mut messages = Vec::with_capacity(max_count);
        let mut current_seq = *self.current_sequence.read().await;
        
        for _ in 0..max_count {
            match self.read_message_at_sequence(current_seq).await? {
                Some(message) => {
                    messages.push(message);
                    current_seq += 1;
                }
                None => {
                    break; // No more messages available
                }
            }
        }
        
        // Update read position if we read any messages
        if !messages.is_empty() {
            *self.current_sequence.write().await = current_seq;
            
            // Update statistics
            let mut stats = self.stats.write().await;
            stats.messages_read += messages.len() as u64;
            stats.read_operations += 1;
            stats.last_read = Some(Instant::now());
        }
        
        Ok(messages)
    }
    
    /// Read messages from a specific sequence number
    pub async fn read_from_sequence(&self, start_sequence: u64, max_count: usize) -> QueueResult<Vec<Arc<ScanMessage>>> {
        if !self.is_active().await || max_count == 0 {
            return Ok(Vec::new());
        }
        
        let mut messages = Vec::with_capacity(max_count);
        let mut seq = start_sequence;
        
        for _ in 0..max_count {
            match self.read_message_at_sequence(seq).await? {
                Some(message) => {
                    messages.push(message);
                    seq += 1;
                }
                None => break,
            }
        }
        
        // Update statistics but don't change read position
        if !messages.is_empty() {
            let mut stats = self.stats.write().await;
            stats.read_operations += 1;
            stats.last_read = Some(Instant::now());
        }
        
        Ok(messages)
    }
    
    /// Acknowledge processing of a message
    pub async fn acknowledge(&self, sequence: u64) -> QueueResult<()> {
        if !self.is_active().await {
            return Err(QueueError::operation_failed("Consumer not active"));
        }
        
        // Update last acknowledged sequence
        let mut last_ack = self.last_acknowledged.write().await;
        if sequence > *last_ack {
            *last_ack = sequence;
        }
        drop(last_ack);
        
        // Update progress in queue registry
        let result = {
            let mut registry = self.queue.consumer_registry.write().await;
            registry.update_progress(&self.consumer_id, sequence)
        };
        
        // Update statistics
        let mut stats = self.stats.write().await;
        match result {
            Ok(()) => {
                stats.messages_acknowledged += 1;
            }
            Err(_) => {
                stats.ack_errors += 1;
            }
        }
        
        result
    }
    
    /// Acknowledge processing of multiple messages
    pub async fn acknowledge_batch(&self, sequences: &[u64]) -> QueueResult<()> {
        if sequences.is_empty() {
            return Ok(());
        }
        
        // Find the highest sequence number
        let max_sequence = *sequences.iter().max().unwrap();
        
        // Acknowledge up to the highest sequence
        self.acknowledge(max_sequence).await
    }
    
    /// Get current read position
    pub async fn current_sequence(&self) -> u64 {
        *self.current_sequence.read().await
    }
    
    /// Get last acknowledged sequence
    pub async fn last_acknowledged_sequence(&self) -> u64 {
        *self.last_acknowledged.read().await
    }
    
    /// Set read position to a specific sequence
    pub async fn seek_to_sequence(&self, sequence: u64) -> QueueResult<()> {
        if !self.is_active().await {
            return Err(QueueError::operation_failed("Consumer not active"));
        }
        
        // Validate sequence is within current range
        let tracker = self.queue.sequence_tracker.read().await;
        let (min_seq, max_seq) = tracker.get_range();
        
        if sequence < min_seq {
            return Err(QueueError::operation_failed(
                format!("Sequence {} below minimum {}", sequence, min_seq)
            ));
        }
        
        if sequence > max_seq + 1 {
            return Err(QueueError::operation_failed(
                format!("Sequence {} beyond maximum {}", sequence, max_seq)
            ));
        }
        
        // Update read position
        *self.current_sequence.write().await = sequence;
        
        log::debug!("Consumer {} seeking to sequence {}", self.consumer_id, sequence);
        Ok(())
    }
    
    /// Get consumer lag (how far behind the latest message)
    pub async fn get_lag(&self) -> u64 {
        let tracker = self.queue.sequence_tracker.read().await;
        let current_seq = *self.current_sequence.read().await;
        tracker.max_sequence.saturating_sub(current_seq)
    }
    
    /// Check if there are messages available to read
    pub async fn has_messages_available(&self) -> bool {
        let current_seq = *self.current_sequence.read().await;
        let tracker = self.queue.sequence_tracker.read().await;
        current_seq <= tracker.max_sequence
    }
    
    /// Wait for new messages to become available
    pub async fn wait_for_messages(&self, timeout: Duration) -> QueueResult<bool> {
        let start = Instant::now();
        
        while start.elapsed() < timeout {
            if self.has_messages_available().await {
                return Ok(true);
            }
            
            // Short sleep to avoid busy waiting
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        Ok(false) // Timeout
    }
    
    /// Read a message at a specific sequence (internal helper)
    async fn read_message_at_sequence(&self, sequence: u64) -> QueueResult<Option<Arc<ScanMessage>>> {
        let messages = self.queue.messages.read().await;
        let tracker = self.queue.sequence_tracker.read().await;
        
        // Check if sequence is valid
        if sequence < tracker.min_sequence {
            // Message has been garbage collected
            return Ok(None);
        }
        
        if sequence > tracker.max_sequence {
            // Message doesn't exist yet
            return Ok(None);
        }
        
        // Calculate position in queue
        let queue_position = sequence - tracker.min_sequence;
        
        if queue_position >= messages.len() as u64 {
            // Position is beyond current queue size
            return Ok(None);
        }
        
        // Get message at position
        if let Some(message) = messages.get(queue_position as usize) {
            // Verify sequence number matches (sanity check)
            if message.header().sequence == sequence {
                Ok(Some(Arc::clone(message)))
            } else {
                // Sequence mismatch - possible race condition
                log::warn!("Sequence mismatch in queue: expected {}, found {}", 
                          sequence, message.header().sequence);
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}

// Extension methods for MultiConsumerQueue to create consumers
impl MultiConsumerQueue {
    /// Register a new consumer and return a handle
    pub async fn register_consumer(&self, plugin_name: String) -> QueueResult<QueueConsumer> {
        self.register_consumer_with_priority(plugin_name, 0).await
    }
    
    /// Register a new consumer with priority and return a handle
    pub async fn register_consumer_with_priority(&self, plugin_name: String, priority: i32) -> QueueResult<QueueConsumer> {
        let consumer_id = format!("{}-{}", plugin_name, uuid::Uuid::now_v7());
        
        // Register in the queue's consumer registry
        {
            let mut registry = self.consumer_registry.write().await;
            registry.register_consumer(consumer_id.clone(), plugin_name.clone(), priority)?;
        }
        
        // Create consumer handle
        let consumer = QueueConsumer::new(
            consumer_id,
            plugin_name,
            Arc::new(self.clone()),
            priority,
        );
        
        log::info!("Registered consumer: {} for plugin: {}", consumer.consumer_id(), consumer.plugin_name());
        
        Ok(consumer)
    }
    
    /// Deregister a consumer
    pub async fn deregister_consumer(&self, consumer: &QueueConsumer) -> QueueResult<()> {
        // Deactivate consumer
        consumer.set_active(false).await;
        
        // Remove from registry
        let mut registry = self.consumer_registry.write().await;
        registry.deregister_consumer(consumer.consumer_id())?;
        
        log::info!("Deregistered consumer: {}", consumer.consumer_id());
        Ok(())
    }
    
    /// Get all active consumers
    pub async fn get_active_consumers(&self) -> Vec<String> {
        let registry = self.consumer_registry.read().await;
        registry.consumers.keys().cloned().collect()
    }
    
    /// Get consumer count
    pub async fn get_consumer_count(&self) -> usize {
        let registry = self.consumer_registry.read().await;
        registry.consumers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};
    // Note: tokio::time imports removed - not used in current tests
    
    fn create_test_message(sequence: u64) -> ScanMessage {
        let header = MessageHeader::new(sequence);
        let data = MessageData::FileInfo {
            path: format!("test{}.rs", sequence),
            size: 1000,
            lines: 50,
        };
        ScanMessage::new(header, data)
    }
    
    #[tokio::test]
    async fn test_consumer_creation() {
        let notification_manager = Arc::new(crate::notifications::AsyncNotificationManager::new());
        let queue = Arc::new(MultiConsumerQueue::new("test-scan".to_string(), notification_manager));
        queue.start().await.unwrap();
        
        let consumer = queue.register_consumer("test-plugin".to_string()).await.unwrap();
        
        assert_eq!(consumer.plugin_name(), "test-plugin");
        assert!(consumer.consumer_id().contains("test-plugin"));
        assert_eq!(consumer.priority().await, 0);
        assert!(consumer.is_active().await);
        assert_eq!(consumer.current_sequence().await, 0);
    }
    
    #[tokio::test]
    async fn test_consumer_read_next() {
        let notification_manager = Arc::new(crate::notifications::AsyncNotificationManager::new());
        let queue = Arc::new(MultiConsumerQueue::new("test-scan".to_string(), notification_manager));
        queue.start().await.unwrap();
        
        // Add a message
        let message = create_test_message(0);
        queue.enqueue(message).await.unwrap();
        
        // Create consumer and read
        let consumer = queue.register_consumer("test-plugin".to_string()).await.unwrap();
        let read_message = consumer.read_next().await.unwrap();
        
        assert!(read_message.is_some());
        let read_message = read_message.unwrap();
        assert_eq!(read_message.header().sequence, 0);
        assert_eq!(consumer.current_sequence().await, 1);
    }
    
    #[tokio::test]
    async fn test_consumer_acknowledgment() {
        let notification_manager = Arc::new(crate::notifications::AsyncNotificationManager::new());
        let queue = Arc::new(MultiConsumerQueue::new("test-scan".to_string(), notification_manager));
        queue.start().await.unwrap();
        
        // Add a message
        let message = create_test_message(0);
        queue.enqueue(message).await.unwrap();
        
        // Create consumer, read, and acknowledge
        let consumer = queue.register_consumer("test-plugin".to_string()).await.unwrap();
        let read_message = consumer.read_next().await.unwrap().unwrap();
        
        consumer.acknowledge(read_message.header().sequence).await.unwrap();
        
        assert_eq!(consumer.last_acknowledged_sequence().await, 0);
    }
    
    #[tokio::test]
    async fn test_consumer_batch_read() {
        let notification_manager = Arc::new(crate::notifications::AsyncNotificationManager::new());
        let queue = Arc::new(MultiConsumerQueue::new("test-scan".to_string(), notification_manager));
        queue.start().await.unwrap();
        
        // Add multiple messages
        for i in 0..5 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Create consumer and read batch
        let consumer = queue.register_consumer("test-plugin".to_string()).await.unwrap();
        let messages = consumer.read_batch(3).await.unwrap();
        
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].header().sequence, 0);
        assert_eq!(messages[2].header().sequence, 2);
        assert_eq!(consumer.current_sequence().await, 3);
    }
    
    #[tokio::test]
    async fn test_consumer_seek() {
        let notification_manager = Arc::new(crate::notifications::AsyncNotificationManager::new());
        let queue = Arc::new(MultiConsumerQueue::new("test-scan".to_string(), notification_manager));
        queue.start().await.unwrap();
        
        // Add multiple messages
        for i in 0..5 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Create consumer and seek
        let consumer = queue.register_consumer("test-plugin".to_string()).await.unwrap();
        consumer.seek_to_sequence(3).await.unwrap();
        
        assert_eq!(consumer.current_sequence().await, 3);
        
        // Read from new position
        let message = consumer.read_next().await.unwrap().unwrap();
        assert_eq!(message.header().sequence, 3);
    }
    
    #[tokio::test]
    async fn test_consumer_lag() {
        let notification_manager = Arc::new(crate::notifications::AsyncNotificationManager::new());
        let queue = Arc::new(MultiConsumerQueue::new("test-scan".to_string(), notification_manager));
        queue.start().await.unwrap();
        
        // Add multiple messages
        for i in 0..5 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Create consumer - should be at lag 5 (sequences 0-4 exist, consumer at 0)
        let consumer = queue.register_consumer("test-plugin".to_string()).await.unwrap();
        let lag = consumer.get_lag().await;
        assert_eq!(lag, 4); // max_sequence (4) - current_sequence (0) = 4
        
        // Read one message, lag should decrease
        consumer.read_next().await.unwrap();
        let lag = consumer.get_lag().await;
        assert_eq!(lag, 3); // max_sequence (4) - current_sequence (1) = 3
    }
    
    #[tokio::test]
    async fn test_consumer_statistics() {
        let notification_manager = Arc::new(crate::notifications::AsyncNotificationManager::new());
        let queue = Arc::new(MultiConsumerQueue::new("test-scan".to_string(), notification_manager));
        queue.start().await.unwrap();
        
        // Add a message
        let message = create_test_message(0);
        queue.enqueue(message).await.unwrap();
        
        // Create consumer, read, and acknowledge
        let consumer = queue.register_consumer("test-plugin".to_string()).await.unwrap();
        let read_message = consumer.read_next().await.unwrap().unwrap();
        consumer.acknowledge(read_message.header().sequence).await.unwrap();
        
        // Statistics tracking functionality removed
    }
    
    #[tokio::test]
    async fn test_multiple_consumers() {
        let notification_manager = Arc::new(crate::notifications::AsyncNotificationManager::new());
        let queue = Arc::new(MultiConsumerQueue::new("test-scan".to_string(), notification_manager));
        queue.start().await.unwrap();
        
        // Add multiple messages
        for i in 0..5 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Create multiple consumers
        let consumer1 = queue.register_consumer("plugin1".to_string()).await.unwrap();
        let consumer2 = queue.register_consumer("plugin2".to_string()).await.unwrap();
        
        // Both should be able to read all messages independently
        let messages1 = consumer1.read_batch(5).await.unwrap();
        let messages2 = consumer2.read_batch(5).await.unwrap();
        
        assert_eq!(messages1.len(), 5);
        assert_eq!(messages2.len(), 5);
        
        // Same messages, different consumer positions
        assert_eq!(messages1[0].header().sequence, messages2[0].header().sequence);
        assert_eq!(consumer1.current_sequence().await, 5);
        assert_eq!(consumer2.current_sequence().await, 5);
        
        // Check consumer count
        assert_eq!(queue.get_consumer_count().await, 2);
    }
}