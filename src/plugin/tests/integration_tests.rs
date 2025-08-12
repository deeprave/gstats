//! Integration Tests for Scanner-Plugin Coordination
//! 
//! Comprehensive end-to-end tests for the complete scanner → plugin coordination workflow.

use crate::notifications::{ScanEvent, AsyncNotificationManager};
use crate::notifications::traits::NotificationManager;
use crate::plugin::{
    PluginRegistry,
    builtin::{commits::CommitsPlugin, metrics::MetricsPlugin, export::ExportPlugin}
};
use crate::plugin::tests::mock_plugins::create_test_context;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Test complete workflow: scan start → progress → completion → plugin processing → export
#[tokio::test]
async fn test_end_to_end_scanner_plugin_coordination() {
    // Setup notification manager
    let notification_manager = Arc::new(AsyncNotificationManager::new());
    
    // Setup plugin registry with notification manager
    let mut registry = PluginRegistry::with_notification_manager(notification_manager.clone());
    
    // Register all plugins
    let commits_plugin = Box::new(CommitsPlugin::new());
    let metrics_plugin = Box::new(MetricsPlugin::new());
    let export_plugin = Box::new(ExportPlugin::new());
    
    let _context = create_test_context();
    
    registry.register_plugin(commits_plugin).await.unwrap();
    registry.register_plugin(metrics_plugin).await.unwrap();
    registry.register_plugin(export_plugin).await.unwrap();
    
    // Subscribe all plugins to notifications
    registry.subscribe_all_plugins().await.unwrap();
    
    // Simulate complete scan workflow
    let scan_id = "integration_test_scan".to_string();
    
    // 1. Scan Started
    let scan_started = ScanEvent::ScanStarted {
        scan_id: scan_id.clone(),
        modes: ScanMode::HISTORY | ScanMode::FILES,
    };
    notification_manager.publish(scan_started).await.unwrap();
    
    // Allow event processing
    sleep(Duration::from_millis(10)).await;
    
    // 2. Scanner emits ScanDataReady for commits
    let commits_data_ready = ScanEvent::ScanDataReady {
        scan_id: scan_id.clone(),
        data_type: "commits".to_string(),
        message_count: 10,
    };
    notification_manager.publish(commits_data_ready).await.unwrap();
    
    // 3. Scanner emits ScanDataReady for files (metrics)
    let files_data_ready = ScanEvent::ScanDataReady {
        scan_id: scan_id.clone(),
        data_type: "files".to_string(),
        message_count: 25,
    };
    notification_manager.publish(files_data_ready).await.unwrap();
    
    // Allow plugin processing
    sleep(Duration::from_millis(20)).await;
    
    // 4. Plugins emit DataReady events
    let commits_ready = ScanEvent::data_ready(
        scan_id.clone(),
        "commits".to_string(),
        "commits".to_string(),
    );
    notification_manager.publish(commits_ready).await.unwrap();
    
    let metrics_ready = ScanEvent::data_ready(
        scan_id.clone(),
        "metrics".to_string(),
        "files".to_string(),
    );
    notification_manager.publish(metrics_ready).await.unwrap();
    
    // Allow export processing
    sleep(Duration::from_millis(20)).await;
    
    // 5. Scan Completed
    let scan_completed = ScanEvent::ScanCompleted {
        scan_id: scan_id.clone(),
        duration: Duration::from_secs(5),
        warnings: vec!["Test warning".to_string()],
    };
    notification_manager.publish(scan_completed).await.unwrap();
    
    // Allow final processing
    sleep(Duration::from_millis(30)).await;
    
    // Verify the workflow completed successfully
    // In a real implementation, we would check plugin states and outputs
    // For now, we verify no panics occurred and all events were processed
    
    // Get subscriber stats to verify event delivery
    let stats = notification_manager.get_stats().await;
    assert!(stats.events_published >= 6); // At least 6 events published
    assert!(stats.events_delivered >= 6); // At least 6 events delivered
}

/// Test error scenarios and event delivery failures
#[tokio::test]
async fn test_error_scenarios_and_delivery_failures() {
    let notification_manager = Arc::new(AsyncNotificationManager::new());
    let mut registry = PluginRegistry::with_notification_manager(notification_manager.clone());
    
    // Register plugins
    let commits_plugin = Box::new(CommitsPlugin::new());
    registry.register_plugin(commits_plugin).await.unwrap();
    registry.subscribe_all_plugins().await.unwrap();
    
    let scan_id = "error_test_scan".to_string();
    
    // Test ScanError event handling
    let scan_error = ScanEvent::ScanError {
        scan_id: scan_id.clone(),
        error: "Test fatal error".to_string(),
        fatal: true,
    };
    
    let result = notification_manager.publish(scan_error).await;
    assert!(result.is_ok());
    
    // Allow error processing
    sleep(Duration::from_millis(10)).await;
    
    // Test ScanWarning event handling
    let scan_warning = ScanEvent::ScanWarning {
        scan_id: scan_id.clone(),
        warning: "Test recoverable warning".to_string(),
        recoverable: true,
    };
    
    let result = notification_manager.publish(scan_warning).await;
    assert!(result.is_ok());
    
    // Allow warning processing
    sleep(Duration::from_millis(10)).await;
    
    // Verify error handling didn't crash the system
    let stats = notification_manager.get_stats().await;
    assert!(stats.events_published >= 2);
}

/// Test plugin lifecycle coordination
#[tokio::test]
async fn test_plugin_lifecycle_coordination() {
    let notification_manager = Arc::new(AsyncNotificationManager::new());
    let mut registry = PluginRegistry::with_notification_manager(notification_manager.clone());
    
    // Register all three plugins for complete lifecycle test
    let commits_plugin = Box::new(CommitsPlugin::new());
    let metrics_plugin = Box::new(MetricsPlugin::new());
    let export_plugin = Box::new(ExportPlugin::new());
    
    registry.register_plugin(commits_plugin).await.unwrap();
    registry.register_plugin(metrics_plugin).await.unwrap();
    registry.register_plugin(export_plugin).await.unwrap();
    
    registry.subscribe_all_plugins().await.unwrap();
    
    let scan_id = "lifecycle_test_scan".to_string();
    
    // Test complete lifecycle: Start → Data → Processing → Completion
    
    // 1. Scan Start
    let start_event = ScanEvent::ScanStarted {
        scan_id: scan_id.clone(),
        modes: ScanMode::HISTORY | ScanMode::FILES,
    };
    notification_manager.publish(start_event).await.unwrap();
    sleep(Duration::from_millis(5)).await;
    
    // 2. Data Ready Events (scanner → plugins)
    let commits_data = ScanEvent::ScanDataReady {
        scan_id: scan_id.clone(),
        data_type: "commits".to_string(),
        message_count: 5,
    };
    notification_manager.publish(commits_data).await.unwrap();
    
    let files_data = ScanEvent::ScanDataReady {
        scan_id: scan_id.clone(),
        data_type: "files".to_string(),
        message_count: 8,
    };
    notification_manager.publish(files_data).await.unwrap();
    sleep(Duration::from_millis(10)).await;
    
    // 3. Plugin Processing Complete (plugins → export)
    let commits_processed = ScanEvent::DataReady {
        scan_id: scan_id.clone(),
        plugin_id: "commits".to_string(),
        data_type: "commits".to_string(),
    };
    notification_manager.publish(commits_processed).await.unwrap();
    
    let metrics_processed = ScanEvent::DataReady {
        scan_id: scan_id.clone(),
        plugin_id: "metrics".to_string(),
        data_type: "files".to_string(),
    };
    notification_manager.publish(metrics_processed).await.unwrap();
    sleep(Duration::from_millis(15)).await;
    
    // 4. Scan Completion
    let completion_event = ScanEvent::ScanCompleted {
        scan_id: scan_id.clone(),
        duration: Duration::from_secs(2),
        warnings: vec![],
    };
    notification_manager.publish(completion_event).await.unwrap();
    sleep(Duration::from_millis(10)).await;
    
    // Verify lifecycle completed successfully
    let stats = notification_manager.get_stats().await;
    assert!(stats.events_published >= 6);
    assert!(stats.events_delivered >= 6);
    
    // Verify no errors occurred during lifecycle
    assert_eq!(stats.delivery_failures, 0);
}

/// Test concurrent plugin processing
#[tokio::test]
async fn test_concurrent_plugin_processing() {
    let notification_manager = Arc::new(AsyncNotificationManager::new());
    let mut registry = PluginRegistry::with_notification_manager(notification_manager.clone());
    
    // Register multiple plugins for concurrent processing
    let commits_plugin = Box::new(CommitsPlugin::new());
    let metrics_plugin = Box::new(MetricsPlugin::new());
    
    registry.register_plugin(commits_plugin).await.unwrap();
    registry.register_plugin(metrics_plugin).await.unwrap();
    registry.subscribe_all_plugins().await.unwrap();
    
    let scan_id = "concurrent_test_scan".to_string();
    
    // Send multiple events concurrently
    let mut handles = vec![];
    
    for i in 0..5 {
        let nm = notification_manager.clone();
        let scan_id_clone = scan_id.clone();
        
        let handle = tokio::spawn(async move {
            let event = ScanEvent::ScanDataReady {
                scan_id: scan_id_clone,
                data_type: format!("data_type_{}", i),
                message_count: i + 1,
            };
            nm.publish(event).await
        });
        
        handles.push(handle);
    }
    
    // Wait for all concurrent events to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
    
    // Allow processing time
    sleep(Duration::from_millis(50)).await;
    
    // Verify concurrent processing succeeded
    let stats = notification_manager.get_stats().await;
    assert!(stats.events_published >= 5);
}

/// Test memory management and resource cleanup
#[tokio::test]
async fn test_memory_management_and_cleanup() {
    let notification_manager = Arc::new(AsyncNotificationManager::new());
    let mut registry = PluginRegistry::with_notification_manager(notification_manager.clone());
    
    // Register export plugin to test cleanup functionality
    let export_plugin = Box::new(ExportPlugin::new());
    registry.register_plugin(export_plugin).await.unwrap();
    registry.subscribe_all_plugins().await.unwrap();
    
    let scan_id = "cleanup_test_scan".to_string();
    
    // Simulate data collection and cleanup cycle
    let commits_ready = ScanEvent::DataReady {
        scan_id: scan_id.clone(),
        plugin_id: "commits".to_string(),
        data_type: "commits".to_string(),
    };
    notification_manager.publish(commits_ready).await.unwrap();
    
    let metrics_ready = ScanEvent::DataReady {
        scan_id: scan_id.clone(),
        plugin_id: "metrics".to_string(),
        data_type: "files".to_string(),
    };
    notification_manager.publish(metrics_ready).await.unwrap();
    
    // Allow export processing and cleanup
    sleep(Duration::from_millis(20)).await;
    
    // Test scan completion triggers cleanup
    let completion_event = ScanEvent::ScanCompleted {
        scan_id: scan_id.clone(),
        duration: Duration::from_secs(1),
        warnings: vec![],
    };
    notification_manager.publish(completion_event).await.unwrap();
    
    // Allow cleanup processing
    sleep(Duration::from_millis(15)).await;
    
    // Verify cleanup completed without errors
    let stats = notification_manager.get_stats().await;
    assert_eq!(stats.delivery_failures, 0);
}
