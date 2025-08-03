//! Tests for Async Notification Manager
//! 
//! Tests real-time notification delivery, filtering, and subscription management.

use super::mock_plugins::*;
use crate::plugin::notification::AsyncNotificationManager;
use crate::plugin::traits::{QueueUpdate, QueueUpdateType, ScanProgress, SystemEvent, SystemEventType, NotificationPreferences};
use crate::plugin::error::PluginError;
use crate::scanner::modes::ScanMode;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::time::timeout;

#[tokio::test]
async fn test_notification_manager_creation() {
    let manager = AsyncNotificationManager::new();
    assert_eq!(manager.subscriber_count(), 0);
    assert!(manager.is_empty());
}

#[tokio::test]
async fn test_subscribe_plugin() {
    let mut manager = AsyncNotificationManager::new();
    let plugin = Arc::new(MockNotificationPlugin::new("test-plugin", false));
    
    let result = manager.subscribe_plugin(plugin.clone()).await;
    assert!(result.is_ok());
    assert_eq!(manager.subscriber_count(), 1);
    assert!(!manager.is_empty());
    
    // Test duplicate subscription
    let result = manager.subscribe_plugin(plugin.clone()).await;
    assert!(result.is_err());
    assert_eq!(manager.subscriber_count(), 1);
}

#[tokio::test]
async fn test_unsubscribe_plugin() {
    let mut manager = AsyncNotificationManager::new();
    let plugin = Arc::new(MockNotificationPlugin::new("test-plugin", false));
    
    manager.subscribe_plugin(plugin.clone()).await.unwrap();
    assert_eq!(manager.subscriber_count(), 1);
    
    let result = manager.unsubscribe_plugin("test-plugin").await;
    assert!(result.is_ok());
    assert_eq!(manager.subscriber_count(), 0);
    assert!(manager.is_empty());
    
    // Test unsubscribing non-existent plugin
    let result = manager.unsubscribe_plugin("missing-plugin").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_queue_update_notification() {
    let mut manager = AsyncNotificationManager::new();
    let plugin = Arc::new(MockNotificationPlugin::new("test-plugin", false));
    
    manager.subscribe_plugin(plugin.clone()).await.unwrap();
    
    let update = QueueUpdate::new(
        "test-queue".to_string(),
        QueueUpdateType::MessageEnqueued,
        10,
        1024,
    );
    
    let result = manager.notify_queue_update(update).await;
    assert!(result.is_ok());
    
    // Give time for async notification
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    let notifications = plugin.received_notifications();
    assert_eq!(notifications.len(), 1);
    assert!(notifications[0].starts_with("queue_update:test-queue"));
}

#[tokio::test]
async fn test_scan_progress_notification() {
    let mut manager = AsyncNotificationManager::new();
    let plugin = Arc::new(MockNotificationPlugin::new("test-plugin", false));
    
    manager.subscribe_plugin(plugin.clone()).await.unwrap();
    
    let progress = ScanProgress::new(
        "test-scan".to_string(),
        ScanMode::FILES,
        50,
        "processing files".to_string(),
    ).with_total_items(100);
    
    let result = manager.notify_scan_progress(progress).await;
    assert!(result.is_ok());
    
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    let notifications = plugin.received_notifications();
    assert_eq!(notifications.len(), 1);
    assert!(notifications[0].starts_with("scan_progress:test-scan:50"));
}

#[tokio::test]
async fn test_error_notification() {
    let mut manager = AsyncNotificationManager::new();
    let plugin = Arc::new(MockNotificationPlugin::new("test-plugin", false));
    
    manager.subscribe_plugin(plugin.clone()).await.unwrap();
    
    let error = PluginError::execution_failed("Test error");
    let result = manager.notify_error(error).await;
    assert!(result.is_ok());
    
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    let notifications = plugin.received_notifications();
    assert_eq!(notifications.len(), 1);
    assert!(notifications[0].starts_with("error:"));
}

#[tokio::test]
async fn test_system_event_notification() {
    let mut manager = AsyncNotificationManager::new();
    let plugin = Arc::new(MockNotificationPlugin::new("test-plugin", false));
    
    manager.subscribe_plugin(plugin.clone()).await.unwrap();
    
    let event = SystemEvent::new(
        SystemEventType::SystemStartup,
        serde_json::json!({"timestamp": "now"}),
    );
    
    let result = manager.notify_system_event(event).await;
    assert!(result.is_ok());
    
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    let notifications = plugin.received_notifications();
    assert_eq!(notifications.len(), 1);
    assert!(notifications[0].starts_with("system_event:"));
}

#[tokio::test]
async fn test_notification_filtering_by_preferences() {
    let mut manager = AsyncNotificationManager::new();
    
    // Create plugin with selective preferences
    let mut plugin = MockNotificationPlugin::new("selective-plugin", false);
    plugin.preferences = NotificationPreferences {
        queue_updates: false,
        scan_progress: true,
        error_notifications: true,
        system_events: vec![SystemEventType::SystemShutdown],
        max_frequency: Some(5),
    };
    let plugin = Arc::new(plugin);
    
    manager.subscribe_plugin(plugin.clone()).await.unwrap();
    
    // Send queue update (should be ignored)
    let queue_update = QueueUpdate::new("test".to_string(), QueueUpdateType::MessageEnqueued, 1, 100);
    manager.notify_queue_update(queue_update).await.unwrap();
    
    // Send scan progress (should be received)
    let scan_progress = ScanProgress::new("test".to_string(), ScanMode::FILES, 1, "test".to_string());
    manager.notify_scan_progress(scan_progress).await.unwrap();
    
    // Send startup event (should be ignored - not in preferences)
    let startup_event = SystemEvent::new(SystemEventType::SystemStartup, serde_json::json!({}));
    manager.notify_system_event(startup_event).await.unwrap();
    
    // Send shutdown event (should be received)
    let shutdown_event = SystemEvent::new(SystemEventType::SystemShutdown, serde_json::json!({}));
    manager.notify_system_event(shutdown_event).await.unwrap();
    
    tokio::time::sleep(Duration::from_millis(20)).await;
    
    let notifications = plugin.received_notifications();
    assert_eq!(notifications.len(), 2); // Only scan progress and shutdown
}

#[tokio::test]
async fn test_multiple_subscribers() {
    let mut manager = AsyncNotificationManager::new();
    
    let plugin1 = Arc::new(MockNotificationPlugin::new("plugin1", false));
    let plugin2 = Arc::new(MockNotificationPlugin::new("plugin2", false));
    let plugin3 = Arc::new(MockNotificationPlugin::new("plugin3", false));
    
    manager.subscribe_plugin(plugin1.clone()).await.unwrap();
    manager.subscribe_plugin(plugin2.clone()).await.unwrap();
    manager.subscribe_plugin(plugin3.clone()).await.unwrap();
    
    assert_eq!(manager.subscriber_count(), 3);
    
    let update = QueueUpdate::new("broadcast-test".to_string(), QueueUpdateType::MessageEnqueued, 5, 512);
    manager.notify_queue_update(update).await.unwrap();
    
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    // All plugins should receive the notification
    assert_eq!(plugin1.received_notifications().len(), 1);
    assert_eq!(plugin2.received_notifications().len(), 1);
    assert_eq!(plugin3.received_notifications().len(), 1);
}

#[tokio::test]
async fn test_notification_with_failing_plugin() {
    let mut manager = AsyncNotificationManager::new();
    
    let working_plugin = Arc::new(MockNotificationPlugin::new("working", false));
    let failing_plugin = Arc::new(MockNotificationPlugin::new("failing", true));
    
    manager.subscribe_plugin(working_plugin.clone()).await.unwrap();
    manager.subscribe_plugin(failing_plugin.clone()).await.unwrap();
    
    let update = QueueUpdate::new("test".to_string(), QueueUpdateType::MessageEnqueued, 1, 100);
    let result = manager.notify_queue_update(update).await;
    
    // Should not fail even if one plugin fails
    assert!(result.is_ok());
    
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    // Working plugin should still receive notification
    assert_eq!(working_plugin.received_notifications().len(), 1);
}

#[tokio::test]
async fn test_concurrent_notifications() {
    let mut manager = AsyncNotificationManager::new();
    let plugin = Arc::new(MockNotificationPlugin::new("concurrent-test", false));
    
    manager.subscribe_plugin(plugin.clone()).await.unwrap();
    
    // Send multiple notifications concurrently
    let mut handles = Vec::new();
    for i in 0..10 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            let update = QueueUpdate::new(
                format!("queue-{}", i),
                QueueUpdateType::MessageEnqueued,
                i,
                i as u64 * 100,
            );
            manager_clone.notify_queue_update(update).await
        });
        handles.push(handle);
    }
    
    // Wait for all notifications
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }
    
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Should receive all 10 notifications
    assert_eq!(plugin.received_notifications().len(), 10);
}

#[tokio::test]
async fn test_notification_timeout_handling() {
    let mut manager = AsyncNotificationManager::with_timeout(Duration::from_millis(100));
    let plugin = Arc::new(MockNotificationPlugin::new("timeout-test", false));
    
    manager.subscribe_plugin(plugin.clone()).await.unwrap();
    
    let update = QueueUpdate::new("test".to_string(), QueueUpdateType::MessageEnqueued, 1, 100);
    
    // This should complete within the timeout
    let result = timeout(Duration::from_millis(200), manager.notify_queue_update(update)).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_ok());
}

#[tokio::test]
async fn test_notification_frequency_limiting() {
    let mut manager = AsyncNotificationManager::new();
    
    // Plugin with max frequency of 2 notifications per second
    let mut plugin = MockNotificationPlugin::new("rate-limited", false);
    plugin.preferences.max_frequency = Some(2);
    let plugin = Arc::new(plugin);
    
    manager.subscribe_plugin(plugin.clone()).await.unwrap();
    
    // Send 5 notifications rapidly
    for i in 0..5 {
        let update = QueueUpdate::new(format!("rapid-{}", i), QueueUpdateType::MessageEnqueued, i, 100);
        manager.notify_queue_update(update).await.unwrap();
    }
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Should be rate limited to 2 notifications
    let notifications = plugin.received_notifications();
    assert!(notifications.len() <= 2);
}

#[tokio::test]
async fn test_shutdown_cleanup() {
    let mut manager = AsyncNotificationManager::new();
    let plugin = Arc::new(MockNotificationPlugin::new("shutdown-test", false));
    
    manager.subscribe_plugin(plugin.clone()).await.unwrap();
    assert_eq!(manager.subscriber_count(), 1);
    
    manager.shutdown().await.unwrap();
    
    // After shutdown, manager should be empty
    assert_eq!(manager.subscriber_count(), 0);
    assert!(manager.is_empty());
    
    // Notifications after shutdown should fail gracefully
    let update = QueueUpdate::new("post-shutdown".to_string(), QueueUpdateType::MessageEnqueued, 1, 100);
    let result = manager.notify_queue_update(update).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_subscription_management() {
    let mut manager = AsyncNotificationManager::new();
    
    // Test getting subscriber list
    let subscribers = manager.get_subscribers().await;
    assert!(subscribers.is_empty());
    
    // Add subscribers
    let plugin1 = Arc::new(MockNotificationPlugin::new("sub1", false));
    let plugin2 = Arc::new(MockNotificationPlugin::new("sub2", false));
    
    manager.subscribe_plugin(plugin1).await.unwrap();
    manager.subscribe_plugin(plugin2).await.unwrap();
    
    let subscribers = manager.get_subscribers().await;
    assert_eq!(subscribers.len(), 2);
    assert!(subscribers.contains(&"sub1".to_string()));
    assert!(subscribers.contains(&"sub2".to_string()));
    
    // Test subscriber filtering by preferences
    let queue_subscribers = manager.get_subscribers_for_queue_updates().await;
    assert_eq!(queue_subscribers.len(), 2); // Default preferences include queue updates
}