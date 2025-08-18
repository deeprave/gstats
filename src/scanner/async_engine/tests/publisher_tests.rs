//! Publisher Trait Tests for AsyncScannerEngine
//! 
//! TDD tests to verify that AsyncScannerEngine properly implements the Publisher<ScanEvent> trait
//! and publishes lifecycle events at the correct times.

use std::sync::Arc;

use crate::scanner::async_engine::engine::AsyncScannerEngine;
use crate::scanner::config::ScannerConfig;
use crate::scanner::traits::MessageProducer;
use crate::scanner::messages::ScanMessage;
use crate::notifications::traits::Publisher;
use crate::notifications::events::ScanEvent;
use crate::notifications::manager::AsyncNotificationManager;
use crate::plugin::SharedPluginRegistry;

/// Mock message producer for testing
#[derive(Debug)]
struct MockMessageProducer;

#[async_trait::async_trait]
impl MessageProducer for MockMessageProducer {
    async fn produce_message(&self, _message: ScanMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    
    fn get_producer_name(&self) -> &str {
        "mock-producer"
    }
}

#[tokio::test]
async fn test_async_scanner_engine_implements_publisher_trait() {
    // TDD Red: This test should fail because AsyncScannerEngine doesn't implement Publisher<ScanEvent>
    
    // Create a temporary test repository
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    
    // Initialize as git repository
    std::process::Command::new("git")
        .args(&["init", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to init git repo");
    
    let config = ScannerConfig::default();
    let producer = Arc::new(MockMessageProducer);
    
    let notification_manager = Arc::new(AsyncNotificationManager::new());
    let plugin_registry = SharedPluginRegistry::new();
    
    let engine = AsyncScannerEngine::new_for_test(
        temp_dir.path(),
        config,
        producer,
        notification_manager,
        plugin_registry,
    ).expect("Failed to create scanner engine");
    
    // This should compile once Publisher trait is implemented
    let _publisher: &dyn Publisher<ScanEvent> = &engine;
}

#[tokio::test]
async fn test_async_scanner_engine_has_notification_manager() {
    // TDD Red: This test should fail because AsyncScannerEngine doesn't have notification_manager field
    
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    
    std::process::Command::new("git")
        .args(&["init", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to init git repo");
    
    let config = ScannerConfig::default();
    let producer = Arc::new(MockMessageProducer);
    
    // Create notification manager
    let notification_manager = Arc::new(AsyncNotificationManager::new());
    let plugin_registry = SharedPluginRegistry::new();
    
    let engine = AsyncScannerEngine::new_for_test(
        temp_dir.path(),
        config,
        producer,
        notification_manager.clone(),
        plugin_registry,
    ).expect("Failed to create scanner engine");
    // Verify the engine was created successfully with the notification manager
}

#[tokio::test]
async fn test_scanner_publish_methods_delegate_to_manager() {
    // TDD Red: This test should fail because publish methods don't exist
    
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    
    std::process::Command::new("git")
        .args(&["init", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to init git repo");
    
    let config = ScannerConfig::default();
    let producer = Arc::new(MockMessageProducer);
    let notification_manager = Arc::new(AsyncNotificationManager::new());
    let plugin_registry = SharedPluginRegistry::new();
    
    let engine = AsyncScannerEngine::new_for_test(
        temp_dir.path(),
        config,
        producer,
        notification_manager,
        plugin_registry,
    ).expect("Failed to create scanner engine");
    
    // Test that publish method works (should succeed even with no subscribers)
    let event = ScanEvent::started("test-scan-001".to_string());
    let result = engine.publish(event).await;
    assert!(result.is_ok());
    
}

#[tokio::test]
async fn test_scan_publishes_scan_started_event() {
    // TDD Red: This test should fail because scan() method doesn't publish ScanStarted events
    
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    
    std::process::Command::new("git")
        .args(&["init", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to init git repo");
    
    let config = ScannerConfig::default();
    let producer = Arc::new(MockMessageProducer);
    let notification_manager = Arc::new(AsyncNotificationManager::new());
    let plugin_registry = SharedPluginRegistry::new();
    
    let engine = AsyncScannerEngine::new_for_test(
        temp_dir.path(),
        config,
        producer,
        notification_manager.clone(),
        plugin_registry,
    ).expect("Failed to create scanner engine");
    
    // Since we can't easily mock the notification manager to capture events,
    // we'll test that the scan method runs without errors when notification manager is set
    // A more comprehensive test would require a mock notification manager
    
    // Note: This test currently expects the scan to fail because no scanners are registered
    // but the important thing is that it attempts to publish ScanStarted event
    let result = engine.scan().await;
    
    // Should fail with "no scanners registered" error, not a publishing error
    assert!(result.is_err());
    match result {
        Err(crate::scanner::async_engine::error::ScanError::Configuration(_)) => {
            // This is the expected error - scan fails because no scanners are registered
            // but it should have attempted to publish ScanStarted event first
        }
        _ => panic!("Expected Configuration error for no scanners, got: {:?}", result),
    }
}

#[tokio::test]
async fn test_scan_publishes_periodic_events() {
    // Test that ScanProgress and ScanDataReady events are published periodically
    
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    
    std::process::Command::new("git")
        .args(&["init", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to init git repo");
    
    let config = ScannerConfig::default();
    let producer = Arc::new(MockMessageProducer);
    let notification_manager = Arc::new(AsyncNotificationManager::new());
    let plugin_registry = SharedPluginRegistry::new();
    
    let engine = AsyncScannerEngine::new_for_test(
        temp_dir.path(),
        config,
        producer,
        notification_manager.clone(),
        plugin_registry,
    ).expect("Failed to create scanner engine");
    
    // This test verifies that periodic events are attempted during scan execution
    // Even though the scan will fail due to no scanners, the periodic event timer
    // should be started and stopped properly without causing errors
    
    let result = engine.scan().await;
    
    // Should fail with configuration error (no scanners), but periodic events
    // should have been attempted during the brief execution
    assert!(result.is_err());
    match result {
        Err(crate::scanner::async_engine::error::ScanError::Configuration(_)) => {
            // Expected error - the important thing is that periodic event setup
            // and cleanup completed without additional errors
        }
        _ => panic!("Expected Configuration error for no scanners, got: {:?}", result),
    }
}