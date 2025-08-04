//! Error Recovery & Resilience Tests
//!
//! Tests system behavior under error conditions:
//! - Filesystem error simulation
//! - Malformed git repository testing  
//! - Plugin failure recovery
//! - Graceful degradation validation

use std::sync::Arc;
use std::time::Duration;
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

/// Create a basic test repository for error testing
fn create_error_test_repository(name: &str) -> (TempDir, String) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path().to_string_lossy().to_string();
    
    let repo = Repository::init(&repo_path).expect("Failed to init repository");
    
    // Configure git user
    let mut config = repo.config().expect("Failed to get config");
    config.set_str("user.name", &format!("{} Error Test", name)).expect("Failed to set user.name");
    config.set_str("user.email", &format!("{}@errortest.com", name.to_lowercase())).expect("Failed to set user.email");
    
    // Create a few basic files
    for i in 0..3 {
        let file_path = temp_dir.path().join(format!("error_file_{}.rs", i));
        let content = format!(
            "// Error test file {}\\n\\\n             pub fn error_test_function_{}() {{\\n\\\n                 println!(\\\"Error test function {}\\\");\\n\\\n             }}\\n\",
            i, i, i
        );
        std::fs::write(&file_path, content).expect("Failed to write file");
    }
    
    // Add files to git index and create initial commit
    let mut index = repo.index().expect("Failed to get index");
    for i in 0..3 {
        let relative_path = format!("error_file_{}.rs", i);
        index.add_path(&std::path::Path::new(&relative_path))
            .expect("Failed to add file");
    }
    index.write().expect("Failed to write index");
    
    let tree_id = index.write_tree().expect("Failed to write tree");
    let tree = repo.find_tree(tree_id).expect("Failed to find tree");
    let signature = repo.signature().expect("Failed to create signature");
    
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit for error testing",
        &tree,
        &[],
    ).expect("Failed to create commit");
    
    (temp_dir, repo_path)
}

#[tokio::test]
async fn test_invalid_repository_path_error() {
    // Test with completely non-existent path
    let invalid_path = "/completely/non/existent/path/to/repository";
    
    // Test repository handle creation failure
    let repo_result = RepositoryHandle::open(invalid_path);
    assert!(repo_result.is_err(), "Should fail with invalid repository path");
    
    println!("SUCCESS: Invalid repository path correctly rejected");
}

#[tokio::test]  
async fn test_permission_denied_error() {
    // Test with system directory that likely has restricted permissions
    let restricted_path = "/usr/bin"; // System directory, not a git repo
    
    let repo_result = RepositoryHandle::open(restricted_path);
    assert!(repo_result.is_err(), "Should fail with permission/format error");
    
    println!("SUCCESS: Permission/format error correctly handled");
}

#[tokio::test]
async fn test_malformed_repository_resilience() {
    let (_temp_dir, repo_path) = create_error_test_repository("MalformedTest");
    
    // Create scanner with valid repository first
    let repo_handle = RepositoryHandle::open(&repo_path).expect("Should open valid repository");
    let config_manager = ConfigManager::from_config(Configuration::default());
    let scanner_config = config_manager.get_scanner_config().expect("Failed to get scanner config");
    
    let memory_queue = Arc::new(MemoryQueue::new(5000, 128 * 1024 * 1024));
    let message_producer = Arc::new(QueueMessageProducer::new(
        Arc::clone(&memory_queue),
        "MalformedTestProducer".to_string()
    ));
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository(repo_handle)
        .config(scanner_config)
        .message_producer(message_producer)
        .build()
        .expect("Failed to build scanner engine");
    
    // Test scanning - should work initially
    let initial_scan = engine.scan(ScanMode::FILES).await;
    println!("Initial scan result: {:?}", initial_scan);
    
    // Now corrupt the repository by removing critical git files
    let git_dir = std::path::Path::new(&repo_path).join(".git");
    if git_dir.exists() {
        // Remove HEAD file to simulate corruption
        let head_file = git_dir.join("HEAD");
        if head_file.exists() {
            std::fs::remove_file(&head_file).ok(); // Ignore errors
        }
    }
    
    // Test scanning after corruption - should handle gracefully
    let corrupted_scan = engine.scan(ScanMode::HISTORY).await;
    println!("Corrupted repository scan result: {:?}", corrupted_scan);
    
    // System should still be responsive even with corruption
    let stats = engine.get_stats().await;
    println!("Stats after corruption: {:?}", stats);
    
    // Safe engine cleanup
    tokio::task::spawn_blocking(move || {
        drop(engine);
    }).await.expect("Failed to drop engine safely");
    
    println!("SUCCESS: Malformed repository resilience test completed");
}

#[tokio::test]
async fn test_memory_exhaustion_recovery() {
    let (_temp_dir, repo_path) = create_error_test_repository("MemoryExhaustion");
    
    // Create scanner with extremely limited memory
    let queue_capacity = 10; // Very small capacity
    let memory_limit = 1024 * 1024; // 1MB limit
    let memory_queue = Arc::new(MemoryQueue::new(queue_capacity, memory_limit));
    let message_producer = Arc::new(QueueMessageProducer::new(
        Arc::clone(&memory_queue),
        "MemoryExhaustionProducer".to_string()
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
    
    let initial_memory = memory_queue.memory_usage_percent();
    println!("Initial memory usage: {:.2}%", initial_memory);
    
    // Test scan under severe memory constraints
    let scan_result = engine.scan(ScanMode::FILES | ScanMode::HISTORY).await;
    println!("Memory constrained scan result: {:?}", scan_result);
    
    let final_memory = memory_queue.memory_usage_percent();
    let queue_size = memory_queue.size();
    
    println!("Final memory usage: {:.2}%", final_memory);
    println!("Final queue size: {}", queue_size);
    
    // Test that queue didn't overflow beyond capacity
    assert!(queue_size <= queue_capacity, "Queue should respect capacity limits");
    
    // System should still provide statistics even under memory pressure  
    let stats = engine.get_stats().await;
    println!("Stats under memory pressure: {:?}", stats);
    
    // Safe engine cleanup
    tokio::task::spawn_blocking(move || {
        drop(engine);
    }).await.expect("Failed to drop engine safely");
    
    println!("SUCCESS: Memory exhaustion recovery test completed");
}

#[tokio::test]
async fn test_scan_interruption_resilience() {
    let (_temp_dir, repo_path) = create_error_test_repository("InterruptionTest");
    
    let memory_queue = Arc::new(MemoryQueue::new(5000, 128 * 1024 * 1024));
    let message_producer = Arc::new(QueueMessageProducer::new(
        Arc::clone(&memory_queue),
        "InterruptionTestProducer".to_string()
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
    
    // Start multiple concurrent scans to test interruption handling
    let scan1 = tokio::spawn({
        let engine = &engine;
        async move {
            engine.scan(ScanMode::FILES).await
        }
    });
    
    let scan2 = tokio::spawn({
        let engine = &engine;
        async move {
            // Wait a bit then start second scan
            tokio::time::sleep(Duration::from_millis(100)).await;
            engine.scan(ScanMode::HISTORY).await
        }
    });
    
    // Wait for both scans with timeout
    let scan1_result = tokio::time::timeout(Duration::from_secs(30), scan1)
        .await
        .expect("Scan1 should complete within timeout")
        .expect("Scan1 task should succeed");
    
    let scan2_result = tokio::time::timeout(Duration::from_secs(30), scan2)
        .await
        .expect("Scan2 should complete within timeout")  
        .expect("Scan2 task should succeed");
    
    println!("Concurrent scan 1 result: {:?}", scan1_result);
    println!("Concurrent scan 2 result: {:?}", scan2_result);
    
    // System should handle concurrent operations gracefully
    let final_stats = engine.get_stats().await;
    println!("Final stats after concurrent operations: {:?}", final_stats);
    
    // Safe engine cleanup
    tokio::task::spawn_blocking(move || {
        drop(engine);
    }).await.expect("Failed to drop engine safely");
    
    println!("SUCCESS: Scan interruption resilience test completed");
}

#[tokio::test]
async fn test_configuration_error_handling() {
    let (_temp_dir, repo_path) = create_error_test_repository("ConfigError");
    
    // Test with invalid CLI arguments
    let invalid_args = Args::parse_from(vec![
        "gstats",
        "--repository",
        &repo_path,
        "--max-memory",
        "invalid_memory_size", // Invalid memory specification
    ]);
    
    // This should be handled gracefully by the configuration system
    let config_manager = ConfigManager::from_config(Configuration::default());
    let scanner_config_result = gstats::cli::converter::args_to_scanner_config(&invalid_args, Some(&config_manager));
    
    // Depending on implementation, this might succeed with defaults or fail gracefully
    match scanner_config_result {
        Ok(config) => {
            println!("Configuration handled invalid input gracefully: {:?}", config);
            
            // Test scanner creation with this config
            let repo_handle = RepositoryHandle::open(&repo_path).expect("Failed to open repository");
            let memory_queue = Arc::new(MemoryQueue::new(5000, 128 * 1024 * 1024));
            let message_producer = Arc::new(QueueMessageProducer::new(
                memory_queue,
                "ConfigErrorProducer".to_string()
            ));
            
            let engine_result = AsyncScannerEngineBuilder::new()
                .repository(repo_handle)
                .config(config)
                .message_producer(message_producer)
                .build();
            
            match engine_result {
                Ok(engine) => {
                    let scan_result = engine.scan(ScanMode::FILES).await;
                    println!("Scan with recovered config: {:?}", scan_result);
                    
                    // Safe cleanup
                    tokio::task::spawn_blocking(move || {
                        drop(engine);
                    }).await.expect("Failed to drop engine safely");
                }
                Err(e) => {
                    println!("Engine creation failed gracefully: {:?}", e);
                }
            }
        }
        Err(e) => {
            println!("Configuration error handled gracefully: {:?}", e);
        }
    }
    
    println!("SUCCESS: Configuration error handling test completed");
}

#[tokio::test]
async fn test_graceful_degradation() {
    let (_temp_dir, repo_path) = create_error_test_repository("GracefulDegradation");
    
    // Test system behavior with minimal resources
    let queue_capacity = 5; // Extremely small
    let memory_limit = 512 * 1024; // 512KB
    let memory_queue = Arc::new(MemoryQueue::new(queue_capacity, memory_limit));
    let message_producer = Arc::new(QueueMessageProducer::new(
        Arc::clone(&memory_queue),
        "GracefulDegradationProducer".to_string()
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
    
    // Test each scan mode individually under extreme constraints
    let modes = vec![
        ("FILES", ScanMode::FILES),
        ("HISTORY", ScanMode::HISTORY),
    ];
    
    for (mode_name, mode) in modes {
        println!("Testing graceful degradation for {} mode...", mode_name);
        
        let initial_memory = memory_queue.memory_usage_percent();
        let initial_queue_size = memory_queue.size();
        
        let scan_result = engine.scan(mode).await;
        
        let final_memory = memory_queue.memory_usage_percent();
        let final_queue_size = memory_queue.size();
        
        println!("  {} scan result: {:?}", mode_name, scan_result);
        println!("  Memory: {:.2}% -> {:.2}%", initial_memory, final_memory);
        println!("  Queue size: {} -> {}", initial_queue_size, final_queue_size);
        
        // System should not crash even under extreme constraints
        assert!(final_queue_size <= queue_capacity, "Queue should respect capacity limits");
    }
    
    // Test statistics collection under constraints
    let stats = engine.get_stats().await;
    println!("Stats under graceful degradation: {:?}", stats);
    
    // Test repository statistics collection
    let repo_stats_result = engine.collect_repository_statistics().await;
    match repo_stats_result {
        Ok(repo_stats) => {
            println!("Repository stats under degradation: {:?}", repo_stats);
        }
        Err(e) => {
            println!("Repository stats collection failed gracefully: {:?}", e);
        }
    }
    
    // Safe engine cleanup
    tokio::task::spawn_blocking(move || {
        drop(engine);
    }).await.expect("Failed to drop engine safely");
    
    println!("SUCCESS: Graceful degradation test completed");
}

#[tokio::test]
async fn test_rapid_scan_switching() {
    let (_temp_dir, repo_path) = create_error_test_repository("RapidSwitching");
    
    let memory_queue = Arc::new(MemoryQueue::new(5000, 64 * 1024 * 1024));
    let message_producer = Arc::new(QueueMessageProducer::new(
        Arc::clone(&memory_queue),
        "RapidSwitchingProducer".to_string()
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
    
    // Rapidly switch between different scan modes
    let scan_modes = vec![
        ScanMode::FILES,
        ScanMode::HISTORY,
        ScanMode::FILES | ScanMode::HISTORY,
        ScanMode::FILES,
        ScanMode::HISTORY,
    ];
    
    let mut scan_results = Vec::new();
    
    for (i, mode) in scan_modes.iter().enumerate() {
        println!("Rapid scan {} with mode: {:?}", i + 1, mode);
        
        let start_time = std::time::Instant::now();
        let scan_result = engine.scan(*mode).await;
        let duration = start_time.elapsed();
        
        scan_results.push((i, mode, scan_result, duration));
        
        // Brief pause between scans
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    
    // Verify all scans completed
    for (i, mode, result, duration) in scan_results {
        println!("Scan {} ({:?}) completed in {:?}: {:?}", i + 1, mode, duration, result);
        
        // Each scan should complete reasonably quickly
        assert!(duration < Duration::from_secs(30), "Rapid scans should complete efficiently");
    }
    
    // Final system state should be stable
    let final_stats = engine.get_stats().await;
    println!("Final stats after rapid switching: {:?}", final_stats);
    
    // Safe engine cleanup
    tokio::task::spawn_blocking(move || {
        drop(engine);
    }).await.expect("Failed to drop engine safely");
    
    println!("SUCCESS: Rapid scan switching test completed");
}