//! Async Performance Validation Tests
//!
//! Tests for validating async performance characteristics including
//! responsiveness under concurrent loads and task coordination.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use tempfile::TempDir;
use git2::Repository;

use gstats::scanner::{ScannerConfig, ScanMode, AsyncScannerEngineBuilder};
use gstats::git::RepositoryHandle;
use gstats::queue::{MemoryQueue, QueueMessageProducer};

/// Create a test repository for async performance testing
fn create_async_test_repository(files: usize, commits: usize) -> (TempDir, String) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path().to_string_lossy().to_string();
    
    let repo = Repository::init(&repo_path).expect("Failed to init repository");
    
    // Configure git user
    let mut config = repo.config().expect("Failed to get config");
    config.set_str("user.name", "Async Test").expect("Failed to set user.name");
    config.set_str("user.email", "async@test.com").expect("Failed to set user.email");
    
    // Create files and commits
    for commit_i in 0..commits {
        for file_i in 0..files {
            let file_path = temp_dir.path().join(format!("async_file_{}_{}.rs", commit_i, file_i));
            let content = format!(
                "// Async test file {} in commit {}\npub fn test_function_{}() {{\n    // Implementation\n}}\n",
                file_i, commit_i, file_i
            );
            std::fs::write(&file_path, content).expect("Failed to write file");
        }
        
        // Add all files for this commit
        let mut index = repo.index().expect("Failed to get index");
        for file_i in 0..files {
            let relative_path = format!("async_file_{}_{}.rs", commit_i, file_i);
            index.add_path(&std::path::Path::new(&relative_path))
                .expect("Failed to add file");
        }
        index.write().expect("Failed to write index");
        
        // Create commit
        let tree_id = index.write_tree().expect("Failed to write tree");
        let tree = repo.find_tree(tree_id).expect("Failed to find tree");
        let signature = repo.signature().expect("Failed to create signature");
        
        let parent_commits = if commit_i == 0 {
            vec![]
        } else {
            vec![repo.head().unwrap().peel_to_commit().unwrap()]
        };
        
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &format!("Async test commit {}", commit_i),
            &tree,
            &parent_commits.iter().collect::<Vec<_>>(),
        ).expect("Failed to create commit");
    }
    
    (temp_dir, repo_path)
}

#[tokio::test]
async fn test_async_scanner_responsiveness() {
    let (_temp_dir, repo_path) = create_async_test_repository(50, 10);
    let rt = Arc::new(Runtime::new().unwrap());
    
    let repo_handle = RepositoryHandle::open(&repo_path).unwrap();
    let config = ScannerConfig::default();
    let memory_queue = Arc::new(MemoryQueue::new(10000, 256 * 1024 * 1024));
    let message_producer = Arc::new(QueueMessageProducer::new(
        Arc::clone(&memory_queue),
        "ResponsivenessProducer".to_string()
    ));
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository(repo_handle)
        .config(config)
        .message_producer(message_producer)
        .runtime(rt)
        .build()
        .unwrap();
    
    // Test that scanning operations remain responsive
    let start = Instant::now();
    
    // Start a scan operation
    let scan_task = tokio::spawn(async move {
        engine.scan(ScanMode::all()).await;
    });
    
    // Verify that we can still do other async work during scanning
    let concurrent_work = tokio::spawn(async {
        for i in 0..100 {
            tokio::task::yield_now().await;
            if i % 10 == 0 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
        "concurrent_work_completed"
    });
    
    // Both tasks should complete without blocking each other
    let (scan_result, work_result) = tokio::join!(scan_task, concurrent_work);
    
    let duration = start.elapsed();
    
    assert!(scan_result.is_ok(), "Scan task should complete successfully");
    let _ = scan_result.unwrap(); // Should be ()
    assert_eq!(work_result.unwrap(), "concurrent_work_completed");
    
    // Should remain responsive (not block completely)
    assert!(duration < Duration::from_secs(30), "Operations should complete within 30 seconds");
}

#[tokio::test]
async fn test_concurrent_scanner_load() {
    let (_temp_dir, repo_path) = create_async_test_repository(30, 8);
    let rt = Arc::new(Runtime::new().unwrap());
    
    // Create multiple concurrent scanner engines
    let concurrent_scanners = 4;
    let mut tasks = Vec::new();
    
    for i in 0..concurrent_scanners {
        let repo_handle = RepositoryHandle::open(&repo_path).unwrap();
        let config = ScannerConfig::builder()
            .max_threads(2)
            .build()
            .unwrap();
        let memory_queue = Arc::new(MemoryQueue::new(10000, 256 * 1024 * 1024));
        let message_producer = Arc::new(QueueMessageProducer::new(
            memory_queue,
            format!("ConcurrentProducer{}", i)
        ));
        
        let engine = AsyncScannerEngineBuilder::new()
            .repository(repo_handle)
            .config(config)
            .message_producer(message_producer)
            .runtime(Arc::clone(&rt))
            .build()
            .unwrap();
        
        let task = tokio::spawn(async move {
            let start = Instant::now();
            engine.scan(ScanMode::FILES).await;
            let duration = start.elapsed();
            (i, (), duration)
        });
        
        tasks.push(task);
    }
    
    // Wait for all concurrent operations to complete
    let start = Instant::now();
    let results = futures::future::join_all(tasks).await;
    let total_duration = start.elapsed();
    
    // Verify all scanners completed successfully
    for (i, result) in results.into_iter().enumerate() {
        let (scanner_id, scan_result, duration) = result.unwrap();
        assert_eq!(scanner_id, i);
        assert_eq!(scan_result, ()); // scan() returns ()
        assert!(duration < Duration::from_secs(20), "Scanner {} took too long: {:?}", i, duration);
    }
    
    // Concurrent execution should not take much longer than sequential
    assert!(total_duration < Duration::from_secs(30), 
        "Concurrent scanners took too long: {:?}", total_duration);
}

#[tokio::test]
async fn test_async_task_coordination() {
    let (_temp_dir, repo_path) = create_async_test_repository(40, 12);
    let rt = Arc::new(Runtime::new().unwrap());
    
    let repo_handle = RepositoryHandle::open(&repo_path).unwrap();
    let config = ScannerConfig::builder()
        .max_threads(4)
        .performance_mode(true)
        .build()
        .unwrap();
    let memory_queue = Arc::new(MemoryQueue::new(10000, 256 * 1024 * 1024));
    let message_producer = Arc::new(QueueMessageProducer::new(
        memory_queue,
        "CoordinationProducer".to_string()
    ));
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository(repo_handle)
        .config(config)
        .message_producer(message_producer)
        .runtime(rt)
        .build()
        .unwrap();
    
    // Test coordination of multiple scan modes
    let start = Instant::now();
    
    let files_scan = engine.scan(ScanMode::FILES);
    let history_scan = engine.scan(ScanMode::HISTORY);
    
    // Both scans should coordinate properly
    let (files_result, history_result) = tokio::try_join!(files_scan, history_scan)
        .expect("Both scans should complete successfully");
    
    let duration = start.elapsed();
    
    // scan() returns (), so both results should be ()
    assert_eq!(files_result, ());
    assert_eq!(history_result, ());
    
    // Coordinated execution should be efficient
    assert!(duration < Duration::from_secs(25), 
        "Coordinated scans took too long: {:?}", duration);
    
    // Verify engine statistics show coordination worked
    let stats = engine.get_stats().await;
    assert!(stats.completed_tasks >= 2, "Should have completed at least 2 tasks");
}

#[tokio::test]
async fn test_backpressure_handling() {
    let (_temp_dir, repo_path) = create_async_test_repository(100, 20);
    let rt = Arc::new(Runtime::new().unwrap());
    
    let repo_handle = RepositoryHandle::open(&repo_path).unwrap();
    let config = ScannerConfig::builder()
        .max_threads(2)
        .with_max_memory(32 * 1024 * 1024) // 32MB limit
        .with_queue_size(1000)
        .build()
        .unwrap();
    let memory_queue = Arc::new(MemoryQueue::new(10000, 256 * 1024 * 1024));
    let message_producer = Arc::new(QueueMessageProducer::new(
        memory_queue,
        "BackpressureProducer".to_string()
    ));
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository(repo_handle)
        .config(config)
        .message_producer(message_producer)
        .runtime(rt)
        .build()
        .unwrap();
    
    // Start scanning operation that might trigger backpressure
    let start = Instant::now();
    engine.scan(ScanMode::all()).await;
    let duration = start.elapsed();
    
    // Should not take excessively long due to backpressure
    assert!(duration < Duration::from_secs(45), 
        "Backpressure handling took too long: {:?}", duration);
    
    let stats = engine.get_stats().await;
    println!("Backpressure test stats: {:?}", stats);
}

#[tokio::test]
async fn test_async_cancellation_safety() {
    let (_temp_dir, repo_path) = create_async_test_repository(60, 15);
    let rt = Arc::new(Runtime::new().unwrap());
    
    let repo_handle = RepositoryHandle::open(&repo_path).unwrap();
    let config = ScannerConfig::default();
    let memory_queue = Arc::new(MemoryQueue::new(10000, 256 * 1024 * 1024));
    let message_producer = Arc::new(QueueMessageProducer::new(
        memory_queue,
        "CancellationProducer".to_string()
    ));
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository(repo_handle)
        .config(config)
        .message_producer(message_producer)
        .runtime(rt)
        .build()
        .unwrap();
    
    // Start a scan operation and cancel it
    let scan_task = tokio::spawn(async move {
        engine.scan(ScanMode::all()).await
    });
    
    // Let it run for a short time then cancel
    tokio::time::sleep(Duration::from_millis(100)).await;
    scan_task.abort();
    
    // Cancellation should be handled gracefully
    let result = scan_task.await;
    assert!(result.is_err()); // Should be cancelled
    assert!(result.unwrap_err().is_cancelled());
    
    // System should remain in a consistent state after cancellation
    // (This test mainly ensures no panics or deadlocks occur)
}

#[tokio::test]
async fn test_async_performance_under_load() {
    let (_temp_dir, repo_path) = create_async_test_repository(80, 25);
    let rt = Arc::new(Runtime::new().unwrap());
    
    // Test performance characteristics under various loads
    let load_scenarios = vec![
        ("light", 1, ScanMode::FILES),
        ("moderate", 2, ScanMode::FILES | ScanMode::HISTORY),
        ("heavy", 4, ScanMode::FILES | ScanMode::HISTORY | ScanMode::METRICS),
    ];
    
    for (scenario_name, concurrent_scans, scan_mode) in load_scenarios {
        let mut tasks = Vec::new();
        let start = Instant::now();
        
        for i in 0..concurrent_scans {
            let repo_handle = RepositoryHandle::open(&repo_path).unwrap();
            let config = ScannerConfig::builder()
                .max_threads(2)
                .performance_mode(true)
                .build()
                .unwrap();
            let memory_queue = Arc::new(MemoryQueue::new(10000, 256 * 1024 * 1024));
            let message_producer = Arc::new(QueueMessageProducer::new(
                memory_queue,
                format!("LoadProducer{}_{}", scenario_name, i)
            ));
            
            let engine = AsyncScannerEngineBuilder::new()
                .repository(repo_handle)
                .config(config)
                .message_producer(message_producer)
                .runtime(Arc::clone(&rt))
                .build()
                .unwrap();
            
            let task = tokio::spawn(async move {
                engine.scan(scan_mode).await;
                ()
            });
            
            tasks.push(task);
        }
        
        // Wait for all tasks to complete
        let results = futures::future::join_all(tasks).await;
        let duration = start.elapsed();
        
        // Verify all completed successfully
        for (i, result) in results.into_iter().enumerate() {
            assert!(result.is_ok(), "Task {} in scenario {} failed", i, scenario_name);
            assert_eq!(result.unwrap(), (), "Scan {} in scenario {} should return ()", i, scenario_name);
        }
        
        // Performance should degrade gracefully under load
        let max_expected_duration = Duration::from_secs(20 + (concurrent_scans * 10) as u64);
        assert!(duration < max_expected_duration, 
            "Scenario {} took too long: {:?}", scenario_name, duration);
        
        println!("Scenario {} ({} concurrent): {:?}", scenario_name, concurrent_scans, duration);
    }
}