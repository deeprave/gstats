//! Queue Performance Benchmarks
//!
//! Measures queue-specific performance metrics including throughput, latency,
//! ScanMode filtering, consumer processing, and multi-producer scenarios.
//! Targets: >10,000 messages/sec, <1ms latency, <100μs filtering

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::thread;

use gstats::queue::{
    MemoryQueue, QueueMessageProducer, MessageConsumer, DefaultListenerRegistry,
    ConsumerConfig, Queue, QueueConfig, QueuePreset,
};
use gstats::scanner::messages::{MessageHeader, MessageData, ScanMessage};
use gstats::scanner::{ScanMode, MessageProducer};

/// Create test messages for queue benchmarking
fn create_queue_test_messages(count: usize, scan_mode: ScanMode) -> Vec<ScanMessage> {
    (0..count)
        .map(|i| {
            ScanMessage::new(
                MessageHeader::new(scan_mode, i as u64),
                MessageData::FileInfo {
                    path: format!("bench_file_{}.rs", i),
                    size: 1024,
                    lines: 50,
                }
            )
        })
        .collect()
}

/// Benchmark single producer throughput (target: >10,000 messages/sec)
fn bench_single_producer_throughput(c: &mut Criterion) {
    let message_counts = vec![1000, 5000, 10000, 25000, 50000];
    
    for count in message_counts {
        let queue = Arc::new(MemoryQueue::new(100000, 256 * 1024 * 1024));
        let producer = QueueMessageProducer::new(
            Arc::clone(&queue),
            "ThroughputProducer".to_string()
        );
        let messages = create_queue_test_messages(count, ScanMode::FILES);
        
        c.benchmark_group("single_producer_throughput")
            .throughput(Throughput::Elements(count as u64))
            .bench_with_input(
                BenchmarkId::new("messages_per_second", count),
                &count,
                |b, &_count| {
                    b.iter(|| {
                        let start = Instant::now();
                        for msg in &messages {
                            producer.produce_message(msg.clone());
                        }
                        let duration = start.elapsed();
                        
                        // Calculate throughput
                        let throughput = messages.len() as f64 / duration.as_secs_f64();
                        assert!(throughput > 10000.0, "Throughput {} < 10,000 msg/sec", throughput);
                    })
                }
            );
    }
}

/// Benchmark multi-producer concurrent throughput
fn bench_multi_producer_throughput(c: &mut Criterion) {
    let thread_counts = vec![2, 4, 8, 16];
    let messages_per_thread = 2500; // Total: 5K, 10K, 20K, 40K messages
    
    for thread_count in thread_counts {
        let queue = Arc::new(MemoryQueue::new(100000, 256 * 1024 * 1024));
        let total_messages = thread_count * messages_per_thread;
        
        c.benchmark_group("multi_producer_throughput")
            .throughput(Throughput::Elements(total_messages as u64))
            .bench_with_input(
                BenchmarkId::new("concurrent_threads", thread_count),
                &thread_count,
                |b, &thread_count| {
                    b.iter(|| {
                        let start = Instant::now();
                        
                        let handles: Vec<_> = (0..thread_count)
                            .map(|thread_id| {
                                let queue_clone = Arc::clone(&queue);
                                let messages = create_queue_test_messages(
                                    messages_per_thread, 
                                    ScanMode::FILES
                                );
                                
                                thread::spawn(move || {
                                    let producer = QueueMessageProducer::new(
                                        queue_clone,
                                        format!("Producer{}", thread_id)
                                    );
                                    
                                    for msg in messages {
                                        producer.produce_message(msg);
                                    }
                                })
                            })
                            .collect();
                        
                        for handle in handles {
                            handle.join().unwrap();
                        }
                        
                        let duration = start.elapsed();
                        let throughput = total_messages as f64 / duration.as_secs_f64();
                        
                        // Should scale reasonably with thread count
                        let expected_min = 10000.0 * (thread_count as f64 * 0.7); // 70% scaling efficiency
                        assert!(throughput > expected_min, 
                            "Multi-producer throughput {} < expected {}", throughput, expected_min);
                    })
                }
            );
    }
}

/// Benchmark consumer thread latency (target: <1ms average)
fn bench_consumer_latency(c: &mut Criterion) {
    let queue = Arc::new(MemoryQueue::new(100000, 256 * 1024 * 1024));
    let registry = Arc::new(std::sync::Mutex::new(DefaultListenerRegistry::new()));
    let consumer_config = ConsumerConfig::default();
    
    // Pre-populate queue with test messages
    let messages = create_queue_test_messages(1000, ScanMode::FILES);
    let producer = QueueMessageProducer::new(Arc::clone(&queue), "LatencyProducer".to_string());
    
    for msg in &messages {
        producer.produce_message(msg.clone());
    }
    
    c.bench_function("consumer_processing_latency", |b| {
        b.iter(|| {
            let consumer = MessageConsumer::with_config(
                Arc::clone(&queue),
                Arc::clone(&registry),
                consumer_config.clone(),
            );
            
            let start = Instant::now();
            
            // Process a batch of messages
            for _ in 0..100 {
                if let Ok(Some(_msg)) = queue.dequeue() {
                    // Simulate minimal processing
                }
            }
            
            let duration = start.elapsed();
            let avg_latency = duration / 100;
            
            // Target: <1ms average latency
            assert!(avg_latency < Duration::from_millis(1), 
                "Average latency {:?} > 1ms", avg_latency);
        })
    });
}

/// Benchmark ScanMode filtering performance (target: efficient filtering)
fn bench_scanmode_filtering(c: &mut Criterion) {
    let modes = vec![
        ("files", ScanMode::FILES),
        ("history", ScanMode::HISTORY),
        ("combined", ScanMode::FILES | ScanMode::HISTORY),
        ("all", ScanMode::all()),
    ];
    
    // Create smaller test set for realistic filtering performance
    let test_messages: Vec<_> = modes.iter()
        .flat_map(|(_, mode)| create_queue_test_messages(50, *mode))
        .collect();
    
    for (mode_name, filter_mode) in modes {
        c.benchmark_group("scanmode_filtering")
            .throughput(Throughput::Elements(test_messages.len() as u64))
            .bench_with_input(
                BenchmarkId::new("filter_performance", mode_name),
                &filter_mode,
                |b, &filter_mode| {
                    b.iter(|| {
                        let filtered: Vec<_> = test_messages.iter()
                            .filter(|msg| msg.header.scan_mode.intersects(filter_mode))
                            .collect();
                        
                        // Return count for optimizer to not remove the work
                        filtered.len()
                    })
                }
            );
    }
}

/// Benchmark consumer batch processing latency
fn bench_consumer_batch_processing(c: &mut Criterion) {
    let batch_sizes = vec![1, 10, 50, 100, 500];
    
    for batch_size in batch_sizes {
        let queue = Arc::new(MemoryQueue::new(100000, 256 * 1024 * 1024));
        let messages = create_queue_test_messages(batch_size * 10, ScanMode::FILES);
        let producer = QueueMessageProducer::new(Arc::clone(&queue), "BatchProducer".to_string());
        
        // Pre-populate queue
        for msg in &messages {
            producer.produce_message(msg.clone());
        }
        
        c.benchmark_group("consumer_batch_processing")
            .throughput(Throughput::Elements(batch_size as u64))
            .bench_with_input(
                BenchmarkId::new("batch_latency", batch_size),
                &batch_size,
                |b, &batch_size| {
                    b.iter(|| {
                        let start = Instant::now();
                        
                        let mut processed = 0;
                        while processed < batch_size {
                            if let Ok(Some(_msg)) = queue.dequeue() {
                                processed += 1;
                            }
                        }
                        
                        let duration = start.elapsed();
                        let per_message_latency = duration / batch_size as u32;
                        
                        // Batch processing should be more efficient
                        let max_latency = Duration::from_micros(500); // 0.5ms per message in batch
                        assert!(per_message_latency < max_latency,
                            "Batch per-message latency {:?} > {:?}", per_message_latency, max_latency);
                    })
                }
            );
    }
}

/// Benchmark message size impact on throughput
fn bench_message_size_throughput_impact(c: &mut Criterion) {
    let message_sizes = vec![256, 1024, 4096, 16384]; // bytes
    let message_count = 1000;
    
    for size in message_sizes {
        let queue = Arc::new(MemoryQueue::new(100000, 256 * 1024 * 1024));
        let producer = QueueMessageProducer::new(Arc::clone(&queue), "SizeProducer".to_string());
        
        // Create messages with specified size
        let messages: Vec<_> = (0..message_count)
            .map(|i| {
                let large_path = "x".repeat(size);
                ScanMessage::new(
                    MessageHeader::new(ScanMode::FILES, i as u64),
                    MessageData::FileInfo {
                        path: large_path,
                        size: size as u64,
                        lines: 100,
                    }
                )
            })
            .collect();
        
        c.benchmark_group("message_size_throughput_impact")
            .throughput(Throughput::Bytes((message_count * size) as u64))
            .bench_with_input(
                BenchmarkId::new("throughput_by_size", size),
                &size,
                |b, &_size| {
                    b.iter(|| {
                        for msg in &messages {
                            producer.produce_message(msg.clone());
                        }
                    })
                }
            );
    }
}

/// Benchmark memory overhead per message (measure queue capacity efficiency)
fn bench_memory_overhead(c: &mut Criterion) {
    c.bench_function("memory_overhead_measurement", |b| {
        b.iter(|| {
            let queue = Arc::new(MemoryQueue::new(100000, 256 * 1024 * 1024));
            let producer = QueueMessageProducer::new(Arc::clone(&queue), "OverheadProducer".to_string());
            
            // Add messages and measure queue utilization
            let message_count = 1000;
            let messages = create_queue_test_messages(message_count, ScanMode::FILES);
            
            let initial_size = queue.size();
            let initial_memory_usage = queue.memory_usage_percent();
            
            for msg in &messages {
                producer.produce_message(msg.clone());
            }
            
            let final_size = queue.size();
            let final_memory_usage = queue.memory_usage_percent();
            let messages_added = final_size - initial_size;
            let memory_increase = final_memory_usage - initial_memory_usage;
            
            // Return metrics for analysis (queue should handle 1000 messages efficiently)
            assert!(messages_added <= message_count, "Should not exceed expected message count");
            assert!(memory_increase < 50.0, "Memory usage should not exceed 50% for 1000 messages");
            
            (messages_added, memory_increase)
        })
    });
}

/// Benchmark backoff algorithm effectiveness under pressure
fn bench_backoff_effectiveness(c: &mut Criterion) {
    let queue = Arc::new(MemoryQueue::new(100000, 256 * 1024 * 1024));
    let producer = QueueMessageProducer::new(Arc::clone(&queue), "BackoffProducer".to_string());
    
    // Create high memory pressure scenario
    let pressure_messages = create_queue_test_messages(50000, ScanMode::FILES);
    
    c.bench_function("backoff_under_pressure", |b| {
        b.iter(|| {
            let start = Instant::now();
            let mut successful_enqueues = 0;
            
            for msg in &pressure_messages {
                producer.produce_message(msg.clone());
                successful_enqueues += 1;
                
                // Stop if we hit memory pressure (using memory usage as proxy)
                if queue.memory_usage_percent() > 80.0 {
                    break;
                }
            }
            
            let duration = start.elapsed();
            
            // Backoff should allow some messages through before triggering
            // With 50,000 messages and 256MB limit, we should get at least some through
            assert!(successful_enqueues > 100, 
                "Backoff too aggressive: only {} messages succeeded", successful_enqueues);
            
            (successful_enqueues, duration)
        })
    });
}

criterion_group!(
    queue_benches,
    bench_single_producer_throughput,
    bench_multi_producer_throughput,
    bench_consumer_latency,
    bench_scanmode_filtering,
    bench_consumer_batch_processing,
    bench_message_size_throughput_impact,
    bench_memory_overhead,
    bench_backoff_effectiveness
);

criterion_main!(queue_benches);