//! Shared Message Queue Implementation
//!
//! Core queue implementation providing producer interface for scanner-to-plugin
//! message coordination. The queue is designed to be thread-safe and async-first
//! with proper event notification and memory monitoring.

use crate::queue::error::{QueueError, QueueResult};
use crate::queue::notifications::{QueueEvent, QueueEventNotifier};
use crate::queue::memory::MemoryMonitor;
use crate::scanner::messages::ScanMessage;
use crate::scanner::modes::ScanMode;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// State of the scanning process
#[derive(Debug, Clone, PartialEq)]
enum ScanState {
    /// Scan has not been started yet
    NotStarted,
    /// Scan is currently in progress
    InProgress { modes: ScanMode },
    /// Scan has been completed
    Completed { total_messages: u64 },
}

/// Shared message queue for coordinating between scanners and plugins
#[derive(Debug)]
pub struct SharedMessageQueue {
    /// Unique identifier for this scanning session
    scan_id: String,
    /// The actual message queue
    queue: Arc<RwLock<VecDeque<ScanMessage>>>,
    /// Current scan state
    scan_state: Arc<RwLock<ScanState>>,
    /// Event notification system
    event_notifier: QueueEventNotifier,
    /// Memory usage monitoring
    memory_monitor: MemoryMonitor,
    /// Maximum queue capacity (0 = unlimited)
    max_capacity: usize,
}

impl SharedMessageQueue {
    /// Create a new shared message queue for the given scan session
    pub fn new(scan_id: String) -> Self {
        Self::with_capacity(scan_id, 0) // Unlimited capacity by default
    }

    /// Create a new shared message queue with specified capacity
    pub fn with_capacity(scan_id: String, max_capacity: usize) -> Self {
        Self {
            scan_id,
            queue: Arc::new(RwLock::new(VecDeque::new())),
            scan_state: Arc::new(RwLock::new(ScanState::NotStarted)),
            event_notifier: QueueEventNotifier::with_default_capacity(),
            memory_monitor: MemoryMonitor::new(),
            max_capacity,
        }
    }

    /// Create a new shared message queue with custom memory threshold
    pub fn with_memory_threshold(scan_id: String, memory_threshold: usize) -> Self {
        Self {
            scan_id,
            queue: Arc::new(RwLock::new(VecDeque::new())),
            scan_state: Arc::new(RwLock::new(ScanState::NotStarted)),
            event_notifier: QueueEventNotifier::with_default_capacity(),
            memory_monitor: MemoryMonitor::with_threshold(memory_threshold),
            max_capacity: 0,
        }
    }

    /// Get the scan ID for this queue
    pub fn scan_id(&self) -> &str {
        &self.scan_id
    }

    /// Subscribe to queue events
    pub fn subscribe_events(&self) -> broadcast::Receiver<QueueEvent> {
        self.event_notifier.subscribe()
    }

    // Producer Interface

    /// Start a new scan with the specified modes
    pub async fn start_scan(&self, modes: ScanMode) -> QueueResult<()> {
        let mut state = self.scan_state.write().await;
        
        match *state {
            ScanState::NotStarted => {
                *state = ScanState::InProgress { modes };
                drop(state); // Release lock before emitting event

                let event = QueueEvent::scan_started(self.scan_id.clone(), modes);
                self.event_notifier.emit(event)?;
                
                log::info!("Started scan '{}' with modes: {:?}", self.scan_id, modes);
                Ok(())
            }
            ScanState::InProgress { .. } => {
                Err(QueueError::ScanAlreadyStarted {
                    scan_id: self.scan_id.clone(),
                })
            }
            ScanState::Completed { .. } => {
                Err(QueueError::ScanAlreadyCompleted {
                    scan_id: self.scan_id.clone(),
                })
            }
        }
    }

    /// Push a message to the queue
    pub async fn push(&self, message: ScanMessage) -> QueueResult<()> {
        // Check if scan is in progress
        {
            let state = self.scan_state.read().await;
            if matches!(*state, ScanState::NotStarted) {
                return Err(QueueError::ScanNotStarted {
                    scan_id: self.scan_id.clone(),
                });
            }
        }

        // Check capacity if limited
        if self.max_capacity > 0 {
            let queue = self.queue.read().await;
            if queue.len() >= self.max_capacity {
                return Err(QueueError::QueueFull);
            }
        }

        // Record memory usage before adding
        self.memory_monitor.record_push(&message).await;

        // Add message to queue
        let queue_size = {
            let mut queue = self.queue.write().await;
            queue.push_back(message);
            queue.len()
        };

        // Emit event
        let event = QueueEvent::message_added(self.scan_id.clone(), 1, queue_size);
        self.event_notifier.emit(event)?;

        // Check for memory warnings
        if self.memory_monitor.is_memory_concerning().await {
            let stats = self.memory_monitor.get_stats().await;
            let warning_event = QueueEvent::memory_warning(
                self.scan_id.clone(),
                stats.current_size,
                stats.warning_threshold,
            );
            self.event_notifier.emit(warning_event)?;
        }

        log::trace!("Pushed message to queue '{}', size: {}", self.scan_id, queue_size);
        Ok(())
    }

    /// Signal that scanning is complete
    pub async fn complete_scan(&self) -> QueueResult<()> {
        let total_messages = {
            let mut state = self.scan_state.write().await;
            match *state {
                ScanState::InProgress { .. } => {
                    let stats = self.memory_monitor.get_stats().await;
                    let total = stats.total_messages_processed / 2; // Divide by 2 since we count push+pop
                    *state = ScanState::Completed { total_messages: total };
                    total
                }
                ScanState::NotStarted => {
                    return Err(QueueError::ScanNotStarted {
                        scan_id: self.scan_id.clone(),
                    });
                }
                ScanState::Completed { .. } => {
                    return Err(QueueError::ScanAlreadyCompleted {
                        scan_id: self.scan_id.clone(),
                    });
                }
            }
        };

        // Emit completion event
        let event = QueueEvent::scan_complete(self.scan_id.clone(), total_messages);
        self.event_notifier.emit(event)?;

        log::info!("Completed scan '{}', total messages: {}", self.scan_id, total_messages);
        self.memory_monitor.log_status().await;
        Ok(())
    }

    // Consumer Interface (Internal - will be abstracted by consumer API)

    /// Pop a message from the queue (returns None if empty)
    pub(crate) async fn pop(&self) -> Option<ScanMessage> {
        let message = {
            let mut queue = self.queue.write().await;
            queue.pop_front()
        };

        if let Some(ref msg) = message {
            self.memory_monitor.record_pop(msg).await;
            
            // Check if queue is now empty and scan is complete
            let should_emit_drained = {
                let queue = self.queue.read().await;
                let state = self.scan_state.read().await;
                queue.is_empty() && matches!(*state, ScanState::Completed { .. })
            };

            if should_emit_drained {
                let event = QueueEvent::queue_drained(self.scan_id.clone());
                let _ = self.event_notifier.emit(event); // Don't fail pop on event error
            }

            log::trace!("Popped message from queue '{}'", self.scan_id);
        }

        message
    }

    /// Pop multiple messages from the queue (up to max_count)
    pub(crate) async fn pop_batch(&self, max_count: usize) -> Vec<ScanMessage> {
        let messages = {
            let mut queue = self.queue.write().await;
            let count = max_count.min(queue.len());
            (0..count).filter_map(|_| queue.pop_front()).collect::<Vec<_>>()
        };

        // Record memory usage for all popped messages
        for message in &messages {
            self.memory_monitor.record_pop(message).await;
        }

        if !messages.is_empty() {
            // Check if queue is now empty and scan is complete
            let should_emit_drained = {
                let queue = self.queue.read().await;
                let state = self.scan_state.read().await;
                queue.is_empty() && matches!(*state, ScanState::Completed { .. })
            };

            if should_emit_drained {
                let event = QueueEvent::queue_drained(self.scan_id.clone());
                let _ = self.event_notifier.emit(event); // Don't fail pop on event error
            }

            log::trace!("Popped {} messages from queue '{}'", messages.len(), self.scan_id);
        }

        messages
    }

    // Status and Monitoring

    /// Get the current queue size
    pub async fn get_queue_size(&self) -> usize {
        self.queue.read().await.len()
    }

    /// Check if the queue is empty
    pub async fn is_empty(&self) -> bool {
        self.queue.read().await.is_empty()
    }

    /// Check if the scan is complete
    pub async fn is_scan_complete(&self) -> bool {
        let state = self.scan_state.read().await;
        matches!(*state, ScanState::Completed { .. })
    }

    /// Check if the scan is in progress
    pub async fn is_scan_in_progress(&self) -> bool {
        let state = self.scan_state.read().await;
        matches!(*state, ScanState::InProgress { .. })
    }

    /// Get current memory statistics
    pub async fn get_memory_stats(&self) -> crate::queue::memory::QueueMemoryStats {
        self.memory_monitor.get_stats().await
    }

    /// Get the number of event subscribers
    pub fn get_subscriber_count(&self) -> usize {
        self.event_notifier.subscriber_count()
    }

    /// Wait for the queue to be drained (empty and scan complete)
    pub async fn wait_for_drain(&self) -> QueueResult<()> {
        let mut event_receiver = self.subscribe_events();
        
        // Check current state first
        if self.is_empty().await && self.is_scan_complete().await {
            return Ok(());
        }

        // Wait for drain event
        while let Ok(event) = event_receiver.recv().await {
            if matches!(event, QueueEvent::QueueDrained { .. }) {
                return Ok(());
            }
        }

        Err(QueueError::operation_failed("Event channel closed while waiting for drain"))
    }
}

impl Clone for SharedMessageQueue {
    fn clone(&self) -> Self {
        Self {
            scan_id: self.scan_id.clone(),
            queue: Arc::clone(&self.queue),
            scan_state: Arc::clone(&self.scan_state),
            event_notifier: self.event_notifier.clone(),
            memory_monitor: self.memory_monitor.clone(),
            max_capacity: self.max_capacity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};
    use tokio::time::{timeout, Duration};

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
    async fn test_queue_creation() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        assert_eq!(queue.scan_id(), "test-scan");
        assert!(queue.is_empty().await);
        assert!(!queue.is_scan_complete().await);
        assert!(!queue.is_scan_in_progress().await);
    }

    #[tokio::test]
    async fn test_scan_lifecycle() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        
        // Start scan
        queue.start_scan(ScanMode::FILES).await.unwrap();
        assert!(queue.is_scan_in_progress().await);
        assert!(!queue.is_scan_complete().await);

        // Complete scan
        queue.complete_scan().await.unwrap();
        assert!(!queue.is_scan_in_progress().await);
        assert!(queue.is_scan_complete().await);
    }

    #[tokio::test]
    async fn test_scan_state_errors() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        
        // Cannot push before starting scan
        let message = create_test_message();
        assert!(queue.push(message).await.is_err());

        // Cannot start scan twice
        queue.start_scan(ScanMode::FILES).await.unwrap();
        assert!(queue.start_scan(ScanMode::HISTORY).await.is_err());

        // Cannot complete scan twice
        queue.complete_scan().await.unwrap();
        assert!(queue.complete_scan().await.is_err());
    }

    #[tokio::test]
    async fn test_message_push_pop() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        queue.start_scan(ScanMode::FILES).await.unwrap();

        let message = create_test_message();
        queue.push(message.clone()).await.unwrap();
        
        assert_eq!(queue.get_queue_size().await, 1);
        assert!(!queue.is_empty().await);

        let popped = queue.pop().await.unwrap();
        assert_eq!(popped.data(), message.data());
        assert!(queue.is_empty().await);
    }

    #[tokio::test]
    async fn test_batch_pop() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        queue.start_scan(ScanMode::FILES).await.unwrap();

        // Push multiple messages
        for _ in 0..5 {
            queue.push(create_test_message()).await.unwrap();
        }

        assert_eq!(queue.get_queue_size().await, 5);

        // Pop batch
        let messages = queue.pop_batch(3).await;
        assert_eq!(messages.len(), 3);
        assert_eq!(queue.get_queue_size().await, 2);

        // Pop remaining
        let messages = queue.pop_batch(10).await; // Request more than available
        assert_eq!(messages.len(), 2);
        assert!(queue.is_empty().await);
    }

    #[tokio::test]
    async fn test_event_notifications() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        let mut event_receiver = queue.subscribe_events();

        // Start scan - should emit ScanStarted
        queue.start_scan(ScanMode::FILES).await.unwrap();
        let event = timeout(Duration::from_millis(100), event_receiver.recv()).await.unwrap().unwrap();
        assert!(matches!(event, QueueEvent::ScanStarted { .. }));

        // Push message - should emit MessageAdded
        queue.push(create_test_message()).await.unwrap();
        let event = timeout(Duration::from_millis(100), event_receiver.recv()).await.unwrap().unwrap();
        assert!(matches!(event, QueueEvent::MessageAdded { .. }));

        // Complete scan - should emit ScanComplete
        queue.complete_scan().await.unwrap();
        let event = timeout(Duration::from_millis(100), event_receiver.recv()).await.unwrap().unwrap();
        assert!(matches!(event, QueueEvent::ScanComplete { .. }));
    }

    #[tokio::test]
    async fn test_capacity_limit() {
        let queue = SharedMessageQueue::with_capacity("test-scan".to_string(), 2);
        queue.start_scan(ScanMode::FILES).await.unwrap();

        // Should be able to add up to capacity
        queue.push(create_test_message()).await.unwrap();
        queue.push(create_test_message()).await.unwrap();

        // Should fail when exceeding capacity
        assert!(queue.push(create_test_message()).await.is_err());
    }

    #[tokio::test]
    async fn test_wait_for_drain() {
        let queue = SharedMessageQueue::new("test-scan".to_string());
        queue.start_scan(ScanMode::FILES).await.unwrap();
        queue.push(create_test_message()).await.unwrap();
        queue.complete_scan().await.unwrap();

        // Should not be drained yet (message still in queue)
        assert!(!queue.is_empty().await);

        // Pop the message in a separate task
        let queue_clone = queue.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            queue_clone.pop().await;
        });

        // Should complete when drained
        timeout(Duration::from_millis(200), queue.wait_for_drain()).await.unwrap().unwrap();
    }
}
