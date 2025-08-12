//! Streaming Producer (Simplified)
//! 
//! Provides streaming integration without queue dependency.
//! Messages are handled directly via plugin callbacks.

use std::time::Duration;
use crate::scanner::traits::MessageProducer;
use crate::scanner::messages::ScanMessage;
use crate::scanner::async_engine::task_manager::MemoryPressureLevel;
use super::error::ScanResult;

/// Configuration for streaming producer
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    /// Batch size for operations (unused but kept for compatibility)
    pub batch_size: usize,
    /// Buffer size for internal queuing (unused but kept for compatibility)
    pub buffer_size: usize,
    /// Timeout for batch operations (unused but kept for compatibility)
    pub batch_timeout: Duration,
    /// Enable adaptive batching (unused but kept for compatibility)
    pub adaptive_batching: bool,
    /// Maximum batch size when adaptive (unused but kept for compatibility)
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

/// Simplified streaming producer that works via callbacks
pub struct StreamingQueueProducer {
    producer_name: String,
}

/// Statistics for streaming producer
#[derive(Debug, Clone)]
pub struct StreamingStats {
    pub messages_produced: usize,
    pub batches_sent: usize,
    pub average_batch_size: f64,
    pub total_bytes_processed: usize,
    pub current_backpressure: MemoryPressureLevel,
    pub adaptive_batch_size: usize,
}

impl Default for StreamingStats {
    fn default() -> Self {
        Self {
            messages_produced: 0,
            batches_sent: 0,
            average_batch_size: 0.0,
            total_bytes_processed: 0,
            current_backpressure: MemoryPressureLevel::Normal,
            adaptive_batch_size: 50,
        }
    }
}

impl StreamingQueueProducer {
    /// Create a new streaming producer (simplified)
    pub fn new(producer_name: String) -> ScanResult<Self> {
        Ok(Self {
            producer_name,
        })
    }

    /// Create with defaults (compatibility method)
    pub fn with_defaults(producer_name: String) -> ScanResult<Self> {
        Self::new(producer_name)
    }

    /// Create with configuration (config parameter ignored)
    pub fn with_config(_config: StreamingConfig, producer_name: String) -> ScanResult<Self> {
        Ok(Self {
            producer_name,
        })
    }

    /// Produce a single message (no-op implementation)
    pub async fn produce_message(&self, _message: ScanMessage) -> ScanResult<()> {
        // Messages are handled directly via plugin callbacks
        log::debug!("Message produced by {} (handled via callbacks)", self.producer_name);
        Ok(())
    }

    /// Produce multiple messages in batch
    pub async fn produce_batch(&self, messages: Vec<ScanMessage>) -> ScanResult<()> {
        for _message in messages {
            self.produce_message(_message).await?;
        }
        Ok(())
    }

    /// Flush pending messages (no-op)
    pub async fn flush(&self) -> ScanResult<()> {
        log::debug!("Flush called on {} (no-op)", self.producer_name);
        Ok(())
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> StreamingStats {
        StreamingStats::default()
    }

    /// Shutdown the producer (no-op)
    pub async fn shutdown(&self) -> ScanResult<()> {
        log::debug!("Shutdown called on {} (no-op)", self.producer_name);
        Ok(())
    }

    /// Get producer name
    pub fn get_name(&self) -> &str {
        &self.producer_name
    }
}

impl MessageProducer for StreamingQueueProducer {
    fn produce_message(&self, _message: ScanMessage) {
        // Messages are handled directly via plugin callbacks
        log::debug!("Message produced by {} (handled via callbacks)", self.producer_name);
    }
    
    fn get_producer_name(&self) -> &str {
        &self.producer_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, MessageData};

    fn create_test_message(id: u64) -> ScanMessage {
        ScanMessage::new(
            MessageHeader::new(id),
            MessageData::FileInfo {
                path: format!("test_{}.rs", id),
                size: 1024,
                lines: 50,
            },
        )
    }

    #[tokio::test]
    async fn test_streaming_producer_creation() {
        let producer = StreamingQueueProducer::new("test".to_string()).unwrap();
        assert_eq!(producer.get_name(), "test");
    }

    #[tokio::test]
    async fn test_produce_single_message() {
        let producer = StreamingQueueProducer::new("test".to_string()).unwrap();
        let message = create_test_message(1);
        
        let result = producer.produce_message(message).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_produce_batch() {
        let producer = StreamingQueueProducer::new("test".to_string()).unwrap();
        let messages = (0..5).map(create_test_message).collect();
        
        let result = producer.produce_batch(messages).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_flush_and_shutdown() {
        let producer = StreamingQueueProducer::new("test".to_string()).unwrap();
        
        assert!(producer.flush().await.is_ok());
        assert!(producer.shutdown().await.is_ok());
    }

    #[tokio::test]
    async fn test_stats() {
        let producer = StreamingQueueProducer::new("test".to_string()).unwrap();
        let stats = producer.get_stats().await;
        
        assert_eq!(stats.messages_produced, 0);
        assert_eq!(stats.batches_sent, 0);
    }
}