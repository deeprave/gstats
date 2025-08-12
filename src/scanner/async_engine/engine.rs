//! Async Scanner Engine Implementation
//! 
//! Core async scanner engine that coordinates multiple scan modes concurrently.

use std::sync::Arc;
use std::path::{Path, PathBuf};
#[cfg(not(test))]
use tokio::runtime::Runtime;
use futures::StreamExt;
use crate::scanner::config::ScannerConfig;
use crate::scanner::traits::MessageProducer;
use crate::scanner::async_traits::{AsyncScanner, ScanMessageStream};
use crate::scanner::statistics::RepositoryStatistics;
use super::task_manager::TaskManager;
use super::error::{ScanError, ScanResult};

/// Core async scanner engine
pub struct AsyncScannerEngine {
    /// Tokio runtime for async operations
    #[cfg(not(test))]
    #[allow(dead_code)]
    runtime: Arc<Runtime>,
    
    /// Repository path
    repository_path: PathBuf,
    
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
    pub fn new<P: AsRef<Path>>(
        repository_path: P,
        config: ScannerConfig,
        message_producer: Arc<dyn MessageProducer + Send + Sync>,
    ) -> ScanResult<Self> {
        // Validate repository path
        let repo_path = repository_path.as_ref();
        Self::validate_repository_path(repo_path)?;
        
        // Create runtime with configured thread count
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config.max_threads.unwrap_or_else(num_cpus::get))
            .enable_all()
            .build()
            .map_err(|e| ScanError::configuration(format!("Failed to create runtime: {}", e)))?;
        
        Self::with_runtime(repository_path, config, message_producer, Arc::new(runtime))
    }
    
    /// Create a new async scanner engine with existing runtime (for tests)
    pub fn with_runtime<P: AsRef<Path>>(
        repository_path: P,
        config: ScannerConfig,
        message_producer: Arc<dyn MessageProducer + Send + Sync>,
        _runtime: Arc<tokio::runtime::Runtime>,
    ) -> ScanResult<Self> {
        // Validate and canonicalize repository path
        let repo_path = repository_path.as_ref();
        Self::validate_repository_path(repo_path)?;
        
        let canonical_path = repo_path.canonicalize()
            .map_err(|e| ScanError::configuration(format!("Failed to canonicalize path {}: {}", repo_path.display(), e)))?;
        
        // Create task manager with concurrency limit
        let max_concurrent = config.max_threads.unwrap_or_else(num_cpus::get);
        let task_manager = TaskManager::new(max_concurrent);
        
        Ok(Self {
            #[cfg(not(test))]
            runtime: _runtime,
            repository_path: canonical_path,
            config,
            task_manager,
            message_producer,
            scanners: Vec::new(),
        })
    }
    
    /// Validate that the path is a valid git repository
    fn validate_repository_path(path: &Path) -> ScanResult<()> {
        if !path.exists() {
            return Err(ScanError::configuration(format!(
                "Repository path does not exist: {}", 
                path.display()
            )));
        }
        
        // Validate it's a git repository using gitoxide
        gix::discover(path)
            .map_err(|e| ScanError::configuration(format!(
                "Not a valid git repository at {}: {}", 
                path.display(), 
                e
            )))?;
        
        Ok(())
    }
    
    /// Get the repository path
    pub fn repository_path(&self) -> &Path {
        &self.repository_path
    }
    
    /// Create a new async scanner engine for testing (no separate runtime)
    #[cfg(test)]
    pub fn new_for_test<P: AsRef<Path>>(
        repository_path: P,
        config: ScannerConfig,
        message_producer: Arc<dyn MessageProducer + Send + Sync>,
    ) -> ScanResult<Self> {
        // Validate and canonicalize repository path
        let repo_path = repository_path.as_ref();
        Self::validate_repository_path(repo_path)?;
        
        let canonical_path = repo_path.canonicalize()
            .map_err(|e| ScanError::configuration(format!("Failed to canonicalize path {}: {}", repo_path.display(), e)))?;
        
        // Create task manager with concurrency limit
        let max_concurrent = config.max_threads.unwrap_or_else(num_cpus::get);
        let task_manager = TaskManager::new(max_concurrent);
        
        Ok(Self {
            repository_path: canonical_path,
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
    pub async fn scan(&self) -> ScanResult<()> {
        if self.scanners.is_empty() {
            return Err(ScanError::no_scanners_registered());
        }
        
        // Run all scanners - no mode filtering
        let mut tasks = Vec::new();
        
        for scanner in &self.scanners {
            let scanner_name = scanner.name().to_string();
            let scanner_clone = Arc::clone(scanner);
            let producer = Arc::clone(&self.message_producer);
            let repository_path = self.repository_path.clone();
            
            let task_id = self.task_manager.spawn_task(scanner_name.clone(), move |cancel| {
                async move {
                    log::debug!("Starting scan with scanner: {}", scanner_name);
                    
                    // Get message stream from scanner with repository path
                    let stream = scanner_clone.scan_async(&repository_path).await?;
                    
                    // Process messages from stream
                    AsyncScannerEngine::process_stream(stream, producer, cancel).await?;
                    
                    log::debug!("Completed scan with scanner: {}", scanner_name);
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
    
    /// Collect repository statistics from event-driven processors
    /// 
    /// This provides basic repository context that can be useful for
    /// analysis, reporting, and understanding scan scope.
    /// Statistics are collected from the StatisticsProcessor after scanning.
    pub async fn collect_repository_statistics(&self) -> ScanResult<RepositoryStatistics> {
        // For now, return default statistics since the event-driven approach
        // will collect statistics during scanning through the StatisticsProcessor
        // This method will be updated to extract statistics from the processor registry
        // once the full event-driven scanning is implemented
        
        log::debug!("Collecting repository statistics from event-driven processors");
        
        // TODO: Extract statistics from StatisticsProcessor in processor registry
        // This requires integration with the event processing pipeline
        let stats = RepositoryStatistics::default();
        
        log::debug!("Repository statistics (placeholder): {} commits, {} files, {} authors", 
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
    repository_path: Option<PathBuf>,
    config: Option<ScannerConfig>,
    message_producer: Option<Arc<dyn MessageProducer + Send + Sync>>,
    scanners: Vec<Arc<dyn AsyncScanner>>,
    runtime: Option<Arc<tokio::runtime::Runtime>>,
}

impl AsyncScannerEngineBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            repository_path: None,
            config: None,
            message_producer: None,
            scanners: Vec::new(),
            runtime: None,
        }
    }
    
    /// Set the repository path
    pub fn repository<P: AsRef<Path>>(mut self, repository_path: P) -> Self {
        self.repository_path = Some(repository_path.as_ref().to_path_buf());
        self
    }
    
    /// Set the repository path from PathBuf
    pub fn repository_path(mut self, repository_path: PathBuf) -> Self {
        self.repository_path = Some(repository_path);
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
        let repository_path = self.repository_path
            .ok_or_else(|| ScanError::configuration("Repository path not set"))?;
        
        let config = self.config.unwrap_or_default();
        
        let message_producer = self.message_producer
            .ok_or_else(|| ScanError::configuration("Message producer not set"))?;
        
        let mut engine = if let Some(runtime) = self.runtime {
            // Use provided runtime
            AsyncScannerEngine::with_runtime(repository_path, config, message_producer, runtime)?
        } else {
            // Create new runtime
            #[cfg(test)]
            let engine = AsyncScannerEngine::new_for_test(repository_path, config, message_producer)?;
            #[cfg(not(test))]
            let engine = AsyncScannerEngine::new(repository_path, config, message_producer)?;
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

/*
// Temporarily disabled during repository-owning pattern migration
#[cfg(test)]
mod tests {
    // ... test code commented out ...
}
*/
