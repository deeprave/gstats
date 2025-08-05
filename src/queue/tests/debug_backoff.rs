//! Debug tests for understanding backoff behavior

use crate::queue::{MemoryQueue, Queue};
use crate::queue::memory_tracker::MemoryTracker;
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
use crate::scanner::modes::ScanMode;
use std::sync::Arc;

#[test]
fn debug_message_sizes() {
    let tracker = Arc::new(MemoryTracker::new(1024));
    let queue = MemoryQueue::with_shared_tracker(100, 1024, tracker.clone());
    
    println!("=== Testing message sizes in fresh queue ===");
    println!("Memory limit: 1024 bytes");
    println!("Starting memory usage: {}", queue.memory_usage());
    
    // Test what the successful test does
    let medium_msg = create_large_test_message(800);
    match queue.enqueue(medium_msg) {
        Ok(()) => {
            println!("✓ Medium message (800) enqueued successfully");
            println!("  Memory usage after medium: {}", queue.memory_usage());
        }
        Err(e) => println!("✗ Medium message (800) failed: {:?}", e),
    }
    
    // Now try what the failing tests do - enqueue another message
    let small_msg = create_test_message("test");
    match queue.enqueue(small_msg) {
        Ok(()) => {
            println!("✓ Small follow-up message enqueued successfully");
            println!("  Memory usage after small: {}", queue.memory_usage());
        }
        Err(e) => println!("✗ Small follow-up message failed: {:?}", e),
    }
    
    // Show tracker status
    println!("Final tracker memory usage: {}", tracker.allocated_bytes());
    println!("Final tracker usage percent: {:.2}%", tracker.usage_percent());
    println!("Final tracker pressure level: {:?}", tracker.get_pressure_level());
}

#[test]
fn debug_backoff_failing_test_scenario() {
    println!("=== Replicating failing test scenario ===");
    let tracker = Arc::new(MemoryTracker::new(1024));
    let queue = MemoryQueue::with_shared_tracker(100, 1024, tracker.clone());
    
    // This is what the failing test does
    let large_message = create_large_test_message(900);
    println!("Trying to enqueue 900-byte message...");
    match queue.enqueue(large_message) {
        Ok(()) => {
            println!("✓ Large message (900) enqueued successfully");
            println!("  Memory usage: {}", queue.memory_usage());
            
            // Enable backoff
            queue.enable_backoff_algorithm();
            
            // Try second message
            println!("Trying second message with backoff...");
            let second_msg = create_test_message("test");
            match queue.enqueue_with_backoff(second_msg) {
                Ok(()) => println!("✓ Second message with backoff succeeded"),
                Err(e) => println!("✗ Second message with backoff failed: {:?}", e),
            }
        }
        Err(e) => println!("✗ Large message (900) failed: {:?}", e),
    }
}

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

fn create_large_test_message(target_size: usize) -> ScanMessage {
    let large_path = "x".repeat(target_size);
    ScanMessage::new(
        MessageHeader::new(ScanMode::FILES, 12345),
        MessageData::FileInfo {
            path: large_path,
            size: target_size as u64,
            lines: 100,
        }
    )
}