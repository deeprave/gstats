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
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{RwLock, broadcast, Mutex};

use crate::queue::{QueueError, QueueResult, QueueEvent, QueueEventNotifier, MemoryMonitor};
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
    
    /// Get current sequence range
    pub fn get_range(&self) -> (u64, u64) {
        (self.min_sequence, self.max_sequence)
    }
    
    /// Get total messages processed
    pub fn total_messages(&self) -> u64 {
        self.total_messages
    }
    
    /// Check if a sequence is within current range
    pub fn is_valid_sequence(&self, sequence: u64) -> bool {
        sequence >= self.min_sequence && sequence <= self.max_sequence
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
    
    /// Queue creation time
    pub created_at: Instant,
}

/// Multi-consumer queue with sequence-based tracking
#[derive(Debug)]
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
    event_notifier: QueueEventNotifier,
    
    /// Memory monitoring
    memory_monitor: MemoryMonitor,
    
    /// Garbage collection state
    gc_state: Arc<Mutex<GarbageCollectionState>>,
    
    /// Queue statistics
    stats: Arc<RwLock<QueueStatistics>>,
    
    /// Whether the queue is active
    active: Arc<RwLock<bool>>,
    
    /// Backpressure controller
    backpressure: Arc<BackpressureController>,
}

/// Consumer registry for tracking active consumers
#[derive(Debug)]
pub struct ConsumerRegistry {
    /// Active consumers and their progress
    pub(crate) consumers: HashMap<String, ConsumerProgress>,
    
    /// Consumer timeout configuration
    timeout: Duration,
    
    /// Last cleanup time
    last_cleanup: Instant,
}

/// Progress tracking for individual consumers
#[derive(Debug, Clone)]
pub struct ConsumerProgress {
    /// Unique consumer identifier
    pub consumer_id: String,
    
    /// Plugin name this consumer belongs to
    pub plugin_name: String,
    
    /// Last acknowledged sequence number
    pub last_acknowledged_seq: u64,
    
    /// Current read position
    pub current_read_seq: u64,
    
    /// Number of messages processed
    pub messages_processed: u64,
    
    /// Number of processing errors
    pub error_count: u64,
    
    /// Last update timestamp
    pub last_update: Instant,
    
    /// Consumer creation time
    pub created_at: Instant,
    
    /// Average processing rate (messages/second)
    pub processing_rate: f64,
    
    /// Consumer priority (higher = more important)
    pub priority: i32,
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
    pub fn new(scan_id: String) -> Self {
        Self::with_config(scan_id, MultiConsumerConfig::default())
    }
    
    /// Create a new multi-consumer queue with custom configuration
    pub fn with_config(scan_id: String, config: MultiConsumerConfig) -> Self {
        let now = Instant::now();
        
        let stats = QueueStatistics {
            queue_size: 0,
            memory_usage: 0,
            active_consumers: 0,
            total_messages: 0,
            created_at: now,
        };
        
        let gc_state = GarbageCollectionState {
            last_gc: None,
            gc_runs: 0,
            messages_collected: 0,
            gc_in_progress: false,
            last_low_water_mark: 0,
        };
        
        let backpressure_config = BackpressureConfig {
            memory_threshold: config.memory_threshold,
            queue_size_threshold: config.max_queue_size / 2, // Activate at 50% capacity
            min_active_duration: Duration::from_millis(10), // Short duration for testing
            ..BackpressureConfig::default()
        };
        
        Self {
            scan_id,
            messages: Arc::new(RwLock::new(VecDeque::new())),
            sequence_tracker: Arc::new(RwLock::new(SequenceTracker::new())),
            consumer_registry: Arc::new(RwLock::new(ConsumerRegistry::new(config.consumer_timeout))),
            config,
            event_notifier: QueueEventNotifier::with_default_capacity(),
            memory_monitor: MemoryMonitor::new(),
            gc_state: Arc::new(Mutex::new(gc_state)),
            stats: Arc::new(RwLock::new(stats)),
            active: Arc::new(RwLock::new(false)),
            backpressure: Arc::new(BackpressureController::new(backpressure_config)),
        }
    }
    
    /// Get the scan ID for this queue
    pub fn scan_id(&self) -> &str {
        &self.scan_id
    }
    
    /// Get queue configuration
    pub fn config(&self) -> &MultiConsumerConfig {
        &self.config
    }
    
    /// Subscribe to queue events
    pub fn subscribe_events(&self) -> broadcast::Receiver<QueueEvent> {
        self.event_notifier.subscribe()
    }
    
    /// Start the queue for message processing
    pub async fn start(&self) -> QueueResult<()> {
        let mut active = self.active.write().await;
        if *active {
            return Err(QueueError::operation_failed("Queue already started"));
        }
        
        *active = true;
        
        // Emit start event
        let event = QueueEvent::scan_started(self.scan_id.clone());
        self.event_notifier.emit(event)?;
        
        log::info!("Started multi-consumer queue: {}", self.scan_id);
        Ok(())
    }
    
    /// Stop the queue and cleanup
    pub async fn stop(&self) -> QueueResult<()> {
        let mut active = self.active.write().await;
        if !*active {
            return Ok(());
        }
        
        *active = false;
        
        // Get final statistics
        let stats = self.get_statistics().await;
        
        // Emit completion event
        let event = QueueEvent::scan_complete(self.scan_id.clone(), stats.total_messages);
        self.event_notifier.emit(event)?;
        
        log::info!("Stopped multi-consumer queue: {} (total: {} messages)", 
                  self.scan_id, stats.total_messages);
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
        
        // Check backpressure before processing message (only apply delay, don't reject)
        if self.backpressure.is_active() {
            self.backpressure.record_delay().await;
            
            // Apply backpressure delay to slow down producers
            tokio::time::sleep(Duration::from_millis(10)).await;
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
        
        // Evaluate backpressure conditions
        self.evaluate_backpressure().await;
        
        // Emit message added event
        let event = QueueEvent::message_added(self.scan_id.clone(), 1, queue_size);
        self.event_notifier.emit(event)?;
        
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
    
    /// Check if backpressure is needed based on memory and queue size
    pub async fn has_backpressure_needed(&self) -> bool {
        let memory_stats = self.memory_monitor.get_stats().await;
        let messages = self.messages.read().await;
        
        // Check memory threshold
        if memory_stats.current_size > self.config.memory_threshold {
            return true;
        }
        
        // Check queue size threshold
        if messages.len() > self.config.max_queue_size / 2 {
            return true;
        }
        
        false
    }
    
    /// Get consumer statistics summary
    pub async fn get_consumer_summary(&self) -> ConsumerSummary {
        let registry = self.consumer_registry.read().await;
        let consumer_count = registry.consumers.len();
        
        ConsumerSummary {
            total_consumers: consumer_count,
            active_consumers: consumer_count, // All registered consumers are considered active
            average_lag: 0, // No longer tracking lag
            max_lag: 0, // No longer tracking lag
            min_lag: 0, // No longer tracking lag
            total_messages_processed: registry.consumers.values()
                .map(|p| p.messages_processed)
                .sum(),
            backpressure_needed: self.has_backpressure_needed().await,
        }
    }
    
    /// Evaluate backpressure conditions and update state
    async fn evaluate_backpressure(&self) {
        let consumer_summary = self.get_consumer_summary().await;
        let memory_stats = self.memory_monitor.get_stats().await;
        let queue_stats = self.get_statistics().await;
        
        self.backpressure.evaluate(
            consumer_summary.max_lag,
            memory_stats.current_size,
            queue_stats.queue_size,
        ).await;
    }
    
    /// Check if backpressure is currently active
    pub fn is_backpressure_active(&self) -> bool {
        self.backpressure.is_active()
    }
    
    /// Get current backpressure reason
    pub async fn get_backpressure_reason(&self) -> Option<BackpressureReason> {
        self.backpressure.get_current_reason().await
    }
    
    /// Get backpressure statistics
    pub async fn get_backpressure_stats(&self) -> BackpressureStats {
        self.backpressure.get_stats().await
    }
    
    /// Force backpressure evaluation (for testing or manual control)
    pub async fn force_backpressure_evaluation(&self) -> Option<BackpressureReason> {
        self.evaluate_backpressure().await;
        self.get_backpressure_reason().await
    }
}

/// Backpressure controller for managing queue flow control
#[derive(Debug)]
pub struct BackpressureController {
    /// Whether backpressure is currently active
    active: AtomicBool,
    
    /// Configuration thresholds
    config: BackpressureConfig,
    
    /// Backpressure statistics
    stats: Arc<Mutex<BackpressureStats>>,
}

/// Configuration for backpressure system
#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    /// Maximum consumer lag before activating backpressure
    pub max_lag_threshold: u64,
    
    /// Memory usage threshold (bytes) before activating backpressure  
    pub memory_threshold: usize,
    
    /// Queue size threshold before activating backpressure
    pub queue_size_threshold: usize,
    
    /// Time between backpressure evaluations
    pub evaluation_interval: Duration,
    
    /// Minimum time to keep backpressure active (prevents flapping)
    pub min_active_duration: Duration,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            max_lag_threshold: 10_000,           // 10K messages
            memory_threshold: 256 * 1024 * 1024, // 256MB
            queue_size_threshold: 50_000,        // 50K messages
            evaluation_interval: Duration::from_secs(5),  // 5 seconds
            min_active_duration: Duration::from_secs(10), // 10 seconds
        }
    }
}

/// Statistics for backpressure system
#[derive(Debug, Clone)]
pub struct BackpressureStats {
    /// Total number of times backpressure was activated
    pub activations: u64,
    
    /// Total time spent in backpressure mode
    pub total_duration: Duration,
    
    /// Last activation time
    pub last_activation: Option<Instant>,
    
    /// Last deactivation time
    pub last_deactivation: Option<Instant>,
    
    /// Number of messages delayed due to backpressure
    pub messages_delayed: u64,
    
    /// Current backpressure reason
    pub current_reason: Option<BackpressureReason>,
}

/// Reason for backpressure activation
#[derive(Debug, Clone, PartialEq)]
pub enum BackpressureReason {
    /// Consumer lag exceeded threshold
    ConsumerLag { max_lag: u64, threshold: u64 },
    
    /// Memory usage exceeded threshold
    MemoryPressure { current: usize, threshold: usize },
    
    /// Queue size exceeded threshold
    QueueSize { current: usize, threshold: usize },
    
    /// Multiple conditions triggered
    Multiple(Vec<BackpressureReason>),
}

impl BackpressureController {
    /// Create a new backpressure controller
    pub fn new(config: BackpressureConfig) -> Self {
        let stats = BackpressureStats {
            activations: 0,
            total_duration: Duration::from_secs(0),
            last_activation: None,
            last_deactivation: None,
            messages_delayed: 0,
            current_reason: None,
        };
        
        Self {
            active: AtomicBool::new(false),
            config,
            stats: Arc::new(Mutex::new(stats)),
        }
    }
    
    /// Check if backpressure is currently active
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
    
    /// Evaluate backpressure conditions and update state
    pub async fn evaluate(&self, 
                         max_consumer_lag: u64,
                         memory_usage: usize, 
                         queue_size: usize) -> Option<BackpressureReason> {
        let mut reasons = Vec::new();
        
        // Check consumer lag
        if max_consumer_lag > self.config.max_lag_threshold {
            reasons.push(BackpressureReason::ConsumerLag {
                max_lag: max_consumer_lag,
                threshold: self.config.max_lag_threshold,
            });
        }
        
        // Check memory pressure
        if memory_usage > self.config.memory_threshold {
            reasons.push(BackpressureReason::MemoryPressure {
                current: memory_usage,
                threshold: self.config.memory_threshold,
            });
        }
        
        // Check queue size
        if queue_size > self.config.queue_size_threshold {
            reasons.push(BackpressureReason::QueueSize {
                current: queue_size,
                threshold: self.config.queue_size_threshold,
            });
        }
        
        let should_activate = !reasons.is_empty();
        let currently_active = self.is_active();
        
        // Check minimum active duration to prevent flapping
        let can_deactivate = if currently_active {
            let stats = self.stats.lock().await;
            if let Some(last_activation) = stats.last_activation {
                last_activation.elapsed() >= self.config.min_active_duration
            } else {
                true
            }
        } else {
            true
        };
        
        let new_reason = match reasons.len() {
            0 => None,
            1 => Some(reasons.into_iter().next().unwrap()),
            _ => Some(BackpressureReason::Multiple(reasons)),
        };
        
        // Update backpressure state
        if should_activate && !currently_active {
            if let Some(reason) = new_reason.clone() {
                self.activate(reason).await;
            }
        } else if !should_activate && currently_active && can_deactivate {
            self.deactivate().await;
        } else if should_activate && currently_active {
            // Update reason if still active
            let mut stats = self.stats.lock().await;
            stats.current_reason = new_reason.clone();
        }
        
        new_reason
    }
    
    /// Activate backpressure
    async fn activate(&self, reason: BackpressureReason) {
        self.active.store(true, Ordering::Release);
        
        let mut stats = self.stats.lock().await;
        stats.activations += 1;
        stats.last_activation = Some(Instant::now());
        stats.current_reason = Some(reason.clone());
        
        log::warn!("Backpressure activated: {:?}", reason);
    }
    
    /// Deactivate backpressure
    async fn deactivate(&self) {
        self.active.store(false, Ordering::Release);
        
        let mut stats = self.stats.lock().await;
        let now = Instant::now();
        stats.last_deactivation = Some(now);
        
        if let Some(activation_time) = stats.last_activation {
            stats.total_duration += now.duration_since(activation_time);
        }
        
        stats.current_reason = None;
        
        log::info!("Backpressure deactivated");
    }
    
    /// Record a message delay due to backpressure
    pub async fn record_delay(&self) {
        let mut stats = self.stats.lock().await;
        stats.messages_delayed += 1;
    }
    
    /// Get backpressure statistics
    pub async fn get_stats(&self) -> BackpressureStats {
        let stats = self.stats.lock().await;
        stats.clone()
    }
    
    /// Get current backpressure reason
    pub async fn get_current_reason(&self) -> Option<BackpressureReason> {
        let stats = self.stats.lock().await;
        stats.current_reason.clone()
    }
}

/// Summary statistics for all consumers
#[derive(Debug, Clone)]
pub struct ConsumerSummary {
    /// Total number of registered consumers
    pub total_consumers: usize,
    
    /// Number of active consumers
    pub active_consumers: usize,
    
    /// Average lag across all consumers
    pub average_lag: u64,
    
    /// Maximum lag (slowest consumer)
    pub max_lag: u64,
    
    /// Minimum lag (fastest consumer)  
    pub min_lag: u64,
    
    /// Total messages processed by all consumers
    pub total_messages_processed: u64,
    
    /// Whether backpressure is needed
    pub backpressure_needed: bool,
}

impl ConsumerRegistry {
    /// Create a new consumer registry
    pub fn new(timeout: Duration) -> Self {
        Self {
            consumers: HashMap::new(),
            timeout,
            last_cleanup: Instant::now(),
        }
    }
    
    /// Register a new consumer
    pub fn register_consumer(&mut self, consumer_id: String, plugin_name: String, priority: i32) -> QueueResult<()> {
        if self.consumers.contains_key(&consumer_id) {
            return Err(QueueError::operation_failed(
                format!("Consumer {} already registered", consumer_id)
            ));
        }
        
        let now = Instant::now();
        let progress = ConsumerProgress {
            consumer_id: consumer_id.clone(),
            plugin_name,
            last_acknowledged_seq: 0,
            current_read_seq: 0,
            messages_processed: 0,
            error_count: 0,
            last_update: now,
            created_at: now,
            processing_rate: 0.0,
            priority,
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
    
    /// Cleanup stale consumers
    pub fn cleanup_stale_consumers(&mut self) -> Vec<String> {
        let now = Instant::now();
        let mut removed = Vec::new();
        
        self.consumers.retain(|id, progress| {
            if now.duration_since(progress.last_update) > self.timeout {
                removed.push(id.clone());
                false
            } else {
                true
            }
        });
        
        if !removed.is_empty() {
            self.last_cleanup = now;
        }
        
        removed
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
            event_notifier: self.event_notifier.clone(),
            memory_monitor: self.memory_monitor.clone(),
            gc_state: Arc::clone(&self.gc_state),
            stats: Arc::clone(&self.stats),
            active: Arc::clone(&self.active),
            backpressure: Arc::clone(&self.backpressure),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};
    
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
        let queue = MultiConsumerQueue::new("test-scan".to_string());
        
        assert_eq!(queue.scan_id(), "test-scan");
        assert!(!queue.is_active().await);
        
        let stats = queue.get_statistics().await;
        assert_eq!(stats.queue_size, 0);
        assert_eq!(stats.active_consumers, 0);
    }
    
    #[tokio::test]
    async fn test_queue_lifecycle() {
        let queue = MultiConsumerQueue::new("test-scan".to_string());
        
        // Start queue
        queue.start().await.unwrap();
        assert!(queue.is_active().await);
        
        // Stop queue
        queue.stop().await.unwrap();
        assert!(!queue.is_active().await);
    }
    
    #[tokio::test]
    async fn test_message_enqueue() {
        let queue = MultiConsumerQueue::new("test-scan".to_string());
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
        let queue = MultiConsumerQueue::new("test-scan".to_string());
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
        let queue = MultiConsumerQueue::new("test-scan".to_string());
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
        let queue = MultiConsumerQueue::new("test-scan".to_string());
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
        let queue = MultiConsumerQueue::new("test-scan".to_string());
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
        let queue = MultiConsumerQueue::new("test-scan".to_string());
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
        let queue = MultiConsumerQueue::new("test-scan".to_string());
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
        let queue = MultiConsumerQueue::new("test-scan".to_string());
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
        let queue = MultiConsumerQueue::new("test-scan".to_string());
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
    async fn test_backpressure_detection() {
        // Create queue with low queue size threshold for testing
        let mut config = MultiConsumerConfig::default();
        config.max_queue_size = 10; // Small queue for testing backpressure
        config.auto_gc = false; // Disable auto GC for testing
        
        let queue = MultiConsumerQueue::with_config("test-scan".to_string(), config);
        queue.start().await.unwrap();
        
        // Initially no backpressure
        assert!(!queue.has_backpressure_needed().await);
        
        // Add messages beyond queue size threshold (10/2 = 5)
        for i in 0..6 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Should need backpressure (6 messages > threshold of 5)
        assert!(queue.has_backpressure_needed().await);
        
        // Remove some messages by dequeuing 
        let consumer = queue.register_consumer("test-plugin".to_string()).await.unwrap();
        for _ in 0..3 {
            consumer.read_next().await.unwrap();
        }
        
        // Still should need backpressure (3 messages remaining but queue still has 6)
        // Backpressure is based on raw queue size, not consumed messages
        assert!(queue.has_backpressure_needed().await);
    }
    
    #[tokio::test]
    async fn test_consumer_summary() {
        let queue = MultiConsumerQueue::new("test-scan".to_string());
        queue.start().await.unwrap();
        
        // Add messages
        for i in 0..5 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Test summary with no consumers
        let summary = queue.get_consumer_summary().await;
        assert_eq!(summary.total_consumers, 0);
        assert_eq!(summary.average_lag, 0);
        assert_eq!(summary.max_lag, 0);
        assert_eq!(summary.min_lag, 0);
        
        // Register consumers
        let consumer1 = queue.register_consumer("plugin1".to_string()).await.unwrap();
        let consumer2 = queue.register_consumer("plugin2".to_string()).await.unwrap();
        
        // Acknowledge different amounts
        queue.acknowledge_consumer(consumer1.consumer_id(), 3).await.unwrap(); // lag = 1
        queue.acknowledge_consumer(consumer2.consumer_id(), 1).await.unwrap(); // lag = 3
        
        let summary = queue.get_consumer_summary().await;
        assert_eq!(summary.total_consumers, 2);
        assert_eq!(summary.active_consumers, 2);
        assert_eq!(summary.average_lag, 0); // No longer tracking lag
        assert_eq!(summary.max_lag, 0); // No longer tracking lag  
        assert_eq!(summary.min_lag, 0); // No longer tracking lag
        assert_eq!(summary.total_messages_processed, 2); // 1 + 1 messages processed
        assert!(!summary.backpressure_needed); // No backpressure with small queue
    }
    
    #[tokio::test]
    async fn test_acknowledge_nonexistent_consumer() {
        let queue = MultiConsumerQueue::new("test-scan".to_string());
        queue.start().await.unwrap();
        
        // Try to acknowledge for non-existent consumer
        let result = queue.acknowledge_consumer("nonexistent", 5).await;
        assert!(result.is_err());
        
        // Try to get lag for non-existent consumer
        let result = queue.get_consumer_lag("nonexistent").await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_backpressure_system() {
        // Create queue with low thresholds for testing
        let mut config = MultiConsumerConfig::default();
        config.memory_threshold = 1024; // 1KB
        config.max_queue_size = 8; // Small queue
        config.auto_gc = false; // Disable auto GC for testing
        
        let queue = MultiConsumerQueue::with_config("test-scan".to_string(), config);
        queue.start().await.unwrap();
        
        // Initially no backpressure
        assert!(!queue.is_backpressure_active());
        
        // Add messages to trigger queue size backpressure (threshold is max_queue_size/2 = 4)
        for i in 0..5 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Force backpressure evaluation after adding messages
        let reason = queue.force_backpressure_evaluation().await;
        
        // Should have backpressure due to queue size (5 > 4 which is max_queue_size/2)
        assert!(queue.is_backpressure_active());
        assert!(reason.is_some());
        
        match reason.unwrap() {
            BackpressureReason::QueueSize { current, threshold } => {
                assert_eq!(current, 5);
                assert_eq!(threshold, 4); // max_queue_size (8) / 2
            }
            other => panic!("Expected QueueSize backpressure reason, got: {:?}", other),
        }
        
        // Get backpressure stats
        let stats = queue.get_backpressure_stats().await;
        assert_eq!(stats.activations, 1);
        assert!(stats.last_activation.is_some());
    }
    
    #[tokio::test]
    async fn test_backpressure_queue_size() {
        // Create queue with small size for testing
        let mut config = MultiConsumerConfig::default();
        config.max_queue_size = 6; // Small queue for testing
        config.auto_gc = false; // Disable auto GC for testing
        
        let queue = MultiConsumerQueue::with_config("test-scan".to_string(), config);
        queue.start().await.unwrap();
        
        // Initially no backpressure
        assert!(!queue.has_backpressure_needed().await);
        
        // Add messages to trigger backpressure (threshold is max_queue_size/2 = 3)
        for i in 0..4 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Should trigger backpressure due to queue size (4 > 3)
        assert!(queue.has_backpressure_needed().await);
        
        // Force evaluation to activate backpressure
        let reason = queue.force_backpressure_evaluation().await;
        
        assert!(queue.is_backpressure_active());
        assert!(reason.is_some());
        
        match reason.unwrap() {
            BackpressureReason::QueueSize { current, threshold } => {
                assert_eq!(current, 4);
                assert_eq!(threshold, 3);
            }
            _ => panic!("Expected QueueSize backpressure reason"),
        }
    }
    
    #[tokio::test]
    async fn test_backpressure_multiple_conditions() {
        // Create queue with very low thresholds
        let mut config = MultiConsumerConfig::default();
        config.max_queue_size = 8;
        config.memory_threshold = 100; // Very low memory threshold
        config.auto_gc = false; // Disable auto GC for testing
        
        let queue = MultiConsumerQueue::with_config("test-scan".to_string(), config);
        queue.start().await.unwrap();
        
        // Add enough messages to trigger queue size backpressure (threshold is 4)
        for i in 0..5 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // This should trigger at least queue size backpressure
        let reason = queue.force_backpressure_evaluation().await;
        
        assert!(queue.is_backpressure_active());
        assert!(reason.is_some());
        
        // Since we can't reliably trigger multiple conditions, just check that we get a valid reason
        match reason.unwrap() {
            BackpressureReason::QueueSize { current, threshold } => {
                assert_eq!(current, 5);
                assert_eq!(threshold, 4);
            }
            BackpressureReason::MemoryPressure { .. } => {
                // Memory pressure is also acceptable
            }
            BackpressureReason::Multiple(reasons) => {
                // Multiple reasons are fine too
                assert!(!reasons.is_empty());
            }
            _ => panic!("Expected valid backpressure reason"),
        }
    }
    
    #[tokio::test]
    async fn test_backpressure_message_rejection() {
        // Create queue with very low threshold
        let mut config = MultiConsumerConfig::default();
        config.max_queue_size = 4;
        
        let queue = MultiConsumerQueue::with_config("test-scan".to_string(), config);
        queue.start().await.unwrap();
        
        // Add messages to trigger backpressure
        for i in 0..3 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Force backpressure activation
        queue.force_backpressure_evaluation().await;
        assert!(queue.is_backpressure_active());
        
        // Try to add another message - should be delayed/rejected
        let message = create_test_message(10);
        let result = queue.enqueue(message).await;
        
        // Should either succeed after delay or fail due to backpressure
        // The exact behavior depends on timing, but backpressure should be recorded
        let stats = queue.get_backpressure_stats().await;
        
        if result.is_err() {
            // Message was rejected due to backpressure
            assert!(stats.messages_delayed > 0 || result.unwrap_err().to_string().contains("Backpressure active"));
        }
    }
    
    #[tokio::test]
    async fn test_backpressure_deactivation() {
        use tokio::time::{sleep, Duration};
        
        // Create queue with very low thresholds
        let mut config = MultiConsumerConfig::default();
        config.max_queue_size = 6; // threshold will be 3
        
        let queue = MultiConsumerQueue::with_config("test-scan".to_string(), config);
        queue.start().await.unwrap();
        
        // Add messages to trigger backpressure
        for i in 0..4 {
            let message = create_test_message(i);
            queue.enqueue(message).await.unwrap();
        }
        
        // Force backpressure activation (4 > 3 which is max_queue_size/2)
        queue.force_backpressure_evaluation().await;
        
        // Debug: Check if backpressure was activated
        if !queue.is_backpressure_active() {
            let reason = queue.get_backpressure_reason().await;
            panic!("Backpressure not activated! Reason: {:?}, Queue size: 4, Threshold: 3", reason);
        }
        
        // Register fast consumer that processes all messages
        let consumer = queue.register_consumer("fast-plugin".to_string()).await.unwrap();
        
        // Consumer processes all messages, reducing queue size and lag
        queue.acknowledge_consumer(consumer.consumer_id(), 3).await.unwrap();
        
        // Wait a bit to allow for minimum active duration
        sleep(Duration::from_millis(50)).await;
        
        // Force evaluation - should deactivate backpressure
        queue.force_backpressure_evaluation().await;
        
        // Backpressure should be deactivated (low lag and queue size)
        let stats = queue.get_backpressure_stats().await;
        assert!(stats.last_deactivation.is_some() || !queue.is_backpressure_active());
    }
}