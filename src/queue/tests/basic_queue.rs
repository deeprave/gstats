//! Basic queue functionality tests (TDD RED phase)

use crate::queue::{MemoryQueue, Queue, QueueError};
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
use crate::scanner::modes::ScanMode;

#[test]
fn test_memory_queue_creation() {
    // This test should fail initially (RED phase)
    // Testing that we can create a MemoryQueue with basic configuration
    let queue = MemoryQueue::new(1000, 64 * 1024 * 1024); // 1000 messages, 64MB limit
    assert_eq!(queue.capacity(), 1000);
    assert_eq!(queue.size(), 0);
    assert!(queue.is_empty());
}

#[test]
fn test_basic_enqueue_dequeue() {
    // This test should fail initially (RED phase)
    // Testing basic enqueue/dequeue operations
    let queue = MemoryQueue::new(10, 1024 * 1024);
    
    let message = create_test_scan_message();
    
    // Test enqueue
    assert!(queue.enqueue(message.clone()).is_ok());
    assert_eq!(queue.size(), 1);
    assert!(!queue.is_empty());
    
    // Test dequeue
    let dequeued = queue.dequeue().unwrap();
    assert!(dequeued.is_some());
    assert_eq!(queue.size(), 0);
    assert!(queue.is_empty());
}

#[test]
fn test_queue_capacity_limits() {
    // This test should fail initially (RED phase)
    // Testing that queue respects capacity limits
    let queue = MemoryQueue::new(2, 1024 * 1024); // Small capacity for testing
    
    let message = create_test_scan_message();
    
    // Fill to capacity
    assert!(queue.enqueue(message.clone()).is_ok());
    assert!(queue.enqueue(message.clone()).is_ok());
    
    // Should fail when over capacity
    assert!(matches!(queue.enqueue(message), Err(QueueError::QueueFull)));
}

#[test]
fn test_empty_queue_dequeue() {
    // This test should fail initially (RED phase)
    // Testing dequeue from empty queue
    let queue = MemoryQueue::new(10, 1024 * 1024);
    
    let result = queue.dequeue().unwrap();
    assert!(result.is_none()); // Empty queue should return None, not error
}

#[test]
fn test_queue_fifo_ordering() {
    // This test should fail initially (RED phase)
    // Testing that queue maintains FIFO ordering
    let queue = MemoryQueue::new(10, 1024 * 1024);
    
    let message1 = create_test_scan_message_with_data("first");
    let message2 = create_test_scan_message_with_data("second");
    let message3 = create_test_scan_message_with_data("third");
    
    // Enqueue in order
    queue.enqueue(message1).unwrap();
    queue.enqueue(message2).unwrap();
    queue.enqueue(message3).unwrap();
    
    // Dequeue should maintain order
    let first = queue.dequeue().unwrap().unwrap();
    let second = queue.dequeue().unwrap().unwrap();
    let third = queue.dequeue().unwrap().unwrap();
    
    // Verify ordering
    assert!(matches!(first.data, MessageData::FileInfo { ref path, .. } if path == "first"));
    assert!(matches!(second.data, MessageData::FileInfo { ref path, .. } if path == "second"));
    assert!(matches!(third.data, MessageData::FileInfo { ref path, .. } if path == "third"));
}

// Helper functions for test message creation
fn create_test_scan_message() -> ScanMessage {
    ScanMessage::new(
        MessageHeader::new(ScanMode::FILES, 12345),
        MessageData::FileInfo {
            path: "test.rs".to_string(),
            size: 1024,
            lines: 50,
        }
    )
}

fn create_test_scan_message_with_data(path: &str) -> ScanMessage {
    ScanMessage::new(
        MessageHeader::new(ScanMode::FILES, 12345),
        MessageData::FileInfo {
            path: path.to_string(),
            size: 1024,
            lines: 50,
        }
    )
}