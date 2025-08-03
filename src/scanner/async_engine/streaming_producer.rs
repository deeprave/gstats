//! Streaming Queue Producer
//! 
//! Provides efficient streaming integration with the memory queue system.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_stream::{Stream, StreamExt};
use crate::scanner::traits::MessageProducer;
use crate::scanner::messages::ScanMessage;
use crate::queue::{self, Queue, MemoryQueue, MemoryPressureLevel};
use super::error::{ScanError, ScanResult};
use super::stream::{StreamProgress, ScanMessageStream};

/// Configuration for streaming producer
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    /// Batch size for queue operations
    pub batch_size: usize,
    /// Buffer size for internal queuing
    pub buffer_size: usize,
    /// Timeout for batch operations
    pub batch_timeout: Duration,
    /// Enable adaptive batching based on queue pressure
    pub adaptive_batching: bool,
    /// Maximum batch size when adaptive
    pub max_adaptive_batch_size: usize,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            batch_size: 50,
            buffer_size: 1000,
            batch_timeout: Duration::from_millis(100),
            adaptive_batching: true,
            max_adaptive_batch_size: 200,
        }
    }
}

/// Streaming producer that efficiently feeds messages to the queue
pub struct StreamingQueueProducer {
    queue: Arc<MemoryQueue>,
    config: StreamingConfig,
    producer_name: String,
    
    // Internal state
    sender: mpsc::UnboundedSender<ProducerCommand>,
    _background_task: tokio::task::JoinHandle<()>,
}

/// Commands sent to the background producer task
#[derive(Debug)]
enum ProducerCommand {
    Message(ScanMessage),
    Batch(Vec<ScanMessage>),
    Flush,
    Shutdown,
}

/// Statistics for streaming producer
#[derive(Debug, Clone)]
pub struct StreamingStats {
    pub messages_produced: usize,
    pub batches_sent: usize,
    pub average_batch_size: f64,
    pub total_bytes_processed: usize,
    pub start_time: Instant,
    pub last_activity: Instant,
    pub current_throughput: f64,
}

impl StreamingStats {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            messages_produced: 0,
            batches_sent: 0,
            average_batch_size: 0.0,
            total_bytes_processed: 0,
            start_time: now,
            last_activity: now,
            current_throughput: 0.0,
        }
    }
    
    fn update(&mut self, batch_size: usize, bytes: usize) {
        self.messages_produced += batch_size;
        self.batches_sent += 1;
        self.total_bytes_processed += bytes;
        self.last_activity = Instant::now();
        
        self.average_batch_size = self.messages_produced as f64 / self.batches_sent as f64;
        
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.current_throughput = self.messages_produced as f64 / elapsed;
        }
    }
    
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
    
    pub fn idle_time(&self) -> Duration {
        self.last_activity.elapsed()
    }
}

impl StreamingQueueProducer {
    /// Create a new streaming producer
    pub fn new(
        queue: Arc<MemoryQueue>,
        config: StreamingConfig,
        producer_name: String,
    ) -> ScanResult<Self> {
        let (sender, receiver) = mpsc::unbounded_channel();
        
        let queue_clone = Arc::clone(&queue);
        let config_clone = config.clone();
        let name_clone = producer_name.clone();
        
        let background_task = tokio::spawn(async move {
            if let Err(e) = Self::background_producer_task(
                queue_clone,
                config_clone,
                name_clone,
                receiver,
            ).await {
                log::error!("Streaming producer background task failed: {}", e);
            }
        });
        
        Ok(Self {
            queue,
            config,
            producer_name,
            sender,
            _background_task: background_task,
        })
    }
    
    /// Create a new streaming producer with default configuration
    pub fn with_defaults(queue: Arc<MemoryQueue>, producer_name: String) -> ScanResult<Self> {
        Self::new(queue, StreamingConfig::default(), producer_name)
    }
    
    /// Process a stream of messages efficiently
    pub async fn process_stream<S>(&self, mut stream: S) -> ScanResult<StreamingStats>
    where
        S: Stream<Item = ScanResult<ScanMessage>> + Unpin,
    {
        let mut batch = Vec::with_capacity(self.config.batch_size);
        let mut stats = StreamingStats::new();
        let mut last_batch_time = Instant::now();
        
        while let Some(result) = stream.next().await {
            match result {
                Ok(message) => {
                    batch.push(message);
                    
                    // Check if we should send the batch
                    let should_send = batch.len() >= self.config.batch_size ||
                        last_batch_time.elapsed() >= self.config.batch_timeout ||
                        (self.config.adaptive_batching && self.should_send_adaptive_batch(&batch));
                    
                    if should_send {
                        let batch_size = batch.len();
                        let total_bytes: usize = batch.iter().map(|m| m.estimate_memory_usage()).sum();
                        
                        self.send_batch(std::mem::take(&mut batch)).await?;
                        stats.update(batch_size, total_bytes);
                        last_batch_time = Instant::now();
                    }
                }
                Err(e) => {
                    // Send any pending batch before propagating error
                    if !batch.is_empty() {
                        let batch_size = batch.len();
                        let total_bytes: usize = batch.iter().map(|m| m.estimate_memory_usage()).sum();
                        
                        self.send_batch(std::mem::take(&mut batch)).await?;
                        stats.update(batch_size, total_bytes);
                    }
                    return Err(e);
                }
            }
        }
        
        // Send any remaining messages
        if !batch.is_empty() {
            let batch_size = batch.len();
            let total_bytes: usize = batch.iter().map(|m| m.estimate_memory_usage()).sum();
            
            self.send_batch(batch).await?;
            stats.update(batch_size, total_bytes);
        }
        
        // Ensure all messages are flushed
        self.flush().await?;
        
        Ok(stats)
    }
    
    /// Send a batch of messages to the background task
    async fn send_batch(&self, batch: Vec<ScanMessage>) -> ScanResult<()> {
        if batch.is_empty() {
            return Ok(());
        }
        
        self.sender.send(ProducerCommand::Batch(batch))
            .map_err(|_| ScanError::stream("Producer channel closed"))?;
        
        Ok(())
    }
    
    /// Flush any pending messages
    pub async fn flush(&self) -> ScanResult<()> {
        self.sender.send(ProducerCommand::Flush)
            .map_err(|_| ScanError::stream("Producer channel closed"))?;
        
        // Give the background task time to process
        tokio::time::sleep(Duration::from_millis(10)).await;
        Ok(())
    }
    
    /// Check if we should send an adaptive batch based on queue pressure
    fn should_send_adaptive_batch(&self, batch: &[ScanMessage]) -> bool {
        if !self.config.adaptive_batching {
            return false;
        }
        
        // Get queue pressure level
        let pressure = self.queue.get_memory_pressure_level();
        
        match pressure {
            MemoryPressureLevel::Normal => {
                // Normal pressure: use larger batches for efficiency
                batch.len() >= self.config.max_adaptive_batch_size
            }
            MemoryPressureLevel::Moderate => {
                // Moderate pressure: use medium batches
                batch.len() >= self.config.batch_size / 2
            }
            MemoryPressureLevel::High | MemoryPressureLevel::Critical => {
                // High/Critical pressure: send immediately to avoid buildup
                !batch.is_empty()
            }
        }
    }
    
    /// Background task that handles actual queue operations
    async fn background_producer_task(
        queue: Arc<MemoryQueue>,
        config: StreamingConfig,
        _producer_name: String,
        mut receiver: mpsc::UnboundedReceiver<ProducerCommand>,
    ) -> ScanResult<()> {
        let mut pending_messages = Vec::new();
        let mut batch_timer = tokio::time::interval(config.batch_timeout);
        batch_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        
        loop {
            tokio::select! {
                // Handle incoming commands
                command = receiver.recv() => {
                    match command {
                        Some(ProducerCommand::Message(message)) => {
                            pending_messages.push(message);
                            
                            if pending_messages.len() >= config.batch_size {
                                Self::send_to_queue(&queue, &mut pending_messages).await?;
                            }
                        }
                        Some(ProducerCommand::Batch(mut batch)) => {
                            pending_messages.append(&mut batch);
                            
                            // Send in chunks if too large
                            while pending_messages.len() >= config.batch_size {
                                let chunk = pending_messages.split_off(config.batch_size);
                                let to_send = std::mem::replace(&mut pending_messages, chunk);
                                for msg in to_send {
                                    queue.enqueue(msg).map_err(|e| ScanError::stream(e.to_string()))?;
                                }
                            }
                        }
                        Some(ProducerCommand::Flush) => {
                            if !pending_messages.is_empty() {
                                Self::send_to_queue(&queue, &mut pending_messages).await?;
                            }
                        }
                        Some(ProducerCommand::Shutdown) | None => {
                            // Send any remaining messages before shutdown
                            if !pending_messages.is_empty() {
                                Self::send_to_queue(&queue, &mut pending_messages).await?;
                            }
                            break;
                        }
                    }
                }
                
                // Handle timeout-based flushing
                _ = batch_timer.tick() => {
                    if !pending_messages.is_empty() {
                        Self::send_to_queue(&queue, &mut pending_messages).await?;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Send pending messages to the queue
    async fn send_to_queue(
        queue: &Arc<MemoryQueue>,
        pending_messages: &mut Vec<ScanMessage>,
    ) -> ScanResult<()> {
        for message in pending_messages.drain(..) {
            queue.enqueue(message)
                .map_err(|e| ScanError::stream(e.to_string()))?;
        }
        Ok(())
    }
    
    /// Get current queue status
    pub fn queue_status(&self) -> String {
        format!(
            "Queue: {} messages, {:.1}% memory",
            self.queue.size(),
            self.queue.memory_usage_percent()
        )
    }
}

impl MessageProducer for StreamingQueueProducer {
    fn produce_message(&self, message: ScanMessage) {
        // Send to background task (non-blocking)
        if let Err(_) = self.sender.send(ProducerCommand::Message(message)) {
            log::error!("Failed to send message to streaming producer: channel closed");
        }
    }
    
    fn get_producer_name(&self) -> &str {
        &self.producer_name
    }
}

/// Stream-to-queue adapter for easy integration
pub struct StreamToQueueAdapter {
    producer: StreamingQueueProducer,
}

impl StreamToQueueAdapter {
    /// Create a new adapter
    pub fn new(queue: Arc<MemoryQueue>, producer_name: String) -> ScanResult<Self> {
        let producer = StreamingQueueProducer::with_defaults(queue, producer_name)?;
        Ok(Self { producer })
    }
    
    /// Create with custom configuration
    pub fn with_config(
        queue: Arc<MemoryQueue>,
        config: StreamingConfig,
        producer_name: String,
    ) -> ScanResult<Self> {
        let producer = StreamingQueueProducer::new(queue, config, producer_name)?;
        Ok(Self { producer })
    }
    
    /// Process a stream and return statistics
    pub async fn process_stream<S>(&self, stream: S) -> ScanResult<StreamingStats>
    where
        S: Stream<Item = ScanResult<ScanMessage>> + Unpin,
    {
        self.producer.process_stream(stream).await
    }
    
    /// Process a stream with progress tracking
    pub async fn process_stream_with_progress<S, F>(
        &self,
        stream: S,
        mut progress_callback: F,
    ) -> ScanResult<StreamingStats>
    where
        S: Stream<Item = ScanResult<ScanMessage>> + Unpin,
        F: FnMut(&StreamProgress),
    {
        use super::stream::ProgressTrackingStream;
        use futures::StreamExt;
        
        let progress_stream = ProgressTrackingStream::new(stream, None);
        let message_stream = futures::StreamExt::map(progress_stream, |result| {
            match result {
                Ok((message, progress)) => {
                    progress_callback(&progress);
                    Ok(message)
                }
                Err(e) => Err(e),
            }
        });
        
        self.producer.process_stream(message_stream).await
    }
    
    /// Get the underlying producer for direct access
    pub fn producer(&self) -> &StreamingQueueProducer {
        &self.producer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::modes::ScanMode;
    use crate::scanner::messages::{MessageHeader, MessageData};
    use futures::stream;
    
    fn create_test_message(id: u64) -> ScanMessage {
        ScanMessage::new(
            MessageHeader::new(ScanMode::FILES, id),
            MessageData::FileInfo {
                path: format!("test_{}.rs", id),
                size: 1024,
                lines: 50,
            },
        )
    }
    
    #[tokio::test]
    async fn test_streaming_producer_basic() {
        let queue = Arc::new(MemoryQueue::new(1000, 1024 * 1024));
        let producer = StreamingQueueProducer::with_defaults(
            Arc::clone(&queue),
            "TestProducer".to_string(),
        ).unwrap();
        
        let messages: Vec<ScanResult<ScanMessage>> = (0..10)
            .map(|i| Ok(create_test_message(i)))
            .collect();
        
        let test_stream = stream::iter(messages);
        let stats = producer.process_stream(test_stream).await.unwrap();
        
        assert_eq!(stats.messages_produced, 10);
        assert!(stats.batches_sent > 0);
        
        // Give background task time to process
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Check that messages made it to the queue
        assert!(queue.size() > 0);
    }
    
    #[tokio::test]
    async fn test_streaming_producer_batching() {
        let queue = Arc::new(MemoryQueue::new(1000, 1024 * 1024));
        let config = StreamingConfig {
            batch_size: 3,
            ..Default::default()
        };
        
        let producer = StreamingQueueProducer::new(
            Arc::clone(&queue),
            config,
            "BatchTest".to_string(),
        ).unwrap();
        
        let messages: Vec<ScanResult<ScanMessage>> = (0..10)
            .map(|i| Ok(create_test_message(i)))
            .collect();
        
        let test_stream = stream::iter(messages);
        let stats = producer.process_stream(test_stream).await.unwrap();
        
        assert_eq!(stats.messages_produced, 10);
        // Should have batches of size 3, 3, 3, 1
        assert_eq!(stats.batches_sent, 4);
        assert!(stats.average_batch_size > 2.0 && stats.average_batch_size < 3.0);
    }
    
    #[tokio::test]
    async fn test_stream_to_queue_adapter() {
        let queue = Arc::new(MemoryQueue::new(1000, 1024 * 1024));
        let adapter = StreamToQueueAdapter::new(
            Arc::clone(&queue),
            "AdapterTest".to_string(),
        ).unwrap();
        
        let messages: Vec<ScanResult<ScanMessage>> = (0..5)
            .map(|i| Ok(create_test_message(i)))
            .collect();
        
        let test_stream = stream::iter(messages);
        let stats = adapter.process_stream(test_stream).await.unwrap();
        
        assert_eq!(stats.messages_produced, 5);
        assert!(stats.current_throughput > 0.0);
    }
    
    #[tokio::test]
    async fn test_progress_tracking_integration() {
        let queue = Arc::new(MemoryQueue::new(1000, 1024 * 1024));
        let adapter = StreamToQueueAdapter::new(
            Arc::clone(&queue),
            "ProgressTest".to_string(),
        ).unwrap();
        
        let messages: Vec<ScanResult<ScanMessage>> = (0..5)
            .map(|i| Ok(create_test_message(i)))
            .collect();
        
        let test_stream = stream::iter(messages);
        let mut progress_updates = 0;
        
        let stats = adapter.process_stream_with_progress(
            test_stream,
            |_progress| {
                progress_updates += 1;
            },
        ).await.unwrap();
        
        assert_eq!(stats.messages_produced, 5);
        assert_eq!(progress_updates, 5);
    }
}