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
use crate::notifications::traits::Publisher;
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
        let mut all_results = vec![message.clone()];

        // Get scanner plugins that support current scan mode
        let plugin_names = match self.get_applicable_plugins().await {
            Ok(plugins) => plugins,
            Err(e) => {
                log::error!("Failed to get plugins: {}", e);
                return all_results;
            }
        };

        // Process message through each scanner plugin
        for plugin_name in plugin_names {
            log::debug!("Processing message through plugin: {}", plugin_name);

            // Get the plugin from registry
            let registry = self.registry.read().await;
            if let Some(plugin) = registry.get_plugin(&plugin_name) {
                // Try to downcast to ScannerPlugin
                if let Some(scanner_plugin) = plugin.as_scanner_plugin() {
                    // All plugins now process all messages (no mode filtering)
                        match scanner_plugin.process_scan_data(&message).await {
                            Ok(processed_messages) => {
                                log::debug!("Plugin {} processed message, got {} results",
                                           plugin_name, processed_messages.len());

                                // Store processed messages for aggregation
                                let message_count = {
                                    let mut aggregated = self.aggregated_data.write().await;
                                    let plugin_data = aggregated.entry(plugin_name.clone()).or_insert_with(Vec::new);
                                    plugin_data.extend(processed_messages.clone());
                                    plugin_data.len()
                                };

                                // Emit ScanDataReady event if scanner publisher is available
                                if let Some(ref publisher) = self.scanner_publisher {
                                    let data_type = self.determine_data_type(&message.data);
                                    if let Err(e) = publisher.publish(crate::notifications::ScanEvent::scan_data_ready(
                                        self.scan_id.clone(),
                                        data_type,
                                        message_count,
                                    )).await {
                                        log::warn!("Failed to publish ScanDataReady event for plugin {}: {}", plugin_name, e);
                                        
                                        // Emit ScanWarning event for notification failure
                                        let warning_msg = format!("Failed to publish ScanDataReady event for plugin '{}': {}", plugin_name, e);
                                        
                                        // Collect warning for final reporting
                                        self.add_warning(warning_msg.clone()).await;
                                        
                                        let warning_event = crate::notifications::ScanEvent::warning(
                                            self.scan_id.clone(),
                                            warning_msg,
                                            true, // recoverable - data processing can continue
                                        );
                                        
                                        if let Err(warn_err) = publisher.publish(warning_event).await {
                                            log::error!("Failed to publish ScanWarning event for notification failure: {}", warn_err);
                                        }
                                    }
                                }

                                all_results.extend(processed_messages);
                            }
                            Err(e) => {
                                log::error!("Plugin {} failed to process message: {}", plugin_name, e);
                                let mut metrics = self.metrics.write().await;
                                metrics.errors += 1;
                                
                                // Determine if this is a fatal error that should stop processing
                                let is_fatal = self.is_fatal_plugin_error(&e);
                                
                                // Emit ScanError event for plugin processing failure
                                if let Some(ref scanner_publisher) = self.scanner_publisher {
                                    let error_msg = format!("Plugin '{}' failed to process message: {}", plugin_name, e);
                                    let error_event = crate::notifications::ScanEvent::error(
                                        self.scan_id.clone(),
                                        error_msg,
                                        is_fatal,
                                    );
                                    
                                    if let Err(publish_err) = scanner_publisher.publish(error_event).await {
                                        log::error!("Failed to publish ScanError event for plugin failure: {}", publish_err);
                                    } else {
                                        log::debug!("Published ScanError event for plugin {} processing failure (fatal: {})", plugin_name, is_fatal);
                                    }
                                }
                                
                                // If fatal error, we could potentially stop processing here
                                // For now, continue with other plugins but log the severity
                                if is_fatal {
                                    log::error!("Fatal error in plugin {}, continuing with remaining plugins", plugin_name);
                                }
                            }
                        }
                } else {
                    log::trace!("Plugin {} is not a ScannerPlugin", plugin_name);
                }
            } else {
                log::warn!("Plugin {} not found in registry", plugin_name);
                
                // Emit ScanWarning event for missing plugin
                if let Some(ref scanner_publisher) = self.scanner_publisher {
                    let warning_msg = format!("Plugin '{}' not found in registry during message processing", plugin_name);
                    
                    // Collect warning for final reporting
                    self.add_warning(warning_msg.clone()).await;
                    
                    let event = crate::notifications::ScanEvent::warning(
                        self.scan_id.clone(),
                        warning_msg,
                        true, // recoverable - processing can continue
                    );
                    
                    if let Err(e) = scanner_publisher.publish(event).await {
                        log::error!("Failed to publish ScanWarning event for missing plugin {}: {}", plugin_name, e);
                    }
                }
            }

            // Update metrics
            let mut metrics = self.metrics.write().await;
            metrics.plugin_executions += 1;
        }

        // Update processing metrics
        let processing_time = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.messages_processed += 1;
        metrics.total_processing_time_ms += processing_time.as_millis() as u64;

        all_results
    }

    /// Get plugins that support the current scan modes
    async fn get_applicable_plugins(&self) -> PluginResult<Vec<String>> {
        let registry = self.registry.read().await;
        let scanner_plugin_names = registry.get_plugins_by_type(crate::plugin::traits::PluginType::Scanner);
        
        let mut applicable = Vec::new();
        for name in scanner_plugin_names {
            if let Some(_plugin) = registry.get_plugin(&name) {
                // Try to downcast to ScannerPlugin to check modes
                // For now, just add all scanner plugins
                applicable.push(name);
            }
        }
        
        Ok(applicable)
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

    /// Finalize scanning by calling aggregate_results() on all ScannerPlugins
    pub async fn finalize_scanning(&self) -> PluginResult<HashMap<String, ScanMessage>> {
        let mut final_aggregated = HashMap::new();

        // Get all plugins that have aggregated data
        let aggregated_data = self.aggregated_data.read().await;
        let plugin_names: Vec<String> = aggregated_data.keys().cloned().collect();
        drop(aggregated_data); // Release the lock

        for plugin_name in plugin_names {
            log::debug!("Finalizing aggregated data for plugin: {}", plugin_name);

            // Get the plugin and its aggregated data
            let registry = self.registry.read().await;
            if let Some(plugin) = registry.get_plugin(&plugin_name) {
                if let Some(scanner_plugin) = plugin.as_scanner_plugin() {
                    // Get the aggregated data for this plugin
                    let aggregated_data = self.aggregated_data.read().await;
                    if let Some(plugin_data) = aggregated_data.get(&plugin_name) {
                        if !plugin_data.is_empty() {
                            match scanner_plugin.aggregate_results(plugin_data.clone()).await {
                                Ok(aggregated_message) => {
                                    log::debug!("Plugin {} aggregated {} messages into final result",
                                               plugin_name, plugin_data.len());
                                    final_aggregated.insert(plugin_name.clone(), aggregated_message.clone());
                                    
                                    // Emit DataReady event when processed data is available for export
                                    if let Some(ref scanner_publisher) = self.scanner_publisher {
                                        let data_type = self.determine_data_type(&aggregated_message.data);
                                        let event = crate::notifications::ScanEvent::data_ready(
                                            self.scan_id.clone(),
                                            plugin_name.clone(),
                                            data_type,
                                        );
                                        
                                        if let Err(e) = scanner_publisher.publish(event).await {
                                            log::warn!("Failed to publish DataReady event for plugin {}: {}", plugin_name, e);
                                            
                                            // Emit ScanWarning event for notification failure
                                            let warning_msg = format!("Failed to publish DataReady event for plugin '{}': {}", plugin_name, e);
                                            
                                            // Collect warning for final reporting
                                            self.add_warning(warning_msg.clone()).await;
                                            
                                            let warning_event = crate::notifications::ScanEvent::warning(
                                                self.scan_id.clone(),
                                                warning_msg,
                                                true, // recoverable - export coordination can continue
                                            );
                                            
                                            if let Err(warn_err) = scanner_publisher.publish(warning_event).await {
                                                log::error!("Failed to publish ScanWarning event for notification failure: {}", warn_err);
                                            }
                                        } else {
                                            log::debug!("Published DataReady event for plugin {} with processed data", plugin_name);
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("Plugin {} failed to aggregate results: {}", plugin_name, e);
                                    
                                    // Emit ScanError event for unrecoverable plugin failure
                                    if let Some(ref scanner_publisher) = self.scanner_publisher {
                                        let error_msg = format!("Plugin '{}' failed to aggregate results: {}", plugin_name, e);
                                        let error_event = crate::notifications::ScanEvent::error(
                                            self.scan_id.clone(),
                                            error_msg,
                                            true, // fatal - plugin aggregation failure prevents completion
                                        );
                                        
                                        if let Err(publish_err) = scanner_publisher.publish(error_event).await {
                                            log::error!("Failed to publish ScanError event for plugin failure: {}", publish_err);
                                        } else {
                                            log::debug!("Published ScanError event for plugin {} aggregation failure", plugin_name);
                                        }
                                    }
                                    
                                    return Err(e);
                                }
                            }
                        } else {
                            log::debug!("Plugin {} has no data to aggregate", plugin_name);
                        }
                    }
                }
            }
        }

        log::info!("Finalized scanning with {} plugins having aggregated data", final_aggregated.len());
        Ok(final_aggregated)
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
    use crate::plugin::tests::mock_plugins::MockScannerPlugin;
    use crate::scanner::messages::{MessageHeader, MessageData};
    use futures::stream;

    async fn create_test_registry() -> SharedPluginRegistry {
        let registry = SharedPluginRegistry::new();
        
        // Add a test plugin
        let plugin = Box::new(MockScannerPlugin::new(
            "test-scanner", 
            ScanMode::FILES,
            false
        ));
        
        registry.inner().write().await.register_plugin(plugin).await.unwrap();
        
        registry
    }

    fn create_test_message() -> ScanMessage {
        ScanMessage::new(
            MessageHeader::new(ScanMode::FILES, 12345),
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
        let executor = PluginExecutor::new(registry, ScanMode::FILES);
        
        let metrics = executor.get_metrics().await;
        assert_eq!(metrics.messages_processed, 0);
        assert_eq!(metrics.plugin_executions, 0);
    }

    #[tokio::test]
    async fn test_message_processing() {
        let registry = create_test_registry().await;
        let executor = PluginExecutor::new(registry.clone(), ScanMode::FILES);
        
        let message = create_test_message();
        let results = executor.process_message(message.clone()).await;

        // Should have original message plus processed messages from ScannerPlugin
        // MockScannerPlugin.process_scan_data() returns 1 processed message
        // So we expect: original (1) + processed (1) = 2 total
        assert_eq!(results.len(), 2, "Expected original message + 1 processed message from ScannerPlugin");

        // First message should be the original
        assert_eq!(results[0], message);

        // Second message should be the processed one (with modified timestamp)
        assert_ne!(results[1], message, "Processed message should be different from original");

        let metrics = executor.get_metrics().await;
        assert_eq!(metrics.messages_processed, 1);
        assert_eq!(metrics.plugin_executions, 1);
    }

    #[tokio::test]
    async fn test_plugin_stream() {
        let registry = create_test_registry().await;
        let executor = Arc::new(PluginExecutor::new(registry, ScanMode::FILES));
        
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
        let executor = PluginExecutor::new(registry.clone(), ScanMode::FILES);

        // Process multiple messages to build up aggregated data
        let message1 = create_test_message();
        let message2 = ScanMessage::new(
            MessageHeader::new(ScanMode::FILES, 12346),
            MessageData::FileInfo {
                path: "test2.rs".to_string(),
                size: 2048,
                lines: 100,
            }
        );

        // Process messages through plugins
        let results1 = executor.process_message(message1.clone()).await;
        let results2 = executor.process_message(message2.clone()).await;

        // Verify messages were processed
        assert!(results1.len() >= 1);
        assert!(results2.len() >= 1);

        // Check that aggregated data was stored
        let aggregated_data = executor.get_aggregated_data("test-scanner").await;
        assert!(aggregated_data.is_some(), "Aggregated data should be stored for test-scanner plugin");

        let data = aggregated_data.unwrap();
        assert!(data.len() >= 2, "Should have aggregated data from both processed messages");

        // Test finalization
        let final_aggregated = executor.finalize_scanning().await.unwrap();
        assert!(final_aggregated.contains_key("test-scanner"), "Should have final aggregated data for test-scanner");

        // Test cleanup
        executor.clear_aggregated_data().await;
        let cleared_data = executor.get_aggregated_data("test-scanner").await;
        assert!(cleared_data.is_none(), "Aggregated data should be cleared");
    }
}