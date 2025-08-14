//! Scanner Performance Benchmarks
//!
//! Measures core scanner operations including file scanning, commit analysis,
//! and async engine performance under various repository sizes.

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tempfile::TempDir;
use git2::Repository;

use gstats::scanner::{
    ScannerConfig, AsyncScannerEngineBuilder,
    async_engine::repository::AsyncRepositoryHandle,
};
use gstats::git::RepositoryHandle;
use gstats::scanner::traits::QueueMessageProducer;

/// Create a test repository with specified number of commits
fn create_test_repository(commit_count: usize) -> (TempDir, String) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path().to_string_lossy().to_string();
    
    let repo = Repository::init(&repo_path).expect("Failed to init repository");
    
    // Configure git user for commits
    let mut config = repo.config().expect("Failed to get config");
    config.set_str("user.name", "Benchmark User").expect("Failed to set user.name");
    config.set_str("user.email", "benchmark@example.com").expect("Failed to set user.email");
    
    // Create commits
    for i in 0..commit_count {
        let file_path = temp_dir.path().join(format!("file_{}.txt", i));
        std::fs::write(&file_path, format!("Content of file {}", i))
            .expect("Failed to write file");
        
        // Add file to index
        let mut index = repo.index().expect("Failed to get index");
        index.add_path(&std::path::Path::new(&format!("file_{}.txt", i)))
            .expect("Failed to add file");
        index.write().expect("Failed to write index");
        
        // Create commit
        let tree_id = index.write_tree().expect("Failed to write tree");
        let tree = repo.find_tree(tree_id).expect("Failed to find tree");
        let signature = repo.signature().expect("Failed to create signature");
        
        let parent_commits = if i == 0 {
            vec![]
        } else {
            vec![repo.head().unwrap().peel_to_commit().unwrap()]
        };
        
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &format!("Commit {}", i),
            &tree,
            &parent_commits.iter().collect::<Vec<_>>(),
        ).expect("Failed to create commit");
    }
    
    (temp_dir, repo_path)
}

/// Benchmark scanner configuration creation
fn bench_scanner_config_creation(c: &mut Criterion) {
    c.bench_function("scanner_config_default", |b| {
        b.iter(|| ScannerConfig::default())
    });
    
    c.bench_function("scanner_config_builder", |b| {
        b.iter(|| {
            ScannerConfig::builder()
                .max_threads(4)
                .chunk_size(1000)
                .buffer_size(8192)
                .performance_mode(true)
                .with_max_memory(128 * 1024 * 1024)
                .build()
                .unwrap()
        })
    });
}

/// Benchmark repository handle operations
fn bench_repository_operations(c: &mut Criterion) {
    let (_temp_dir, repo_path) = create_test_repository(10);
    
    c.bench_function("repository_handle_open", |b| {
        b.iter(|| RepositoryHandle::open(&repo_path).unwrap())
    });
    
    let repo_handle = RepositoryHandle::open(&repo_path).unwrap();
    let async_repo = AsyncRepositoryHandle::new(repo_handle);
    
    let rt = Runtime::new().unwrap();
    c.bench_function("async_repository_stats", |b| {
        b.iter(|| {
            rt.block_on(async {
                async_repo.get_repository_stats().await.unwrap()
            })
        })
    });
    
    c.bench_function("async_repository_commit_count", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Get repository stats for benchmarking - object_count gives us a proxy for complexity
                async_repo.get_repository_stats().await.unwrap().object_count
            })
        })
    });
}

/// Benchmark scanner engine creation
fn bench_scanner_engine_creation(c: &mut Criterion) {
    let (_temp_dir, repo_path) = create_test_repository(10);
    let repo_handle = RepositoryHandle::open(&repo_path).unwrap();
    let config = ScannerConfig::default();
    
    let rt = Arc::new(Runtime::new().unwrap());
    let queue = gstats::queue::SharedMessageQueue::new("benchmark-scan".to_string());
    let message_producer = Arc::new(QueueMessageProducer::new(
        queue,
        "BenchmarkProducer".to_string()
    ));
    
    c.bench_function("async_scanner_engine_creation", |b| {
        b.iter(|| {
            AsyncScannerEngineBuilder::new()
                .repository(repo_handle.clone())
                .config(config.clone())
                .message_producer(message_producer.clone())
                .runtime(Arc::clone(&rt))
                .build()
                .unwrap()
        })
    });
}

/// Benchmark scanner performance with different repository sizes
fn bench_scanner_repository_sizes(c: &mut Criterion) {
    let rt = Arc::new(Runtime::new().unwrap());
    
    let sizes = vec![10, 50, 100, 500];
    
    for size in sizes {
        let (_temp_dir, repo_path) = create_test_repository(size);
        let repo_handle = RepositoryHandle::open(&repo_path).unwrap();
        let config = ScannerConfig::default();
        
        let queue = gstats::queue::SharedMessageQueue::new("benchmark-scan".to_string());
        let message_producer = Arc::new(QueueMessageProducer::new(
            queue,
            "BenchmarkProducer".to_string()
        ));
        
        c.benchmark_group("scanner_repository_sizes")
            .throughput(Throughput::Elements(size as u64))
            .bench_with_input(
                BenchmarkId::new("scan_files", size),
                &size,
                |b, &_size| {
                    let engine = AsyncScannerEngineBuilder::new()
                        .repository(repo_handle.clone())
                        .config(config.clone())
                        .message_producer(message_producer.clone())
                        .runtime(Arc::clone(&rt))
                        .build()
                        .unwrap();
                    
                    b.iter(|| {
                        rt.block_on(async {
                            engine.scan().await.unwrap()
                        })
                    })
                }
            );
    }
}

/// Benchmark complete repository scanning
fn bench_complete_scan(c: &mut Criterion) {
    let (_temp_dir, repo_path) = create_test_repository(100);
    let repo_handle = RepositoryHandle::open(&repo_path).unwrap();
    let config = ScannerConfig::default();
    
    let rt = Arc::new(Runtime::new().unwrap());
    let queue = gstats::queue::SharedMessageQueue::new("benchmark-scan".to_string());
    let message_producer = Arc::new(QueueMessageProducer::new(
        queue,
        "BenchmarkProducer".to_string()
    ));
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository(repo_handle)
        .config(config)
        .message_producer(message_producer)
        .runtime(Arc::clone(&rt))
        .build()
        .unwrap();
    
    c.bench_function("complete_repository_scan", |b| {
        b.iter(|| {
            rt.block_on(async {
                engine.scan().await.unwrap()
            })
        })
    });
}

criterion_group!(
    scanner_benches,
    bench_scanner_config_creation,
    bench_repository_operations,
    bench_scanner_engine_creation,
    bench_scanner_repository_sizes,
    bench_complete_scan
);

criterion_main!(scanner_benches);