//! Concurrency and Task Coordination Tests

use crate::scanner::async_engine::error::{ScanResult, ScanError};
use crate::scanner::async_engine::engine::AsyncScannerEngineBuilder;
use crate::scanner::async_engine::task_manager::TaskManager;
use crate::scanner::async_traits::*;
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
use crate::scanner::traits::MessageProducer;
use std::path::PathBuf;
use async_trait::async_trait;
use futures::stream;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Barrier;
use crate::notifications::manager::AsyncNotificationManager;
use crate::plugin::SharedPluginRegistry;

struct ConcurrentScanner {
    name: String,
    message_count: usize,
    work_duration_ms: u64,
    start_barrier: Option<Arc<Barrier>>,
}

#[async_trait]
impl AsyncScanner for ConcurrentScanner {
    fn name(&self) -> &str {
        &self.name
    }
    
    
    async fn scan_async(&self, _repository_path: &std::path::Path) -> ScanResult<ScanMessageStream> {
        let count = self.message_count;
        let work_duration = self.work_duration_ms;
        let barrier = self.start_barrier.clone();
        
        let stream = stream::unfold(0, move |i| {
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
                    MessageHeader::new(i as u64),
                    MessageData::FileInfo {
                        path: format!("concurrent_file_{}.rs", i),
                        size: 1024,
                        lines: 50,
                    },
                );
                
                Some((Ok(message), i + 1))
            }
        });
        
        Ok(Box::pin(stream))
    }
}

struct CountingProducer {
    total_count: Arc<AtomicUsize>,
}

impl CountingProducer {
    fn new() -> Self {
        Self {
            total_count: Arc::new(AtomicUsize::new(0)),
        }
    }
    
    fn get_total_count(&self) -> usize {
        self.total_count.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl MessageProducer for CountingProducer {
    async fn produce_message(&self, _message: ScanMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.total_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    
    fn get_producer_name(&self) -> &str {
        "CountingProducer"
    }
}

#[tokio::test]
async fn test_multiple_concurrent_scanners() {
    let repo_path = PathBuf::from(".");
    let producer = Arc::new(CountingProducer::new());
    let producer_ref = Arc::clone(&producer);
    let notification_manager = Arc::new(AsyncNotificationManager::new());
    
    let plugin_registry = SharedPluginRegistry::new();
    let mut builder = AsyncScannerEngineBuilder::new()
        .repository_path(repo_path)
        .message_producer(producer)
        .notification_manager(notification_manager)
        .plugin_registry(plugin_registry);
    
    // Add multiple scanners
    builder = builder
        .add_scanner(Arc::new(ConcurrentScanner {
            name: "ConcurrentFileProcessor".to_string(),
            message_count: 10,
            work_duration_ms: 10,
            start_barrier: None,
        }))
        .add_scanner(Arc::new(ConcurrentScanner {
            name: "ConcurrentHistoryProcessor".to_string(),
            message_count: 10,
            work_duration_ms: 10,
            start_barrier: None,
        }));
    
    let engine = builder.build().unwrap();
    
    let start = Instant::now();
    engine.scan().await.unwrap();
    let duration = start.elapsed();
    
    // Wait for async message production
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Both scanners should have run concurrently
    assert!(duration.as_millis() < 200); // Should be much less than sequential (200ms)
    
    assert_eq!(producer_ref.get_total_count(), 20);
}

#[tokio::test]
async fn test_task_concurrency_limit() {
    let repo_path = PathBuf::from(".");
    let producer = Arc::new(CountingProducer::new());
    let notification_manager = Arc::new(AsyncNotificationManager::new());
    
    // Create config with limited concurrency
    let mut config = crate::scanner::config::ScannerConfig::default();
    config.max_threads = Some(2); // Limit to 2 concurrent tasks
    
    let plugin_registry = SharedPluginRegistry::new();
    let mut builder = AsyncScannerEngineBuilder::new()
        .repository_path(repo_path)
        .config(config)
        .message_producer(producer)
        .notification_manager(notification_manager)
        .plugin_registry(plugin_registry);
    
    // Add 3 scanners that will try to run concurrently
    // Remove barrier to avoid synchronization issues
    for i in 0..3 {
        builder = builder.add_scanner(Arc::new(ConcurrentScanner {
            name: format!("Scanner{}", i),
            message_count: 2, // Increase message count for better timing measurement
            work_duration_ms: 100,
            start_barrier: None, // Remove barrier
        }));
    }
    
    let engine = builder.build().unwrap();
    
    let start = Instant::now();
    
    // Start scan
    engine.scan().await.unwrap();
    let total_duration = start.elapsed();
    
    // With concurrency limit of 2, the 3 tasks should run as 2+1
    // Each scanner produces 2 messages with 100ms delay = 200ms per scanner
    // With 2 concurrent: first 2 scanners run in parallel (200ms), then 3rd scanner (200ms) = ~400ms total
    // Without limit: all 3 would run in parallel = ~200ms total
    assert!(total_duration.as_millis() >= 350); // Should take longer due to concurrency limit
    assert!(total_duration.as_millis() < 600); // But not too long
}

#[tokio::test]
async fn test_task_manager_resource_limits() {
    let manager = TaskManager::new(2); // Limit to 2 concurrent tasks
    
    let start = Instant::now();
    let mut task_ids = Vec::new();
    
    // Spawn 4 tasks
    for _i in 0..4 {
        let task_id = manager.spawn_task(format!("task-{}", _i), move |_cancel| async move {
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
    let repo_path = PathBuf::from(".");
    let producer = Arc::new(CountingProducer::new());
    let producer_ref = Arc::clone(&producer);
    let notification_manager = Arc::new(AsyncNotificationManager::new());
    
    struct ErrorScanner {
        name: String,
        error_after: usize,
    }
    
    #[async_trait]
    impl AsyncScanner for ErrorScanner {
        fn name(&self) -> &str {
            &self.name
        }
        
        
        async fn scan_async(&self, _repository_path: &std::path::Path) -> ScanResult<ScanMessageStream> {
            let error_after = self.error_after;
            
            let stream = stream::unfold(0, move |i| async move {
                if i == error_after {
                    Some((Err(ScanError::stream("Simulated error")), i + 1))
                } else if i < 5 { // Always produce exactly 5 messages
                    let msg = ScanMessage::new(
                        MessageHeader::new(i as u64),
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
    
    let plugin_registry = SharedPluginRegistry::new();
    let engine = AsyncScannerEngineBuilder::new()
        .repository_path(repo_path)
        .message_producer(producer)
        .notification_manager(notification_manager)
        .plugin_registry(plugin_registry)
        .add_scanner(Arc::new(ErrorScanner {
            name: "GoodScanner".to_string(),
            error_after: 10, // Won't error within 5 messages
        }))
        .add_scanner(Arc::new(ErrorScanner {
            name: "BadScanner".to_string(),
            error_after: 3, // Will error after 3 messages
        }))
        .build()
        .unwrap();
    
    let result = engine.scan().await;
    assert!(result.is_err()); // Should fail due to BadScanner
    
    // Wait for async operations
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Both scanners should have produced messages before error
    assert!(producer_ref.get_total_count() >= 3);
}

#[tokio::test]
async fn test_active_task_tracking() {
    let manager = TaskManager::new(10);
    
    // Spawn several tasks with different durations
    let task1 = manager.spawn_task("task-1".to_string(), |_| async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }).await.unwrap();
    
    let task2 = manager.spawn_task("task-2".to_string(), |_| async {
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok(())
    }).await.unwrap();
    
    // Check active tasks
    tokio::time::sleep(Duration::from_millis(50)).await;
    let active = manager.get_active_tasks();
    assert_eq!(active.len(), 2);
    
    // Verify task info
    let task_ids: Vec<crate::scanner::async_engine::task_manager::TaskId> = active
        .into_iter()
        .map(|(id, _duration)| id)
        .collect();
    
    // Task manager should have both tasks registered
    assert!(task_ids.iter().any(|id| id.as_str() == task1.as_str()));
    assert!(task_ids.iter().any(|id| id.as_str() == task2.as_str()));
    
    // Wait for first task to complete
    manager.wait_for_task(&task1, None).await.unwrap();
    assert_eq!(manager.active_task_count(), 1);
    
    // Wait for second task
    manager.wait_for_task(&task2, None).await.unwrap();
    assert_eq!(manager.active_task_count(), 0);
}