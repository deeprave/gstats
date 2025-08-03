//! Listener system tests

use crate::queue::{Queue, MessageListener, ListenerRegistry, DefaultListenerRegistry, MessageConsumer, ConsumerConfig};
use crate::queue::memory_tracker::MemoryTracker;
use crate::queue::MemoryQueue;
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
use crate::scanner::modes::ScanMode;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

// Performance test listener
struct PerformanceTestListener {
    id: String,
    interested_modes: ScanMode,
    received_count: Arc<AtomicUsize>,
    processing_time: Duration,
}

impl PerformanceTestListener {
    fn new(id: &str, modes: ScanMode, processing_time: Duration) -> Self {
        Self {
            id: id.to_string(),
            interested_modes: modes,
            received_count: Arc::new(AtomicUsize::new(0)),
            processing_time,
        }
    }

    fn received_count(&self) -> usize {
        self.received_count.load(Ordering::Relaxed)
    }
}

impl MessageListener for PerformanceTestListener {
    fn interested_modes(&self) -> ScanMode {
        self.interested_modes
    }
    
    fn on_message(&self, _message: &ScanMessage) -> Result<(), Box<dyn std::error::Error>> {
        // Simulate processing time
        if !self.processing_time.is_zero() {
            std::thread::sleep(self.processing_time);
        }
        
        self.received_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    
    fn listener_id(&self) -> String {
        self.id.clone()
    }
}

fn create_test_message(mode: ScanMode, content: &str) -> ScanMessage {
    ScanMessage::new(
        MessageHeader::new(mode, 12345),
        MessageData::FileInfo {
            path: format!("test_{}.rs", content),
            size: 1024,
            lines: 50,
        }
    )
}

#[test]
fn test_listener_notification_performance() {
    // Test that listener notifications are efficient
    let tracker = Arc::new(MemoryTracker::new(10 * 1024 * 1024)); // 10MB
    let queue = Arc::new(MemoryQueue::with_shared_tracker(1000, 10 * 1024 * 1024, tracker));
    let registry = Arc::new(Mutex::new(DefaultListenerRegistry::new()));
    
    // Register multiple listeners with different processing times
    let fast_listener = Arc::new(PerformanceTestListener::new("fast", ScanMode::FILES, Duration::from_micros(10)));
    let medium_listener = Arc::new(PerformanceTestListener::new("medium", ScanMode::FILES, Duration::from_micros(50)));
    let slow_listener = Arc::new(PerformanceTestListener::new("slow", ScanMode::FILES, Duration::from_micros(100)));
    
    let fast_count = Arc::clone(&fast_listener.received_count);
    let medium_count = Arc::clone(&medium_listener.received_count);
    let slow_count = Arc::clone(&slow_listener.received_count);
    
    {
        let mut registry_guard = registry.lock().unwrap();
        registry_guard.register_listener(fast_listener).unwrap();
        registry_guard.register_listener(medium_listener).unwrap();
        registry_guard.register_listener(slow_listener).unwrap();
    }
    
    // Configure consumer for performance
    let config = ConsumerConfig {
        poll_interval_ms: 1,
        batch_size: 10,
        notification_timeout_ms: 1000,
        continue_on_error: true,
    };
    
    let mut consumer = MessageConsumer::with_config(Arc::clone(&queue), Arc::clone(&registry), config);
    consumer.start().unwrap();
    
    // Enqueue multiple messages and measure processing time
    let message_count = 100;
    let start_time = Instant::now();
    
    for i in 0..message_count {
        let message = create_test_message(ScanMode::FILES, &format!("perf_{}", i));
        queue.enqueue(message).unwrap();
    }
    
    // Wait for all messages to be processed
    let timeout = Duration::from_secs(5);
    let poll_interval = Duration::from_millis(10);
    let mut elapsed = Duration::from_secs(0);
    
    while elapsed < timeout {
        let total_received = fast_count.load(Ordering::Relaxed) 
            + medium_count.load(Ordering::Relaxed) 
            + slow_count.load(Ordering::Relaxed);
            
        if total_received >= message_count * 3 { // 3 listeners per message
            break;
        }
        
        std::thread::sleep(poll_interval);
        elapsed += poll_interval;
    }
    
    let processing_time = start_time.elapsed();
    consumer.stop().unwrap();
    
    // Verify all listeners received all messages
    assert_eq!(fast_count.load(Ordering::Relaxed), message_count);
    assert_eq!(medium_count.load(Ordering::Relaxed), message_count);
    assert_eq!(slow_count.load(Ordering::Relaxed), message_count);
    
    // Check performance metrics
    let metrics = consumer.get_metrics();
    assert_eq!(metrics.messages_processed, message_count as u64);
    assert_eq!(metrics.notifications_sent, (message_count * 3) as u64);
    assert_eq!(metrics.notification_errors, 0);
    
    // Performance assertion - should process 100 messages with 3 listeners each in reasonable time
    assert!(processing_time < Duration::from_secs(2), 
        "Processing took too long: {:?}", processing_time);
    
    println!("Processed {} messages to {} listeners in {:?}", 
        message_count, 3, processing_time);
    println!("Average notification latency: {:?}", metrics.average_notification_latency);
}

#[test]
fn test_listener_lookup_efficiency() {
    // Test that listener lookup is efficient with many listeners
    let registry = DefaultListenerRegistry::new();
    let mut registry = registry;
    
    // Register many listeners with different scan modes
    let listener_count = 1000;
    for i in 0..listener_count {
        let mode = match i % 3 {
            0 => ScanMode::FILES,
            1 => ScanMode::HISTORY,
            _ => ScanMode::METRICS,
        };
        
        let listener = Arc::new(PerformanceTestListener::new(
            &format!("listener_{}", i),
            mode,
            Duration::from_nanos(0),
        ));
        registry.register_listener(listener).unwrap();
    }
    
    assert_eq!(registry.listener_count(), listener_count);
    
    // Measure lookup performance
    let start_time = Instant::now();
    let lookups = 1000;
    
    for _ in 0..lookups {
        let files_listeners = registry.get_interested_listeners(ScanMode::FILES);
        let history_listeners = registry.get_interested_listeners(ScanMode::HISTORY);
        let metrics_listeners = registry.get_interested_listeners(ScanMode::METRICS);
        
        // Verify expected counts (roughly 1/3 each)
        assert!(files_listeners.len() > 300 && files_listeners.len() < 400);
        assert!(history_listeners.len() > 300 && history_listeners.len() < 400);
        assert!(metrics_listeners.len() > 300 && metrics_listeners.len() < 400);
    }
    
    let lookup_time = start_time.elapsed();
    let avg_lookup_time = lookup_time / (lookups * 3); // 3 lookups per iteration
    
    // Lookup should be fast even with many listeners
    assert!(avg_lookup_time < Duration::from_micros(100), 
        "Average lookup time too slow: {:?}", avg_lookup_time);
    
    println!("Average listener lookup time: {:?} with {} listeners", 
        avg_lookup_time, listener_count);
}

#[test]
fn test_batch_processing_optimization() {
    // Test that batch processing improves throughput
    let tracker = Arc::new(MemoryTracker::new(10 * 1024 * 1024));
    let queue = Arc::new(MemoryQueue::with_shared_tracker(2000, 10 * 1024 * 1024, tracker));
    let registry = Arc::new(Mutex::new(DefaultListenerRegistry::new()));
    
    // Register a fast listener
    let listener = Arc::new(PerformanceTestListener::new("batch_test", ScanMode::FILES, Duration::from_nanos(0)));
    let received_count = Arc::clone(&listener.received_count);
    
    {
        let mut registry_guard = registry.lock().unwrap();
        registry_guard.register_listener(listener).unwrap();
    }
    
    // Test with large batch size
    let config = ConsumerConfig {
        poll_interval_ms: 1,
        batch_size: 100, // Large batch
        notification_timeout_ms: 1000,
        continue_on_error: true,
    };
    
    let mut consumer = MessageConsumer::with_config(Arc::clone(&queue), Arc::clone(&registry), config);
    consumer.start().unwrap();
    
    // Enqueue many messages quickly
    let message_count = 1000;
    let start_time = Instant::now();
    
    for i in 0..message_count {
        let message = create_test_message(ScanMode::FILES, &format!("batch_{}", i));
        queue.enqueue(message).unwrap();
    }
    
    // Wait for processing
    let timeout = Duration::from_secs(10);
    let poll_interval = Duration::from_millis(10);
    let mut elapsed = Duration::from_secs(0);
    
    while elapsed < timeout {
        if received_count.load(Ordering::Relaxed) >= message_count {
            break;
        }
        std::thread::sleep(poll_interval);
        elapsed += poll_interval;
    }
    
    let processing_time = start_time.elapsed();
    consumer.stop().unwrap();
    
    // Verify all messages processed
    assert_eq!(received_count.load(Ordering::Relaxed), message_count);
    
    // Check batch processing metrics
    let metrics = consumer.get_metrics();
    assert!(metrics.batches_processed > 0);
    assert!(metrics.average_batch_size > 1.0); // Should be batching
    
    // Should process efficiently due to batching
    assert!(processing_time < Duration::from_secs(5),
        "Batch processing took too long: {:?}", processing_time);
    
    println!("Batch processed {} messages in {:?}", message_count, processing_time);
    println!("Average batch size: {:.2}", metrics.average_batch_size);
    println!("Batches processed: {}", metrics.batches_processed);
}

#[test]
fn test_scan_mode_intersection_filtering() {
    // Test that ScanMode intersection filtering works correctly
    let registry = DefaultListenerRegistry::new();
    let mut registry = registry;
    
    // Register listeners with different mode combinations
    let files_only = Arc::new(PerformanceTestListener::new("files_only", ScanMode::FILES, Duration::from_nanos(0)));
    let history_only = Arc::new(PerformanceTestListener::new("history_only", ScanMode::HISTORY, Duration::from_nanos(0)));
    let files_and_history = Arc::new(PerformanceTestListener::new("files_and_history", ScanMode::FILES | ScanMode::HISTORY, Duration::from_nanos(0)));
    let all_modes = Arc::new(PerformanceTestListener::new("all_modes", ScanMode::FILES | ScanMode::HISTORY | ScanMode::METRICS, Duration::from_nanos(0)));
    
    registry.register_listener(files_only).unwrap();
    registry.register_listener(history_only).unwrap();
    registry.register_listener(files_and_history).unwrap();
    registry.register_listener(all_modes).unwrap();
    
    // Test FILES mode filtering
    let files_listeners = registry.get_interested_listeners(ScanMode::FILES);
    assert_eq!(files_listeners.len(), 3); // files_only, files_and_history, all_modes
    
    let files_ids: Vec<String> = files_listeners.iter().map(|l| l.listener_id()).collect();
    assert!(files_ids.contains(&"files_only".to_string()));
    assert!(files_ids.contains(&"files_and_history".to_string()));
    assert!(files_ids.contains(&"all_modes".to_string()));
    assert!(!files_ids.contains(&"history_only".to_string()));
    
    // Test HISTORY mode filtering
    let history_listeners = registry.get_interested_listeners(ScanMode::HISTORY);
    assert_eq!(history_listeners.len(), 3); // history_only, files_and_history, all_modes
    
    // Test METRICS mode filtering
    let metrics_listeners = registry.get_interested_listeners(ScanMode::METRICS);
    assert_eq!(metrics_listeners.len(), 1); // all_modes only
    
    // Test combined mode filtering
    let files_or_history = registry.get_interested_listeners(ScanMode::FILES | ScanMode::HISTORY);
    assert_eq!(files_or_history.len(), 4); // All listeners match: files_only (FILES), history_only (HISTORY), files_and_history (both), all_modes (both)
}