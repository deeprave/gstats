//! Versioned message integration tests

use crate::queue::{MemoryQueue, QueueError};
use crate::queue::versioned_message::{QueueMessage, MessageType, MessagePayload};
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
use crate::scanner::modes::ScanMode;

#[test]
fn test_versioned_queue_creation() {
    // This test should fail initially (RED phase)
    // Test that we can create a VersionedMemoryQueue
    let queue = MemoryQueue::new_versioned(10, 1024); // 10 messages, 1KB limit
    assert_eq!(queue.capacity(), 10);
    assert_eq!(queue.size(), 0);
    assert!(queue.is_empty());
}

#[test]
fn test_versioned_message_enqueue_dequeue() {
    // This test should fail initially (RED phase)
    // Test enqueue/dequeue with versioned messages
    let queue = MemoryQueue::new_versioned(10, 1024);
    
    let scan_message = create_test_scan_message();
    let versioned_message = QueueMessage::from_scan_message(scan_message);
    
    // Test enqueue
    assert!(queue.enqueue_versioned(versioned_message.clone()).is_ok());
    assert_eq!(queue.size(), 1);
    assert!(!queue.is_empty());
    
    // Test dequeue
    let dequeued = queue.dequeue_versioned().unwrap();
    assert!(dequeued.is_some());
    assert_eq!(queue.size(), 0);
    assert!(queue.is_empty());
    
    // Verify the dequeued message
    let dequeued_msg = dequeued.unwrap();
    assert_eq!(dequeued_msg.version, versioned_message.version);
    assert!(matches!(dequeued_msg.message_type, MessageType::ScanMessage));
    assert!(dequeued_msg.try_extract_scan_message().is_ok());
}

#[test]
fn test_version_compatibility_checking() {
    // This test should fail initially (RED phase)
    // Test that queue checks version compatibility
    let queue = MemoryQueue::new_versioned(10, 1024);
    
    let mut message = QueueMessage::from_scan_message(create_test_scan_message());
    
    // Compatible version should work
    assert!(message.is_version_compatible());
    assert!(queue.enqueue_versioned(message.clone()).is_ok());
    
    // Incompatible version should be rejected
    message.version = 2_00_00; // Future major version
    assert!(!message.is_version_compatible());
    
    // Queue should handle incompatible versions gracefully
    let result = queue.enqueue_versioned(message);
    assert!(matches!(result, Err(QueueError::VersioningError(_))));
}

#[test]
fn test_message_type_routing() {
    // This test should fail initially (RED phase)
    // Test that different message types can be handled
    let queue = MemoryQueue::new_versioned(10, 1024);
    
    let scan_message = QueueMessage::from_scan_message(create_test_scan_message());
    
    // Create a metrics message (future message type)
    let metrics_message = QueueMessage {
        version: 1_00_00,
        message_type: MessageType::MetricsMessage,
        enqueue_timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        payload: MessagePayload::Raw(b"metrics data".to_vec()),
    };
    
    // Both should be enqueueable
    assert!(queue.enqueue_versioned(scan_message).is_ok());
    assert!(queue.enqueue_versioned(metrics_message).is_ok());
    assert_eq!(queue.size(), 2);
    
    // Both should be dequeueable in FIFO order
    let first = queue.dequeue_versioned().unwrap().unwrap();
    let second = queue.dequeue_versioned().unwrap().unwrap();
    
    assert!(matches!(first.message_type, MessageType::ScanMessage));
    assert!(matches!(second.message_type, MessageType::MetricsMessage));
}

#[test]
fn test_message_serialization_in_queue() {
    // This test should fail initially (RED phase)
    // Test that messages can be serialized/deserialized through queue
    let queue = MemoryQueue::new_versioned(10, 1024);
    
    let original_scan = create_test_scan_message();
    let original_message = QueueMessage::from_scan_message(original_scan.clone());
    
    // Enqueue and dequeue
    queue.enqueue_versioned(original_message).unwrap();
    let dequeued = queue.dequeue_versioned().unwrap().unwrap();
    
    // Verify data integrity
    let extracted_scan = dequeued.try_extract_scan_message().unwrap();
    assert_eq!(extracted_scan.header.scan_mode, original_scan.header.scan_mode);
    assert_eq!(extracted_scan.header.timestamp, original_scan.header.timestamp);
    
    // Verify message data
    match (&extracted_scan.data, &original_scan.data) {
        (MessageData::FileInfo { path: p1, size: s1, lines: l1 }, 
         MessageData::FileInfo { path: p2, size: s2, lines: l2 }) => {
            assert_eq!(p1, p2);
            assert_eq!(s1, s2);
            assert_eq!(l1, l2);
        }
        _ => panic!("Message data mismatch"),
    }
}

#[test]
fn test_backward_compatibility() {
    // This test should fail initially (RED phase)
    // Test that older message versions are handled gracefully
    let queue = MemoryQueue::new_versioned(10, 1024);
    
    let mut old_message = QueueMessage::from_scan_message(create_test_scan_message());
    old_message.version = 1_00_00; // Same major version, but could be older minor version
    
    // Should be compatible
    assert!(old_message.is_version_compatible());
    assert!(queue.enqueue_versioned(old_message).is_ok());
    
    let dequeued = queue.dequeue_versioned().unwrap().unwrap();
    assert_eq!(dequeued.version, 1_00_00);
    assert!(dequeued.try_extract_scan_message().is_ok());
}

#[test]
fn test_forward_compatibility() {
    // This test should fail initially (RED phase)
    // Test that unknown message types are preserved
    let queue = MemoryQueue::new_versioned(10, 1024);
    
    let unknown_message = QueueMessage {
        version: 1_00_00,
        message_type: MessageType::Unknown("FutureMessageType".to_string()),
        enqueue_timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        payload: MessagePayload::Raw(b"unknown future data".to_vec()),
    };
    
    // Should be able to enqueue and dequeue unknown types
    assert!(queue.enqueue_versioned(unknown_message.clone()).is_ok());
    let dequeued = queue.dequeue_versioned().unwrap().unwrap();
    
    assert!(matches!(dequeued.message_type, MessageType::Unknown(_)));
    assert!(matches!(dequeued.payload, MessagePayload::Raw(_)));
    
    // Data should be preserved exactly
    if let MessagePayload::Raw(data) = &dequeued.payload {
        assert_eq!(data, b"unknown future data");
    } else {
        panic!("Expected raw payload");
    }
}

#[test]
fn test_versioned_memory_tracking() {
    // This test should fail initially (RED phase)
    // Test that versioned messages work with memory tracking
    let queue = MemoryQueue::new_versioned_with_memory_tracking(10, 1024);
    
    let message = QueueMessage::from_scan_message(create_test_scan_message());
    
    let initial_usage = queue.memory_usage();
    assert_eq!(initial_usage, 0);
    
    // Enqueue should increase memory usage
    queue.enqueue_versioned(message).unwrap();
    let after_enqueue = queue.memory_usage();
    assert!(after_enqueue > initial_usage);
    
    // Dequeue should decrease memory usage
    let dequeued = queue.dequeue_versioned().unwrap();
    assert!(dequeued.is_some());
    let after_dequeue = queue.memory_usage();
    assert!(after_dequeue <= initial_usage);
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