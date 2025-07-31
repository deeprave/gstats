//! Memory-Conscious Queue Implementation
//! 
//! MPSC queue with memory monitoring and backoff capabilities

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use crossbeam_queue::SegQueue;
use crate::queue::{Queue, QueueError};
use crate::queue::memory_tracker::MemoryTracker;
use crate::queue::versioned_message::QueueMessage;
use crate::scanner::messages::ScanMessage;

/// Memory-monitored MPSC queue for ScanMessages
pub struct MemoryQueue {
    inner: Arc<SegQueue<ScanMessage>>,
    capacity: usize,
    memory_limit: usize,
    current_size: Arc<AtomicUsize>,
    memory_tracker: Option<Arc<MemoryTracker>>,
}

/// Memory-monitored MPSC queue for versioned messages
pub struct VersionedMemoryQueue {
    inner: Arc<SegQueue<QueueMessage>>,
    capacity: usize,
    memory_limit: usize,
    current_size: Arc<AtomicUsize>,
    memory_tracker: Option<Arc<MemoryTracker>>,
}

impl MemoryQueue {
    /// Create a new memory queue with specified capacity and memory limit
    pub fn new(capacity: usize, memory_limit: usize) -> Self {
        Self {
            inner: Arc::new(SegQueue::new()),
            capacity,
            memory_limit,
            current_size: Arc::new(AtomicUsize::new(0)),
            memory_tracker: None,
        }
    }
    
    /// Create a new memory queue with memory tracking enabled
    pub fn with_memory_tracking(capacity: usize, memory_limit: usize) -> Self {
        Self {
            inner: Arc::new(SegQueue::new()),
            capacity,
            memory_limit,
            current_size: Arc::new(AtomicUsize::new(0)),
            memory_tracker: Some(Arc::new(MemoryTracker::new(memory_limit))),
        }
    }
    
    /// Get current memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        match &self.memory_tracker {
            Some(tracker) => tracker.allocated_bytes(),
            None => 0,
        }
    }
    
    /// Get memory usage as percentage of limit
    pub fn memory_usage_percent(&self) -> f64 {
        match &self.memory_tracker {
            Some(tracker) => tracker.usage_percent(),
            None => 0.0,
        }
    }
    
    /// Check if memory usage exceeds given threshold percentage
    pub fn exceeds_memory_threshold(&self, threshold_percent: f64) -> bool {
        match &self.memory_tracker {
            Some(tracker) => tracker.exceeds_threshold(threshold_percent),
            None => false,
        }
    }
    
    /// Create a new versioned memory queue
    pub fn new_versioned(capacity: usize, memory_limit: usize) -> VersionedMemoryQueue {
        VersionedMemoryQueue {
            inner: Arc::new(SegQueue::new()),
            capacity,
            memory_limit,
            current_size: Arc::new(AtomicUsize::new(0)),
            memory_tracker: None,
        }
    }
    
    /// Create a new versioned memory queue with memory tracking
    pub fn new_versioned_with_memory_tracking(capacity: usize, memory_limit: usize) -> VersionedMemoryQueue {
        VersionedMemoryQueue {
            inner: Arc::new(SegQueue::new()),
            capacity,
            memory_limit,
            current_size: Arc::new(AtomicUsize::new(0)),
            memory_tracker: Some(Arc::new(MemoryTracker::new(memory_limit))),
        }
    }

    /// Estimate memory size of a message
    fn estimate_message_size(message: &ScanMessage) -> usize {
        // Simple estimation - in practice this could be more sophisticated
        std::mem::size_of::<ScanMessage>() + 
        bincode::serialized_size(message).unwrap_or(256) as usize
    }
}

impl Queue<ScanMessage> for MemoryQueue {
    fn enqueue(&self, message: ScanMessage) -> Result<(), QueueError> {
        // Check capacity limit
        let current_size = self.current_size.load(Ordering::Relaxed);
        if current_size >= self.capacity {
            return Err(QueueError::QueueFull);
        }
        
        // Check memory limit if tracking is enabled
        if let Some(tracker) = &self.memory_tracker {
            let message_size = Self::estimate_message_size(&message);
            if !tracker.allocate(message_size) {
                return Err(QueueError::MemoryLimitExceeded);
            }
        }
        
        // Enqueue the message
        self.inner.push(message);
        self.current_size.fetch_add(1, Ordering::Relaxed);
        
        Ok(())
    }
    
    fn dequeue(&self) -> Result<Option<ScanMessage>, QueueError> {
        match self.inner.pop() {
            Some(message) => {
                self.current_size.fetch_sub(1, Ordering::Relaxed);
                
                // Update memory tracking if enabled
                if let Some(tracker) = &self.memory_tracker {
                    let message_size = Self::estimate_message_size(&message);
                    tracker.deallocate(message_size);
                }
                
                Ok(Some(message))
            }
            None => Ok(None), // Empty queue, not an error
        }
    }
    
    fn size(&self) -> usize {
        self.current_size.load(Ordering::Relaxed)
    }
    
    fn is_empty(&self) -> bool {
        self.size() == 0
    }
    
    fn capacity(&self) -> usize {
        self.capacity
    }
}

impl VersionedMemoryQueue {
    /// Get current memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        match &self.memory_tracker {
            Some(tracker) => tracker.allocated_bytes(),
            None => 0,
        }
    }
    
    /// Get memory usage as percentage of limit
    pub fn memory_usage_percent(&self) -> f64 {
        match &self.memory_tracker {
            Some(tracker) => tracker.usage_percent(),
            None => 0.0,
        }
    }
    
    /// Check if memory usage exceeds given threshold percentage
    pub fn exceeds_memory_threshold(&self, threshold_percent: f64) -> bool {
        match &self.memory_tracker {
            Some(tracker) => tracker.exceeds_threshold(threshold_percent),
            None => false,
        }
    }
    
    /// Get current queue size
    pub fn size(&self) -> usize {
        self.current_size.load(Ordering::Relaxed)
    }
    
    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.size() == 0
    }
    
    /// Get queue capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    
    /// Enqueue a versioned message
    pub fn enqueue_versioned(&self, message: QueueMessage) -> Result<(), QueueError> {
        // Check version compatibility
        if !message.is_version_compatible() {
            return Err(QueueError::VersioningError(format!(
                "Incompatible message version: {}", message.version
            )));
        }
        
        // Check capacity limit
        let current_size = self.current_size.load(Ordering::Relaxed);
        if current_size >= self.capacity {
            return Err(QueueError::QueueFull);
        }
        
        // Check memory limit if tracking is enabled
        if let Some(tracker) = &self.memory_tracker {
            let message_size = Self::estimate_versioned_message_size(&message);
            if !tracker.allocate(message_size) {
                return Err(QueueError::MemoryLimitExceeded);
            }
        }
        
        // Enqueue the message
        self.inner.push(message);
        self.current_size.fetch_add(1, Ordering::Relaxed);
        
        Ok(())
    }
    
    /// Dequeue a versioned message
    pub fn dequeue_versioned(&self) -> Result<Option<QueueMessage>, QueueError> {
        match self.inner.pop() {
            Some(message) => {
                self.current_size.fetch_sub(1, Ordering::Relaxed);
                
                // Update memory tracking if enabled
                if let Some(tracker) = &self.memory_tracker {
                    let message_size = Self::estimate_versioned_message_size(&message);
                    tracker.deallocate(message_size);
                }
                
                Ok(Some(message))
            }
            None => Ok(None), // Empty queue, not an error
        }
    }
    
    /// Estimate memory size of a versioned message
    fn estimate_versioned_message_size(message: &QueueMessage) -> usize {
        // Simple estimation - in practice this could be more sophisticated
        std::mem::size_of::<QueueMessage>() + 
        bincode::serialized_size(message).unwrap_or(512) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};
    use crate::scanner::modes::ScanMode;

    #[test]
    fn test_memory_queue_creation_basic() {
        let queue = MemoryQueue::new(100, 1024 * 1024);
        assert_eq!(queue.capacity(), 100);
        // Other assertions will fail until we implement the methods
    }

    fn create_test_message() -> ScanMessage {
        ScanMessage::new(
            MessageHeader::new(ScanMode::FILES, 12345),
            MessageData::FileInfo {
                path: "test.rs".to_string(),
                size: 1024,
                lines: 50,
            }
        )
    }
}