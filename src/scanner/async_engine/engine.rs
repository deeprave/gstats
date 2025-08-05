//! Async Scanner Engine Implementation
//! 
//! Core async scanner engine that coordinates multiple scan modes concurrently.

use std::sync::Arc;
#[cfg(not(test))]
use tokio::runtime::Runtime;
use futures::StreamExt;
use crate::scanner::config::ScannerConfig;
use crate::scanner::modes::ScanMode;
use crate::scanner::traits::MessageProducer;
use crate::scanner::async_traits::{AsyncScanner, ScanMessageStream};
use crate::scanner::statistics::{RepositoryStatistics, RepositoryStatsCollector};
use crate::git::RepositoryHandle;
use super::task_manager::TaskManager;
use super::error::{ScanError, ScanResult};

/// Core async scanner engine
pub struct AsyncScannerEngine {
    /// Tokio runtime for async operations
    #[cfg(not(test))]
    #[allow(dead_code)]
    runtime: Arc<Runtime>,
    
    /// Repository handle
    repository: Arc<RepositoryHandle>,
    
    /// Scanner configuration
    #[allow(dead_code)]
    config: ScannerConfig,
    
    /// Task coordination manager
    task_manager: TaskManager,
    
    /// Message producer for queue integration
    message_producer: Arc<dyn MessageProducer + Send + Sync>,
    
    /// Registered scanners
    scanners: Vec<Arc<dyn AsyncScanner>>,
}

impl AsyncScannerEngine {
    /// Create a new async scanner engine
    pub fn new(
        repository: RepositoryHandle,
        config: ScannerConfig,
        message_producer: Arc<dyn MessageProducer + Send + Sync>,
    ) -> ScanResult<Self> {
        // Create runtime with configured thread count
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config.max_threads.unwrap_or_else(num_cpus::get))
            .enable_all()
            .build()
            .map_err(|e| ScanError::configuration(format!("Failed to create runtime: {}", e)))?;
        
        Self::with_runtime(repository, config, message_producer, Arc::new(runtime))
    }
    
    /// Create a new async scanner engine with existing runtime (for tests)
    pub fn with_runtime(
        repository: RepositoryHandle,
        config: ScannerConfig,
        message_producer: Arc<dyn MessageProducer + Send + Sync>,
        runtime: Arc<tokio::runtime::Runtime>,
    ) -> ScanResult<Self> {
        // Create task manager with concurrency limit
        let max_concurrent = config.max_threads.unwrap_or_else(num_cpus::get);
        let task_manager = TaskManager::new(max_concurrent);
        
        Ok(Self {
            #[cfg(not(test))]
            runtime,
            repository: Arc::new(repository),
            config,
            task_manager,
            message_producer,
            scanners: Vec::new(),
        })
    }
    
    /// Create a new async scanner engine for testing (no separate runtime)
    #[cfg(test)]
    pub fn new_for_test(
        repository: RepositoryHandle,
        config: ScannerConfig,
        message_producer: Arc<dyn MessageProducer + Send + Sync>,
    ) -> ScanResult<Self> {
        // Create task manager with concurrency limit
        let max_concurrent = config.max_threads.unwrap_or_else(num_cpus::get);
        let task_manager = TaskManager::new(max_concurrent);
        
        Ok(Self {
            repository: Arc::new(repository),
            config,
            task_manager,
            message_producer,
            scanners: Vec::new(),
        })
    }
    
    /// Register a scanner with the engine
    pub fn register_scanner(&mut self, scanner: Arc<dyn AsyncScanner>) {
        self.scanners.push(scanner);
    }
    
    /// Execute scan with specified modes
    pub async fn scan(&self, modes: ScanMode) -> ScanResult<()> {
        if self.scanners.is_empty() {
            return Err(ScanError::no_scanners_registered());
        }
        
        // Find scanners that support the requested modes
        let mut mode_scanners: Vec<(ScanMode, Arc<dyn AsyncScanner>)> = Vec::new();
        
        for mode in modes.iter() {
            let mut found = false;
            for scanner in &self.scanners {
                if scanner.supports_mode(mode) {
                    mode_scanners.push((mode, Arc::clone(scanner)));
                    found = true;
                    break;
                }
            }
            
            if !found {
                log::debug!("Mode {:?} will be handled by plugin processing", mode);
            }
        }
        
        if mode_scanners.is_empty() {
            return Err(ScanError::InvalidMode(modes));
        }
        
        // Spawn tasks for each mode
        let mut tasks = Vec::new();
        
        for (mode, scanner) in mode_scanners {
            let scanner_name = scanner.name().to_string();
            let producer = Arc::clone(&self.message_producer);
            
            let task_id = self.task_manager.spawn_task(mode, move |cancel| {
                async move {
                    log::debug!("Starting {} scan with scanner: {}", mode.bits(), scanner_name);
                    
                    // Get message stream from scanner
                    let stream = scanner.scan_async(mode).await?;
                    
                    // Process messages from stream
                    AsyncScannerEngine::process_stream(stream, producer, cancel).await?;
                    
                    log::debug!("Completed {} scan", mode.bits());
                    Ok(())
                }
            }).await?;
            
            tasks.push(task_id);
        }
        
        // Wait for all tasks to complete
        for task_id in tasks {
            self.task_manager.wait_for_task(&task_id, None).await?;
        }
        
        // Check for any errors
        let errors = self.task_manager.get_errors().await;
        if !errors.is_empty() {
            let error_msgs: Vec<String> = errors.iter()
                .map(|e| format!("{}: {}", e.task_id, e.error))
                .collect();
            return Err(ScanError::task(error_msgs.join(", ")));
        }
        
        Ok(())
    }
    
    /// Process messages from a stream
    async fn process_stream(
        mut stream: ScanMessageStream,
        producer: Arc<dyn MessageProducer + Send + Sync>,
        cancel: tokio_util::sync::CancellationToken,
    ) -> ScanResult<()> {
        let mut count = 0;
        
        loop {
            tokio::select! {
                // Check for cancellation
                _ = cancel.cancelled() => {
                    log::info!("Stream processing cancelled after {} messages", count);
                    return Err(ScanError::Cancelled);
                }
                
                // Process next message
                message = stream.next() => {
                    match message {
                        Some(Ok(msg)) => {
                            producer.produce_message(msg);
                            count += 1;
                        }
                        Some(Err(e)) => {
                            log::error!("Stream error: {}", e);
                            return Err(e);
                        }
                        None => {
                            // Stream completed
                            log::debug!("Stream completed with {} messages", count);
                            break;
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Cancel all active scans
    pub async fn cancel(&self) {
        log::info!("Cancelling all active scans");
        self.task_manager.cancel_all().await;
    }
    
    /// Get engine statistics
    pub async fn get_stats(&self) -> EngineStats {
        EngineStats {
            active_tasks: self.task_manager.active_task_count(),
            completed_tasks: self.task_manager.completed_task_count().await,
            registered_scanners: self.scanners.len(),
            errors: self.task_manager.get_errors().await.len(),
            repository_stats: None,
        }
    }
    
    /// Get comprehensive engine statistics including repository context
    pub async fn get_comprehensive_stats(&self) -> ScanResult<EngineStats> {
        let repository_stats = self.collect_repository_statistics().await.ok();
        
        Ok(EngineStats {
            active_tasks: self.task_manager.active_task_count(),
            completed_tasks: self.task_manager.completed_task_count().await,
            registered_scanners: self.scanners.len(),
            errors: self.task_manager.get_errors().await.len(),
            repository_stats,
        })
    }
    
    /// Check if engine is idle (no active tasks)
    pub fn is_idle(&self) -> bool {
        self.task_manager.active_task_count() == 0
    }
    
    /// Collect repository statistics
    /// 
    /// This provides basic repository context that can be useful for
    /// analysis, reporting, and understanding scan scope.
    pub async fn collect_repository_statistics(&self) -> ScanResult<RepositoryStatistics> {
        let collector = RepositoryStatsCollector::new();
        
        // Spawn blocking task for git operations
        let repo = Arc::clone(&self.repository);
        let stats = tokio::task::spawn_blocking(move || {
            collector.collect_statistics(&repo)
        }).await
        .map_err(|e| ScanError::task(format!("Failed to collect statistics: {}", e)))?
        .map_err(|e| ScanError::repository(format!("Statistics collection error: {}", e)))?;
        
        log::debug!("Collected repository statistics: {} commits, {} files, {} authors", 
                   stats.total_commits, stats.total_files, stats.total_authors);
        
        Ok(stats)
    }
}

/// Engine statistics
#[derive(Debug, Clone)]
pub struct EngineStats {
    pub active_tasks: usize,
    pub completed_tasks: usize,
    pub registered_scanners: usize,
    pub errors: usize,
    pub repository_stats: Option<RepositoryStatistics>,
}

/// Builder for async scanner engine
pub struct AsyncScannerEngineBuilder {
    repository: Option<RepositoryHandle>,
    config: Option<ScannerConfig>,
    message_producer: Option<Arc<dyn MessageProducer + Send + Sync>>,
    scanners: Vec<Arc<dyn AsyncScanner>>,
    runtime: Option<Arc<tokio::runtime::Runtime>>,
}

impl AsyncScannerEngineBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            repository: None,
            config: None,
            message_producer: None,
            scanners: Vec::new(),
            runtime: None,
        }
    }
    
    /// Set the repository
    pub fn repository(mut self, repository: RepositoryHandle) -> Self {
        self.repository = Some(repository);
        self
    }
    
    /// Set the configuration
    pub fn config(mut self, config: ScannerConfig) -> Self {
        self.config = Some(config);
        self
    }
    
    /// Set the message producer
    pub fn message_producer(mut self, producer: Arc<dyn MessageProducer + Send + Sync>) -> Self {
        self.message_producer = Some(producer);
        self
    }
    
    /// Add a scanner
    pub fn add_scanner(mut self, scanner: Arc<dyn AsyncScanner>) -> Self {
        self.scanners.push(scanner);
        self
    }
    
    /// Set the runtime (optional - if not set, a new runtime will be created)
    pub fn runtime(mut self, runtime: Arc<tokio::runtime::Runtime>) -> Self {
        self.runtime = Some(runtime);
        self
    }
    
    /// Build the engine
    pub fn build(self) -> ScanResult<AsyncScannerEngine> {
        let repository = self.repository
            .ok_or_else(|| ScanError::configuration("Repository not set"))?;
        
        let config = self.config.unwrap_or_default();
        
        let message_producer = self.message_producer
            .ok_or_else(|| ScanError::configuration("Message producer not set"))?;
        
        let mut engine = if let Some(runtime) = self.runtime {
            // Use provided runtime
            AsyncScannerEngine::with_runtime(repository, config, message_producer, runtime)?
        } else {
            // Create new runtime
            #[cfg(test)]
            let engine = AsyncScannerEngine::new_for_test(repository, config, message_producer)?;
            #[cfg(not(test))]
            let engine = AsyncScannerEngine::new(repository, config, message_producer)?;
            engine
        };
        
        for scanner in self.scanners {
            engine.register_scanner(scanner);
        }
        
        Ok(engine)
    }
}

impl Default for AsyncScannerEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
    use futures::stream;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    struct MockMessageProducer {
        count: Arc<AtomicUsize>,
    }
    
    impl MessageProducer for MockMessageProducer {
        fn produce_message(&self, _message: ScanMessage) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }
        
        fn get_producer_name(&self) -> &str {
            "MockProducer"
        }
    }
    
    struct MockAsyncScanner {
        name: String,
        supported_modes: ScanMode,
        message_count: usize,
    }
    
    #[async_trait]
    impl AsyncScanner for MockAsyncScanner {
        fn name(&self) -> &str {
            &self.name
        }
        
        fn supports_mode(&self, mode: ScanMode) -> bool {
            self.supported_modes.contains(mode)
        }
        
        async fn scan_async(&self, mode: ScanMode) -> ScanResult<ScanMessageStream> {
            let messages: Vec<ScanResult<ScanMessage>> = (0..self.message_count)
                .map(|i| Ok(ScanMessage::new(
                    MessageHeader::new(mode, 12345 + i as u64),
                    MessageData::FileInfo {
                        path: format!("file{}.rs", i),
                        size: 1024,
                        lines: 50,
                    },
                )))
                .collect();
            
            Ok(Box::pin(stream::iter(messages)))
        }
    }
    
    #[tokio::test]
    async fn test_engine_creation() {
        let repo = RepositoryHandle::open(".").unwrap();
        let config = ScannerConfig::default();
        let producer = Arc::new(MockMessageProducer {
            count: Arc::new(AtomicUsize::new(0)),
        });
        
        let engine = AsyncScannerEngine::new_for_test(repo, config, producer).unwrap();
        assert!(engine.is_idle());
        
        let stats = engine.get_stats().await;
        assert_eq!(stats.active_tasks, 0);
        assert_eq!(stats.registered_scanners, 0);
    }
    
    #[tokio::test]
    async fn test_scanner_registration() {
        let repo = RepositoryHandle::open(".").unwrap();
        let producer = Arc::new(MockMessageProducer {
            count: Arc::new(AtomicUsize::new(0)),
        });
        
        let mut engine = AsyncScannerEngine::new_for_test(repo, ScannerConfig::default(), producer).unwrap();
        
        let scanner = Arc::new(MockAsyncScanner {
            name: "TestScanner".to_string(),
            supported_modes: ScanMode::FILES,
            message_count: 5,
        });
        
        engine.register_scanner(scanner);
        
        let stats = engine.get_stats().await;
        assert_eq!(stats.registered_scanners, 1);
    }
    
    #[tokio::test]
    async fn test_repository_statistics_collection() {
        let repo = RepositoryHandle::open(".").unwrap();
        let producer = Arc::new(MockMessageProducer {
            count: Arc::new(AtomicUsize::new(0)),
        });
        
        let engine = AsyncScannerEngine::new_for_test(repo, ScannerConfig::default(), producer).unwrap();
        
        // Test basic statistics collection
        let stats_result = engine.collect_repository_statistics().await;
        assert!(stats_result.is_ok());
        
        let stats = stats_result.unwrap();
        assert!(stats.total_commits > 0, "Should have commits");
        assert!(stats.total_files > 0, "Should have files");
        assert!(stats.total_authors > 0, "Should have authors");
        assert!(stats.repository_size > 0, "Should have size");
        
        // Test comprehensive stats
        let comprehensive_stats_result = engine.get_comprehensive_stats().await;
        assert!(comprehensive_stats_result.is_ok());
        
        let comprehensive_stats = comprehensive_stats_result.unwrap();
        assert!(comprehensive_stats.repository_stats.is_some());
        
        let repo_stats = comprehensive_stats.repository_stats.unwrap();
        assert_eq!(repo_stats.total_commits, stats.total_commits);
        assert_eq!(repo_stats.total_files, stats.total_files);
        assert_eq!(repo_stats.total_authors, stats.total_authors);
    }
    
    #[tokio::test]
    async fn test_scanning() {
        let repo = RepositoryHandle::open(".").unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let producer = Arc::new(MockMessageProducer {
            count: Arc::clone(&count),
        });
        
        let scanner = Arc::new(MockAsyncScanner {
            name: "TestScanner".to_string(),
            supported_modes: ScanMode::FILES,
            message_count: 10,
        });
        
        let mut engine = AsyncScannerEngine::new_for_test(repo, ScannerConfig::default(), producer).unwrap();
        engine.register_scanner(scanner);
        
        engine.scan(ScanMode::FILES).await.unwrap();
        
        assert_eq!(count.load(Ordering::Relaxed), 10);
        
        let stats = engine.get_stats().await;
        assert_eq!(stats.completed_tasks, 1);
        assert_eq!(stats.errors, 0);
    }
}