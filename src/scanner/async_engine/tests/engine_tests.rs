//! Engine Integration Tests

use crate::scanner::async_engine::error::{ScanResult, ScanError};
use crate::scanner::async_engine::engine::AsyncScannerEngineBuilder;
use crate::scanner::async_traits::*;
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
use crate::scanner::traits::MessageProducer;
use async_trait::async_trait;
use futures::stream;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use std::path::PathBuf;

struct TestMessageProducer {
    messages: Arc<tokio::sync::Mutex<Vec<ScanMessage>>>,
    count: Arc<AtomicUsize>,
}

impl TestMessageProducer {
    fn new() -> Self {
        Self {
            messages: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            count: Arc::new(AtomicUsize::new(0)),
        }
    }
    
    async fn get_messages(&self) -> Vec<ScanMessage> {
        self.messages.lock().await.clone()
    }
    
    fn message_count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl MessageProducer for TestMessageProducer {
    async fn produce_message(&self, message: ScanMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.messages.lock().await.push(message);
        self.count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    
    fn get_producer_name(&self) -> &str {
        "TestProducer"
    }
}

struct DelayedScanner {
    delay_ms: u64,
    message_count: usize,
}

#[async_trait]
impl AsyncScanner for DelayedScanner {
    fn name(&self) -> &str {
        "DelayedScanner"
    }
    
    
    async fn scan_async(&self, _repository_path: &std::path::Path) -> ScanResult<ScanMessageStream> {
        let delay = self.delay_ms;
        let count = self.message_count;
        
        let stream = stream::unfold((0, delay), move |(i, delay)| async move {
            if i >= count {
                return None;
            }
            
            tokio::time::sleep(Duration::from_millis(delay)).await;
            
            let message = ScanMessage::new(
                MessageHeader::new(1000 + i as u64),
                MessageData::FileInfo {
                    path: format!("delayed_file_{}.rs", i),
                    size: (1024 * (i + 1)) as u64,
                    lines: (50 * (i + 1)) as u32,
                },
            );
            
            Some((Ok(message), (i + 1, delay)))
        });
        
        Ok(Box::pin(stream))
    }
}

#[tokio::test]
async fn test_engine_builder() {
    let repo_path = PathBuf::from(".");
    let producer = Arc::new(TestMessageProducer::new());
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository_path(repo_path)
        .message_producer(producer)
        .build();
    
    assert!(engine.is_ok());
}

#[tokio::test]
async fn test_engine_without_scanners() {
    let repo_path = PathBuf::from(".");
    let producer = Arc::new(TestMessageProducer::new());
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository_path(repo_path)
        .message_producer(producer)
        .build()
        .unwrap();
    
    let result = engine.scan().await;
    assert!(matches!(result, Err(ScanError::Configuration(_))));
}

#[tokio::test]
async fn test_single_scanner_operation() {
    let repo_path = PathBuf::from(".");
    let producer = Arc::new(TestMessageProducer::new());
    let producer_ref = Arc::clone(&producer);
    
    let scanner = Arc::new(DelayedScanner {
        delay_ms: 10,
        message_count: 5,
    });
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository_path(repo_path)
        .message_producer(producer)
        .add_scanner(scanner)
        .build()
        .unwrap();
    
    engine.scan().await.unwrap();
    
    // Wait a bit for async message production
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    assert_eq!(producer_ref.message_count(), 5);
    
    let messages = producer_ref.get_messages().await;
    assert_eq!(messages.len(), 5);
    // All messages should have sequence numbers
    assert!(messages.iter().all(|m| m.header.sequence > 0));
}

#[tokio::test]
async fn test_scanner_operation() {
    let repo_path = PathBuf::from(".");
    let producer = Arc::new(TestMessageProducer::new());
    
    let scanner = Arc::new(DelayedScanner {
        delay_ms: 10,
        message_count: 5,
    });
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository_path(repo_path)
        .message_producer(producer)
        .add_scanner(scanner)
        .build()
        .unwrap();
    
    let result = engine.scan().await;
    assert!(result.is_ok()); // Engine should operate successfully
}

#[tokio::test]
async fn test_cancellation() {
    let repo_path = PathBuf::from(".");
    let producer = Arc::new(TestMessageProducer::new());
    let producer_ref = Arc::clone(&producer);
    
    let scanner = Arc::new(DelayedScanner {
        delay_ms: 100, // Longer delay to ensure we can cancel
        message_count: 10,
    });
    
    let engine = Arc::new(AsyncScannerEngineBuilder::new()
        .repository_path(repo_path)
        .message_producer(producer)
        .add_scanner(scanner)
        .build()
        .unwrap());
    
    // Start scan in background
    let engine_clone = engine.clone();
    let scan_handle = tokio::spawn(async move {
        engine_clone.scan().await
    });
    
    // Cancel after a short delay
    tokio::time::sleep(Duration::from_millis(250)).await;
    engine.cancel().await;
    
    let result = scan_handle.await.unwrap();
    assert!(matches!(result, Err(ScanError::Cancelled)));
    
    // Should have produced fewer than 10 messages
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(producer_ref.message_count() < 10);
}

#[tokio::test]
async fn test_engine_stats() {
    let repo_path = PathBuf::from(".");
    let producer = Arc::new(TestMessageProducer::new());
    
    let scanner = Arc::new(DelayedScanner {
        delay_ms: 10,
        message_count: 3,
    });
    
    let engine = AsyncScannerEngineBuilder::new()
        .repository_path(repo_path)
        .message_producer(producer)
        .add_scanner(scanner)
        .build()
        .unwrap();
    
    let initial_stats = engine.get_stats().await;
    assert_eq!(initial_stats.active_tasks, 0);
    assert_eq!(initial_stats.completed_tasks, 0);
    assert_eq!(initial_stats.registered_scanners, 1);
    assert_eq!(initial_stats.errors, 0);
    
    engine.scan().await.unwrap();
    
    let final_stats = engine.get_stats().await;
    assert_eq!(final_stats.active_tasks, 0);
    assert_eq!(final_stats.completed_tasks, 1);
    assert_eq!(final_stats.errors, 0);
}