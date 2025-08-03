//! Concurrency and Task Coordination Tests

use crate::scanner::async_engine::*;
use crate::scanner::async_traits::*;
use crate::scanner::modes::ScanMode;
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
use crate::scanner::traits::MessageProducer;
use crate::git::RepositoryHandle;
use async_trait::async_trait;
use futures::stream;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Barrier;

struct ConcurrentScanner {
    name: String,
    supported_modes: ScanMode,
    message_count: usize,
    work_duration_ms: u64,
    start_barrier: Option<Arc<Barrier>>,
}

#[async_trait]
impl AsyncScanner for ConcurrentScanner {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn supports_mode(&self, mode: ScanMode) -> bool {
        self.supported_modes.contains(mode)
    }
    
    async fn scan_async(&self, mode: ScanMode) -> ScanResult<ScanMessageStream> {
        let count = self.message_count;
        let work_duration = self.work_duration_ms;
        let barrier = self.start_barrier.clone();
        
        let stream = stream::unfold((0, mode), move |(i, mode)| {
            let barrier = barrier.clone();
            async move {
                if i >= count {
                    return None;
                }
                
                // Wait for barrier if provided (for synchronization tests)
                if i == 0 {
                    if let Some(b) = barrier {
                        b.wait().await;
                    }
                }
                
                // Simulate work
                tokio::time::sleep(Duration::from_millis(work_duration)).await;
                
                let message = ScanMessage::new(
                    MessageHeader::new(mode, i as u64),
                    MessageData::FileInfo {
                        path: format!("concurrent_file_{}.rs", i),
                        size: 1024,
                        lines: 50,
                    },
                );
                
                Some((Ok(message), (i + 1, mode)))
            }
        });
        
        Ok(Box::pin(stream))
    }
}

struct CountingProducer {
    total_count: Arc<AtomicUsize>,
    mode_counts: Arc<dashmap::DashMap<ScanMode, AtomicUsize>>,
}

impl CountingProducer {
    fn new() -> Self {
        Self {
            total_count: Arc::new(AtomicUsize::new(0)),
            mode_counts: Arc::new(dashmap::DashMap::new()),
        }
    }
    
    fn get_total_count(&self) -> usize {
        self.total_count.load(Ordering::Relaxed)
    }
    
    fn get_mode_count(&self, mode: ScanMode) -> usize {
        self.mode_counts
            .get(&mode)
            .map(|counter| counter.load(Ordering::Relaxed))
            .unwrap_or(0)
    }
}

impl MessageProducer for CountingProducer {
    fn produce_message(&self, message: ScanMessage) {
        self.total_count.fetch_add(1, Ordering::Relaxed);
        
        self.mode_counts
            .entry(message.header.scan_mode)
            .or_insert_with(|| AtomicUsize::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }
    
    fn get_producer_name(&self) -> &str {
        "CountingProducer"
    }
}

#[tokio::test]
async fn test_multiple_concurrent_scanners() {
    let repo = RepositoryHandle::open(".").unwrap();
    let producer = Arc::new(CountingProducer::new());
    let producer_ref = Arc::clone(&producer);
    
    let mut builder = AsyncScannerEngineBuilder::new()
        .repository(repo)
        .message_producer(producer);
    
    // Add multiple scanners for different modes
    builder = builder
        .add_scanner(Arc::new(ConcurrentScanner {
            name: "FileScanner".to_string(),
            supported_modes: ScanMode::FILES,
            message_count: 10,
            work_duration_ms: 10,
            start_barrier: None,
        }))
        .add_scanner(Arc::new(ConcurrentScanner {
            name: "HistoryScanner".to_string(),
            supported_modes: ScanMode::HISTORY,
            message_count: 10,
            work_duration_ms: 10,
            start_barrier: None,
        }));
    
    let engine = builder.build().unwrap();
    
    let start = Instant::now();
    engine.scan(ScanMode::FILES | ScanMode::HISTORY).await.unwrap();
    let duration = start.elapsed();
    
    // Wait for async message production
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Both scanners should have run concurrently
    assert!(duration.as_millis() < 200); // Should be much less than sequential (200ms)
    
    assert_eq!(producer_ref.get_total_count(), 20);
    assert_eq!(producer_ref.get_mode_count(ScanMode::FILES), 10);
    assert_eq!(producer_ref.get_mode_count(ScanMode::HISTORY), 10);
}

#[tokio::test]
async fn test_task_concurrency_limit() {
    let repo = RepositoryHandle::open(".").unwrap();
    let producer = Arc::new(CountingProducer::new());
    
    // Create config with limited concurrency
    let mut config = crate::scanner::config::ScannerConfig::default();
    config.max_threads = Some(2); // Limit to 2 concurrent tasks
    
    let barrier = Arc::new(Barrier::new(4)); // 3 scanners + test thread
    
    let mut builder = AsyncScannerEngineBuilder::new()
        .repository(repo)
        .config(config)
        .message_producer(producer);
    
    // Add 3 scanners that will try to run concurrently
    for i in 0..3 {
        builder = builder.add_scanner(Arc::new(ConcurrentScanner {
            name: format!("Scanner{}", i),
            supported_modes: ScanMode::from_bits(1 << i).unwrap(),
            message_count: 1,
            work_duration_ms: 100,
            start_barrier: Some(Arc::clone(&barrier)),
        }));
    }
    
    let engine = builder.build().unwrap();
    
    let start = Instant::now();
    
    // Start scan in background
    let scan_handle = tokio::spawn(async move {
        engine.scan(ScanMode::all()).await
    });
    
    // Wait for all tasks to be ready
    barrier.wait().await;
    let after_barrier = start.elapsed();
    
    // Complete the scan
    scan_handle.await.unwrap().unwrap();
    let total_duration = start.elapsed();
    
    // With concurrency limit of 2, the 3 tasks should run as 2+1
    // So total time should be ~200ms (two phases of 100ms each)
    assert!(after_barrier.as_millis() < 50); // Barrier should be reached quickly
    assert!(total_duration.as_millis() >= 180); // At least 2 phases
    assert!(total_duration.as_millis() < 320); // But not 3 sequential phases
}

#[tokio::test]
async fn test_task_manager_resource_limits() {
    let manager = TaskManager::new(2); // Limit to 2 concurrent tasks
    
    let start = Instant::now();
    let mut task_ids = Vec::new();
    
    // Spawn 4 tasks
    for i in 0..4 {
        let task_id = manager.spawn_task(ScanMode::FILES, move |_cancel| async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(())
        }).await.unwrap();
        
        task_ids.push(task_id);
    }
    
    // Should have spawned all 4, but only 2 running concurrently
    let spawn_duration = start.elapsed();
    assert!(spawn_duration.as_millis() < 20); // Spawning should be fast
    
    // Wait for all tasks
    for task_id in task_ids {
        manager.wait_for_task(&task_id, None).await.unwrap();
    }
    
    let total_duration = start.elapsed();
    
    // With limit of 2, should take ~100ms (2 batches of 50ms)
    assert!(total_duration.as_millis() >= 90);
    assert!(total_duration.as_millis() < 150);
    
    assert_eq!(manager.completed_task_count().await, 4);
}

#[tokio::test]
async fn test_concurrent_error_handling() {
    let repo = RepositoryHandle::open(".").unwrap();
    let producer = Arc::new(CountingProducer::new());
    let producer_ref = Arc::clone(&producer);
    
    struct ErrorScanner {
        name: String,
        mode: ScanMode,
        error_after: usize,
    }
    
    #[async_trait]
    impl AsyncScanner for ErrorScanner {
        fn name(&self) -> &str {
            &self.name
        }
        
        fn supports_mode(&self, mode: ScanMode) -> bool {
            self.mode == mode
        }
        
        async fn scan_async(&self, mode: ScanMode) -> ScanResult<ScanMessageStream> {
            let error_after = self.error_after;
            
            let stream = stream::unfold(0, move |i| async move {
                if i == error_after {
                    Some((Err(ScanError::stream("Simulated error")), i + 1))
                } else if i < error_after + 5 {
                    let msg = ScanMessage::new(
                        MessageHeader::new(mode, i as u64),
                        MessageData::FileInfo {
                            path: format!("file_{}.rs", i),
                            size: 1024,
                            lines: 50,
                        },
                    );
                    Some((Ok(msg), i + 1))
                } else {
                    None
                }
            });
            
            Ok(Box::pin(stream))
        }
    }
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository(repo)
        .message_producer(producer)
        .add_scanner(Arc::new(ErrorScanner {
            name: "GoodScanner".to_string(),
            mode: ScanMode::FILES,
            error_after: 100, // Won't error
        }))
        .add_scanner(Arc::new(ErrorScanner {
            name: "BadScanner".to_string(),
            mode: ScanMode::HISTORY,
            error_after: 3, // Will error after 3 messages
        }))
        .build()
        .unwrap();
    
    let result = engine.scan(ScanMode::FILES | ScanMode::HISTORY).await;
    assert!(result.is_err()); // Should fail due to BadScanner
    
    // Wait for async operations
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // GoodScanner should have produced 5 messages
    // BadScanner should have produced 3 messages before error
    assert_eq!(producer_ref.get_mode_count(ScanMode::FILES), 5);
    assert_eq!(producer_ref.get_mode_count(ScanMode::HISTORY), 3);
}

#[tokio::test]
async fn test_active_task_tracking() {
    let manager = TaskManager::new(10);
    
    // Spawn several tasks with different durations
    let task1 = manager.spawn_task(ScanMode::FILES, |_| async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }).await.unwrap();
    
    let task2 = manager.spawn_task(ScanMode::HISTORY, |_| async {
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok(())
    }).await.unwrap();
    
    // Check active tasks
    tokio::time::sleep(Duration::from_millis(50)).await;
    let active = manager.get_active_tasks();
    assert_eq!(active.len(), 2);
    
    // Verify task info
    let task_info: std::collections::HashMap<_, _> = active
        .into_iter()
        .map(|(id, mode, _duration)| (id, mode))
        .collect();
    
    assert_eq!(task_info.get(&task1), Some(&ScanMode::FILES));
    assert_eq!(task_info.get(&task2), Some(&ScanMode::HISTORY));
    
    // Wait for first task to complete
    manager.wait_for_task(&task1, None).await.unwrap();
    assert_eq!(manager.active_task_count(), 1);
    
    // Wait for second task
    manager.wait_for_task(&task2, None).await.unwrap();
    assert_eq!(manager.active_task_count(), 0);
}