//! End-to-End Integration Performance Benchmarks
//!
//! Measures complete workflow performance from CLI parsing through scanning
//! to output generation, including configuration loading and plugin processing.

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tempfile::TempDir;
use git2::Repository;

use gstats::cli::{Args, args::parse_args_from};
use gstats::config::ConfigManager;
use gstats::git::RepositoryHandle;
use gstats::scanner::{ScannerConfig, ScanMode, AsyncScannerEngineBuilder};
use gstats::queue::{MemoryQueue, QueueMessageProducer};

/// Create a test repository with realistic content
fn create_realistic_test_repository(file_count: usize, commits: usize) -> (TempDir, String) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path().to_string_lossy().to_string();
    
    let repo = Repository::init(&repo_path).expect("Failed to init repository");
    
    // Configure git user
    let mut config = repo.config().expect("Failed to get config");
    config.set_str("user.name", "Integration Test").expect("Failed to set user.name");
    config.set_str("user.email", "test@example.com").expect("Failed to set user.email");
    
    // Create realistic file structure
    let src_dir = temp_dir.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("Failed to create src dir");
    
    let test_dir = temp_dir.path().join("tests");
    std::fs::create_dir_all(&test_dir).expect("Failed to create tests dir");
    
    // Create files with realistic content
    for i in 0..file_count {
        let file_content = format!(
            r#"//! Module {}
//! 
//! This is a test module for integration benchmarking.

use std::sync::Arc;
use std::collections::HashMap;

/// Test struct for benchmarking
#[derive(Debug, Clone)]
pub struct TestStruct{} {{
    pub id: u64,
    pub name: String,
    pub data: Vec<u8>,
    pub metadata: HashMap<String, String>,
}}

impl TestStruct{} {{
    /// Create a new test struct
    pub fn new(id: u64, name: &str) -> Self {{
        Self {{
            id,
            name: name.to_string(),
            data: vec![0u8; 1024],
            metadata: HashMap::new(),
        }}
    }}
    
    /// Process some data
    pub fn process_data(&mut self, input: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {{
        self.data.extend_from_slice(input);
        Ok(self.data.clone())
    }}
    
    /// Get statistics
    pub fn get_stats(&self) -> (usize, usize) {{
        (self.data.len(), self.metadata.len())
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;
    
    #[test]
    fn test_struct_creation() {{
        let test_struct = TestStruct{}::new(42, "test");
        assert_eq!(test_struct.id, 42);
        assert_eq!(test_struct.name, "test");
    }}
    
    #[test]
    fn test_data_processing() {{
        let mut test_struct = TestStruct{}::new(1, "processor");
        let result = test_struct.process_data(b"hello world");
        assert!(result.is_ok());
    }}
}}
"#,
            i, i, i, i, i, i
        );
        
        let file_path = if i % 4 == 0 {
            test_dir.join(format!("test_{}.rs", i))
        } else {
            src_dir.join(format!("module_{}.rs", i))
        };
        
        std::fs::write(&file_path, file_content).expect("Failed to write file");
    }
    
    // Create commits with realistic history
    for commit_i in 0..commits {
        let mut index = repo.index().expect("Failed to get index");
        
        // Add all files to this commit
        for i in 0..file_count {
            let relative_path = if i % 4 == 0 {
                format!("tests/test_{}.rs", i)
            } else {
                format!("src/module_{}.rs", i)
            };
            
            if std::path::Path::new(&temp_dir.path().join(&relative_path)).exists() {
                index.add_path(&std::path::Path::new(&relative_path))
                    .expect("Failed to add file");
            }
        }
        
        index.write().expect("Failed to write index");
        
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
            &format!("Add modules and tests - commit {}", commit_i),
            &tree,
            &parent_commits.iter().collect::<Vec<_>>(),
        ).expect("Failed to create commit");
    }
    
    (temp_dir, repo_path)
}

/// Benchmark complete CLI argument parsing
fn bench_cli_parsing(c: &mut Criterion) {
    let test_args = vec![
        vec!["gstats", "--scan-mode", "files"],
        vec!["gstats", "--scan-mode", "history", "--max-threads", "4"],
        vec!["gstats", "--scan-mode", "all", "--performance-mode", "--max-memory", "128MB"],
        vec!["gstats", "--repository", "/tmp/test", "--config", "config.toml", "--verbose"],
    ];
    
    for (i, args) in test_args.iter().enumerate() {
        c.bench_function(&format!("cli_parsing_scenario_{}", i), |b| {
            b.iter(|| {
                parse_args_from(args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
            })
        });
    }
}

/// Benchmark configuration loading
fn bench_configuration_loading(c: &mut Criterion) {
    // Create temporary config file
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("gstats.toml");
    
    let config_content = r#"
[scanner]
max-memory = "256MB"
queue-size = 5000
max-threads = 8
performance-mode = true

[logging]
level = "info"
output = "stdout"

[plugins]
enabled = ["commits", "metrics"]
"#;
    std::fs::write(&config_path, config_content).expect("Failed to write config");
    
    c.bench_function("config_loading_from_file", |b| {
        b.iter(|| {
            ConfigManager::from_file(&config_path).unwrap()
        })
    });
    
    c.bench_function("config_loading_default", |b| {
        b.iter(|| {
            ConfigManager::default()
        })
    });
}

/// Benchmark end-to-end scanning workflow
fn bench_end_to_end_workflow(c: &mut Criterion) {
    let repository_sizes = vec![
        ("small", 10, 5),    // 10 files, 5 commits
        ("medium", 50, 20),  // 50 files, 20 commits
        ("large", 200, 50),  // 200 files, 50 commits
    ];
    
    let rt = Arc::new(Runtime::new().unwrap());
    
    for (size_name, file_count, commit_count) in repository_sizes {
        let (_temp_dir, repo_path) = create_realistic_test_repository(file_count, commit_count);
        
        c.benchmark_group("end_to_end_workflow")
            .throughput(Throughput::Elements(file_count as u64))
            .bench_with_input(
                BenchmarkId::new("complete_scan", size_name),
                &(repo_path.clone(), file_count, commit_count),
                |b, (repo_path, _file_count, _commit_count)| {
                    b.iter(|| {
                        rt.block_on(async {
                            // Parse CLI args
                            let args = parse_args_from(vec![
                                "gstats".to_string(),
                                "--repository".to_string(),
                                repo_path.clone(),
                                "--scan-mode".to_string(),
                                "all".to_string(),
                            ]);
                            
                            // Load configuration
                            let config_manager = ConfigManager::default();
                            let scanner_config = config_manager.get_scanner_config().unwrap();
                            
                            // Setup repository and scanner
                            let repo_handle = RepositoryHandle::open(repo_path).unwrap();
                            let memory_queue = Arc::new(MemoryQueue::new());
                            let message_producer = Arc::new(QueueMessageProducer::new(
                                Arc::clone(&memory_queue),
                                "IntegrationProducer".to_string()
                            ));
                            
                            let engine = AsyncScannerEngineBuilder::new()
                                .repository(repo_handle)
                                .config(scanner_config)
                                .message_producer(message_producer)
                                .runtime(Arc::clone(&rt))
                                .build()
                                .unwrap();
                            
                            // Execute scan
                            engine.scan(ScanMode::all()).await.unwrap();
                            
                            // Get final statistics
                            engine.get_stats().await
                        })
                    })
                }
            );
    }
}

/// Benchmark scanner configuration creation and validation
fn bench_scanner_config_integration(c: &mut Criterion) {
    c.bench_function("scanner_config_from_cli", |b| {
        b.iter(|| {
            let args = parse_args_from(vec![
                "gstats".to_string(),
                "--max-threads".to_string(),
                "8".to_string(),
                "--max-memory".to_string(),
                "512MB".to_string(),
                "--performance-mode".to_string(),
            ]);
            
            let config_manager = ConfigManager::default();
            let scanner_config = gstats::cli::converter::args_to_scanner_config(&args, Some(&config_manager));
            scanner_config.unwrap()
        })
    });
}

/// Benchmark memory queue integration with real workloads
fn bench_queue_integration_realistic(c: &mut Criterion) {
    let (_temp_dir, repo_path) = create_realistic_test_repository(100, 25);
    let rt = Arc::new(Runtime::new().unwrap());
    
    c.bench_function("queue_integration_realistic_load", |b| {
        b.iter(|| {
            rt.block_on(async {
                let repo_handle = RepositoryHandle::open(&repo_path).unwrap();
                let config = ScannerConfig::default();
                let memory_queue = Arc::new(MemoryQueue::new());
                let message_producer = Arc::new(QueueMessageProducer::new(
                    Arc::clone(&memory_queue),
                    "RealisticProducer".to_string()
                ));
                
                let engine = AsyncScannerEngineBuilder::new()
                    .repository(repo_handle)
                    .config(config)
                    .message_producer(message_producer)
                    .runtime(Arc::clone(&rt))
                    .build()
                    .unwrap();
                
                // Run files scan
                engine.scan(ScanMode::FILES).await.unwrap();
                
                // Measure queue statistics
                let queue_size = memory_queue.current_size();
                let is_under_pressure = memory_queue.is_under_pressure();
                
                (queue_size, is_under_pressure)
            })
        })
    });
}

/// Benchmark async engine coordination
fn bench_async_engine_coordination(c: &mut Criterion) {
    let (_temp_dir, repo_path) = create_realistic_test_repository(50, 15);
    let rt = Arc::new(Runtime::new().unwrap());
    
    c.bench_function("async_engine_task_coordination", |b| {
        b.iter(|| {
            rt.block_on(async {
                let repo_handle = RepositoryHandle::open(&repo_path).unwrap();
                let config = ScannerConfig::builder()
                    .max_threads(4)
                    .performance_mode(true)
                    .build()
                    .unwrap();
                
                let memory_queue = Arc::new(MemoryQueue::new());
                let message_producer = Arc::new(QueueMessageProducer::new(
                    Arc::clone(&memory_queue),
                    "CoordinationProducer".to_string()
                ));
                
                let engine = AsyncScannerEngineBuilder::new()
                    .repository(repo_handle)
                    .config(config)
                    .message_producer(message_producer)
                    .runtime(Arc::clone(&rt))
                    .build()
                    .unwrap();
                
                // Test concurrent scanning
                let files_task = engine.scan(ScanMode::FILES);
                let history_task = engine.scan(ScanMode::HISTORY);
                
                // Wait for both to complete
                tokio::try_join!(files_task, history_task).unwrap();
                
                engine.get_stats().await
            })
        })
    });
}

/// Benchmark scalability with repository size
fn bench_scalability_repository_size(c: &mut Criterion) {
    let sizes = vec![
        ("tiny", 5, 3),
        ("small", 25, 10),
        ("medium", 100, 30),
        ("large", 500, 100),
    ];
    
    let rt = Arc::new(Runtime::new().unwrap());
    
    for (size_name, file_count, commit_count) in sizes {
        let (_temp_dir, repo_path) = create_realistic_test_repository(file_count, commit_count);
        
        c.benchmark_group("scalability_repository_size")
            .throughput(Throughput::Elements((file_count + commit_count) as u64))
            .bench_with_input(
                BenchmarkId::new("linear_scaling", size_name),
                &(repo_path.clone(), file_count, commit_count),
                |b, (repo_path, file_count, commit_count)| {
                    b.iter(|| {
                        rt.block_on(async {
                            let repo_handle = RepositoryHandle::open(repo_path).unwrap();
                            let config = ScannerConfig::default();
                            let memory_queue = Arc::new(MemoryQueue::new());
                            let message_producer = Arc::new(QueueMessageProducer::new(
                                Arc::clone(&memory_queue),
                                "ScalabilityProducer".to_string()
                            ));
                            
                            let engine = AsyncScannerEngineBuilder::new()
                                .repository(repo_handle)
                                .config(config)
                                .message_producer(message_producer)
                                .runtime(Arc::clone(&rt))
                                .build()
                                .unwrap();
                            
                            let start = std::time::Instant::now();
                            engine.scan(ScanMode::all()).await.unwrap();
                            let duration = start.elapsed();
                            
                            // Performance should scale reasonably
                            let items_per_second = (*file_count + *commit_count) as f64 / duration.as_secs_f64();
                            
                            (duration, items_per_second)
                        })
                    })
                }
            );
    }
}

criterion_group!(
    integration_benches,
    bench_cli_parsing,
    bench_configuration_loading,
    bench_end_to_end_workflow,
    bench_scanner_config_integration,
    bench_queue_integration_realistic,
    bench_async_engine_coordination,
    bench_scalability_repository_size
);

criterion_main!(integration_benches);