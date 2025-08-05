//! Async Scanner Traits
//! 
//! Trait definitions for async scanning operations with streaming support.

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use crate::scanner::modes::ScanMode;
use crate::scanner::messages::ScanMessage;
use crate::scanner::async_engine::error::{ScanError, ScanResult};

/// Type alias for a boxed stream of scan messages
pub type ScanMessageStream = Pin<Box<dyn Stream<Item = ScanResult<ScanMessage>> + Send>>;

/// Async scanner trait with streaming interface
#[async_trait]
pub trait AsyncScanner: Send + Sync {
    /// Get the name of this scanner
    fn name(&self) -> &str;
    
    /// Check if this scanner supports the given mode
    fn supports_mode(&self, mode: ScanMode) -> bool;
    
    /// Perform an async scan with the specified modes
    /// Returns a stream of scan messages
    async fn scan_async(&self, modes: ScanMode) -> ScanResult<ScanMessageStream>;
    
    /// Get estimated message count for progress tracking (optional)
    async fn estimate_message_count(&self, _modes: ScanMode) -> Option<usize> {
        None
    }
}

/// Streaming message producer trait
#[async_trait]
pub trait StreamingMessageProducer: Send + Sync {
    /// Produce a single message asynchronously
    async fn produce_message(&self, message: ScanMessage) -> ScanResult<()>;
    
    /// Produce a batch of messages efficiently
    async fn produce_batch(&self, messages: Vec<ScanMessage>) -> ScanResult<()> {
        for message in messages {
            self.produce_message(message).await?;
        }
        Ok(())
    }
    
    /// Get the producer name
    fn name(&self) -> &str;
}

/// Async scan progress tracker
#[async_trait]
pub trait AsyncScanProgress: Send + Sync {
    /// Update progress with current count
    async fn update_progress(&self, current: usize, total: Option<usize>);
    
    /// Mark scan as completed
    async fn complete(&self);
    
    /// Report an error
    async fn report_error(&self, error: &ScanError);
}

/// Async repository scanner that can handle multiple scan modes
#[async_trait]
pub trait AsyncRepositoryScanner: AsyncScanner {
    /// Set the repository path
    async fn set_repository(&mut self, path: &str) -> ScanResult<()>;
    
    /// Get supported scan modes for the current repository
    async fn get_available_modes(&self) -> ScanMode;
}

/// Factory trait for creating scanners
#[async_trait]
pub trait AsyncScannerFactory: Send + Sync {
    /// Create a scanner for the specified repository
    async fn create_scanner(&self, repository_path: &str) -> ScanResult<Box<dyn AsyncScanner>>;
    
    /// Get factory name
    fn name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{stream, StreamExt};
    use crate::scanner::messages::{MessageHeader, MessageData};
    
    struct MockScanner {
        name: String,
        supported_modes: ScanMode,
    }
    
    #[async_trait]
    impl AsyncScanner for MockScanner {
        fn name(&self) -> &str {
            &self.name
        }
        
        fn supports_mode(&self, mode: ScanMode) -> bool {
            self.supported_modes.contains(mode)
        }
        
        async fn scan_async(&self, modes: ScanMode) -> ScanResult<ScanMessageStream> {
            if !self.supports_mode(modes) {
                return Err(ScanError::InvalidMode(modes));
            }
            
            // Create a simple stream with test messages
            let messages = vec![
                Ok(ScanMessage::new(
                    MessageHeader::new(modes, 12345),
                    MessageData::FileInfo {
                        path: "test.rs".to_string(),
                        size: 1024,
                        lines: 50,
                    },
                )),
            ];
            
            Ok(Box::pin(stream::iter(messages)))
        }
    }
    
    #[tokio::test]
    async fn test_async_scanner_trait() {
        let scanner = MockScanner {
            name: "TestScanner".to_string(),
            supported_modes: ScanMode::FILES | ScanMode::HISTORY,
        };
        
        assert_eq!(scanner.name(), "TestScanner");
        assert!(scanner.supports_mode(ScanMode::FILES));
        assert!(!scanner.supports_mode(ScanMode::SECURITY));
        
        // Test successful scan
        let stream = scanner.scan_async(ScanMode::FILES).await.unwrap();
        let messages: Vec<_> = stream.collect::<Vec<_>>().await;
        assert_eq!(messages.len(), 1);
        
        // Test unsupported mode
        let result = scanner.scan_async(ScanMode::SECURITY).await;
        assert!(matches!(result, Err(ScanError::InvalidMode(_))));
    }
    
    struct MockProducer {
        messages: tokio::sync::Mutex<Vec<ScanMessage>>,
    }
    
    #[async_trait]
    impl StreamingMessageProducer for MockProducer {
        async fn produce_message(&self, message: ScanMessage) -> ScanResult<()> {
            let mut messages = self.messages.lock().await;
            messages.push(message);
            Ok(())
        }
        
        fn name(&self) -> &str {
            "MockProducer"
        }
    }
    
    #[tokio::test]
    async fn test_streaming_producer() {
        let producer = MockProducer {
            messages: tokio::sync::Mutex::new(Vec::new()),
        };
        
        let message = ScanMessage::new(
            MessageHeader::new(ScanMode::FILES, 12345),
            MessageData::FileInfo {
                path: "test.rs".to_string(),
                size: 1024,
                lines: 50,
            },
        );
        
        producer.produce_message(message.clone()).await.unwrap();
        
        let messages = producer.messages.lock().await;
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].header.scan_mode, ScanMode::FILES);
    }
}