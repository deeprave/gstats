//! ScanProcessor to MessageListener Bridge
//! 
//! Provides an adapter to use existing ScanProcessor implementations
//! as MessageListeners in the observer pattern.

use std::sync::Arc;
use crate::queue::MessageListener;
use crate::scanner::messages::ScanMessage;
use crate::scanner::modes::ScanMode;
use crate::scanner::traits::ScanProcessor;

/// Bridge adapter to use ScanProcessor as MessageListener
pub struct ScanProcessorBridge {
    processor: Arc<dyn ScanProcessor + Send + Sync>,
    supported_modes: ScanMode,
    id: String,
    name: String,
}

impl ScanProcessorBridge {
    /// Create a new bridge adapter
    pub fn new(processor: Arc<dyn ScanProcessor + Send + Sync>, supported_modes: ScanMode, id: String) -> Self {
        let name = format!("ScanProcessorBridge({})", id);
        Self {
            processor,
            supported_modes,
            id,
            name,
        }
    }

    /// Create a new bridge adapter with custom name
    pub fn with_name(
        processor: Arc<dyn ScanProcessor + Send + Sync>,
        supported_modes: ScanMode,
        id: String,
        name: String,
    ) -> Self {
        Self {
            processor,
            supported_modes,
            id,
            name,
        }
    }

    /// Get the wrapped processor
    pub fn processor(&self) -> &Arc<dyn ScanProcessor + Send + Sync> {
        &self.processor
    }

    /// Get supported scan modes
    pub fn supported_modes(&self) -> ScanMode {
        self.supported_modes
    }
}

impl MessageListener for ScanProcessorBridge {
    fn interested_modes(&self) -> ScanMode {
        self.supported_modes
    }
    
    fn on_message(&self, message: &ScanMessage) -> Result<(), Box<dyn std::error::Error>> {
        // Delegate to the wrapped ScanProcessor
        self.processor.process_message(message)
    }
    
    fn listener_id(&self) -> String {
        self.id.clone()
    }
    
    fn listener_name(&self) -> String {
        self.name.clone()
    }
}

/// Builder for creating ScanProcessorBridge instances
pub struct ScanProcessorBridgeBuilder {
    processor: Option<Arc<dyn ScanProcessor + Send + Sync>>,
    supported_modes: Option<ScanMode>,
    id: Option<String>,
    name: Option<String>,
}

impl ScanProcessorBridgeBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            processor: None,
            supported_modes: None,
            id: None,
            name: None,
        }
    }

    /// Set the processor to wrap
    pub fn processor(mut self, processor: Arc<dyn ScanProcessor + Send + Sync>) -> Self {
        self.processor = Some(processor);
        self
    }

    /// Set the supported scan modes
    pub fn supported_modes(mut self, modes: ScanMode) -> Self {
        self.supported_modes = Some(modes);
        self
    }

    /// Set the listener ID
    pub fn id<S: Into<String>>(mut self, id: S) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the listener name
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Build the bridge adapter
    pub fn build(self) -> Result<ScanProcessorBridge, String> {
        let processor = self.processor.ok_or("Processor is required")?;
        let supported_modes = self.supported_modes.ok_or("Supported modes are required")?;
        let id = self.id.ok_or("ID is required")?;

        if let Some(name) = self.name {
            Ok(ScanProcessorBridge::with_name(processor, supported_modes, id, name))
        } else {
            Ok(ScanProcessorBridge::new(processor, supported_modes, id))
        }
    }
}

impl Default for ScanProcessorBridgeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for creating bridges from common processor types
pub mod bridge_utils {
    use super::*;

    /// Create a bridge for a files-only processor
    pub fn files_processor_bridge(
        processor: Arc<dyn ScanProcessor + Send + Sync>,
        id: String,
    ) -> ScanProcessorBridge {
        ScanProcessorBridge::new(processor, ScanMode::FILES, id)
    }

    /// Create a bridge for a history-only processor
    pub fn history_processor_bridge(
        processor: Arc<dyn ScanProcessor + Send + Sync>,
        id: String,
    ) -> ScanProcessorBridge {
        ScanProcessorBridge::new(processor, ScanMode::HISTORY, id)
    }

    /// Create a bridge for a metrics-only processor
    pub fn metrics_processor_bridge(
        processor: Arc<dyn ScanProcessor + Send + Sync>,
        id: String,
    ) -> ScanProcessorBridge {
        ScanProcessorBridge::new(processor, ScanMode::METRICS, id)
    }

    /// Create a bridge for a processor that handles all scan modes
    pub fn universal_processor_bridge(
        processor: Arc<dyn ScanProcessor + Send + Sync>,
        id: String,
    ) -> ScanProcessorBridge {
        ScanProcessorBridge::new(
            processor,
            ScanMode::FILES | ScanMode::HISTORY | ScanMode::METRICS,
            id,
        )
    }

    /// Create a bridge with automatic mode detection based on processor name/type
    pub fn auto_detect_bridge(
        processor: Arc<dyn ScanProcessor + Send + Sync>,
        id: String,
    ) -> ScanProcessorBridge {
        // Simple heuristic based on ID to determine likely supported modes
        let modes = if id.to_lowercase().contains("file") {
            ScanMode::FILES
        } else if id.to_lowercase().contains("history") || id.to_lowercase().contains("git") {
            ScanMode::HISTORY
        } else if id.to_lowercase().contains("metric") || id.to_lowercase().contains("stat") {
            ScanMode::METRICS
        } else {
            // Default to all modes if we can't determine
            ScanMode::FILES | ScanMode::HISTORY | ScanMode::METRICS
        };

        ScanProcessorBridge::new(processor, modes, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};
    use anyhow::Result;

    // Mock processor for testing
    #[derive(Debug)]
    struct MockProcessor {
        id: String,
        processed_count: std::sync::atomic::AtomicUsize,
        should_error: std::sync::atomic::AtomicBool,
    }

    unsafe impl Send for MockProcessor {}
    unsafe impl Sync for MockProcessor {}

    impl MockProcessor {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_string(),
                processed_count: std::sync::atomic::AtomicUsize::new(0),
                should_error: std::sync::atomic::AtomicBool::new(false),
            }
        }

        fn set_should_error(&self, should_error: bool) {
            self.should_error.store(should_error, std::sync::atomic::Ordering::Relaxed);
        }

        fn processed_count(&self) -> usize {
            self.processed_count.load(std::sync::atomic::Ordering::Relaxed)
        }
    }

    impl ScanProcessor for MockProcessor {
        fn process_message(&self, _message: &ScanMessage) -> Result<(), Box<dyn std::error::Error>> {
            if self.should_error.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("Test error".into());
            }
            
            self.processed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        }

        fn get_processed_count(&self) -> usize {
            self.processed_count.load(std::sync::atomic::Ordering::Relaxed)
        }

        fn reset(&self) {
            self.processed_count.store(0, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn create_test_message(mode: ScanMode) -> ScanMessage {
        ScanMessage::new(
            MessageHeader::new(mode, 12345),
            MessageData::FileInfo {
                path: "test.rs".to_string(),
                size: 1024,
                lines: 50,
            }
        )
    }

    #[test]
    fn test_bridge_creation() {
        let processor = Arc::new(MockProcessor::new("test"));
        let bridge = ScanProcessorBridge::new(processor, ScanMode::FILES, "test_bridge".to_string());
        
        assert_eq!(bridge.listener_id(), "test_bridge");
        assert_eq!(bridge.interested_modes(), ScanMode::FILES);
        assert_eq!(bridge.listener_name(), "ScanProcessorBridge(test_bridge)");
    }

    #[test]
    fn test_bridge_with_custom_name() {
        let processor = Arc::new(MockProcessor::new("test"));
        let bridge = ScanProcessorBridge::with_name(
            processor,
            ScanMode::FILES,
            "test_bridge".to_string(),
            "Custom Bridge Name".to_string(),
        );
        
        assert_eq!(bridge.listener_id(), "test_bridge");
        assert_eq!(bridge.listener_name(), "Custom Bridge Name");
    }

    #[test]
    fn test_message_processing() {
        let processor = Arc::new(MockProcessor::new("test"));
        let processor_ref = Arc::clone(&processor);
        let bridge = ScanProcessorBridge::new(processor, ScanMode::FILES, "test_bridge".to_string());
        
        let message = create_test_message(ScanMode::FILES);
        
        assert!(bridge.on_message(&message).is_ok());
        assert_eq!(processor_ref.processed_count(), 1);
    }

    #[test]
    fn test_error_propagation() {
        let processor = Arc::new(MockProcessor::new("test"));
        processor.set_should_error(true);
        let bridge = ScanProcessorBridge::new(processor, ScanMode::FILES, "test_bridge".to_string());
        
        let message = create_test_message(ScanMode::FILES);
        
        assert!(bridge.on_message(&message).is_err());
    }

    #[test]
    fn test_builder() {
        let processor = Arc::new(MockProcessor::new("test"));
        
        let bridge = ScanProcessorBridgeBuilder::new()
            .processor(processor)
            .supported_modes(ScanMode::FILES | ScanMode::HISTORY)
            .id("builder_test")
            .name("Builder Test Bridge")
            .build()
            .unwrap();
        
        assert_eq!(bridge.listener_id(), "builder_test");
        assert_eq!(bridge.listener_name(), "Builder Test Bridge");
        assert_eq!(bridge.interested_modes(), ScanMode::FILES | ScanMode::HISTORY);
    }

    #[test]
    fn test_builder_missing_required_fields() {
        // Missing processor
        let result = ScanProcessorBridgeBuilder::new()
            .supported_modes(ScanMode::FILES)
            .id("test")
            .build();
        assert!(result.is_err());
        
        // Missing supported modes
        let processor = Arc::new(MockProcessor::new("test"));
        let result = ScanProcessorBridgeBuilder::new()
            .processor(processor)
            .id("test")
            .build();
        assert!(result.is_err());
        
        // Missing ID
        let processor = Arc::new(MockProcessor::new("test"));
        let result = ScanProcessorBridgeBuilder::new()
            .processor(processor)
            .supported_modes(ScanMode::FILES)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_utility_functions() {
        let processor = Arc::new(MockProcessor::new("test"));
        
        // Test files processor bridge
        let files_bridge = bridge_utils::files_processor_bridge(Arc::clone(&processor), "files_test".to_string());
        assert_eq!(files_bridge.interested_modes(), ScanMode::FILES);
        
        // Test history processor bridge
        let history_bridge = bridge_utils::history_processor_bridge(Arc::clone(&processor), "history_test".to_string());
        assert_eq!(history_bridge.interested_modes(), ScanMode::HISTORY);
        
        // Test metrics processor bridge
        let metrics_bridge = bridge_utils::metrics_processor_bridge(Arc::clone(&processor), "metrics_test".to_string());
        assert_eq!(metrics_bridge.interested_modes(), ScanMode::METRICS);
        
        // Test universal processor bridge
        let universal_bridge = bridge_utils::universal_processor_bridge(Arc::clone(&processor), "universal_test".to_string());
        assert_eq!(universal_bridge.interested_modes(), ScanMode::FILES | ScanMode::HISTORY | ScanMode::METRICS);
    }

    #[test]
    fn test_auto_detect_bridge() {
        let processor = Arc::new(MockProcessor::new("test"));
        
        // Test file detection
        let file_bridge = bridge_utils::auto_detect_bridge(Arc::clone(&processor), "file_processor".to_string());
        assert_eq!(file_bridge.interested_modes(), ScanMode::FILES);
        
        // Test history detection
        let history_bridge = bridge_utils::auto_detect_bridge(Arc::clone(&processor), "git_history_processor".to_string());
        assert_eq!(history_bridge.interested_modes(), ScanMode::HISTORY);
        
        // Test metrics detection
        let metrics_bridge = bridge_utils::auto_detect_bridge(Arc::clone(&processor), "statistics_processor".to_string());
        assert_eq!(metrics_bridge.interested_modes(), ScanMode::METRICS);
        
        // Test unknown defaults to all
        let unknown_bridge = bridge_utils::auto_detect_bridge(Arc::clone(&processor), "unknown_processor".to_string());
        assert_eq!(unknown_bridge.interested_modes(), ScanMode::FILES | ScanMode::HISTORY | ScanMode::METRICS);
    }
}