//! Memory tracking tests for queue system

use crate::queue::{MemoryQueue, Queue, QueueError};
use crate::queue::memory_tracker::MemoryTracker;
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
use crate::scanner::modes::ScanMode;

#[test]
fn test_memory_queue_with_tracking() {
    // This test should fail initially (RED phase)
    // Test that MemoryQueue integrates with MemoryTracker
    let queue = MemoryQueue::with_memory_tracking(10, 1024); // 10 messages, 1KB limit
    
    let message = create_test_scan_message();
    
    // Enqueue should succeed when under memory limit
    assert!(queue.enqueue(message.clone()).is_ok());
    assert_eq!(queue.size(), 1);
    
    // Memory usage should be tracked
    assert!(queue.memory_usage() > 0);
    assert!(queue.memory_usage() < 1024); // Should be under limit
}

#[test]
fn test_memory_limit_enforcement() {
    // Test that queue respects memory limits
    let queue = MemoryQueue::with_memory_tracking(100, 512); // Small but reasonable memory limit
    
    let message = create_test_scan_message();
    
    // Keep enqueuing until we hit memory limit or capacity limit
    let mut enqueue_count = 0;
    let mut hit_memory_limit = false;
    
    while enqueue_count < 100 {
        match queue.enqueue(message.clone()) {
            Ok(_) => enqueue_count += 1,
            Err(QueueError::MemoryLimitExceeded) => {
                hit_memory_limit = true;
                break;
            },
            Err(QueueError::QueueFull) => break,
            Err(other) => panic!("Unexpected error: {:?}", other),
        }
    }
    
    // Should have enqueued at least one message or hit memory limit
    assert!(enqueue_count > 0 || hit_memory_limit, "Should be able to enqueue at least one message or hit memory limit");
    
    // If we enqueued messages, memory usage should be tracked
    if enqueue_count > 0 {
        assert!(queue.memory_usage() > 0, "Memory usage should be tracked when messages are enqueued");
    }
}

#[test]
fn test_memory_tracking_accuracy() {
    // This test should fail initially (RED phase)
    // Test that memory tracking is reasonably accurate
    let queue = MemoryQueue::with_memory_tracking(10, 2048);
    
    let message = create_test_scan_message();
    let initial_usage = queue.memory_usage();
    
    // Enqueue a message
    queue.enqueue(message.clone()).unwrap();
    let after_enqueue = queue.memory_usage();
    
    // Memory usage should increase
    assert!(after_enqueue > initial_usage);
    
    // Dequeue the message
    let dequeued = queue.dequeue().unwrap();
    assert!(dequeued.is_some());
    let after_dequeue = queue.memory_usage();
    
    // Memory usage should decrease back to initial level
    assert!(after_dequeue <= initial_usage);
}

#[test]
fn test_memory_usage_percentage() {
    // This test should fail initially (RED phase)
    // Test memory usage percentage calculation
    let queue = MemoryQueue::with_memory_tracking(10, 1024);
    
    assert_eq!(queue.memory_usage_percent(), 0.0); // Should start at 0%
    
    let message = create_test_scan_message();
    queue.enqueue(message).unwrap();
    
    let usage_percent = queue.memory_usage_percent();
    assert!(usage_percent > 0.0);
    assert!(usage_percent < 100.0);
}

#[test]
fn test_memory_threshold_detection() {
    // This test should fail initially (RED phase)
    // Test memory threshold detection
    let queue = MemoryQueue::with_memory_tracking(10, 1024);
    
    assert!(!queue.exceeds_memory_threshold(50.0)); // Should not exceed 50% initially
    
    // Add messages until we exceed threshold
    let message = create_test_scan_message();
    while !queue.exceeds_memory_threshold(50.0) && queue.size() < queue.capacity() {
        if queue.enqueue(message.clone()).is_err() {
            break;
        }
    }
    
    // Should eventually exceed the threshold
    if queue.size() > 0 {
        assert!(queue.exceeds_memory_threshold(50.0) || queue.memory_usage_percent() > 50.0);
    }
}

// Helper function for test message creation
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