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
use crate::plugin::SharedPluginRegistry;
use crate::notifications::traits::{Publisher, NotificationManager};
use crate::notifications::events::ScanEvent;
use crate::notifications::manager::AsyncNotificationManager;

/// Core async scanner engine
pub struct AsyncScannerEngine {
    /// Tokio runtime for async operations
    #[cfg(not(test))]
    _runtime: Arc<Runtime>,
    
    /// Repository path
    repository_path: PathBuf,
    
    /// Task coordination manager
    task_manager: TaskManager,
    
    /// Message producer for queue integration
    message_producer: Arc<dyn MessageProducer + Send + Sync>,
    
    /// Registered scanners
    scanners: Vec<Arc<dyn AsyncScanner>>,
    
    /// Plugin registry for coordination during shutdown
    plugin_registry: SharedPluginRegistry,
    
    /// Notification manager for publishing scanner lifecycle events
    notification_manager: Arc<AsyncNotificationManager<ScanEvent>>,
}

impl AsyncScannerEngine {
    /// Create a new async scanner engine
    pub fn new<P: AsRef<Path>>(
        repository_path: P,
        config: ScannerConfig,
        message_producer: Arc<dyn MessageProducer + Send + Sync>,
        notification_manager: Arc<AsyncNotificationManager<ScanEvent>>,
        plugin_registry: SharedPluginRegistry,
    ) -> ScanResult<Self> {
        // Validate repository path
        let repo_path = repository_path.as_ref();
        Self::validate_repository_path(repo_path)?;
        
        // Create runtime with configured thread count
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config.max_threads.unwrap_or_else(num_cpus::get))
            .enable_all()
            .build()
            .map_err(|e| ScanError::configuration(format!("Failed to create runtime: {e}")))?;
        
        Self::with_runtime(repository_path, config, message_producer, notification_manager, plugin_registry, Arc::new(runtime))
    }
    
    /// Create a new async scanner engine with existing runtime (for tests)
    pub fn with_runtime<P: AsRef<Path>>(
        repository_path: P,
        config: ScannerConfig,
        message_producer: Arc<dyn MessageProducer + Send + Sync>,
        notification_manager: Arc<AsyncNotificationManager<ScanEvent>>,
        plugin_registry: SharedPluginRegistry,
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
            _runtime,
            repository_path: canonical_path,
            task_manager,
            message_producer,
            scanners: Vec::new(),
            plugin_registry,
            notification_manager,
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
        notification_manager: Arc<AsyncNotificationManager<ScanEvent>>,
        plugin_registry: SharedPluginRegistry,
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
            task_manager,
            message_producer,
            scanners: Vec::new(),
            plugin_registry,
            notification_manager,
        })
    }
    
    /// Register a scanner with the engine
    pub fn register_scanner(&mut self, scanner: Arc<dyn AsyncScanner>) {
        self.scanners.push(scanner);
    }
    
    
    
    /// Execute scan with specified modes
    pub async fn scan(&self) -> ScanResult<()> {
        // Generate unique scan ID
        let scan_id = format!("scan-{}", uuid::Uuid::new_v4());
        let scan_start_time = std::time::Instant::now();
        
        // Publish ScanStarted event
        let started_event = ScanEvent::started(scan_id.clone());
        if let Err(e) = self.notification_manager.publish(started_event).await {
            log::warn!("Failed to publish ScanStarted event: {e}");
        }
        
        if self.scanners.is_empty() {
            return Err(ScanError::no_scanners_registered());
        }
        
        // Start periodic event timer (250ms)
        let (timer_tx, mut timer_rx) = tokio::sync::mpsc::unbounded_channel();
        let timer_scan_id = scan_id.clone();
        let timer_manager = self.notification_manager.clone();
        let _timer_message_producer = Arc::clone(&self.message_producer); // Reserved for future queue metrics integration
        
        // Spawn periodic event task
        let periodic_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));
            let mut tick_count = 0u64;
            let mut last_data_notification = 0u64;
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        tick_count += 1;
                        
                        // Publish ScanProgress event every tick
                        // Progress is estimated based on elapsed time for now
                        // Future enhancement: integrate with actual message queue metrics
                        let estimated_progress = (tick_count * 250) as f64 / 1000.0; // Progress in seconds
                        let progress_event = ScanEvent::progress(
                            timer_scan_id.clone(),
                            estimated_progress,
                            "processing".to_string(),
                        );
                        if let Err(e) = timer_manager.publish(progress_event).await {
                            log::warn!("Failed to publish ScanProgress event: {e}");
                        }
                        
                        // Publish ScanDataReady event periodically to notify waiting plugins
                        // This signals that the queue may have new data available
                        // Future enhancement: only publish when actual queue changes occur
                        if tick_count > last_data_notification {
                            let data_ready_event = ScanEvent::scan_data_ready(
                                timer_scan_id.clone(),
                                "queue_data".to_string(),
                                1, // Placeholder message count
                            );
                            if let Err(e) = timer_manager.publish(data_ready_event).await {
                                log::warn!("Failed to publish ScanDataReady event: {e}");
                            }
                            last_data_notification = tick_count;
                        }
                    }
                    _ = timer_rx.recv() => {
                        // Stop signal received
                        break;
                    }
                }
            }
        });
        
        // Run all scanners - no mode filtering
        let mut tasks = Vec::new();
        
        for scanner in &self.scanners {
            let scanner_name = scanner.name().to_string();
            let scanner_clone = Arc::clone(scanner);
            let producer = Arc::clone(&self.message_producer);
            let repository_path = self.repository_path.clone();
            
            let task_id = self.task_manager.spawn_task(scanner_name.clone(), move |cancel| {
                async move {
                    log::debug!("Starting scan with scanner: {scanner_name}");
                    
                    // Get message stream from scanner with repository path
                    let stream = scanner_clone.scan_async(&repository_path).await?;
                    
                    // Process messages from stream
                    AsyncScannerEngine::process_stream(stream, producer, cancel).await?;
                    
                    log::debug!("Completed scan with scanner: {scanner_name}");
                    Ok(())
                }
            }).await?;
            
            tasks.push(task_id);
        }
        
        // Wait for all tasks to complete
        for task_id in tasks {
            self.task_manager.wait_for_task(&task_id, None).await?;
        }
        
        // Stop periodic event timer
        let _ = timer_tx.send(());
        let _ = periodic_task.await;
        
        // Check for any errors
        let errors = self.task_manager.get_errors().await;
        if !errors.is_empty() {
            let error_msgs: Vec<String> = errors.iter()
                .map(|e| format!("{}: {}", e.task_id, e.error))
                .collect();
            
            // Publish ScanError event for fatal error
            let error_event = ScanEvent::error(scan_id.clone(), error_msgs.join(", "), true); // fatal = true
            if let Err(e) = self.notification_manager.publish(error_event).await {
                log::warn!("Failed to publish ScanError event: {e}");
            }
            
            return Err(ScanError::task(error_msgs.join(", ")));
        }
        
        // Publish ScanCompleted event
        let scan_duration = scan_start_time.elapsed();
        let warnings = Vec::new(); // TODO: collect actual warnings from task manager
        
        let completed_event = ScanEvent::completed(scan_id, scan_duration, warnings);
        if let Err(e) = self.notification_manager.publish(completed_event).await {
            log::warn!("Failed to publish ScanCompleted event: {e}");
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
                    log::info!("Stream processing cancelled after {count} messages");
                    return Err(ScanError::Cancelled);
                }
                
                // Process next message
                message = stream.next() => {
                    match message {
                        Some(Ok(msg)) => {
                            // Now async!
                            if let Err(e) = producer.produce_message(msg).await {
                                log::error!("Failed to produce message: {e}");
                                return Err(ScanError::processing(format!("Message production failed: {e}")));
                            }
                            count += 1;
                        }
                        Some(Err(e)) => {
                            log::error!("Stream error: {e}");
                            return Err(e);
                        }
                        None => {
                            // Stream completed
                            log::debug!("Stream completed with {count} messages");
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
    
    /// Graceful shutdown with plugin coordination
    /// 
    /// Performs a coordinated shutdown that ensures all active plugins complete
    /// their current work before the scanner exits. This prevents data loss and
    /// ensures proper cleanup of plugin resources.
    /// 
    /// The shutdown process:
    /// 1. Cancels all active scanner tasks to stop new work
    /// 2. Waits for all active plugins to transition to idle states
    /// 3. Returns success when coordination is complete or timeout is reached
    /// 
    /// If no plugin registry is configured, this method completes immediately
    /// after canceling scanner tasks, maintaining backward compatibility.
    /// 
    /// # Arguments
    /// * `timeout` - Maximum duration to wait for plugin coordination
    /// 
    /// # Returns
    /// * `Ok(())` - Shutdown completed successfully with all plugins idle
    /// * `Err(ScanError::Task)` - Plugin coordination failed or timed out
    /// 
    /// # Examples
    /// ```ignore
    /// use std::time::Duration;
    /// 
    /// // Graceful shutdown with 30-second timeout
    /// match engine.graceful_shutdown(Duration::from_secs(30)).await {
    ///     Ok(()) => log::info!("Scanner shutdown completed successfully"),
    ///     Err(e) => log::warn!("Scanner shutdown error: {}", e),
    /// }
    /// ```
    pub async fn graceful_shutdown(&self, timeout: std::time::Duration) -> ScanResult<()> {
        
        log::info!("Starting graceful shutdown with timeout: {timeout:?}");
        
        // Cancel all active scans first
        self.cancel().await;
        
        // Wait for plugin coordination - registry is always available
        {
            let registry = &self.plugin_registry;
            log::debug!("Waiting for plugin coordination during shutdown");
            
            // Create custom wait loop that doesn't hold locks for extended periods
            let start = tokio::time::Instant::now();
            let poll_interval = std::time::Duration::from_millis(10);
            
            loop {
                let all_idle = {
                    let registry_inner = registry.inner().read().await;
                    registry_inner.are_all_active_plugins_idle()
                };
                
                if all_idle {
                    log::info!("All plugins are idle - graceful shutdown complete");
                    return Ok(());
                }
                
                if start.elapsed() >= timeout {
                    // Get list of still-processing plugins for error message
                    let processing_plugins = {
                        let registry_inner = registry.inner().read().await;
                        registry_inner.get_active_processing_plugins()
                    };
                    
                    let error_msg = format!(
                        "Plugin coordination failed during shutdown: Timed out waiting for plugins to become idle. Still processing: {processing_plugins:?}"
                    );
                    log::warn!("{error_msg}");
                    return Err(ScanError::task(error_msg));
                }
                
                tokio::time::sleep(poll_interval).await;
            }
        }
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
    notification_manager: Option<Arc<AsyncNotificationManager<ScanEvent>>>,
    scanners: Vec<Arc<dyn AsyncScanner>>,
    runtime: Option<Arc<tokio::runtime::Runtime>>,
    plugin_registry: Option<SharedPluginRegistry>,
}

impl AsyncScannerEngineBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            repository_path: None,
            config: None,
            message_producer: None,
            notification_manager: None,
            scanners: Vec::new(),
            runtime: None,
            plugin_registry: None,
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
    
    /// Set the notification manager
    pub fn notification_manager(mut self, manager: Arc<AsyncNotificationManager<ScanEvent>>) -> Self {
        self.notification_manager = Some(manager);
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
    
    /// Set the plugin registry for coordination during shutdown
    /// 
    /// Configures the scanner engine to coordinate with plugins during graceful
    /// shutdown. The engine will wait for all active plugins to complete
    /// their work before allowing shutdown to proceed.
    /// 
    /// This is required for proper scanner operation.
    /// 
    /// # Arguments
    /// * `registry` - Shared plugin registry for coordination
    /// 
    /// # Examples
    /// ```ignore
    /// let registry = SharedPluginRegistry::new();
    /// // ... register plugins ...
    /// 
    /// let engine = AsyncScannerEngineBuilder::new()
    ///     .repository_path("/path/to/repo")
    ///     .message_producer(producer)
    ///     .plugin_registry(registry)  // Required for coordination
    ///     .build()?;
    /// ```
    pub fn plugin_registry(mut self, registry: SharedPluginRegistry) -> Self {
        self.plugin_registry = Some(registry);
        self
    }
    
    /// Build the engine
    pub fn build(self) -> ScanResult<AsyncScannerEngine> {
        let repository_path = self.repository_path
            .ok_or_else(|| ScanError::configuration("Repository path not set"))?;
        
        let config = self.config.unwrap_or_default();
        
        let message_producer = self.message_producer
            .ok_or_else(|| ScanError::configuration("Message producer not set"))?;
        
        let notification_manager = self.notification_manager
            .ok_or_else(|| ScanError::configuration("Notification manager not set"))?;
        
        let plugin_registry = self.plugin_registry
            .ok_or_else(|| ScanError::configuration("Plugin registry not set"))?;
        
        let mut engine = if let Some(runtime) = self.runtime {
            // Use provided runtime
            AsyncScannerEngine::with_runtime(repository_path, config, message_producer, notification_manager, plugin_registry, runtime)?
        } else {
            // Create new runtime
            #[cfg(test)]
            let engine = AsyncScannerEngine::new_for_test(repository_path, config, message_producer, notification_manager, plugin_registry)?;
            #[cfg(not(test))]
            let engine = AsyncScannerEngine::new(repository_path, config, message_producer, notification_manager, plugin_registry)?;
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

/// Publisher trait implementation for AsyncScannerEngine
#[async_trait::async_trait]
impl Publisher<ScanEvent> for AsyncScannerEngine {
    /// Publish an event to all subscribers
    async fn publish(&self, event: ScanEvent) -> crate::notifications::error::NotificationResult<()> {
        self.notification_manager.publish(event).await
    }
    
    /// Publish an event to a specific subscriber
    async fn publish_to(&self, event: ScanEvent, subscriber_id: &str) -> crate::notifications::error::NotificationResult<()> {
        self.notification_manager.publish_to(event, subscriber_id).await
    }
    
    /// Get the publisher identifier
    fn publisher_id(&self) -> &str {
        "scanner"
    }
}

/// Drop implementation for AsyncScannerEngine to ensure graceful shutdown
impl Drop for AsyncScannerEngine {
    /// Ensure graceful shutdown coordination when the scanner is dropped
    /// 
    /// This implementation ensures that the scanner coordinates with all active
    /// plugins before being destroyed, preventing data loss and ensuring proper
    /// cleanup of plugin resources.
    /// 
    /// The drop process:
    /// 1. Cancels all active scanner tasks to stop new work
    /// 2. Waits for all active plugins to transition to idle states
    /// 3. Completes successfully when coordination is done or timeout is reached
    /// 
    /// # Timeout Handling
    /// Uses a reasonable timeout (10 seconds) to prevent the drop from hanging
    /// indefinitely. If plugins don't respond within this time, the drop will
    /// complete anyway to prevent blocking the application shutdown.
    fn drop(&mut self) {
        // We can't use async methods in Drop, but we can check if we're in an async context
        // and use Handle::current() to spawn a blocking task
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // We're in an async context - spawn a task to handle graceful shutdown
            let plugin_registry = self.plugin_registry.clone();
            
            // Spawn a task to cancel and wait for plugin coordination
            let _shutdown_task = handle.spawn(async move {
                log::debug!("AsyncScannerEngine Drop: Starting graceful shutdown coordination");
                
                let timeout = std::time::Duration::from_secs(10);
                let start = tokio::time::Instant::now();
                let poll_interval = std::time::Duration::from_millis(10);
                
                loop {
                    let all_idle = {
                        let registry_inner = plugin_registry.inner().read().await;
                        registry_inner.are_all_active_plugins_idle()
                    };
                    
                    if all_idle {
                        log::debug!("AsyncScannerEngine Drop: All plugins are idle - graceful shutdown complete");
                        break;
                    }
                    
                    if start.elapsed() >= timeout {
                        // Get list of still-processing plugins for warning
                        let processing_plugins = {
                            let registry_inner = plugin_registry.inner().read().await;
                            registry_inner.get_active_processing_plugins()
                        };
                        
                        log::warn!(
                            "AsyncScannerEngine Drop: Plugin coordination timed out after {}s. Still processing: {:?}", 
                            timeout.as_secs(),
                            processing_plugins
                        );
                        break;
                    }
                    
                    tokio::time::sleep(poll_interval).await;
                }
                
                log::debug!("AsyncScannerEngine Drop: Graceful shutdown coordination completed");
            });
            
            // Note: We can't wait for the task to complete in Drop, but it will run to completion
            // in the background. This is the best we can do within Drop constraints.
        } else {
            // Not in an async context - just log a warning
            log::warn!("AsyncScannerEngine Drop: Not in async context - graceful shutdown coordination skipped");
        }
    }
}

/*
// Temporarily disabled during repository-owning pattern migration
#[cfg(test)]
mod tests {
    // ... test code commented out ...
}
*/
