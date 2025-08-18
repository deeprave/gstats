//! Multi-Consumer Queue Implementation
//!
//! This module provides a multi-consumer queue architecture that allows multiple
//! plugins to consume messages independently using sequence-based tracking.
//! 
//! # Architecture
//!
//! The multi-consumer queue uses a Kafka-like approach with sequence numbers
//! to track consumer progress. Each message has a sequence number, and consumers
//! track their position using these sequences.
//!
//! ## Key Components:
//! - **MultiConsumerQueue**: Core queue with Arc-wrapped messages
//! - **SequenceTracker**: Manages sequence number allocation and ranges
//! - **ConsumerRegistry**: Tracks active consumers and their progress
//! - **GarbageCollector**: Removes messages below low water mark
//!
//! ## Memory Management:
//! - Messages are Arc-wrapped for efficient sharing
//! - Garbage collection based on low water mark (minimum consumer sequence)
//! - Configurable memory thresholds and collection intervals

use std::collections::{VecDeque, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};

use crate::queue::{QueueError, QueueResult, MemoryMonitor};
use crate::notifications::AsyncNotificationManager;
use crate::notifications::events::QueueEvent;
use crate::notifications::traits::{Publisher, NotificationManager};
use crate::scanner::messages::ScanMessage;

/// Configuration for multi-consumer queue
#[derive(Debug, Clone)]
pub struct MultiConsumerConfig {
    /// Maximum number of messages to keep in queue
    pub max_queue_size: usize,
    
    /// Memory threshold for garbage collection (bytes)
    pub memory_threshold: usize,
    
    /// Garbage collection interval
    pub gc_interval: Duration,
    
    /// Timeout for stale consumer detection
    pub consumer_timeout: Duration,
    
    /// Enable automatic garbage collection
    pub auto_gc: bool,
    
    /// Batch size for garbage collection
    pub gc_batch_size: usize,
}

impl Default for MultiConsumerConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 100_000,           // 100K messages
            memory_threshold: 512 * 1024 * 1024, // 512MB
            gc_interval: Duration::from_secs(30),  // 30 seconds
            consumer_timeout: Duration::from_secs(300), // 5 minutes
            auto_gc: true,
            gc_batch_size: 1000,               // Remove 1K messages at a time
        }
    }
}

/// Tracks sequence number allocation and ranges
#[derive(Debug)]
pub struct SequenceTracker {
    /// Next sequence number to assign
    pub(crate) next_sequence: u64,
    
    /// Minimum sequence number currently in queue
    pub(crate) min_sequence: u64,
    
    /// Maximum sequence number assigned
    pub(crate) max_sequence: u64,
    
    /// Total messages processed through queue
    total_messages: u64,
}

impl SequenceTracker {
    /// Create a new sequence tracker
    pub fn new() -> Self {
        Self {
            next_sequence: 0,
            min_sequence: 0,
            max_sequence: 0,
            total_messages: 0,
        }
    }
    
    /// Allocate the next sequence number
    pub fn next_sequence(&mut self) -> u64 {
        let seq = self.next_sequence;
        self.next_sequence += 1;
        self.max_sequence = seq;
        self.total_messages += 1;
        seq
    }
    
    /// Update minimum sequence after garbage collection
    pub fn update_min_sequence(&mut self, new_min: u64) {
        self.min_sequence = new_min;
    }
    
    /// Get the current sequence range (min, max)
    pub fn get_range(&self) -> (u64, u64) {
        (self.min_sequence, self.max_sequence)
    }
    
    /// Check if a sequence number is currently valid (within range)
    pub fn is_valid_sequence(&self, sequence: u64) -> bool {
        sequence >= self.min_sequence && sequence <= self.max_sequence
    }
    
    /// Get total messages processed
    pub fn total_messages(&self) -> u64 {
        self.total_messages
    }
    
}

impl Default for SequenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for the multi-consumer queue
#[derive(Debug, Clone)]
pub struct QueueStatistics {
    /// Current queue size (number of messages)
    pub queue_size: usize,
    
    /// Current memory usage (bytes)
    pub memory_usage: u64,
    
    /// Number of active consumers
    pub active_consumers: usize,
    
    /// Total messages processed
    pub total_messages: u64,
    
}

/// Multi-consumer queue with sequence-based tracking
pub struct MultiConsumerQueue {
    /// Unique identifier for this queue
    scan_id: String,
    
    /// Message storage with Arc for efficient sharing
    pub(crate) messages: Arc<RwLock<VecDeque<Arc<ScanMessage>>>>,
    
    /// Sequence tracking
    pub(crate) sequence_tracker: Arc<RwLock<SequenceTracker>>,
    
    /// Consumer registry
    pub(crate) consumer_registry: Arc<RwLock<ConsumerRegistry>>,
    
    /// Queue configuration
    config: MultiConsumerConfig,
    
    /// Event notification system
    notification_manager: Arc<AsyncNotificationManager<QueueEvent>>,
    
    /// Memory monitoring
    memory_monitor: MemoryMonitor,
    
    /// Garbage collection state
    gc_state: Arc<Mutex<GarbageCollectionState>>,
    
    /// Queue statistics
    stats: Arc<RwLock<QueueStatistics>>,
    
    /// Whether the queue is active
    active: Arc<RwLock<bool>>,
    
}

/// Consumer registry for tracking active consumers
#[derive(Debug)]
pub struct ConsumerRegistry {
    /// Active consumers and their progress
    pub(crate) consumers: HashMap<String, ConsumerProgress>,
    
}

/// Progress tracking for individual consumers
#[derive(Debug, Clone)]
pub struct ConsumerProgress {
    
    /// Last acknowledged sequence number
    pub last_acknowledged_seq: u64,
    
    
    /// Number of messages processed
    pub messages_processed: u64,
    
    
    /// Last update timestamp
    pub last_update: Instant,
    
    /// Consumer creation time
    pub created_at: Instant,
    
    /// Average processing rate (messages/second)
    pub processing_rate: f64,
    
}

/// Garbage collection state and statistics
#[derive(Debug)]
struct GarbageCollectionState {
    /// Last garbage collection run
    last_gc: Option<Instant>,
    
    /// Number of GC runs
    gc_runs: u64,
    
    /// Total messages garbage collected
    messages_collected: u64,
    
    /// Whether GC is currently running
    gc_in_progress: bool,
    
    /// Last low water mark used
    last_low_water_mark: u64,
}

impl MultiConsumerQueue {
    /// Create a new multi-consumer queue
    pub fn new(scan_id: String, notification_manager: Arc<AsyncNotificationManager<QueueEvent>>) -> Self {
        Self::with_config(scan_id, MultiConsumerConfig::default(), notification_manager)
    }
    
    /// Create a new multi-consumer queue with custom configuration
    pub fn with_config(scan_id: String, config: MultiConsumerConfig, notification_manager: Arc<AsyncNotificationManager<QueueEvent>>) -> Self {
        let stats = QueueStatistics {
            queue_size: 0,
            memory_usage: 0,
            active_consumers: 0,
            total_messages: 0,
        };
        
        let gc_state = GarbageCollectionState {
            last_gc: None,
            gc_runs: 0,
            messages_collected: 0,
            gc_in_progress: false,
            last_low_water_mark: 0,
        };
        
        Self {
            scan_id,
            messages: Arc::new(RwLock::new(VecDeque::new())),
            sequence_tracker: Arc::new(RwLock::new(SequenceTracker::new())),
            consumer_registry: Arc::new(RwLock::new(ConsumerRegistry::new(config.consumer_timeout))),
            config,
            notification_manager,
            memory_monitor: MemoryMonitor::new(),
            gc_state: Arc::new(Mutex::new(gc_state)),
            stats: Arc::new(RwLock::new(stats)),
            active: Arc::new(RwLock::new(false)),
        }
    }
    
    
    
    /// Start the queue for message processing
    pub async fn start(&self) -> QueueResult<()> {
        let mut active = self.active.write().await;
        if *active {
            return Err(QueueError::operation_failed("Queue already started"));
        }
        
        *active = true;
        
        
        log::info!("Started multi-consumer queue: {}", self.scan_id);
        Ok(())
    }
    
    
    /// Check if queue is active
    pub async fn is_active(&self) -> bool {
        *self.active.read().await
    }
    
    /// Add a message to the queue
    pub async fn enqueue(&self, mut message: ScanMessage) -> QueueResult<u64> {
        if !self.is_active().await {
            return Err(QueueError::operation_failed("Queue not active"));
        }
        
        
        // Assign sequence number
        let sequence = {
            let mut tracker = self.sequence_tracker.write().await;
            let seq = tracker.next_sequence();
            message.header.sequence = seq;
            seq
        };
        
        // Check queue size limits
        {
            let messages = self.messages.read().await;
            if messages.len() >= self.config.max_queue_size {
                return Err(QueueError::QueueFull);
            }
        }
        
        // Wrap message in Arc for sharing
        let arc_message = Arc::new(message);
        
        // Record memory usage
        self.memory_monitor.record_push(&arc_message).await;
        
        // Add to queue
        let queue_size = {
            let mut messages = self.messages.write().await;
            messages.push_back(arc_message);
            messages.len()
        };
        
        // Update statistics
        self.update_queue_stats(queue_size).await;
        
        
        // Check for memory pressure
        if self.should_gc().await {
            self.trigger_garbage_collection().await?;
        }
        
        log::trace!("Enqueued message {} to queue '{}', size: {}", 
                   sequence, self.scan_id, queue_size);
        
        Ok(sequence)
    }
    
    /// Calculate the low water mark (minimum sequence across all consumers)
    pub async fn calculate_low_water_mark(&self) -> u64 {
        let registry = self.consumer_registry.read().await;
        let tracker = self.sequence_tracker.read().await;
        
        if registry.consumers.is_empty() {
            // No consumers, can use max sequence
            tracker.max_sequence
        } else {
            // Find minimum acknowledged sequence across all active consumers
            registry.consumers
                .values()
                .map(|progress| progress.last_acknowledged_seq)
                .min()
                .unwrap_or(tracker.max_sequence)
        }
    }
    
    /// Check if garbage collection should run
    async fn should_gc(&self) -> bool {
        if !self.config.auto_gc {
            return false;
        }
        
        let gc_state = self.gc_state.lock().await;
        
        // Don't run if already in progress
        if gc_state.gc_in_progress {
            return false;
        }
        
        // Check time-based trigger
        if let Some(last_gc) = gc_state.last_gc {
            if last_gc.elapsed() < self.config.gc_interval {
                return false;
            }
        }
        
        // Check size-based trigger
        let messages = self.messages.read().await;
        if messages.len() > self.config.max_queue_size / 2 {
            return true;
        }
        
        // Check memory-based trigger
        let memory_stats = self.memory_monitor.get_stats().await;
        memory_stats.current_size > self.config.memory_threshold
    }
    
    /// Trigger garbage collection
    async fn trigger_garbage_collection(&self) -> QueueResult<()> {
        let low_water_mark = self.calculate_low_water_mark().await;
        
        let mut gc_state = self.gc_state.lock().await;
        
        // Skip if already in progress or no improvement
        if gc_state.gc_in_progress || low_water_mark <= gc_state.last_low_water_mark {
            return Ok(());
        }
        
        gc_state.gc_in_progress = true;
        gc_state.last_low_water_mark = low_water_mark;
        drop(gc_state);
        
        // Perform garbage collection
        let collected = self.garbage_collect_messages(low_water_mark).await?;
        
        // Update GC state
        let mut gc_state = self.gc_state.lock().await;
        gc_state.gc_in_progress = false;
        gc_state.last_gc = Some(Instant::now());
        gc_state.gc_runs += 1;
        gc_state.messages_collected += collected;
        
        if collected > 0 {
            log::debug!("Garbage collected {} messages below sequence {} in queue '{}'", 
                       collected, low_water_mark, self.scan_id);
        }
        
        Ok(())
    }
    
    /// Perform actual garbage collection
    async fn garbage_collect_messages(&self, low_water_mark: u64) -> QueueResult<u64> {
        let mut messages = self.messages.write().await;
        let mut sequence_tracker = self.sequence_tracker.write().await;
        
        let mut collected = 0u64;
        let batch_size = self.config.gc_batch_size as u64;
        
        // Remove messages in batches to avoid blocking too long
        while let Some(front) = messages.front() {
            if front.header().sequence >= low_water_mark {
                break; // Reached messages we need to keep
            }
            
            if collected >= batch_size {
                break; // Collected enough for this run
            }
            
            if let Some(message) = messages.pop_front() {
                self.memory_monitor.record_pop(&message).await;
                collected += 1;
            }
        }
        
        // Update sequence tracker
        if collected > 0 {
            sequence_tracker.update_min_sequence(low_water_mark);
        }
        
        Ok(collected)
    }
    
    /// Update queue statistics
    async fn update_queue_stats(&self, queue_size: usize) {
        let mut stats = self.stats.write().await;
        let registry = self.consumer_registry.read().await;
        let memory_stats = self.memory_monitor.get_stats().await;
        let tracker = self.sequence_tracker.read().await;
        
        stats.queue_size = queue_size;
        stats.memory_usage = memory_stats.current_size as u64;
        stats.active_consumers = registry.consumers.len();
        stats.total_messages = tracker.total_messages();
    }
    
    /// Get current queue statistics
    pub async fn get_statistics(&self) -> QueueStatistics {
        let messages = self.messages.read().await;
        self.update_queue_stats(messages.len()).await;
        self.stats.read().await.clone()
    }
    
    /// Get memory statistics
    pub async fn get_memory_stats(&self) -> crate::queue::memory::QueueMemoryStats {
        self.memory_monitor.get_stats().await
    }
    
    /// Get messages starting from a specific sequence number
    pub async fn get_messages_from(&self, start_sequence: u64, limit: usize) -> QueueResult<Vec<Arc<ScanMessage>>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        
        let messages = self.messages.read().await;
        let tracker = self.sequence_tracker.read().await;
        
        // Check if start sequence is valid
        if start_sequence < tracker.min_sequence {
            return Err(QueueError::operation_failed(
                format!("Start sequence {} below minimum {}", start_sequence, tracker.min_sequence)
            ));
        }
        
        if start_sequence > tracker.max_sequence {
            return Ok(Vec::new()); // No messages available yet
        }
        
        let mut result = Vec::with_capacity(limit.min(messages.len()));
        let mut current_seq = start_sequence;
        
        for _ in 0..limit {
            if current_seq > tracker.max_sequence {
                break; // Reached end of available messages
            }
            
            // Calculate position in queue
            let queue_position = current_seq - tracker.min_sequence;
            
            if queue_position >= messages.len() as u64 {
                break; // Beyond current queue size
            }
            
            if let Some(message) = messages.get(queue_position as usize) {
                // Verify sequence number matches (sanity check)
                if message.header().sequence == current_seq {
                    result.push(Arc::clone(message));
                } else {
                    log::warn!("Sequence mismatch in queue: expected {}, found {}", 
                              current_seq, message.header().sequence);
                    break;
                }
            } else {
                break;
            }
            
            current_seq += 1;
        }
        
        Ok(result)
    }
    
    /// Get a specific message by sequence number
    pub async fn get_message_by_seq(&self, sequence: u64) -> QueueResult<Option<Arc<ScanMessage>>> {
        let messages = self.messages.read().await;
        let tracker = self.sequence_tracker.read().await;
        
        // Check if sequence is within valid range
        if sequence < tracker.min_sequence {
            return Ok(None); // Message has been garbage collected
        }
        
        if sequence > tracker.max_sequence {
            return Ok(None); // Message doesn't exist yet
        }
        
        // Calculate position in queue
        let queue_position = sequence - tracker.min_sequence;
        
        if queue_position >= messages.len() as u64 {
            return Ok(None); // Position is beyond current queue size
        }
        
        // Get message at position
        if let Some(message) = messages.get(queue_position as usize) {
            // Verify sequence number matches (sanity check)
            if message.header().sequence == sequence {
                Ok(Some(Arc::clone(message)))
            } else {
                log::warn!("Sequence mismatch in queue: expected {}, found {}", 
                          sequence, message.header().sequence);
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
    
    /// Get the current sequence range (min, max)
    pub async fn get_sequence_range(&self) -> (u64, u64) {
        let tracker = self.sequence_tracker.read().await;
        tracker.get_range()
    }
    
    /// Check if a sequence number is currently valid (within range)
    pub async fn is_valid_sequence(&self, sequence: u64) -> bool {
        let tracker = self.sequence_tracker.read().await;
        tracker.is_valid_sequence(sequence)
    }
    
    /// Get the next sequence number that would be assigned
    pub async fn get_next_sequence(&self) -> u64 {
        let tracker = self.sequence_tracker.read().await;
        tracker.next_sequence
    }
    
    /// Get total number of messages processed through the queue
    pub async fn get_total_messages(&self) -> u64 {
        let tracker = self.sequence_tracker.read().await;
        tracker.total_messages()
    }
    
    /// Acknowledge message processing for a consumer (alternative interface)
    pub async fn acknowledge_consumer(&self, consumer_id: &str, sequence: u64) -> QueueResult<()> {
        let mut registry = self.consumer_registry.write().await;
        registry.update_progress(consumer_id, sequence)
    }
    
    /// Get lag for a specific consumer
    pub async fn get_consumer_lag(&self, consumer_id: &str) -> QueueResult<u64> {
        let registry = self.consumer_registry.read().await;
        let tracker = self.sequence_tracker.read().await;
        
        if let Some(progress) = registry.get_progress(consumer_id) {
            Ok(tracker.max_sequence.saturating_sub(progress.last_acknowledged_seq))
        } else {
            Err(QueueError::operation_failed(
                format!("Consumer {} not found", consumer_id)
            ))
        }
    }
    
    /// Get the slowest consumer (highest lag)
    pub async fn get_slowest_consumer(&self) -> Option<(String, ConsumerProgress, u64)> {
        let registry = self.consumer_registry.read().await;
        let tracker = self.sequence_tracker.read().await;
        
        registry.consumers
            .iter()
            .map(|(id, progress)| {
                let lag = tracker.max_sequence.saturating_sub(progress.last_acknowledged_seq);
                (id.clone(), progress.clone(), lag)
            })
            .max_by_key(|(_, _, lag)| *lag)
    }
    
    /// Get all consumer lags as a map
    pub async fn get_all_consumer_lags(&self) -> HashMap<String, u64> {
        let registry = self.consumer_registry.read().await;
        let tracker = self.sequence_tracker.read().await;
        
        registry.consumers
            .iter()
            .map(|(id, progress)| {
                let lag = tracker.max_sequence.saturating_sub(progress.last_acknowledged_seq);
                (id.clone(), lag)
            })
            .collect()
    }
    
    /// Get consumers that are lagging behind by more than threshold
    pub async fn get_lagging_consumers(&self, lag_threshold: u64) -> Vec<(String, ConsumerProgress, u64)> {
        let registry = self.consumer_registry.read().await;
        let tracker = self.sequence_tracker.read().await;
        
        registry.consumers
            .iter()
            .filter_map(|(id, progress)| {
                let lag = tracker.max_sequence.saturating_sub(progress.last_acknowledged_seq);
                if lag > lag_threshold {
                    Some((id.clone(), progress.clone(), lag))
                } else {
                    None
                }
            })
            .collect()
    }
    
    
    
}

impl ConsumerRegistry {
    /// Create a new consumer registry
    pub fn new(_timeout: Duration) -> Self {
        Self {
            consumers: HashMap::new(),
        }
    }
    
    /// Register a new consumer
    pub fn register_consumer(&mut self, consumer_id: String, _plugin_name: String, _priority: i32) -> QueueResult<()> {
        if self.consumers.contains_key(&consumer_id) {
            return Err(QueueError::operation_failed(
                format!("Consumer {} already registered", consumer_id)
            ));
        }
        
        let now = Instant::now();
        let progress = ConsumerProgress {
            last_acknowledged_seq: 0,
            messages_processed: 0,
            last_update: now,
            created_at: now,
            processing_rate: 0.0,
        };
        
        self.consumers.insert(consumer_id, progress);
        Ok(())
    }
    
    /// Deregister a consumer
    pub fn deregister_consumer(&mut self, consumer_id: &str) -> QueueResult<()> {
        if self.consumers.remove(consumer_id).is_none() {
            return Err(QueueError::operation_failed(
                format!("Consumer {} not found", consumer_id)
            ));
        }
        Ok(())
    }
    
    /// Update consumer progress
    pub fn update_progress(&mut self, consumer_id: &str, acknowledged_seq: u64) -> QueueResult<()> {
        if let Some(progress) = self.consumers.get_mut(consumer_id) {
            progress.last_acknowledged_seq = acknowledged_seq;
            progress.messages_processed += 1;
            progress.last_update = Instant::now();
            
            // Update processing rate
            let elapsed = progress.created_at.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                progress.processing_rate = progress.messages_processed as f64 / elapsed;
            }
            
            Ok(())
        } else {
            Err(QueueError::operation_failed(
                format!("Consumer {} not found", consumer_id)
            ))
        }
    }
    
    
    /// Get all consumer progress
    pub fn get_all_progress(&self) -> Vec<&ConsumerProgress> {
        self.consumers.values().collect()
    }
    
    /// Get specific consumer progress
    pub fn get_progress(&self, consumer_id: &str) -> Option<&ConsumerProgress> {
        self.consumers.get(consumer_id)
    }
}

impl Clone for MultiConsumerQueue {
    fn clone(&self) -> Self {
        Self {
            scan_id: self.scan_id.clone(),
            messages: Arc::clone(&self.messages),
            sequence_tracker: Arc::clone(&self.sequence_tracker),
            consumer_registry: Arc::clone(&self.consumer_registry),
            config: self.config.clone(),
            notification_manager: Arc::clone(&self.notification_manager),
            memory_monitor: self.memory_monitor.clone(),
            gc_state: Arc::clone(&self.gc_state),
            stats: Arc::clone(&self.stats),
            active: Arc::clone(&self.active),
        }
    }
}

/// Publisher trait implementation for MultiConsumerQueue
#[async_trait::async_trait]
impl Publisher<QueueEvent> for MultiConsumerQueue {
    /// Publish a queue event to all subscribers
    async fn publish(&self, event: QueueEvent) -> crate::notifications::error::NotificationResult<()> {
        self.notification_manager.publish(event).await
    }
    
    /// Publish a queue event to a specific subscriber
    async fn publish_to(&self, event: QueueEvent, subscriber_id: &str) -> crate::notifications::error::NotificationResult<()> {
        self.notification_manager.publish_to(event, subscriber_id).await
    }
    
    /// Get the publisher identifier
    fn publisher_id(&self) -> &str {
        "queue"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};
    
    fn create_test_notification_manager() -> Arc<AsyncNotificationManager<QueueEvent>> {
        Arc::new(AsyncNotificationManager::new())
    }
    
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
    async fn test_multi_consumer_queue_creation() {
        let queue = MultiConsumerQueue::new("test-scan".to_string(), create_test_notification_manager());
        
        assert_eq!(queue.scan_id, "test-scan");
        assert!(!queue.is_active().await);
        
        let stats = queue.get_statistics().await;
        assert_eq!(stats.queue_size, 0);
        assert_eq!(stats.active_consumers, 0);
    }
    
    #[tokio::test]
    async fn test_queue_lifecycle() {
        let queue = MultiConsumerQueue::new("test-scan".to_string(), create_test_notification_manager());
        
        // Start queue
        queue.start().await.unwrap();
        assert!(queue.is_active().await);
    }
    
    #[tokio::test]
    async fn test_message_enqueue() {
        let queue = MultiConsumerQueue::new("test-scan".to_string(), create_test_notification_manager());
        queue.start().await.unwrap();
        
        let message = create_test_message(0);
        let sequence = queue.enqueue(message).await.unwrap();
        
        assert_eq!(sequence, 0);
        
        let stats = queue.get_statistics().await;
        assert_eq!(stats.queue_size, 1);
        assert_eq!(stats.total_messages, 1);
    }
    
    #[tokio::test]
    async fn test_sequence_tracker() {
        let mut tracker = SequenceTracker::new();
        
        assert_eq!(tracker.next_sequence(), 0);
        assert_eq!(tracker.next_sequence(), 1);
        assert_eq!(tracker.next_sequence(), 2);
        
        assert_eq!(tracker.get_range(), (0, 2));
        assert_eq!(tracker.total_messages(), 3);
        
        tracker.update_min_sequence(1);
        assert_eq!(tracker.get_range(), (1, 2));
    }
    
    #[tokio::test]
    async fn test_consumer_registry() {
        let mut registry = ConsumerRegistry::new(Duration::from_secs(60));
        
        // Register consumer
        registry.register_consumer(
            "test-consumer".to_string(), 
            "test-plugin".to_string(), 
            0
        ).unwrap();
        
        assert_eq!(registry.consumers.len(), 1);
        
        // Update progress
        registry.update_progress("test-consumer", 5).unwrap();
        
        let progress = registry.get_progress("test-consumer").unwrap();
        assert_eq!(progress.last_acknowledged_seq, 5);
        assert_eq!(progress.messages_processed, 1);
        
        // Deregister consumer
        registry.deregister_consumer("test-consumer").unwrap();
        assert_eq!(registry.consumers.len(), 0);
    }
    
    #[tokio::test]
    async fn test_low_water_mark_calculation() {
        let queue = MultiConsumerQueue::new("test-scan".to_string(), create_test_notification_manager());
        queue.start().await.unwrap();
        
        // No consumers - should use max sequence
        let low_water_mark = queue.calculate_low_water_mark().await;
        assert_eq!(low_water_mark, 0);
        
        // Add some messages
        for i in 0..5 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Still no consumers
        let low_water_mark = queue.calculate_low_water_mark().await;
        assert_eq!(low_water_mark, 4); // Max sequence
    }
    
    #[tokio::test]
    async fn test_queue_statistics() {
        let queue = MultiConsumerQueue::new("test-scan".to_string(), create_test_notification_manager());
        queue.start().await.unwrap();
        
        // Add messages
        for i in 0..3 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        let stats = queue.get_statistics().await;
        assert_eq!(stats.queue_size, 3);
        assert_eq!(stats.total_messages, 3);
        assert_eq!(stats.active_consumers, 0);
    }
    
    #[tokio::test]
    async fn test_get_messages_from() {
        let queue = MultiConsumerQueue::new("test-scan".to_string(), create_test_notification_manager());
        queue.start().await.unwrap();
        
        // Add messages
        for i in 0..5 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Get messages from sequence 1, limit 3
        let messages = queue.get_messages_from(1, 3).await.unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].header().sequence, 1);
        assert_eq!(messages[1].header().sequence, 2);
        assert_eq!(messages[2].header().sequence, 3);
        
        // Get messages from sequence 3, limit 10 (should only get 2)
        let messages = queue.get_messages_from(3, 10).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].header().sequence, 3);
        assert_eq!(messages[1].header().sequence, 4);
        
        // Try to get messages from non-existent sequence
        let messages = queue.get_messages_from(100, 5).await.unwrap();
        assert_eq!(messages.len(), 0);
        
        // Test limit 0
        let messages = queue.get_messages_from(0, 0).await.unwrap();
        assert_eq!(messages.len(), 0);
    }
    
    #[tokio::test]
    async fn test_get_message_by_seq() {
        let queue = MultiConsumerQueue::new("test-scan".to_string(), create_test_notification_manager());
        queue.start().await.unwrap();
        
        // Add messages
        for i in 0..3 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Get existing message
        let message = queue.get_message_by_seq(1).await.unwrap();
        assert!(message.is_some());
        assert_eq!(message.unwrap().header().sequence, 1);
        
        // Get non-existent message (future)
        let message = queue.get_message_by_seq(100).await.unwrap();
        assert!(message.is_none());
        
        // Get message at boundary
        let message = queue.get_message_by_seq(2).await.unwrap();
        assert!(message.is_some());
        assert_eq!(message.unwrap().header().sequence, 2);
    }
    
    #[tokio::test]
    async fn test_sequence_range_methods() {
        let queue = MultiConsumerQueue::new("test-scan".to_string(), create_test_notification_manager());
        queue.start().await.unwrap();
        
        // Initial range should be (0, 0) with no messages
        let range = queue.get_sequence_range().await;
        assert_eq!(range, (0, 0));
        
        // Add messages and check range updates
        for i in 0..3 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        let range = queue.get_sequence_range().await;
        assert_eq!(range, (0, 2));
        
        // Test validity checks
        assert!(queue.is_valid_sequence(0).await);
        assert!(queue.is_valid_sequence(1).await);
        assert!(queue.is_valid_sequence(2).await);
        assert!(!queue.is_valid_sequence(3).await);
        
        // Test next sequence
        let next_seq = queue.get_next_sequence().await;
        assert_eq!(next_seq, 3);
        
        // Test total messages
        let total = queue.get_total_messages().await;
        assert_eq!(total, 3);
    }
    
    #[tokio::test]
    async fn test_sequence_retrieval_with_gaps() {
        let queue = MultiConsumerQueue::new("test-scan".to_string(), create_test_notification_manager());
        queue.start().await.unwrap();
        
        // Add messages
        for i in 0..5 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Simulate garbage collection by updating min sequence
        {
            let mut tracker = queue.sequence_tracker.write().await;
            tracker.update_min_sequence(2);
        }
        
        // Update message queue to simulate GC (remove first 2 messages)
        {
            let mut messages = queue.messages.write().await;
            messages.pop_front();
            messages.pop_front();
        }
        
        // Now sequences 0,1 should be unavailable
        let message = queue.get_message_by_seq(0).await.unwrap();
        assert!(message.is_none());
        
        let message = queue.get_message_by_seq(1).await.unwrap();
        assert!(message.is_none());
        
        // But sequences 2,3,4 should still be available
        let message = queue.get_message_by_seq(2).await.unwrap();
        assert!(message.is_some());
        assert_eq!(message.unwrap().header().sequence, 2);
        
        // Test get_messages_from with GC'd sequences
        let result = queue.get_messages_from(0, 5).await;
        assert!(result.is_err()); // Should error because start sequence is below minimum
        
        let messages = queue.get_messages_from(2, 5).await.unwrap();
        assert_eq!(messages.len(), 3); // Should get sequences 2, 3, 4
        assert_eq!(messages[0].header().sequence, 2);
        assert_eq!(messages[2].header().sequence, 4);
    }
    
    #[tokio::test]
    async fn test_consumer_acknowledgment_system() {
        let queue = MultiConsumerQueue::new("test-scan".to_string(), create_test_notification_manager());
        queue.start().await.unwrap();
        
        // Add messages
        for i in 0..5 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Register multiple consumers
        let consumer1 = queue.register_consumer("plugin1".to_string()).await.unwrap();
        let consumer2 = queue.register_consumer("plugin2".to_string()).await.unwrap();
        
        // Acknowledge different sequences for each consumer
        queue.acknowledge_consumer(consumer1.consumer_id(), 2).await.unwrap();
        queue.acknowledge_consumer(consumer2.consumer_id(), 1).await.unwrap();
        
        // Test consumer lag calculation
        let lag1 = queue.get_consumer_lag(consumer1.consumer_id()).await.unwrap();
        let lag2 = queue.get_consumer_lag(consumer2.consumer_id()).await.unwrap();
        
        // Max sequence is 4, so:
        // Consumer1: acknowledged 2, lag = 4 - 2 = 2
        // Consumer2: acknowledged 1, lag = 4 - 1 = 3  
        assert_eq!(lag1, 2);
        assert_eq!(lag2, 3);
        
        // Test slowest consumer
        let slowest = queue.get_slowest_consumer().await;
        assert!(slowest.is_some());
        let (slowest_id, _, slowest_lag) = slowest.unwrap();
        assert_eq!(slowest_id, consumer2.consumer_id());
        assert_eq!(slowest_lag, 3);
        
        // Test all consumer lags
        let all_lags = queue.get_all_consumer_lags().await;
        assert_eq!(all_lags.len(), 2);
        assert_eq!(all_lags[consumer1.consumer_id()], 2);
        assert_eq!(all_lags[consumer2.consumer_id()], 3);
    }
    
    #[tokio::test]
    async fn test_lagging_consumers() {
        let queue = MultiConsumerQueue::new("test-scan".to_string(), create_test_notification_manager());
        queue.start().await.unwrap();
        
        // Add messages
        for i in 0..10 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Register consumers with different acknowledgment levels
        let consumer1 = queue.register_consumer("fast-plugin".to_string()).await.unwrap();
        let consumer2 = queue.register_consumer("slow-plugin".to_string()).await.unwrap();
        let consumer3 = queue.register_consumer("medium-plugin".to_string()).await.unwrap();
        
        // Fast consumer: acknowledged up to 8 (lag = 1)
        queue.acknowledge_consumer(consumer1.consumer_id(), 8).await.unwrap();
        
        // Slow consumer: acknowledged up to 3 (lag = 6) 
        queue.acknowledge_consumer(consumer2.consumer_id(), 3).await.unwrap();
        
        // Medium consumer: acknowledged up to 6 (lag = 3)
        queue.acknowledge_consumer(consumer3.consumer_id(), 6).await.unwrap();
        
        // Test lagging consumers with threshold 4
        let lagging = queue.get_lagging_consumers(4).await;
        assert_eq!(lagging.len(), 1);
        assert_eq!(lagging[0].0, consumer2.consumer_id());
        assert_eq!(lagging[0].2, 6); // lag
        
        // Test lagging consumers with threshold 2
        let lagging = queue.get_lagging_consumers(2).await;
        assert_eq!(lagging.len(), 2); // slow and medium consumers
        
        // Test lagging consumers with threshold 0
        let lagging = queue.get_lagging_consumers(0).await;
        assert_eq!(lagging.len(), 3); // all consumers have some lag
    }
    
    
    
    #[tokio::test]
    async fn test_acknowledge_nonexistent_consumer() {
        let queue = MultiConsumerQueue::new("test-scan".to_string(), create_test_notification_manager());
        queue.start().await.unwrap();
        
        // Try to acknowledge for non-existent consumer
        let result = queue.acknowledge_consumer("nonexistent", 5).await;
        assert!(result.is_err());
        
        // Try to get lag for non-existent consumer
        let result = queue.get_consumer_lag("nonexistent").await;
        assert!(result.is_err());
    }
}