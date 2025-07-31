//! Memory monitoring system tests

use crate::queue::{MemoryQueue, Queue, MemoryTracker, MemoryPressureLevel, MemoryStatistics, LeakInformation, MemoryHistorySample};
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
use crate::scanner::modes::ScanMode;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn test_memory_tracker_detailed_reporting() {
    // This test should fail initially (RED phase)
    // Test that MemoryTracker provides detailed memory usage information
    let tracker = MemoryTracker::new(1024 * 1024); // 1MB limit
    
    // Test allocation tracking
    assert!(tracker.allocate(1024)); // 1KB
    assert_eq!(tracker.allocated_bytes(), 1024);
    assert_eq!(tracker.available_bytes(), 1024 * 1024 - 1024);
    assert_eq!(tracker.peak_bytes(), 1024);
    
    // Test more allocations
    assert!(tracker.allocate(2048)); // 2KB more
    assert_eq!(tracker.allocated_bytes(), 3072); // 3KB total
    assert_eq!(tracker.peak_bytes(), 3072);
    
    // Test deallocation
    tracker.deallocate(1024);
    assert_eq!(tracker.allocated_bytes(), 2048);
    assert_eq!(tracker.peak_bytes(), 3072); // Peak remains unchanged
    
    // Test allocation count tracking
    assert_eq!(tracker.allocation_count(), 2);
    assert_eq!(tracker.deallocation_count(), 1);
}

#[test]
fn test_memory_statistics_collection() {
    // This test should fail initially (RED phase)
    // Test that we can collect comprehensive memory statistics
    let tracker = MemoryTracker::new(10 * 1024 * 1024); // 10MB limit
    
    // Perform various operations
    tracker.allocate(1024);
    tracker.allocate(2048);
    tracker.deallocate(1024);
    tracker.allocate(4096);
    
    // Get statistics snapshot
    let stats = tracker.get_statistics();
    
    assert_eq!(stats.current_bytes, 6144); // 2048 + 4096
    assert_eq!(stats.peak_bytes, 6144); // Peak was 6144 after final allocation
    assert_eq!(stats.total_allocations, 3);
    assert_eq!(stats.total_deallocations, 1);
    assert_eq!(stats.limit_bytes, 10 * 1024 * 1024);
    assert!(stats.average_allocation_size > 0.0);
    assert!(stats.fragmentation_ratio >= 0.0);
}

#[test]
fn test_memory_pressure_levels() {
    // This test should fail initially (RED phase)
    // Test memory pressure level detection
    let tracker = MemoryTracker::new(1024 * 1024); // 1MB limit
    
    // Test pressure levels
    assert_eq!(tracker.get_pressure_level(), MemoryPressureLevel::Normal);
    
    // Allocate 50% - should be Normal
    tracker.allocate(512 * 1024);
    assert_eq!(tracker.get_pressure_level(), MemoryPressureLevel::Normal);
    
    // Allocate to 75% - should be Moderate
    tracker.allocate(256 * 1024);
    assert_eq!(tracker.get_pressure_level(), MemoryPressureLevel::Moderate);
    
    // Allocate to 90% - should be High
    tracker.allocate(154 * 1024);
    assert_eq!(tracker.get_pressure_level(), MemoryPressureLevel::High);
    
    // Allocate to 95%+ - should be Critical
    tracker.allocate(51 * 1024);
    assert_eq!(tracker.get_pressure_level(), MemoryPressureLevel::Critical);
}

#[test]
fn test_memory_history_tracking() {
    // This test should fail initially (RED phase)
    // Test that we can track memory usage history over time
    let tracker = MemoryTracker::new(1024 * 1024);
    
    // Enable history tracking
    tracker.enable_history_tracking(100); // Keep last 100 samples
    
    // Simulate usage over time
    for i in 0..10 {
        tracker.allocate(1024 * (i + 1));
        thread::sleep(Duration::from_millis(10));
    }
    
    let history = tracker.get_usage_history();
    assert!(history.len() >= 10);
    
    // Verify history contains increasing usage
    let mut last_usage = 0;
    for sample in &history {
        assert!(sample.bytes_allocated >= last_usage);
        assert!(sample.timestamp > 0);
        last_usage = sample.bytes_allocated;
    }
    
    // Test history window limit
    for _ in 0..100 {
        tracker.allocate(100);
        thread::sleep(Duration::from_millis(1));
    }
    
    assert!(tracker.get_usage_history().len() <= 100);
}

#[test]
fn test_per_queue_memory_accounting() {
    // This test should fail initially (RED phase)
    // Test that each queue tracks its own memory usage independently
    let global_tracker = Arc::new(MemoryTracker::new(10 * 1024 * 1024)); // 10MB global
    
    let queue1 = MemoryQueue::with_shared_tracker(100, 2 * 1024 * 1024, global_tracker.clone());
    let queue2 = MemoryQueue::with_shared_tracker(100, 2 * 1024 * 1024, global_tracker.clone());
    
    // Queue 1 operations - smaller message
    let msg1 = create_test_message("small");
    queue1.enqueue(msg1.clone()).unwrap();
    
    // Queue 2 operations - larger message  
    let msg2 = create_test_message("much_larger_message_content_here");
    queue2.enqueue(msg2.clone()).unwrap();
    
    // Check individual queue memory usage
    let queue1_usage = queue1.get_memory_statistics();
    let queue2_usage = queue2.get_memory_statistics();
    
    assert!(queue1_usage.allocated_bytes > 0);
    assert!(queue2_usage.allocated_bytes > 0);
    assert_eq!(queue1_usage.message_count, 1);
    assert_eq!(queue2_usage.message_count, 1);
    
    // Check global tracker sees combined usage
    let global_usage = global_tracker.allocated_bytes();
    
    // The global tracker should see the total of both allocations
    let expected_total = queue1_usage.allocated_bytes + queue2_usage.allocated_bytes;
    assert_eq!(global_usage, expected_total);
}

#[test]
fn test_memory_fragmentation_detection() {
    // This test should fail initially (RED phase)
    // Test detection of memory fragmentation patterns
    let tracker = MemoryTracker::new(10 * 1024 * 1024);
    
    // Create fragmentation pattern: allocate and deallocate in a pattern
    let mut allocations = vec![];
    
    // Allocate blocks of varying sizes
    for i in 0..20 {
        let size = 1024 * ((i % 5) + 1); // 1KB to 5KB
        if tracker.allocate(size) {
            allocations.push(size);
        }
    }
    
    // Deallocate every other block to create fragmentation
    for (i, &size) in allocations.iter().enumerate() {
        if i % 2 == 0 {
            tracker.deallocate(size);
        }
    }
    
    // Check fragmentation metrics
    let stats = tracker.get_statistics();
    assert!(stats.fragmentation_ratio > 0.0);
    assert!(stats.fragmentation_ratio <= 1.0);
    
    // Test defragmentation recommendation
    assert!(tracker.should_recommend_defragmentation());
}

#[test]
fn test_memory_leak_detection() {
    // This test should fail initially (RED phase)
    // Test basic memory leak detection
    let tracker = MemoryTracker::new(10 * 1024 * 1024);
    
    // Enable leak detection
    tracker.enable_leak_detection();
    
    // Simulate normal usage pattern
    for _ in 0..100 {
        tracker.allocate(1024);
        tracker.deallocate(1024);
    }
    
    // Check no leaks detected
    assert!(!tracker.has_potential_leak());
    
    // Simulate leak pattern (allocations without deallocations)
    for _ in 0..50 {
        tracker.allocate(1024);
    }
    
    // Check leak detection
    assert!(tracker.has_potential_leak());
    
    let leak_info = tracker.get_leak_information();
    assert!(leak_info.potential_leak_bytes > 0);
    assert!(leak_info.allocation_deallocation_ratio > 1.0);
}

#[test]
fn test_memory_reporting_interface() {
    // This test should fail initially (RED phase)
    // Test comprehensive memory reporting interface
    let queue = MemoryQueue::with_memory_tracking(100, 5 * 1024 * 1024);
    
    // Add some messages
    for i in 0..10 {
        let msg = create_test_message(&format!("message_{}", i));
        queue.enqueue(msg).unwrap();
    }
    
    // Get memory report
    let report = queue.generate_memory_report();
    
    // Verify report contents
    assert!(report.contains("Memory Usage Report"));
    assert!(report.contains("Current Usage:"));
    assert!(report.contains("Peak Usage:"));
    assert!(report.contains("Available:"));
    assert!(report.contains("Usage Percentage:"));
    assert!(report.contains("Message Count:"));
    assert!(report.contains("Average Message Size:"));
    assert!(report.contains("Memory Pressure:"));
    
    // Test detailed report
    let detailed_report = queue.generate_detailed_memory_report();
    assert!(detailed_report.contains("Allocation History"));
    assert!(detailed_report.contains("Fragmentation"));
    assert!(detailed_report.contains("Recommendations"));
}

// Helper function to create test messages
fn create_test_message(content: &str) -> ScanMessage {
    ScanMessage::new(
        MessageHeader::new(ScanMode::FILES, 12345),
        MessageData::FileInfo {
            path: format!("test/{}.rs", content),
            size: 1024,
            lines: 50,
        }
    )
}

// Helper function to estimate message size (mirrors the queue implementation)
fn estimate_message_size(message: &ScanMessage) -> usize {
    std::mem::size_of::<ScanMessage>() + 
    bincode::serialized_size(message).unwrap_or(256) as usize
}