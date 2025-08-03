//! Scanner Integration for Memory-Conscious Queue
//! 
//! Provides integration between the scanner module and the queue system,
//! enabling message producers to enqueue scan results and processors
//! to consume them through the listener pattern.

use std::sync::{Arc, Mutex};
use crate::queue::{Queue, MemoryQueue, MessageConsumer, ConsumerConfig, DefaultListenerRegistry};
use crate::queue::{ScanProcessorBridge, QueueDebug, ListenerRegistry};
use crate::scanner::traits::{MessageProducer, ScanProcessor};
use crate::scanner::messages::ScanMessage;
use crate::scanner::modes::ScanMode;

/// Queue-based message producer implementation
pub struct QueueMessageProducer {
    queue: Arc<MemoryQueue>,
    name: String,
    mode_filter: Option<ScanMode>,
}

impl QueueMessageProducer {
    /// Create a new queue message producer
    pub fn new(queue: Arc<MemoryQueue>, name: String) -> Self {
        Self {
            queue,
            name,
            mode_filter: None,
        }
    }

    /// Create a producer that only accepts specific scan modes
    pub fn with_mode_filter(queue: Arc<MemoryQueue>, name: String, modes: ScanMode) -> Self {
        Self {
            queue,
            name,
            mode_filter: Some(modes),
        }
    }

    /// Get the underlying queue
    pub fn queue(&self) -> &Arc<MemoryQueue> {
        &self.queue
    }
}

impl MessageProducer for QueueMessageProducer {
    fn produce_message(&self, message: ScanMessage) {
        // Apply mode filter if configured
        if let Some(filter) = self.mode_filter {
            if !message.header.scan_mode.intersects(filter) {
                log::debug!("Producer '{}' filtered out message with mode {:?}", 
                    self.name, message.header.scan_mode);
                return;
            }
        }

        // Try to enqueue the message
        match self.queue.enqueue(message.clone()) {
            Ok(()) => {
                log::debug!("Producer '{}' enqueued message", self.name);
            }
            Err(e) => {
                // Try with backoff if regular enqueue fails
                match self.queue.enqueue_with_backoff(message) {
                    Ok(()) => {
                        log::debug!("Producer '{}' enqueued message with backoff", self.name);
                    }
                    Err(e) => {
                        log::warn!("Producer '{}' failed to enqueue message: {}", self.name, e);
                    }
                }
            }
        }
    }
    
    fn get_producer_name(&self) -> &str {
        &self.name
    }
}

/// Scanner integration manager
pub struct ScannerQueueIntegration {
    queue: Arc<MemoryQueue>,
    registry: Arc<Mutex<DefaultListenerRegistry>>,
    consumer: Option<MessageConsumer>,
    producers: Vec<Arc<QueueMessageProducer>>,
}

impl ScannerQueueIntegration {
    /// Create a new scanner queue integration
    pub fn new(queue: Arc<MemoryQueue>) -> Self {
        let registry = Arc::new(Mutex::new(DefaultListenerRegistry::new()));
        
        Self {
            queue,
            registry,
            consumer: None,
            producers: Vec::new(),
        }
    }

    /// Create a new message producer for this queue
    pub fn create_producer(&mut self, name: String) -> Arc<QueueMessageProducer> {
        let producer = Arc::new(QueueMessageProducer::new(Arc::clone(&self.queue), name));
        self.producers.push(Arc::clone(&producer));
        producer
    }

    /// Create a filtered message producer
    pub fn create_filtered_producer(&mut self, name: String, modes: ScanMode) -> Arc<QueueMessageProducer> {
        let producer = Arc::new(QueueMessageProducer::with_mode_filter(
            Arc::clone(&self.queue), 
            name, 
            modes
        ));
        self.producers.push(Arc::clone(&producer));
        producer
    }

    /// Register a scan processor as a listener
    pub fn register_processor(&mut self, 
        processor: Arc<dyn ScanProcessor + Send + Sync>, 
        modes: ScanMode,
        id: String
    ) -> Result<(), String> {
        let bridge = Arc::new(ScanProcessorBridge::new(processor, modes, id));
        
        if let Ok(mut registry) = self.registry.lock() {
            registry.register_listener(bridge)
                .map_err(|e| format!("Failed to register processor: {}", e))
        } else {
            Err("Failed to acquire registry lock".to_string())
        }
    }

    /// Start the consumer thread
    pub fn start_consumer(&mut self, config: Option<ConsumerConfig>) -> Result<(), String> {
        if self.consumer.is_some() {
            return Err("Consumer already started".to_string());
        }

        let config = config.unwrap_or_else(|| ConsumerConfig {
            poll_interval_ms: 10,
            batch_size: 50,
            notification_timeout_ms: 1000,
            continue_on_error: true,
        });

        let mut consumer = MessageConsumer::with_config(
            Arc::clone(&self.queue),
            Arc::clone(&self.registry),
            config
        );

        consumer.start()
            .map_err(|e| format!("Failed to start consumer: {}", e))?;

        self.consumer = Some(consumer);
        log::info!("Scanner queue consumer started");
        
        Ok(())
    }

    /// Stop the consumer thread
    pub fn stop_consumer(&mut self) -> Result<(), String> {
        if let Some(mut consumer) = self.consumer.take() {
            consumer.stop()
                .map_err(|e| format!("Failed to stop consumer: {}", e))?;
            log::info!("Scanner queue consumer stopped");
        }
        Ok(())
    }

    /// Get queue status
    pub fn status(&self) -> String {
        let queue_status = self.queue.debug_status();
        
        let consumer_status = if let Some(ref consumer) = self.consumer {
            if consumer.is_running() {
                let metrics = consumer.get_metrics();
                format!(
                    "Consumer: running, {} processed, {} errors",
                    metrics.messages_processed,
                    metrics.notification_errors
                )
            } else {
                "Consumer: stopped".to_string()
            }
        } else {
            "Consumer: not started".to_string()
        };

        let listener_count = if let Ok(registry) = self.registry.lock() {
            registry.listener_count()
        } else {
            0
        };

        format!(
            "{}\n{}\nProducers: {}, Listeners: {}",
            queue_status,
            consumer_status,
            self.producers.len(),
            listener_count
        )
    }

    /// Get the underlying queue
    pub fn queue(&self) -> &Arc<MemoryQueue> {
        &self.queue
    }

    /// Get the listener registry
    pub fn registry(&self) -> &Arc<Mutex<DefaultListenerRegistry>> {
        &self.registry
    }
    
    /// Get the consumer if started
    pub fn get_consumer(&self) -> Option<&MessageConsumer> {
        self.consumer.as_ref()
    }
}

impl Drop for ScannerQueueIntegration {
    fn drop(&mut self) {
        let _ = self.stop_consumer();
    }
}

/// Builder for scanner queue integration
pub struct ScannerQueueBuilder {
    capacity: usize,
    memory_limit: usize,
    consumer_config: Option<ConsumerConfig>,
}

impl ScannerQueueBuilder {
    /// Create a new builder with defaults
    pub fn new() -> Self {
        Self {
            capacity: 10000,
            memory_limit: 100 * 1024 * 1024, // 100MB default
            consumer_config: None,
        }
    }

    /// Set queue capacity
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    /// Set memory limit in bytes
    pub fn memory_limit(mut self, limit: usize) -> Self {
        self.memory_limit = limit;
        self
    }

    /// Set consumer configuration
    pub fn consumer_config(mut self, config: ConsumerConfig) -> Self {
        self.consumer_config = Some(config);
        self
    }

    /// Build the scanner queue integration
    pub fn build(self) -> ScannerQueueIntegration {
        let queue = Arc::new(MemoryQueue::with_memory_tracking(
            self.capacity,
            self.memory_limit
        ));
        
        let mut integration = ScannerQueueIntegration::new(queue);
        
        // Start consumer if config provided
        if let Some(config) = self.consumer_config {
            let _ = integration.start_consumer(Some(config));
        }
        
        integration
    }
}

impl Default for ScannerQueueBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestProcessor {
        count: AtomicUsize,
    }

    impl TestProcessor {
        fn new() -> Self {
            Self {
                count: AtomicUsize::new(0),
            }
        }

        fn processed_count(&self) -> usize {
            self.count.load(Ordering::Relaxed)
        }
    }

    impl ScanProcessor for TestProcessor {
        fn process_message(&self, _message: &ScanMessage) -> Result<(), Box<dyn std::error::Error>> {
            self.count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn get_processed_count(&self) -> usize {
            self.processed_count()
        }

        fn reset(&self) {
            self.count.store(0, Ordering::Relaxed);
        }
    }

    #[test]
    fn test_queue_message_producer() {
        let queue = Arc::new(MemoryQueue::new(100, 1024 * 1024));
        let producer = QueueMessageProducer::new(Arc::clone(&queue), "test_producer".to_string());
        
        let message = ScanMessage::new(
            MessageHeader::new(ScanMode::FILES, 12345),
            MessageData::FileInfo {
                path: "test.rs".to_string(),
                size: 1024,
                lines: 50,
            }
        );
        
        producer.produce_message(message);
        assert_eq!(queue.size(), 1);
    }

    #[test]
    fn test_scanner_integration() {
        let mut integration = ScannerQueueBuilder::new()
            .capacity(100)
            .memory_limit(1024 * 1024)
            .build();
        
        // Create a producer
        let producer = integration.create_producer("test_producer".to_string());
        
        // Register a processor
        let processor = Arc::new(TestProcessor::new());
        let processor_ref = Arc::clone(&processor);
        
        integration.register_processor(processor, ScanMode::FILES, "test_processor".to_string()).unwrap();
        
        // Start consumer
        integration.start_consumer(None).unwrap();
        
        // Produce a message
        let message = ScanMessage::new(
            MessageHeader::new(ScanMode::FILES, 12345),
            MessageData::FileInfo {
                path: "test.rs".to_string(),
                size: 1024,
                lines: 50,
            }
        );
        
        producer.produce_message(message);
        
        // Wait for processing
        std::thread::sleep(std::time::Duration::from_millis(50));
        
        // Check processor received the message
        assert_eq!(processor_ref.processed_count(), 1);
        
        // Check status
        let status = integration.status();
        assert!(status.contains("Queue:"));
        assert!(status.contains("Consumer: running"));
        assert!(status.contains("Producers: 1"));
        assert!(status.contains("Listeners: 1"));
    }

    #[test]
    fn test_mode_filtering() {
        let queue = Arc::new(MemoryQueue::new(100, 1024 * 1024));
        let producer = QueueMessageProducer::with_mode_filter(
            Arc::clone(&queue), 
            "filtered_producer".to_string(),
            ScanMode::FILES
        );
        
        // Message with FILES mode should be accepted
        let files_message = ScanMessage::new(
            MessageHeader::new(ScanMode::FILES, 12345),
            MessageData::FileInfo {
                path: "test.rs".to_string(),
                size: 1024,
                lines: 50,
            }
        );
        
        producer.produce_message(files_message);
        assert_eq!(queue.size(), 1);
        
        // Message with HISTORY mode should be filtered out
        let history_message = ScanMessage::new(
            MessageHeader::new(ScanMode::HISTORY, 12346),
            MessageData::CommitInfo {
                hash: "abc123".to_string(),
                author: "test@example.com".to_string(),
                message: "Test commit".to_string(),
            }
        );
        
        producer.produce_message(history_message);
        assert_eq!(queue.size(), 1); // Still 1, message was filtered
    }
}