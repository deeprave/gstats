//! Memory Usage and Allocation Benchmarks
//!
//! Measures memory consumption patterns, allocation rates, and memory pressure
//! response across different queue sizes and scanning workloads.

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::sync::Arc;
use std::time::Duration;

use gstats::queue::{
    MemoryQueue, QueueMessageProducer, MemoryTracker, MemoryPressureLevel,
    BackoffAlgorithm, BackoffConfig, BackoffStrategy,
};
use gstats::scanner::messages::{MessageHeader, MessageData, ScanMessage};
use gstats::scanner::ScanMode;

/// Create test messages of various sizes
fn create_test_messages(count: usize, message_size: usize) -> Vec<ScanMessage> {
    (0..count)
        .map(|i| {
            let large_content = "x".repeat(message_size);
            ScanMessage::new(
                MessageHeader::new(ScanMode::FILES, i as u64),
                MessageData::FileInfo {
                    path: format!("test_file_{}.rs", i),
                    size: message_size as u64,
                    lines: 100,
                }
            )
        })
        .collect()
}

/// Benchmark memory queue creation and basic operations
fn bench_memory_queue_operations(c: &mut Criterion) {
    c.bench_function("memory_queue_creation", |b| {
        b.iter(|| MemoryQueue::new())
    });
    
    let queue = Arc::new(MemoryQueue::new());
    let messages = create_test_messages(100, 1024);
    
    c.bench_function("memory_queue_enqueue_single", |b| {
        b.iter(|| {
            queue.enqueue(messages[0].clone()).unwrap()
        })
    });
    
    // Fill queue for dequeue benchmark
    for msg in &messages {
        let _ = queue.enqueue(msg.clone());
    }
    
    c.bench_function("memory_queue_dequeue_single", |b| {
        b.iter(|| {
            queue.try_dequeue().unwrap()
        })
    });
}

/// Benchmark memory queue with different message sizes
fn bench_memory_queue_message_sizes(c: &mut Criterion) {
    let sizes = vec![256, 1024, 4096, 16384, 65536]; // bytes
    
    for size in sizes {
        let queue = Arc::new(MemoryQueue::new());
        let messages = create_test_messages(10, size);
        
        c.benchmark_group("memory_queue_message_sizes")
            .throughput(Throughput::Bytes(size as u64))
            .bench_with_input(
                BenchmarkId::new("enqueue", size),
                &size,
                |b, &_size| {
                    b.iter(|| {
                        for msg in &messages {
                            queue.enqueue(msg.clone()).unwrap();
                        }
                    })
                }
            );
    }
}

/// Benchmark memory queue with different batch sizes
fn bench_memory_queue_batch_sizes(c: &mut Criterion) {
    let batch_sizes = vec![1, 10, 100, 1000, 10000];
    
    for batch_size in batch_sizes {
        let queue = Arc::new(MemoryQueue::new());
        let messages = create_test_messages(batch_size, 1024);
        
        c.benchmark_group("memory_queue_batch_sizes")
            .throughput(Throughput::Elements(batch_size as u64))
            .bench_with_input(
                BenchmarkId::new("batch_enqueue", batch_size),
                &batch_size,
                |b, &_batch_size| {
                    b.iter(|| {
                        for msg in &messages {
                            queue.enqueue(msg.clone()).unwrap();
                        }
                    })
                }
            );
    }
}

/// Benchmark memory tracking and pressure detection
fn bench_memory_tracking(c: &mut Criterion) {
    c.bench_function("memory_tracker_creation", |b| {
        b.iter(|| MemoryTracker::new(64 * 1024 * 1024, Duration::from_secs(1)))
    });
    
    let tracker = MemoryTracker::new(64 * 1024 * 1024, Duration::from_secs(1));
    
    c.bench_function("memory_tracker_update", |b| {
        b.iter(|| {
            tracker.update_usage(1024 * 1024) // 1MB
        })
    });
    
    c.bench_function("memory_tracker_pressure_check", |b| {
        b.iter(|| {
            tracker.get_pressure_level()
        })
    });
    
    c.bench_function("memory_tracker_statistics", |b| {
        b.iter(|| {
            tracker.get_statistics()
        })
    });
}

/// Benchmark backoff algorithm under memory pressure
fn bench_backoff_algorithm(c: &mut Criterion) {
    let strategies = vec![
        ("exponential", BackoffStrategy::Exponential),
        ("linear", BackoffStrategy::Linear),
        ("fixed", BackoffStrategy::Fixed),
    ];
    
    for (name, strategy) in strategies {
        let config = BackoffConfig {
            strategy,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(1000),
            backoff_factor: 2.0,
            jitter: false,
        };
        let algorithm = BackoffAlgorithm::new(config);
        
        c.bench_function(&format!("backoff_algorithm_{}", name), |b| {
            b.iter(|| {
                algorithm.should_backoff(MemoryPressureLevel::High)
            })
        });
    }
}

/// Benchmark memory pressure response under load
fn bench_memory_pressure_response(c: &mut Criterion) {
    let queue = Arc::new(MemoryQueue::new());
    let producer = QueueMessageProducer::new(
        Arc::clone(&queue),
        "BenchmarkProducer".to_string()
    );
    
    // Create different pressure scenarios
    let pressure_levels = vec![
        ("low", 100),      // 100 messages
        ("medium", 1000),  // 1K messages  
        ("high", 10000),   // 10K messages
        ("extreme", 50000), // 50K messages
    ];
    
    for (level_name, message_count) in pressure_levels {
        let messages = create_test_messages(message_count, 2048);
        
        c.benchmark_group("memory_pressure_response")
            .throughput(Throughput::Elements(message_count as u64))
            .bench_with_input(
                BenchmarkId::new("producer_under_pressure", level_name),
                &message_count,
                |b, &_count| {
                    b.iter(|| {
                        for msg in &messages {
                            let _ = producer.produce_message(msg.clone());
                        }
                    })
                }
            );
    }
}

/// Benchmark concurrent memory operations
fn bench_concurrent_memory_ops(c: &mut Criterion) {
    let queue = Arc::new(MemoryQueue::new());
    let messages = create_test_messages(1000, 1024);
    
    c.bench_function("concurrent_memory_ops", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let queue_clone = Arc::clone(&queue);
                    let messages_clone = messages.clone();
                    std::thread::spawn(move || {
                        for msg in messages_clone {
                            let _ = queue_clone.enqueue(msg);
                        }
                    })
                })
                .collect();
            
            for handle in handles {
                handle.join().unwrap();
            }
        })
    });
}

/// Benchmark memory allocation patterns
fn bench_memory_allocation_patterns(c: &mut Criterion) {
    // Test different allocation patterns
    c.bench_function("frequent_small_allocations", |b| {
        b.iter(|| {
            let mut messages = Vec::new();
            for i in 0..1000 {
                messages.push(create_test_messages(1, 256)[0].clone());
            }
            messages
        })
    });
    
    c.bench_function("infrequent_large_allocations", |b| {
        b.iter(|| {
            create_test_messages(10, 65536)
        })
    });
    
    c.bench_function("mixed_allocation_pattern", |b| {
        b.iter(|| {
            let mut all_messages = Vec::new();
            all_messages.extend(create_test_messages(100, 256));   // Small messages
            all_messages.extend(create_test_messages(10, 4096));   // Medium messages  
            all_messages.extend(create_test_messages(1, 65536));   // Large messages
            all_messages
        })
    });
}

criterion_group!(
    memory_benches,
    bench_memory_queue_operations,
    bench_memory_queue_message_sizes,
    bench_memory_queue_batch_sizes,
    bench_memory_tracking,
    bench_backoff_algorithm,
    bench_memory_pressure_response,
    bench_concurrent_memory_ops,
    bench_memory_allocation_patterns
);

criterion_main!(memory_benches);