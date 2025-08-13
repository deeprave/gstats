//! Plugin Executor for Scanner Integration
//!
//! Integrates the plugin system with the async scanner engine to process messages
//! in real-time as they flow through the scanning pipeline.

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::RwLock;
use futures::{Stream, StreamExt};
use crate::scanner::messages::{ScanMessage, MessageData};
use crate::scanner::async_engine::error::{ScanError, ScanResult};
use crate::plugin::{
    PluginResult, SharedPluginRegistry, PluginRegistry
};
use std::pin::Pin;
use std::task::{Context, Poll};
use pin_project::pin_project;
use std::collections::HashMap;

/// Plugin executor that processes messages through registered plugins
pub struct PluginExecutor {
    registry: Arc<RwLock<PluginRegistry>>,
    /// Track execution metrics
    metrics: Arc<RwLock<ExecutionMetrics>>,
    /// Store aggregated results per plugin for later execution
    aggregated_data: Arc<RwLock<HashMap<String, Vec<ScanMessage>>>>,
    /// Optional scanner publisher for emitting ScanDataReady events
    scanner_publisher: Option<crate::scanner::ScannerPublisher>,
    /// Scan ID for event correlation
    scan_id: String,
    /// Collect warnings during scanning for final reporting
    warnings: Arc<RwLock<Vec<String>>>,
}

#[derive(Debug, Default, Clone)]
pub struct ExecutionMetrics {
    pub messages_processed: u64,
    pub plugin_executions: u64,
    pub errors: u64,
    pub total_processing_time_ms: u64,
}

impl PluginExecutor {
    /// Create a new plugin executor
    pub fn new(registry: SharedPluginRegistry) -> Self {
        Self {
            registry: registry.inner().clone(),
            metrics: Arc::new(RwLock::new(ExecutionMetrics::default())),
            aggregated_data: Arc::new(RwLock::new(HashMap::new())),
            scanner_publisher: None,
            scan_id: "default_scan".to_string(),
            warnings: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create a new plugin executor with scanner publisher for event emission
    pub fn with_scanner_publisher(
        registry: SharedPluginRegistry, 
        scanner_publisher: crate::scanner::ScannerPublisher,
        scan_id: String,
    ) -> Self {
        Self {
            registry: registry.inner().clone(),
            metrics: Arc::new(RwLock::new(ExecutionMetrics::default())),
            aggregated_data: Arc::new(RwLock::new(HashMap::new())),
            scanner_publisher: Some(scanner_publisher),
            scan_id,
            warnings: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Process a single message through all applicable plugins
    pub async fn process_message(&self, message: ScanMessage) -> Vec<ScanMessage> {
        let start_time = std::time::Instant::now();
        let mut all_results = Vec::new();

        // Get scanner plugins that support current scan mode
        let _plugin_names = match self.get_applicable_plugins().await {
            Ok(plugins) => plugins,
            Err(e) => {
                log::error!("Failed to get plugins: {}", e);
                return vec![message];
            }
        };

        // Note: ScannerPlugin processing has been removed as part of GS-72
        // Plugins now only execute via Plugin.execute() method
        // The scanner streams events directly to the queue without plugin processing during scanning
        
        log::debug!("Message processing through ScannerPlugin removed - streaming to queue instead");
        
        // Return the original message (no processing during scanning)
        all_results.push(message);
        
        // Update processing metrics
        let processing_time = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.messages_processed += 1;
        metrics.total_processing_time_ms += processing_time.as_millis() as u64;

        all_results
    }

    /// Get plugins that support the current scan modes
    /// Note: Returns empty list as ScannerPlugin processing has been removed (GS-72)
    async fn get_applicable_plugins(&self) -> PluginResult<Vec<String>> {
        // ScannerPlugin processing removed - plugins now only execute via Plugin.execute()
        // Scanner streams events directly to queue without plugin processing during scanning
        Ok(Vec::new())
    }

    /// Process a message through active plugins only (GS-73: Plugin Activation Architecture)
    /// This method implements activation-aware processing where only active plugins receive messages
    pub async fn process_message_through_active_plugins(&self, _message: ScanMessage) -> PluginResult<Vec<String>> {
        let registry = self.registry.read().await;
        let active_plugins = registry.get_active_plugins();
        
        if active_plugins.is_empty() {
            log::debug!("No active plugins to process message through");
            return Ok(Vec::new());
        }

        let mut processed_plugins = Vec::new();
        
        for plugin_name in &active_plugins {
            // In the full implementation, this would actually call Plugin.execute() 
            // on each active plugin with the message
            log::debug!("Processing message through active plugin: {}", plugin_name);
            processed_plugins.push(plugin_name.clone());
        }
        
        log::debug!("Processed message through {} active plugins", processed_plugins.len());
        Ok(processed_plugins)
    }

    /// Determine data type from message data for ScanDataReady events
    fn determine_data_type(&self, message_data: &MessageData) -> String {
        match message_data {
            MessageData::CommitInfo { .. } => "commits".to_string(),
            MessageData::FileInfo { .. } => "files".to_string(),
            MessageData::MetricInfo { .. } => "metrics".to_string(),
            MessageData::ChangeFrequencyInfo { .. } => "change_frequency".to_string(),
            MessageData::DependencyInfo { .. } => "dependencies".to_string(),
            MessageData::SecurityInfo { .. } => "security".to_string(),
            MessageData::PerformanceInfo { .. } => "performance".to_string(),
            MessageData::RepositoryStatistics { .. } => "repository_statistics".to_string(),
            MessageData::None => "unknown".to_string(),
        }
    }

    /// Create a plugin-processing stream from an input stream
    pub fn create_plugin_stream<S>(&self, input: S) -> PluginStream<S>
    where
        S: Stream<Item = ScanResult<ScanMessage>> + Send + 'static,
    {
        PluginStream::new(input, Arc::new(self.clone()))
    }

    /// Get execution metrics
    pub async fn get_metrics(&self) -> ExecutionMetrics {
        self.metrics.read().await.clone()
    }

    /// Finalize scanning - ScannerPlugin aggregation removed (GS-72)
    /// Plugins now handle their own data processing via Plugin.execute()
    pub async fn finalize_scanning(&self) -> PluginResult<HashMap<String, ScanMessage>> {
        log::debug!("Finalizing scanning - ScannerPlugin aggregation removed");
        
        // Return empty map since ScannerPlugin processing has been removed
        // Plugins now process data via Plugin.execute() method instead
        Ok(HashMap::new())
    }

    /// Get aggregated data for a specific plugin (for use in plugin execute methods)
    pub async fn get_aggregated_data(&self, plugin_name: &str) -> Option<Vec<ScanMessage>> {
        let aggregated_data = self.aggregated_data.read().await;
        aggregated_data.get(plugin_name).cloned()
    }

    /// Clear all aggregated data (useful for cleanup)
    pub async fn clear_aggregated_data(&self) {
        let mut aggregated_data = self.aggregated_data.write().await;
        aggregated_data.clear();
    }
    
    /// Determine if a plugin error is fatal and should stop processing
    fn is_fatal_plugin_error(&self, error: &crate::plugin::error::PluginError) -> bool {
        use crate::plugin::error::PluginError;
        
        match error {
            // Fatal errors that should stop processing
            PluginError::InitializationFailed { .. } => true,
            PluginError::ConfigurationError { .. } => true,
            PluginError::DependencyError { .. } => true,
            PluginError::LoadingFailed { .. } => true,
            PluginError::RegistryError { .. } => true,
            
            // Non-fatal errors that allow continued processing
            PluginError::ExecutionFailed { .. } => false,
            PluginError::PluginNotFound { .. } => false,
            PluginError::VersionIncompatible { .. } => false,
            PluginError::NotificationFailed { .. } => false,
            PluginError::DiscoveryFailed { .. } => false,
            PluginError::DiscoveryError { .. } => false,
            PluginError::DescriptorParseError { .. } => false,
            PluginError::AsyncError { .. } => false,
            
            // Default to non-fatal for unknown errors
            _ => false,
        }
    }
    
    /// Add a warning to the collection for final reporting
    pub async fn add_warning(&self, warning: String) {
        let mut warnings = self.warnings.write().await;
        warnings.push(warning);
    }
    
    /// Get all collected warnings for final reporting
    pub async fn get_warnings(&self) -> Vec<String> {
        let warnings = self.warnings.read().await;
        warnings.clone()
    }
    
    /// Clear all collected warnings
    pub async fn clear_warnings(&self) {
        let mut warnings = self.warnings.write().await;
        warnings.clear();
    }
}

impl Clone for PluginExecutor {
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            metrics: Arc::clone(&self.metrics),
            aggregated_data: Arc::clone(&self.aggregated_data),
            scanner_publisher: self.scanner_publisher.clone(),
            scan_id: self.scan_id.clone(),
            warnings: Arc::clone(&self.warnings),
        }
    }
}

/// Stream wrapper that processes messages through plugins
#[pin_project]
pub struct PluginStream<S> {
    #[pin]
    inner: S,
    executor: Arc<PluginExecutor>,
    /// Buffer for messages generated by plugins
    buffer: Vec<ScanMessage>,
}

impl<S> PluginStream<S>
where
    S: Stream<Item = ScanResult<ScanMessage>>,
{
    pub fn new(stream: S, executor: Arc<PluginExecutor>) -> Self {
        Self {
            inner: stream,
            executor,
            buffer: Vec::new(),
        }
    }
}

impl<S> Stream for PluginStream<S>
where
    S: Stream<Item = ScanResult<ScanMessage>> + Send,
{
    type Item = ScanResult<ScanMessage>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // First, check if we have buffered messages
        if let Some(message) = this.buffer.pop() {
            return Poll::Ready(Some(Ok(message)));
        }

        // Poll the inner stream
        match this.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(message))) => {
                // Process message through plugins
                let executor = Arc::clone(this.executor);
                let message_clone = message.clone();
                
                // We need to process the message, but we're in a sync context
                // For now, we'll just pass through the original message and
                // queue plugin processing as a separate task
                let runtime = tokio::runtime::Handle::current();
                runtime.spawn(async move {
                    let results = executor.process_message(message_clone).await;
                    // In a real implementation, we'd need a way to feed these
                    // back into the stream or a separate channel
                    log::trace!("Plugin processing generated {} messages", results.len());
                });

                Poll::Ready(Some(Ok(message)))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Plugin-aware message processor for scanner integration
pub struct PluginMessageProcessor {
    executor: PluginExecutor,
    output_channel: tokio::sync::mpsc::Sender<ScanMessage>,
}

impl PluginMessageProcessor {
    /// Create a new plugin message processor
    pub fn new(
        registry: SharedPluginRegistry, 
        output_channel: tokio::sync::mpsc::Sender<ScanMessage>,
    ) -> Self {
        Self {
            executor: PluginExecutor::new(registry),
            output_channel,
        }
    }

    /// Process a stream of messages through plugins
    pub async fn process_stream<S>(&self, mut stream: S) -> ScanResult<()>
    where
        S: Stream<Item = ScanResult<ScanMessage>> + Send + Unpin,
    {
        while let Some(result) = stream.next().await {
            match result {
                Ok(message) => {
                    // Process through plugins
                    let results = self.executor.process_message(message).await;
                    
                    // Send all results to output channel
                    for msg in results {
                        if let Err(e) = self.output_channel.send(msg).await {
                            log::error!("Failed to send message to output: {}", e);
                            return Err(ScanError::stream(format!("Channel send failed: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    log::error!("Stream error: {}", e);
                    return Err(e);
                }
            }
        }
        
        Ok(())
    }

    /// Get execution metrics
    pub async fn get_metrics(&self) -> ExecutionMetrics {
        self.executor.get_metrics().await
    }
}

/// Extension trait for integrating plugins with scanner streams
#[async_trait]
pub trait PluginStreamExt: Stream<Item = ScanResult<ScanMessage>> + Send + Sized + 'static {
    /// Process messages through plugins
    fn with_plugins(self, executor: Arc<PluginExecutor>) -> PluginStream<Self> {
        PluginStream::new(self, executor)
    }
}

// Implement the extension trait for all suitable streams
impl<S> PluginStreamExt for S 
where 
    S: Stream<Item = ScanResult<ScanMessage>> + Send + 'static 
{}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::tests::mock_plugins::MockPlugin;
    use crate::scanner::messages::{MessageHeader, MessageData};
    use futures::stream;

    async fn create_test_registry() -> SharedPluginRegistry {
        let registry = SharedPluginRegistry::new();
        
        // Add a test plugin (using MockPlugin instead of MockScannerPlugin)
        let plugin = Box::new(MockPlugin::new(
            "test-plugin", 
            false
        ));
        
        registry.inner().write().await.register_plugin(plugin).await.unwrap();
        
        registry
    }

    fn create_test_message() -> ScanMessage {
        ScanMessage::new(
            MessageHeader::new(12345),
            MessageData::FileInfo {
                path: "test.rs".to_string(),
                size: 1024,
                lines: 50,
            }
        )
    }

    #[tokio::test]
    async fn test_plugin_executor_creation() {
        let registry = create_test_registry().await;
        let executor = PluginExecutor::new(registry);
        
        let metrics = executor.get_metrics().await;
        assert_eq!(metrics.messages_processed, 0);
        assert_eq!(metrics.plugin_executions, 0);
    }

    #[tokio::test]
    async fn test_message_processing() {
        let registry = create_test_registry().await;
        let executor = PluginExecutor::new(registry.clone());
        
        let message = create_test_message();
        let results = executor.process_message(message.clone()).await;

        // With ScannerPlugin processing removed (GS-72), we only get the original message back
        // Scanner now streams events directly to queue without plugin processing during scanning
        assert_eq!(results.len(), 1, "Expected only original message since ScannerPlugin processing removed");

        // Message should be the original unchanged
        assert_eq!(results[0], message);

        let metrics = executor.get_metrics().await;
        assert_eq!(metrics.messages_processed, 1);
        // plugin_executions should be 0 since no ScannerPlugin processing occurs
        assert_eq!(metrics.plugin_executions, 0);
    }

    #[tokio::test]
    async fn test_plugin_stream() {
        let registry = create_test_registry().await;
        let executor = Arc::new(PluginExecutor::new(registry));
        
        // Create a test stream
        let messages = vec![
            Ok(create_test_message()),
            Ok(create_test_message()),
        ];
        let stream = stream::iter(messages);
        
        // Wrap with plugin processing
        let mut plugin_stream = stream.with_plugins(executor);
        
        // Collect results
        let mut count = 0;
        while let Some(result) = plugin_stream.next().await {
            assert!(result.is_ok());
            count += 1;
        }
        
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_aggregated_data_storage_and_retrieval() {
        let registry = create_test_registry().await;
        let executor = PluginExecutor::new(registry.clone());

    async fn test_aggregated_data_storage_and_retrieval() {
        let registry = create_test_registry().await;
        let executor = PluginExecutor::new(registry.clone());

        // Process multiple messages to build up aggregated data
        let message1 = create_test_message();
        let message2 = ScanMessage::new(
            MessageHeader::new(12346),
            MessageData::FileInfo {
                path: "test2.rs".to_string(),
                size: 2048,
                lines: 100,
            }
        );

        // Process messages through plugins
        let results1 = executor.process_message(message1.clone()).await;
        let results2 = executor.process_message(message2.clone()).await;

        // With ScannerPlugin processing removed, we only get original messages back
        assert_eq!(results1.len(), 1);
        assert_eq!(results2.len(), 1);

        // Check that no aggregated data is stored (ScannerPlugin processing removed)
        let aggregated_data = executor.get_aggregated_data("test-plugin").await;
        assert!(aggregated_data.is_none(), "No aggregated data should be stored since ScannerPlugin processing removed");

        // Test finalization returns empty map
        let final_aggregated = executor.finalize_scanning().await.unwrap();
        assert!(final_aggregated.is_empty(), "Should have empty final aggregated data since ScannerPlugin processing removed");

        // Test cleanup (should be no-op now)
        executor.clear_aggregated_data().await;
        let cleared_data = executor.get_aggregated_data("test-plugin").await;
        assert!(cleared_data.is_none(), "No aggregated data to clear");
    }
    }
}