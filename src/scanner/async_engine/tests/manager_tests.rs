//! AsyncScannerManager Tests
//! 
//! Tests for the simplified AsyncScannerManager architecture.

use std::sync::Arc;
use std::path::PathBuf;
use crate::scanner::async_engine::engine::AsyncScannerManagerBuilder;
use crate::scanner::config::ScannerConfig;
use crate::scanner::traits::MessageProducer;
use crate::scanner::messages::ScanMessage;
use crate::notifications::AsyncNotificationManager;
use crate::notifications::events::ScanEvent;
use crate::plugin::SharedPluginRegistry;
use async_trait::async_trait;

/// Simple test message producer
struct TestMessageProducer {
    message_count: Arc<std::sync::atomic::AtomicU64>,
}

impl TestMessageProducer {
    fn new() -> Self {
        Self {
            message_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }
    
    fn get_count(&self) -> u64 {
        self.message_count.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[async_trait]
impl MessageProducer for TestMessageProducer {
    async fn send_message(&self, _message: ScanMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.message_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
}

#[tokio::test]
async fn test_scanner_manager_creation() {
    let repo_path = PathBuf::from("/tmp/test-repo");
    let config = Arc::new(ScannerConfig::default());
    let producer = Arc::new(TestMessageProducer::new());
    let notification_manager = Arc::new(AsyncNotificationManager::<ScanEvent>::new());
    let registry = SharedPluginRegistry::new();
    
    let manager = AsyncScannerManagerBuilder::new()
        .repository_path(repo_path)
        .config(config)
        .message_producer(producer.clone())
        .notification_manager(notification_manager)
        .plugin_registry(registry)
        .build()
        .expect("Should build AsyncScannerManager");
    
    // Verify manager was created successfully
    assert_eq!(producer.get_count(), 0);
}

#[tokio::test]
async fn test_scanner_manager_empty_scan() {
    let repo_path = PathBuf::from("/tmp/nonexistent-repo");
    let config = Arc::new(ScannerConfig::default());
    let producer = Arc::new(TestMessageProducer::new());
    let notification_manager = Arc::new(AsyncNotificationManager::<ScanEvent>::new());
    let registry = SharedPluginRegistry::new();
    
    let manager = AsyncScannerManagerBuilder::new()
        .repository_path(repo_path)
        .config(config)
        .message_producer(producer.clone())
        .notification_manager(notification_manager)
        .plugin_registry(registry)
        .build()
        .expect("Should build AsyncScannerManager");
    
    // For a non-existent repo, scan should complete without sending messages
    let result = manager.scan().await;
    
    // The result may be error or ok depending on implementation, but producer should be unchanged
    assert_eq!(producer.get_count(), 0);
}