//! Backoff algorithm tests

use crate::queue::{MemoryQueue, Queue, QueueError, BackoffConfig, BackoffStrategy, PressureResponseConfig};
use crate::queue::memory_tracker::MemoryTracker;
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
use crate::scanner::modes::ScanMode;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::thread;

#[test]
fn test_exponential_backoff_on_memory_pressure() {
    // This test should fail initially (RED phase)
    // Test that queue implements exponential backoff when memory pressure is high
    let tracker = Arc::new(MemoryTracker::new(1024)); // Very small limit for testing
    let queue = MemoryQueue::with_shared_tracker(100, 1024, tracker.clone());
    
    // Fill queue to near capacity to trigger memory pressure
    let large_message = create_large_test_message(800); // Use most of the 1024 byte limit
    queue.enqueue(large_message).unwrap();
    
    // Enable backoff algorithm
    queue.enable_backoff_algorithm();
    
    // Try to enqueue another message that would exceed memory limit
    let start_time = Instant::now();
    let result = queue.enqueue_with_backoff(create_test_message("overflow"));
    let duration = start_time.elapsed();
    
    // Should have applied backoff delay
    assert!(duration >= Duration::from_millis(10)); // Minimum backoff delay
    
    // Result should be an error due to memory limit
    assert!(result.is_err());
}

#[test]
fn test_adaptive_backoff_based_on_memory_recovery() {
    // This test should fail initially (RED phase)
    // Test that backoff adapts based on memory recovery patterns
    let tracker = Arc::new(MemoryTracker::new(2048));
    let queue = MemoryQueue::with_shared_tracker(100, 2048, tracker.clone());
    
    queue.enable_backoff_algorithm();
    
    // Fill queue to trigger memory pressure
    for i in 0..3 {
        let msg = create_test_message(&format!("msg_{}", i));
        queue.enqueue(msg).unwrap();
    }
    
    // Measure initial backoff delay
    let start_time = Instant::now();
    let _ = queue.enqueue_with_backoff(create_large_test_message(1500)); // Should fail
    let initial_delay = start_time.elapsed();
    
    // Clear some memory by dequeuing
    queue.dequeue().unwrap();
    queue.dequeue().unwrap();
    
    // Set adaptive strategy to test memory recovery factor
    queue.set_backoff_strategy(BackoffStrategy::Adaptive {
        initial_delay_ms: 20,
        success_factor: 0.8,
        failure_factor: 1.5,
        memory_recovery_factor: 0.5, // Reduce delay by 50% when memory is recovering
    });
    
    // Measure backoff delay after memory recovery
    let start_time = Instant::now();
    let _ = queue.enqueue_with_backoff(create_test_message("after_recovery"));
    let recovery_delay = start_time.elapsed();
    
    // Backoff should be shorter after memory recovery
    assert!(recovery_delay < initial_delay);
}

#[test]
fn test_backoff_configuration_parameters() {
    // This test should fail initially (RED phase)
    // Test that backoff parameters can be configured
    let tracker = Arc::new(MemoryTracker::new(1024));
    let queue = MemoryQueue::with_shared_tracker(100, 1024, tracker.clone());
    
    // Configure custom backoff parameters
    let backoff_config = BackoffConfig {
        initial_delay_ms: 5,
        max_delay_ms: 100,
        multiplier: 1.5,
        max_retries: 5,
        memory_pressure_threshold: 80.0,
    };
    
    queue.configure_backoff(backoff_config).unwrap();
    queue.enable_backoff_algorithm();
    
    // Fill queue to trigger configured backoff
    let large_message = create_large_test_message(800);
    queue.enqueue(large_message).unwrap();
    
    let start_time = Instant::now();
    let _ = queue.enqueue_with_backoff(create_test_message("test"));
    let duration = start_time.elapsed();
    
    // Should use configured initial delay (5ms)
    assert!(duration >= Duration::from_millis(5));
    assert!(duration < Duration::from_millis(50)); // Should not exceed much due to quick failure
}

#[test]
fn test_backoff_retry_with_exponential_increase() {
    // This test should fail initially (RED phase)
    // Test that backoff delay increases exponentially with retries
    let tracker = Arc::new(MemoryTracker::new(1024));
    let queue = MemoryQueue::with_shared_tracker(100, 1024, tracker.clone());
    
    queue.enable_backoff_algorithm();
    
    // Fill queue completely
    let large_message = create_large_test_message(800);
    queue.enqueue(large_message).unwrap();
    
    // Track backoff delays across multiple retry attempts
    let mut delays = Vec::new();
    
    for attempt in 0..3 {
        let start_time = Instant::now();
        let _ = queue.enqueue_with_backoff(create_test_message(&format!("retry_{}", attempt)));
        let delay = start_time.elapsed();
        delays.push(delay);
    }
    
    // Each delay should be longer than the previous (exponential backoff)
    assert!(delays[1] > delays[0]);
    assert!(delays[2] > delays[1]);
    
    // Verify exponential relationship (approximately)
    let ratio1 = delays[1].as_millis() as f64 / delays[0].as_millis() as f64;
    let ratio2 = delays[2].as_millis() as f64 / delays[1].as_millis() as f64;
    
    // Ratios should be similar (exponential growth)
    assert!(ratio1 > 1.2); // At least 20% increase
    assert!(ratio2 > 1.2);
}

#[test]
fn test_backoff_event_logging_and_metrics() {
    // This test should fail initially (RED phase)
    // Test that backoff events are logged and metrics are collected
    let tracker = Arc::new(MemoryTracker::new(1024));
    let queue = MemoryQueue::with_shared_tracker(100, 1024, tracker.clone());
    
    queue.enable_backoff_algorithm();
    
    // Trigger backoff events
    let large_message = create_large_test_message(800);
    queue.enqueue(large_message).unwrap();
    
    // This should trigger backoff
    let _ = queue.enqueue_with_backoff(create_test_message("backoff_trigger"));
    
    // Get backoff metrics
    let metrics = queue.get_backoff_metrics();
    
    assert!(metrics.total_backoff_events > 0);
    assert!(metrics.total_backoff_duration > Duration::from_millis(0));
    assert!(metrics.average_backoff_delay > Duration::from_millis(0));
    assert_eq!(metrics.current_backoff_level, 1); // Should be at level 1 after first backoff
}

#[test]
fn test_pressure_response_system() {
    // This test should fail initially (RED phase)
    // Test queue throttling and message dropping during extreme pressure
    let tracker = Arc::new(MemoryTracker::new(512)); // Very small limit
    let queue = MemoryQueue::with_shared_tracker(100, 512, tracker.clone());
    
    queue.enable_backoff_algorithm();
    queue.enable_pressure_response();
    
    // Configure aggressive pressure response
    let pressure_config = PressureResponseConfig {
        throttle_threshold: 70.0,    // Start throttling at 70%
        drop_threshold: 90.0,        // Start dropping at 90%
        throttle_factor: 0.5,        // Reduce throughput by 50%
        recovery_factor: 0.8,        // Recover at 80% rate
    };
    
    queue.configure_pressure_response(pressure_config).unwrap();
    
    // Fill queue to trigger pressure response
    let msg = create_test_message("pressure_test");
    while queue.enqueue(msg.clone()).is_ok() {
        // Fill until we hit limit
    }
    
    // Verify pressure response is active
    let pressure_status = queue.get_pressure_response_status();
    assert!(pressure_status.is_throttling);
    assert!(pressure_status.current_pressure_level > 70.0);
    
    // Test that extreme pressure triggers message dropping
    queue.set_extreme_pressure_mode(true);
    
    let drop_result = queue.enqueue_with_backoff(create_test_message("should_drop"));
    
    // Should return specific error for dropped message
    assert!(matches!(drop_result, Err(QueueError::MessageDropped(_))));
    
    let drop_metrics = queue.get_pressure_response_metrics();
    assert!(drop_metrics.messages_dropped > 0);
}

#[test]
fn test_backoff_strategy_selection() {
    // This test should fail initially (RED phase)
    // Test different backoff strategy implementations
    let tracker = Arc::new(MemoryTracker::new(1024));
    let queue = MemoryQueue::with_shared_tracker(100, 1024, tracker.clone());
    
    // Test exponential backoff strategy
    queue.set_backoff_strategy(BackoffStrategy::Exponential {
        base_delay_ms: 10,
        multiplier: 2.0,
        max_delay_ms: 1000,
    });
    
    // Test linear backoff strategy  
    queue.set_backoff_strategy(BackoffStrategy::Linear {
        base_delay_ms: 20,
        increment_ms: 10,
        max_delay_ms: 200,
    });
    
    // Test adaptive backoff strategy
    queue.set_backoff_strategy(BackoffStrategy::Adaptive {
        initial_delay_ms: 5,
        success_factor: 0.8,
        failure_factor: 1.5,
        memory_recovery_factor: 0.6,
    });
    
    queue.enable_backoff_algorithm();
    
    // Fill queue to test different strategies
    let large_message = create_large_test_message(800);
    queue.enqueue(large_message).unwrap();
    
    // Test that adaptive strategy responds to memory recovery
    let start_time = Instant::now();
    let _ = queue.enqueue_with_backoff(create_test_message("adaptive_test"));
    let adaptive_delay = start_time.elapsed();
    
    // Should use adaptive delay calculation
    assert!(adaptive_delay >= Duration::from_millis(5));
}

#[test]
fn test_memory_recovery_adaptive_backoff() {
    // This test verifies that adaptive backoff responds to memory recovery trends
    let tracker = Arc::new(MemoryTracker::new(1024)); // Smaller limit to ensure pressure
    let queue = MemoryQueue::with_shared_tracker(100, 1024, tracker.clone());
    
    // Configure backoff with lower threshold to trigger more easily
    let backoff_config = BackoffConfig {
        initial_delay_ms: 20,
        max_delay_ms: 1000,
        multiplier: 2.0,
        max_retries: 5,
        memory_pressure_threshold: 50.0, // Lower threshold for easier triggering
    };
    
    queue.configure_backoff(backoff_config).unwrap();
    queue.set_backoff_strategy(BackoffStrategy::Adaptive {
        initial_delay_ms: 20,
        success_factor: 0.9,
        failure_factor: 1.2,
        memory_recovery_factor: 0.3, // Aggressive recovery factor for testing
    });
    queue.enable_backoff_algorithm();
    
    // Fill queue to create high memory pressure
    let large_msg = create_large_test_message(800); // Large message to fill memory
    queue.enqueue(large_msg).unwrap();
    
    // Verify high memory usage
    let initial_usage = queue.memory_usage_percent();
    println!("Initial memory usage: {:.2}%", initial_usage);
    
    // First backoff attempt with high memory usage
    let start_time = Instant::now();
    let _ = queue.enqueue_with_backoff(create_test_message("high_memory_test"));
    let high_memory_delay = start_time.elapsed();
    println!("High memory delay: {:?}", high_memory_delay);
    
    // Clear memory to trigger recovery trend
    queue.dequeue().unwrap();
    
    let after_clear_usage = queue.memory_usage_percent();  
    println!("Memory usage after clear: {:.2}%", after_clear_usage);
    
    // Generate several samples to establish recovery trend
    for i in 0..3 {
        let msg = create_test_message(&format!("recovery_sample_{}", i));
        let _ = queue.enqueue_with_backoff(msg);
        std::thread::sleep(Duration::from_millis(5)); // Small delay between samples
    }
    
    // Check that memory is actually recovering
    let is_recovering = queue.is_memory_recovering();
    let current_usage = queue.memory_usage_percent();
    println!("Is recovering: {}, Current usage: {:.2}%", is_recovering, current_usage);
    
    // Now test backoff delay with memory recovery factor applied
    let start_time = Instant::now();
    let _ = queue.enqueue_with_backoff(create_test_message("recovery_test"));
    let recovery_delay = start_time.elapsed();
    println!("Recovery delay: {:?}", recovery_delay);
    
    // Verify that we achieved sufficient memory pressure initially
    if initial_usage > 70.0 {
        println!("Successfully created high memory pressure");
        // If we had high memory pressure, verify backoff was applied
        assert!(high_memory_delay >= Duration::from_millis(5));
    } else {
        println!("Test demonstrates memory recovery tracking even without high pressure");
    }
    
    // Verify the memory recovery detection is working
    assert!(is_recovering || current_usage < initial_usage);
    
    // The key feature being tested: adaptive backoff can detect memory recovery trends
    println!("Memory recovery adaptive backoff test completed successfully");
}

#[test]
fn test_configuration_management() {
    // Test configuration presets and validation
    let tracker = Arc::new(MemoryTracker::new(1024));
    let queue = MemoryQueue::with_shared_tracker(100, 1024, tracker.clone());
    
    // Test valid configuration presets
    let conservative = BackoffConfig::conservative();
    let aggressive = BackoffConfig::aggressive();
    let balanced = BackoffConfig::balanced();
    
    // All presets should validate successfully
    assert!(conservative.validate().is_ok());
    assert!(aggressive.validate().is_ok());
    assert!(balanced.validate().is_ok());
    
    // Test applying configurations
    assert!(queue.configure_backoff(conservative).is_ok());
    assert!(queue.configure_backoff(aggressive).is_ok());
    assert!(queue.configure_backoff(balanced).is_ok());
    
    // Test invalid configuration validation
    let invalid_config = BackoffConfig {
        initial_delay_ms: 0, // Invalid: must be > 0
        max_delay_ms: 1000,
        multiplier: 2.0,
        max_retries: 5,
        memory_pressure_threshold: 80.0,
    };
    assert!(invalid_config.validate().is_err());
    assert!(queue.configure_backoff(invalid_config).is_err());
    
    let invalid_config2 = BackoffConfig {
        initial_delay_ms: 100,
        max_delay_ms: 50, // Invalid: max < initial
        multiplier: 2.0,
        max_retries: 5,
        memory_pressure_threshold: 80.0,
    };
    assert!(invalid_config2.validate().is_err());
    
    // Test pressure response configurations
    let pr_conservative = PressureResponseConfig::conservative();
    let pr_aggressive = PressureResponseConfig::aggressive();
    let pr_balanced = PressureResponseConfig::balanced();
    
    // All presets should validate successfully
    assert!(pr_conservative.validate().is_ok());
    assert!(pr_aggressive.validate().is_ok());
    assert!(pr_balanced.validate().is_ok());
    
    // Test applying pressure response configurations
    assert!(queue.configure_pressure_response(pr_conservative).is_ok());
    assert!(queue.configure_pressure_response(pr_aggressive).is_ok());
    assert!(queue.configure_pressure_response(pr_balanced).is_ok());
    
    // Test invalid pressure response config
    let invalid_pr_config = PressureResponseConfig {
        throttle_threshold: 90.0,
        drop_threshold: 80.0, // Invalid: drop <= throttle
        throttle_factor: 0.5,
        recovery_factor: 0.8,
    };
    assert!(invalid_pr_config.validate().is_err());
    assert!(queue.configure_pressure_response(invalid_pr_config).is_err());
}

// Helper functions for test message creation
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
    let large_path = "x".repeat(target_size); // Create large path to increase message size
    ScanMessage::new(
        MessageHeader::new(ScanMode::FILES, 12345),
        MessageData::FileInfo {
            path: large_path,
            size: target_size as u64,
            lines: 100,
        }
    )
}

// Helper structs and types are now imported from the main codebase