//! Consumer Thread Implementation
//! 
//! Provides a dedicated consumer thread for dequeuing messages and notifying
//! registered listeners based on their interest patterns.

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use crossbeam_channel::{self, Receiver, Sender};
use crate::queue::{Queue, MemoryQueue, QueueError, ListenerRegistry};
use crate::queue::listener::DefaultListenerRegistry;
use crate::scanner::messages::ScanMessage;

/// Consumer thread configuration
#[derive(Debug, Clone)]
pub struct ConsumerConfig {
    /// Polling interval for checking queue
    pub poll_interval_ms: u64,
    /// Maximum messages to process per batch
    pub batch_size: usize,
    /// Timeout for listener notifications
    pub notification_timeout_ms: u64,
    /// Whether to continue on listener errors
    pub continue_on_error: bool,
}

impl Default for ConsumerConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 10,
            batch_size: 100,
            notification_timeout_ms: 1000,
            continue_on_error: true,
        }
    }
}

/// Consumer thread metrics
#[derive(Debug, Default)]
pub struct ConsumerMetrics {
    pub messages_processed: u64,
    pub notifications_sent: u64,
    pub notification_errors: u64,
    pub batches_processed: u64,
    pub average_batch_size: f64,
    pub average_notification_latency: Duration,
}

/// Consumer thread for processing queue messages and notifying listeners
pub struct MessageConsumer {
    queue: Arc<MemoryQueue>,
    registry: Arc<Mutex<DefaultListenerRegistry>>,
    config: ConsumerConfig,
    running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
    control_sender: Option<Sender<ConsumerCommand>>,
    metrics: Arc<Mutex<ConsumerMetrics>>,
}

/// Commands for controlling the consumer thread
#[derive(Debug)]
enum ConsumerCommand {
    Stop,
    UpdateConfig(ConsumerConfig),
    GetMetrics,
}

impl MessageConsumer {
    /// Create a new message consumer
    pub fn new(queue: Arc<MemoryQueue>, registry: Arc<Mutex<DefaultListenerRegistry>>) -> Self {
        Self {
            queue,
            registry,
            config: ConsumerConfig::default(),
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
            control_sender: None,
            metrics: Arc::new(Mutex::new(ConsumerMetrics::default())),
        }
    }

    /// Create a new message consumer with custom configuration
    pub fn with_config(
        queue: Arc<MemoryQueue>,
        registry: Arc<Mutex<DefaultListenerRegistry>>,
        config: ConsumerConfig,
    ) -> Self {
        Self {
            queue,
            registry,
            config,
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
            control_sender: None,
            metrics: Arc::new(Mutex::new(ConsumerMetrics::default())),
        }
    }

    /// Start the consumer thread
    pub fn start(&mut self) -> Result<(), QueueError> {
        if self.running.load(Ordering::Relaxed) {
            return Err(QueueError::InvalidConfiguration("Consumer already running".to_string()));
        }

        let (sender, receiver) = crossbeam_channel::unbounded();
        self.control_sender = Some(sender);

        let queue = Arc::clone(&self.queue);
        let registry = Arc::clone(&self.registry);
        let config = self.config.clone();
        let running = Arc::clone(&self.running);
        let metrics = Arc::clone(&self.metrics);

        self.running.store(true, Ordering::Relaxed);

        let handle = thread::spawn(move || {
            Self::consumer_loop(queue, registry, config, running, receiver, metrics);
        });

        self.thread_handle = Some(handle);
        Ok(())
    }

    /// Stop the consumer thread
    pub fn stop(&mut self) -> Result<(), QueueError> {
        if !self.running.load(Ordering::Relaxed) {
            return Ok(()); // Already stopped
        }

        // Send stop command
        if let Some(sender) = &self.control_sender {
            let _ = sender.send(ConsumerCommand::Stop);
        }

        self.running.store(false, Ordering::Relaxed);

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            handle.join().map_err(|_| {
                QueueError::InvalidConfiguration("Failed to join consumer thread".to_string())
            })?;
        }

        self.control_sender = None;
        Ok(())
    }

    /// Update consumer configuration
    pub fn update_config(&mut self, config: ConsumerConfig) -> Result<(), QueueError> {
        self.config = config.clone();
        
        if let Some(sender) = &self.control_sender {
            sender.send(ConsumerCommand::UpdateConfig(config))
                .map_err(|_| QueueError::InvalidConfiguration("Failed to send config update".to_string()))?;
        }
        
        Ok(())
    }

    /// Get consumer metrics
    pub fn get_metrics(&self) -> ConsumerMetrics {
        if let Ok(metrics) = self.metrics.lock() {
            ConsumerMetrics {
                messages_processed: metrics.messages_processed,
                notifications_sent: metrics.notifications_sent,
                notification_errors: metrics.notification_errors,
                batches_processed: metrics.batches_processed,
                average_batch_size: metrics.average_batch_size,
                average_notification_latency: metrics.average_notification_latency,
            }
        } else {
            ConsumerMetrics::default()
        }
    }

    /// Check if consumer is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Main consumer loop
    fn consumer_loop(
        queue: Arc<MemoryQueue>,
        registry: Arc<Mutex<DefaultListenerRegistry>>,
        mut config: ConsumerConfig,
        running: Arc<AtomicBool>,
        control_receiver: Receiver<ConsumerCommand>,
        metrics: Arc<Mutex<ConsumerMetrics>>,
    ) {
        let poll_interval = Duration::from_millis(config.poll_interval_ms);

        while running.load(Ordering::Relaxed) {
            // Check for control commands
            if let Ok(command) = control_receiver.try_recv() {
                match command {
                    ConsumerCommand::Stop => break,
                    ConsumerCommand::UpdateConfig(new_config) => {
                        config = new_config;
                    }
                    ConsumerCommand::GetMetrics => {
                        // Metrics are accessed directly via get_metrics()
                    }
                }
            }

            // Process a batch of messages
            let _batch_start = Instant::now();
            let mut batch_count = 0;

            for _ in 0..config.batch_size {
                match queue.dequeue() {
                    Ok(Some(message)) => {
                        batch_count += 1;
                        if let Err(e) = Self::process_message(&message, &registry, &config, &metrics) {
                            log::error!("Error processing message: {}", e);
                            if !config.continue_on_error {
                                break;
                            }
                        }
                    }
                    Ok(None) => {
                        // Queue is empty
                        break;
                    }
                    Err(e) => {
                        log::error!("Error dequeuing message: {}", e);
                        if !config.continue_on_error {
                            break;
                        }
                    }
                }
            }

            // Update batch metrics
            if batch_count > 0 {
                if let Ok(mut metrics_guard) = metrics.lock() {
                    metrics_guard.batches_processed += 1;
                    let _total_messages = metrics_guard.messages_processed + batch_count;
                    metrics_guard.average_batch_size = 
                        (metrics_guard.average_batch_size * (metrics_guard.batches_processed - 1) as f64 + batch_count as f64) 
                        / metrics_guard.batches_processed as f64;
                }
            }

            // Sleep if no messages processed to avoid busy waiting
            if batch_count == 0 {
                thread::sleep(poll_interval);
            }
        }
    }

    /// Process a single message by notifying interested listeners
    fn process_message(
        message: &ScanMessage,
        registry: &Arc<Mutex<DefaultListenerRegistry>>,
        config: &ConsumerConfig,
        metrics: &Arc<Mutex<ConsumerMetrics>>,
    ) -> Result<(), QueueError> {
        let notification_start = Instant::now();
        let mut notifications_sent = 0;
        let mut notification_errors = 0;

        // Get message scan mode for listener filtering
        let message_mode = message.header.scan_mode;

        // Get interested listeners
        let listeners = if let Ok(registry_guard) = registry.lock() {
            registry_guard.get_interested_listeners(message_mode)
        } else {
            return Err(QueueError::ListenerError("Failed to acquire registry lock".to_string()));
        };

        // Notify each interested listener
        for listener in listeners {
            match listener.on_message(message) {
                Ok(()) => {
                    notifications_sent += 1;
                }
                Err(e) => {
                    notification_errors += 1;
                    log::warn!("Listener '{}' failed to process message: {}", listener.listener_id(), e);
                    
                    if !config.continue_on_error {
                        return Err(QueueError::ListenerError(format!(
                            "Listener notification failed: {}", e
                        )));
                    }
                }
            }
        }

        let notification_duration = notification_start.elapsed();

        // Update metrics
        if let Ok(mut metrics_guard) = metrics.lock() {
            metrics_guard.messages_processed += 1;
            metrics_guard.notifications_sent += notifications_sent;
            metrics_guard.notification_errors += notification_errors;
            
            // Update average notification latency
            let total_notifications = metrics_guard.notifications_sent;
            if total_notifications > 0 {
                let current_avg = metrics_guard.average_notification_latency.as_nanos() as f64;
                let new_duration = notification_duration.as_nanos() as f64;
                let new_avg = (current_avg * (total_notifications - notifications_sent) as f64 + new_duration) 
                    / total_notifications as f64;
                metrics_guard.average_notification_latency = Duration::from_nanos(new_avg as u64);
            } else {
                metrics_guard.average_notification_latency = notification_duration;
            }
        }

        Ok(())
    }
}

impl Drop for MessageConsumer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::memory_tracker::MemoryTracker;
    use crate::scanner::messages::{MessageHeader, MessageData};
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    // Mock listener for testing
    struct TestListener {
        id: String,
        interested_modes: ScanMode,
        received_count: Arc<AtomicUsize>,
        should_error: Arc<AtomicBool>,
    }

    impl TestListener {
        fn new(id: &str, modes: ScanMode) -> Self {
            Self {
                id: id.to_string(),
                interested_modes: modes,
                received_count: Arc::new(AtomicUsize::new(0)),
                should_error: Arc::new(AtomicBool::new(false)),
            }
        }

        fn set_should_error(&self, should_error: bool) {
            self.should_error.store(should_error, Ordering::Relaxed);
        }

        fn received_count(&self) -> usize {
            self.received_count.load(Ordering::Relaxed)
        }
    }

    impl MessageListener for TestListener {
        fn interested_modes(&self) -> ScanMode {
            self.interested_modes
        }
        
        fn on_message(&self, _message: &ScanMessage) -> Result<(), Box<dyn std::error::Error>> {
            if self.should_error.load(Ordering::Relaxed) {
                return Err("Test error".into());
            }
            
            self.received_count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        
        fn listener_id(&self) -> String {
            self.id.clone()
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
    fn test_consumer_creation() {
        let tracker = Arc::new(MemoryTracker::new(1024));
        let queue = Arc::new(MemoryQueue::with_shared_tracker(100, 1024, tracker));
        let registry = Arc::new(Mutex::new(DefaultListenerRegistry::new()));
        
        let consumer = MessageConsumer::new(queue, registry);
        assert!(!consumer.is_running());
    }

    #[test]
    fn test_consumer_start_stop() {
        let tracker = Arc::new(MemoryTracker::new(1024));
        let queue = Arc::new(MemoryQueue::with_shared_tracker(100, 1024, tracker));
        let registry = Arc::new(Mutex::new(DefaultListenerRegistry::new()));
        
        let mut consumer = MessageConsumer::new(queue, registry);
        
        assert!(consumer.start().is_ok());
        assert!(consumer.is_running());
        
        assert!(consumer.stop().is_ok());
        assert!(!consumer.is_running());
    }

    #[test]
    fn test_message_processing() {
        let tracker = Arc::new(MemoryTracker::new(1024));
        let queue = Arc::new(MemoryQueue::with_shared_tracker(100, 1024, tracker));
        let registry = Arc::new(Mutex::new(DefaultListenerRegistry::new()));
        
        // Register a test listener
        let listener = Arc::new(TestListener::new("test", ScanMode::FILES));
        let listener_count_ref = Arc::clone(&listener.received_count);
        
        {
            let mut registry_guard = registry.lock().unwrap();
            registry_guard.register_listener(listener).unwrap();
        }
        
        // Start consumer
        let mut consumer = MessageConsumer::new(Arc::clone(&queue), Arc::clone(&registry));
        consumer.start().unwrap();
        
        // Enqueue a message
        let message = create_test_message(ScanMode::FILES);
        queue.enqueue(message).unwrap();
        
        // Wait for processing
        thread::sleep(Duration::from_millis(50));
        
        // Check that listener received the message
        assert_eq!(listener_count_ref.load(Ordering::Relaxed), 1);
        
        consumer.stop().unwrap();
    }

    #[test]
    fn test_listener_filtering() {
        let tracker = Arc::new(MemoryTracker::new(1024));
        let queue = Arc::new(MemoryQueue::with_shared_tracker(100, 1024, tracker));
        let registry = Arc::new(Mutex::new(DefaultListenerRegistry::new()));
        
        // Register listeners with different interests
        let files_listener = Arc::new(TestListener::new("files", ScanMode::FILES));
        let history_listener = Arc::new(TestListener::new("history", ScanMode::HISTORY));
        
        let files_count = Arc::clone(&files_listener.received_count);
        let history_count = Arc::clone(&history_listener.received_count);
        
        {
            let mut registry_guard = registry.lock().unwrap();
            registry_guard.register_listener(files_listener).unwrap();
            registry_guard.register_listener(history_listener).unwrap();
        }
        
        // Start consumer
        let mut consumer = MessageConsumer::new(Arc::clone(&queue), Arc::clone(&registry));
        consumer.start().unwrap();
        
        // Enqueue a FILES message
        let files_message = create_test_message(ScanMode::FILES);
        queue.enqueue(files_message).unwrap();
        
        // Wait for processing
        thread::sleep(Duration::from_millis(50));
        
        // Only files listener should have received the message
        assert_eq!(files_count.load(Ordering::Relaxed), 1);
        assert_eq!(history_count.load(Ordering::Relaxed), 0);
        
        consumer.stop().unwrap();
    }

    #[test]
    fn test_error_isolation() {
        let tracker = Arc::new(MemoryTracker::new(1024));
        let queue = Arc::new(MemoryQueue::with_shared_tracker(100, 1024, tracker));
        let registry = Arc::new(Mutex::new(DefaultListenerRegistry::new()));
        
        // Register listeners - one that will error, one that won't
        let good_listener = Arc::new(TestListener::new("good", ScanMode::FILES));
        let bad_listener = Arc::new(TestListener::new("bad", ScanMode::FILES));
        
        let good_count = Arc::clone(&good_listener.received_count);
        let bad_count = Arc::clone(&bad_listener.received_count);
        
        // Make the bad listener error
        bad_listener.set_should_error(true);
        
        {
            let mut registry_guard = registry.lock().unwrap();
            registry_guard.register_listener(good_listener).unwrap();
            registry_guard.register_listener(bad_listener).unwrap();
        }
        
        // Start consumer with continue_on_error = true
        let config = ConsumerConfig {
            continue_on_error: true,
            ..ConsumerConfig::default()
        };
        let mut consumer = MessageConsumer::with_config(Arc::clone(&queue), Arc::clone(&registry), config);
        consumer.start().unwrap();
        
        // Enqueue a message
        let message = create_test_message(ScanMode::FILES);
        queue.enqueue(message).unwrap();
        
        // Wait for processing
        thread::sleep(Duration::from_millis(50));
        
        // Good listener should have received the message, bad listener should have errored
        assert_eq!(good_count.load(Ordering::Relaxed), 1);
        assert_eq!(bad_count.load(Ordering::Relaxed), 0); // Errored before incrementing
        
        // Check metrics show the error
        let metrics = consumer.get_metrics();
        assert_eq!(metrics.messages_processed, 1);
        assert_eq!(metrics.notifications_sent, 1); // Only good listener succeeded
        assert_eq!(metrics.notification_errors, 1); // Bad listener errored
        
        consumer.stop().unwrap();
    }
}