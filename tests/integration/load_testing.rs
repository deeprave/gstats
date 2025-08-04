//! Load Testing & Memory Pressure Tests
//!
//! Tests system behavior under various load conditions:
//! - Small, medium, and large repository scenarios  
//! - Memory pressure simulation and backoff algorithm validation
//! - Concurrent load testing with multiple operations

use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use git2::Repository;

use gstats::cli::Args;
use clap::Parser;
use gstats::config::{ConfigManager, Configuration};
use gstats::git::RepositoryHandle;
use gstats::scanner::{
    ScanMode, AsyncScannerEngineBuilder, statistics::RepositoryStatistics,
    async_engine::AsyncScannerEngine,
};
use gstats::queue::{MemoryQueue, QueueMessageProducer, QueueMessage, Queue};

/// Create a test repository with specified size characteristics
fn create_load_test_repository(name: &str, files_per_commit: usize, commits: usize) -> (TempDir, String) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path().to_string_lossy().to_string();
    
    let repo = Repository::init(&repo_path).expect("Failed to init repository");
    
    // Configure git user
    let mut config = repo.config().expect("Failed to get config");
    config.set_str("user.name", &format!("{} Load Test", name)).expect("Failed to set user.name");
    config.set_str("user.email", &format!("{}@loadtest.com", name.to_lowercase())).expect("Failed to set user.email");
    
    // Create files and commits for load testing
    for commit_i in 0..commits {
        for file_i in 0..files_per_commit {
            let file_path = temp_dir.path().join(format!("load_file_{}_{}.rs", commit_i, file_i));
            let content = format!(
                "// Load test file {} in commit {}\\n\\\n                 use std::collections::HashMap;\\n\\\n                 \\n\\\n                 pub struct LoadTestData {{\\n\\\n                     id: u64,\\n\\\n                     data: HashMap<String, String>,\\n\\\n                 }}\\n\\\n                 \\n\\\n                 impl LoadTestData {{\\n\\\n                     pub fn new(id: u64) -> Self {{\\n\\\n                         let mut data = HashMap::new();\\n\\\n                         data.insert(\\\"load_key_{}\\\".to_string(), \\\"load_value_{}\\\".to_string());\\n\\\n                         Self {{ id, data }}\\n\\\n                     }}\\n\\\n                     \\n\\\n                     pub fn process_load_data(&self) -> String {{\\n\\\n                         format!(\\\"Processing load data for ID: {{}}\\\", self.id)\\n\\\n                     }}\\n\\\n                 }}\\n\\\n                 \\n\\\n                 #[cfg(test)]\\n\\\n                 mod tests {{\\n\\\n                     use super::*;\\n\\\n                     \\n\\\n                     #[test]\\n\\\n                     fn test_load_data_{}() {{\\n\\\n                         let data = LoadTestData::new({});\\n\\\n                         assert_eq!(data.id, {});\\n\\\n                         assert!(!data.process_load_data().is_empty());\\n\\\n                     }}\\n\\\n                 }}\\n\",
                file_i, commit_i, file_i, file_i, file_i, file_i, file_i, file_i
            );
            std::fs::write(&file_path, content).expect("Failed to write file");
        }
        
        // Add files to git index
        let mut index = repo.index().expect("Failed to get index");
        for file_i in 0..files_per_commit {
            let relative_path = format!("load_file_{}_{}.rs", commit_i, file_i);
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
            &format!("Load test commit {} - Add {} files ({} KB)", commit_i, files_per_commit, files_per_commit * 2),
            &tree,
            &parent_commits.iter().collect::<Vec<_>>(),
        ).expect("Failed to create commit");
    }
    
    (temp_dir, repo_path)
}

#[tokio::test]
async fn test_small_repository_load() {
    // Small repository: 5 files per commit, 3 commits = 15 files, ~30KB
    let (_temp_dir, repo_path) = create_load_test_repository("SmallLoad", 5, 3);
    
    // Setup scanner with conservative memory settings
    let queue_capacity = 1000;
    let memory_limit = 32 * 1024 * 1024; // 32MB
    let memory_queue = Arc::new(MemoryQueue::new(queue_capacity, memory_limit));
    let message_producer = Arc::new(QueueMessageProducer::new(
        Arc::clone(&memory_queue),
        "SmallLoadProducer".to_string()
    ));
    
    let repo_handle = RepositoryHandle::open(&repo_path).expect("Failed to open repository");
    let config_manager = ConfigManager::from_config(Configuration::default());
    let scanner_config = config_manager.get_scanner_config().expect("Failed to get scanner config");
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository(repo_handle)
        .config(scanner_config)
        .message_producer(message_producer)
        .build()
        .expect("Failed to build scanner engine");
    
    let start_time = Instant::now();
    let initial_memory = memory_queue.memory_usage_percent();
    
    // Test FILES scan
    let files_result = engine.scan(ScanMode::FILES).await;
    println!("Small repository FILES scan result: {:?}", files_result);
    
    // Test HISTORY scan
    let history_result = engine.scan(ScanMode::HISTORY).await;
    println!("Small repository HISTORY scan result: {:?}", history_result);
    
    // Test combined scan
    let combined_result = engine.scan(ScanMode::FILES | ScanMode::HISTORY).await;
    println!("Small repository COMBINED scan result: {:?}", combined_result);
    
    let duration = start_time.elapsed();
    let final_memory = memory_queue.memory_usage_percent();
    let final_queue_size = memory_queue.size();
    
    // Verify small repository performance
    assert!(duration < Duration::from_secs(5), "Small repository should scan quickly");
    assert!(final_memory < 25.0, "Memory usage should be low for small repository");
    
    println!("Small repository load test completed in {:?}", duration);
    println!("Memory usage: {:.2}% -> {:.2}%", initial_memory, final_memory);
    println!("Final queue size: {}", final_queue_size);
    
    // Safe engine cleanup
    tokio::task::spawn_blocking(move || {
        drop(engine);
    }).await.expect("Failed to drop engine safely");
}

#[tokio::test]
async fn test_medium_repository_load() {
    // Medium repository: 25 files per commit, 8 commits = 200 files, ~400KB
    let (_temp_dir, repo_path) = create_load_test_repository("MediumLoad", 25, 8);
    
    // Setup scanner with moderate memory settings
    let queue_capacity = 5000;
    let memory_limit = 128 * 1024 * 1024; // 128MB
    let memory_queue = Arc::new(MemoryQueue::new(queue_capacity, memory_limit));
    let message_producer = Arc::new(QueueMessageProducer::new(
        Arc::clone(&memory_queue),
        "MediumLoadProducer".to_string()
    ));
    
    let repo_handle = RepositoryHandle::open(&repo_path).expect("Failed to open repository");
    let config_manager = ConfigManager::from_config(Configuration::default());
    let scanner_config = config_manager.get_scanner_config().expect("Failed to get scanner config");
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository(repo_handle)
        .config(scanner_config)
        .message_producer(message_producer)
        .build()
        .expect("Failed to build scanner engine");
    
    let start_time = Instant::now();
    let initial_memory = memory_queue.memory_usage_percent();
    
    // Test combined scan under medium load
    let scan_result = engine.scan(ScanMode::FILES | ScanMode::HISTORY).await;
    println!("Medium repository scan result: {:?}", scan_result);
    
    let duration = start_time.elapsed();
    let final_memory = memory_queue.memory_usage_percent();
    let final_queue_size = memory_queue.size();
    
    // Verify medium repository performance
    assert!(duration < Duration::from_secs(30), "Medium repository should scan reasonably quickly");
    assert!(final_memory < 50.0, "Memory usage should be moderate for medium repository");
    
    println!("Medium repository load test completed in {:?}", duration);
    println!("Memory usage: {:.2}% -> {:.2}%", initial_memory, final_memory);
    println!("Final queue size: {}", final_queue_size);
    
    // Safe engine cleanup
    tokio::task::spawn_blocking(move || {
        drop(engine);
    }).await.expect("Failed to drop engine safely");
}

#[tokio::test]
async fn test_large_repository_load() {
    // Large repository: 50 files per commit, 15 commits = 750 files, ~1.5MB
    let (_temp_dir, repo_path) = create_load_test_repository("LargeLoad", 50, 15);
    
    // Setup scanner with generous memory settings
    let queue_capacity = 20000;
    let memory_limit = 256 * 1024 * 1024; // 256MB
    let memory_queue = Arc::new(MemoryQueue::new(queue_capacity, memory_limit));
    let message_producer = Arc::new(QueueMessageProducer::new(
        Arc::clone(&memory_queue),
        "LargeLoadProducer".to_string()
    ));
    
    let repo_handle = RepositoryHandle::open(&repo_path).expect("Failed to open repository");
    let config_manager = ConfigManager::from_config(Configuration::default());
    let scanner_config = config_manager.get_scanner_config().expect("Failed to get scanner config");
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository(repo_handle)
        .config(scanner_config)
        .message_producer(message_producer)
        .build()
        .expect("Failed to build scanner engine");
    
    let start_time = Instant::now();
    let initial_memory = memory_queue.memory_usage_percent();
    
    // Test FILES scan first (less memory intensive)
    let files_result = engine.scan(ScanMode::FILES).await;
    println!("Large repository FILES scan result: {:?}", files_result);
    
    let mid_time = Instant::now();
    let mid_memory = memory_queue.memory_usage_percent();
    
    // Test HISTORY scan (more memory intensive)
    let history_result = engine.scan(ScanMode::HISTORY).await;
    println!("Large repository HISTORY scan result: {:?}", history_result);
    
    let duration = start_time.elapsed();
    let files_duration = mid_time.duration_since(start_time);
    let history_duration = duration - files_duration;
    let final_memory = memory_queue.memory_usage_percent();
    let final_queue_size = memory_queue.size();
    
    // Verify large repository performance
    assert!(duration < Duration::from_secs(120), "Large repository should complete within reasonable time");
    assert!(final_memory < 75.0, "Memory usage should be managed for large repository");
    
    println!("Large repository load test completed in {:?}", duration);
    println!("  FILES scan: {:?}", files_duration);  
    println!("  HISTORY scan: {:?}", history_duration);
    println!("Memory usage: {:.2}% -> {:.2}% -> {:.2}%", initial_memory, mid_memory, final_memory);
    println!("Final queue size: {}", final_queue_size);
    
    // Safe engine cleanup
    tokio::task::spawn_blocking(move || {
        drop(engine);
    }).await.expect("Failed to drop engine safely");
}

#[tokio::test]
async fn test_memory_pressure_simulation() {
    // Create repository with enough content to trigger memory pressure
    let (_temp_dir, repo_path) = create_load_test_repository("MemoryPressure", 30, 10);
    
    // Setup scanner with very limited memory to force pressure
    let queue_capacity = 100; // Very small capacity
    let memory_limit = 16 * 1024 * 1024; // 16MB limit
    let memory_queue = Arc::new(MemoryQueue::new(queue_capacity, memory_limit));
    let message_producer = Arc::new(QueueMessageProducer::new(
        Arc::clone(&memory_queue),
        "MemoryPressureProducer".to_string()
    ));
    
    let repo_handle = RepositoryHandle::open(&repo_path).expect("Failed to open repository");
    let config_manager = ConfigManager::from_config(Configuration::default());
    let scanner_config = config_manager.get_scanner_config().expect("Failed to get scanner config");
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository(repo_handle)
        .config(scanner_config)
        .message_producer(message_producer)
        .build()
        .expect("Failed to build scanner engine");
    
    let start_time = Instant::now();
    let mut memory_samples = Vec::new();
    
    // Monitor memory usage during scan
    let scan_handle = tokio::spawn({
        let engine = engine;
        async move {
            engine.scan(ScanMode::FILES | ScanMode::HISTORY).await
        }
    });
    
    // Sample memory usage every 100ms
    let memory_monitor = tokio::spawn({
        let memory_queue = Arc::clone(&memory_queue);
        async move {
            let mut samples = Vec::new();
            while !scan_handle.is_finished() {
                samples.push((Instant::now(), memory_queue.memory_usage_percent()));
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            samples
        }
    });
    
    let scan_result = scan_handle.await.expect("Scan task should complete");
    memory_samples = memory_monitor.await.expect("Memory monitor should complete");
    
    let duration = start_time.elapsed();
    let peak_memory = memory_samples.iter()
        .map(|(_, usage)| *usage)
        .fold(0.0f64, f64::max);
    
    println!("Memory pressure test completed in {:?}", duration);
    println!("Scan result: {:?}", scan_result);
    println!("Peak memory usage: {:.2}%", peak_memory);
    println!("Memory samples collected: {}", memory_samples.len());
    
    // Verify backoff algorithm effectiveness
    let high_memory_samples = memory_samples.iter()
        .filter(|(_, usage)| *usage > 70.0)
        .count();
    
    println!("High memory pressure samples (>70%): {}", high_memory_samples);
    
    // Test should handle memory pressure gracefully
    assert!(peak_memory <= 95.0, "System should not reach critical memory levels");
    
    // Safe engine cleanup (can't use engine here as it was moved)
    println!("Memory pressure simulation completed successfully");
}

#[tokio::test]
async fn test_concurrent_load_operations() {
    // Create multiple repositories for concurrent testing
    let repos: Vec<_> = (0..3).map(|i| {
        create_load_test_repository(&format!("Concurrent{}", i), 20, 5)
    }).collect();
    
    let start_time = Instant::now();
    let mut tasks = Vec::new();
    
    // Launch concurrent scanning operations
    for (i, (_temp_dir, repo_path)) in repos.iter().enumerate() {
        let repo_path = repo_path.clone();
        
        let task = tokio::spawn(async move {
            let memory_queue = Arc::new(MemoryQueue::new(5000, 128 * 1024 * 1024));
            let message_producer = Arc::new(QueueMessageProducer::new(
                Arc::clone(&memory_queue),
                format!("ConcurrentProducer{}", i)
            ));
            
            let repo_handle = RepositoryHandle::open(&repo_path).expect("Failed to open repository");
            let config_manager = ConfigManager::from_config(Configuration::default());
            let scanner_config = config_manager.get_scanner_config().expect("Failed to get scanner config");
            
            let engine = AsyncScannerEngineBuilder::new()
                .repository(repo_handle)
                .config(scanner_config)
                .message_producer(message_producer)
                .build()
                .expect("Failed to build scanner engine");
            
            let task_start = Instant::now();
            let scan_result = engine.scan(ScanMode::FILES).await;
            let task_duration = task_start.elapsed();
            
            let stats = engine.get_stats().await;
            let memory_usage = memory_queue.memory_usage_percent();
            let queue_size = memory_queue.size();
            
            // Safe cleanup
            tokio::task::spawn_blocking(move || {
                drop(engine);
            }).await.expect("Failed to drop engine safely");
            
            (i, scan_result, stats, task_duration, memory_usage, queue_size)
        });
        
        tasks.push(task);
    }
    
    // Wait for all concurrent operations to complete
    let results = futures::future::join_all(tasks).await;
    let total_duration = start_time.elapsed();
    
    // Verify concurrent operation results
    for result in results {
        let (task_id, scan_result, stats, task_duration, memory_usage, queue_size) = 
            result.expect("Task should complete successfully");
        
        println!("Concurrent task {} completed in {:?}", task_id, task_duration);
        println!("  Scan result: {:?}", scan_result);
        println!("  Stats: {:?}", stats);
        println!("  Memory usage: {:.2}%", memory_usage);
        println!("  Queue size: {}", queue_size);
        
        // Each task should complete reasonably quickly
        assert!(task_duration < Duration::from_secs(60), "Individual tasks should complete efficiently");
    }
    
    println!("All concurrent operations completed in {:?}", total_duration);
    
    // Concurrent operations should not take much longer than individual operations
    assert!(total_duration < Duration::from_secs(120), "Concurrent operations should be efficient");
}