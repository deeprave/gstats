//! Plugin-Enabled Scanner Adapter
//!
//! Integrates the plugin system with async scanner infrastructure to enable
//! plugin processing during repository scanning.

use std::sync::Arc;
use async_trait::async_trait;
use futures::Stream;
use pin_project::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::scanner::modes::ScanMode;
use crate::scanner::messages::ScanMessage;
use crate::scanner::async_traits::{AsyncScanner, ScanMessageStream};
use crate::scanner::async_engine::error::{ScanError, ScanResult};
use crate::plugin::{SharedPluginRegistry, PluginExecutor};

/// Scanner adapter that processes messages through plugins
pub struct PluginScanner {
    /// Inner scanner to wrap
    inner_scanner: Arc<dyn AsyncScanner>,
    /// Plugin registry for processing
    plugin_registry: SharedPluginRegistry,
    /// Name for this scanner instance
    name: String,
}

impl PluginScanner {
    /// Create a new plugin-enabled scanner
    pub fn new(
        scanner: Arc<dyn AsyncScanner>,
        plugin_registry: SharedPluginRegistry,
    ) -> Self {
        let name = format!("PluginScanner<{}>", scanner.name());
        Self {
            inner_scanner: scanner,
            plugin_registry,
            name,
        }
    }

    /// Wrap a scanner to add plugin processing
    pub fn wrap(scanner: impl AsyncScanner + 'static, plugin_registry: SharedPluginRegistry) -> Arc<Self> {
        Arc::new(Self::new(Arc::new(scanner), plugin_registry))
    }
}

#[async_trait]
impl AsyncScanner for PluginScanner {
    fn name(&self) -> &str {
        &self.name
    }

    fn supports_mode(&self, mode: ScanMode) -> bool {
        self.inner_scanner.supports_mode(mode)
    }

    async fn scan_async(&self, repository_path: &std::path::Path, mode: ScanMode) -> ScanResult<ScanMessageStream> {
        // Get the stream from inner scanner with repository path
        let inner_stream = self.inner_scanner.scan_async(repository_path, mode).await?;
        
        // Create plugin executor for this scan mode
        let executor = Arc::new(PluginExecutor::new(
            self.plugin_registry.clone(),
            mode
        ));

        // Wrap the stream with plugin processing
        let plugin_stream = PluginProcessingStream::new(inner_stream, executor);
        
        Ok(Box::pin(plugin_stream))
    }

    async fn estimate_message_count(&self, repository_path: &std::path::Path, modes: ScanMode) -> Option<usize> {
        // Estimate might be higher due to plugin-generated messages
        if let Some(base_count) = self.inner_scanner.estimate_message_count(repository_path, modes).await {
            // Add 20% for potential plugin-generated messages
            Some((base_count as f64 * 1.2) as usize)
        } else {
            None
        }
    }
}

/// Stream that processes messages through plugins
#[pin_project]
struct PluginProcessingStream {
    #[pin]
    inner: ScanMessageStream,
    executor: Arc<PluginExecutor>,
    /// Buffer for plugin-generated messages
    message_buffer: Vec<ScanMessage>,
    /// Track if we've completed the inner stream
    inner_done: bool,
}

impl PluginProcessingStream {
    fn new(inner: ScanMessageStream, executor: Arc<PluginExecutor>) -> Self {
        Self {
            inner,
            executor,
            message_buffer: Vec::new(),
            inner_done: false,
        }
    }
}

impl Stream for PluginProcessingStream {
    type Item = ScanResult<ScanMessage>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // First, drain any buffered messages
        if let Some(message) = this.message_buffer.pop() {
            return Poll::Ready(Some(Ok(message)));
        }

        // If inner stream is done and buffer is empty, we're done
        if *this.inner_done {
            return Poll::Ready(None);
        }

        // Poll inner stream for next message
        match this.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(message))) => {
                // Clone executor for async processing
                let executor = Arc::clone(this.executor);
                let message_clone = message.clone();

                // Process through plugins asynchronously
                // We need to spawn this as a separate task to avoid blocking
                let executor_clone = Arc::clone(&executor);
                let _handle = tokio::spawn(async move {
                    executor_clone.process_message(message_clone).await
                });
                
                // For now, we'll just pass through the original message
                // A more complete implementation would properly handle the async processing
                this.message_buffer.push(message.clone());

                // Return the first result (or signal pending if none)
                if let Some(first) = this.message_buffer.pop() {
                    Poll::Ready(Some(Ok(first)))
                } else {
                    // No results from plugins, try polling again
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => {
                *this.inner_done = true;
                // Check if we still have buffered messages
                if let Some(message) = this.message_buffer.pop() {
                    Poll::Ready(Some(Ok(message)))
                } else {
                    Poll::Ready(None)
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Builder for creating plugin-enabled scanner configurations
pub struct PluginScannerBuilder {
    scanners: Vec<Arc<dyn AsyncScanner>>,
    plugin_registry: Option<SharedPluginRegistry>,
}

impl PluginScannerBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            scanners: Vec::new(),
            plugin_registry: None,
        }
    }

    /// Add a scanner to be wrapped with plugin processing
    pub fn add_scanner(mut self, scanner: Arc<dyn AsyncScanner>) -> Self {
        self.scanners.push(scanner);
        self
    }

    /// Set the plugin registry
    pub fn plugin_registry(mut self, registry: SharedPluginRegistry) -> Self {
        self.plugin_registry = Some(registry);
        self
    }

    /// Build plugin-enabled scanners
    pub fn build(self) -> Result<Vec<Arc<dyn AsyncScanner>>, ScanError> {
        let registry = self.plugin_registry
            .ok_or_else(|| ScanError::configuration("Plugin registry not set"))?;

        let wrapped_scanners: Vec<Arc<dyn AsyncScanner>> = self.scanners
            .into_iter()
            .map(|scanner| {
                Arc::new(PluginScanner::new(scanner, registry.clone())) as Arc<dyn AsyncScanner>
            })
            .collect();

        Ok(wrapped_scanners)
    }
}

impl Default for PluginScannerBuilder {
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
