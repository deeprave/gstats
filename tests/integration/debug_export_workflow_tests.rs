//! Integration tests for debug plugin --export workflow
//! 
//! Tests the complete end-to-end coordination between debug and export plugins
//! via the notification system when using the --export flag.

use std::sync::Arc;
use tokio::sync::RwLock;
use gstats::plugin::{Plugin, PluginContext};
use gstats::plugin::builtin::debug::DebugPlugin;
use gstats::plugin::builtin::export::ExportPlugin;
use gstats::notifications::{AsyncNotificationManager, traits::{NotificationManager, Subscriber}};
use gstats::notifications::events::PluginEvent;
use gstats::scanner::{ScannerConfig, QueryParams};
use gstats::queue::notifications::QueueEvent;

fn create_test_context_with_notifications() -> PluginContext {
    let notification_manager = Arc::new(
        AsyncNotificationManager::<PluginEvent>::new()
    );
    
    PluginContext::new(
        Arc::new(ScannerConfig::default()),
        Arc::new(QueryParams::default()),
    ).with_notification_manager(notification_manager)
}

#[tokio::test]
async fn test_debug_export_workflow_integration() {
    // Set up notification manager shared between plugins
    let notification_manager = Arc::new(
        AsyncNotificationManager::<PluginEvent>::new()
    );
    
    let context = PluginContext::new(
        Arc::new(ScannerConfig::default()),
        Arc::new(QueryParams::default()),
    ).with_notification_manager(notification_manager.clone());
    
    // Initialize debug plugin with export mode
    let mut debug_plugin = DebugPlugin::new();
    debug_plugin.initialize(&context).await.unwrap();
    
    // Parse --export argument to enable export mode
    let args = vec!["--export".to_string(), "--verbose".to_string()];
    debug_plugin.parse_plugin_args(&args).await.unwrap();
    
    // Verify export mode is enabled
    assert!(*debug_plugin.export_enabled.read().await);
    
    // Initialize export plugin with subscription
    let mut export_plugin = ExportPlugin::new();
    export_plugin.initialize(&context).await.unwrap();
    
    // Manually subscribe export plugin to notifications
    // (This would normally happen during initialization)
    let export_subscriber = Arc::new(export_plugin.clone());
    notification_manager.subscribe(export_subscriber).await.unwrap();
    
    // Add some test statistics to debug plugin
    {
        let mut stats = debug_plugin.stats.write().await;
        stats.messages_processed = 150;
        stats.commit_messages = 75;
        stats.file_changes = 60;
        stats.file_info = 15;
        stats.queue_events = 5;
    }
    
    // Simulate scan completion which should trigger export
    let scan_complete_event = QueueEvent::scan_complete("integration-test-scan".to_string(), 150);
    debug_plugin.handle_queue_event(&scan_complete_event).await.unwrap();
    
    // Give some time for async notification processing
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    
    // Verify that export plugin received the data
    let export_coordinator = export_plugin.data_coordinator.read().await;
    assert!(export_coordinator.has_data_from("debug"));
    
    // Verify scan ID was set in export plugin
    let export_scan_id = export_plugin.current_scan_id.read().await;
    assert!(export_scan_id.is_some());
    
    // Verify notification manager has subscribers
    assert!(notification_manager.subscriber_count().await > 0);
}

#[tokio::test]
async fn test_debug_plugin_output_suppression_integration() {
    let context = create_test_context_with_notifications();
    
    // Initialize debug plugin
    let mut debug_plugin = DebugPlugin::new();
    debug_plugin.initialize(&context).await.unwrap();
    
    // Test normal mode (no export)
    {
        let mut config = debug_plugin.config.write().await;
        config.verbose = true;
    }
    *debug_plugin.export_enabled.write().await = false;
    
    // Verify conditions for normal display
    let export_enabled = *debug_plugin.export_enabled.read().await;
    let config = debug_plugin.config.read().await;
    assert!(config.verbose && !export_enabled); // Should display
    
    // Enable export mode
    let args = vec!["--export".to_string()];
    debug_plugin.parse_plugin_args(&args).await.unwrap();
    
    // Verify conditions for suppressed display
    let export_enabled = *debug_plugin.export_enabled.read().await;
    let config = debug_plugin.config.read().await;
    assert!(config.verbose && export_enabled); // Should NOT display
}

#[tokio::test]
async fn test_export_plugin_notification_subscription_integration() {
    let context = create_test_context_with_notifications();
    
    // Initialize export plugin
    let mut export_plugin = ExportPlugin::new();
    export_plugin.initialize(&context).await.unwrap();
    
    // Verify subscriber properties
    assert_eq!(export_plugin.subscriber_id(), "export-plugin");
    
    // Create a test event
    let test_export_data = Arc::new(
        gstats::plugin::data_export::PluginDataExport {
            plugin_id: "test-plugin".to_string(),
            title: "Integration Test Data".to_string(),
            description: Some("Test data for integration testing".to_string()),
            data_type: gstats::plugin::data_export::DataExportType::Tabular,
            schema: gstats::plugin::data_export::DataSchema {
                columns: vec![
                    gstats::plugin::data_export::ColumnDef::new("test_metric", gstats::plugin::data_export::ColumnType::String)
                        .with_description("Test metric".to_string()),
                ],
                metadata: std::collections::HashMap::new(),
            },
            data: gstats::plugin::data_export::DataPayload::Rows(Arc::new(vec![])),
            export_hints: gstats::plugin::data_export::ExportHints::default(),
            timestamp: std::time::SystemTime::now(),
        }
    );
    
    let event = PluginEvent::DataReady {
        plugin_id: "test-plugin".to_string(),
        scan_id: "test-scan".to_string(),
        export: test_export_data,
    };
    
    // Handle the event
    let result = export_plugin.handle_event(event).await;
    assert!(result.is_ok());
    
    // Verify the event was processed
    let coordinator = export_plugin.data_coordinator.read().await;
    assert!(coordinator.has_data_from("test-plugin"));
}

#[tokio::test]
async fn test_notification_manager_plugin_coordination() {
    let notification_manager = Arc::new(
        AsyncNotificationManager::<PluginEvent>::new()
    );
    
    // Test initial state
    assert_eq!(notification_manager.subscriber_count().await, 0);
    
    // Create and subscribe export plugin
    let export_plugin = ExportPlugin::new();
    let export_subscriber = Arc::new(export_plugin);
    notification_manager.subscribe(export_subscriber.clone()).await.unwrap();
    
    // Verify subscription
    assert_eq!(notification_manager.subscriber_count().await, 1);
    assert!(notification_manager.has_subscriber("export-plugin").await);
    
    // Test publishing an event
    let test_data = Arc::new(
        gstats::plugin::data_export::PluginDataExport {
            plugin_id: "test-publisher".to_string(),
            title: "Test Publication".to_string(),
            description: None,
            data_type: gstats::plugin::data_export::DataExportType::Tabular,
            schema: gstats::plugin::data_export::DataSchema {
                columns: vec![],
                metadata: std::collections::HashMap::new(),
            },
            data: gstats::plugin::data_export::DataPayload::Rows(Arc::new(vec![])),
            export_hints: gstats::plugin::data_export::ExportHints::default(),
            timestamp: std::time::SystemTime::now(),
        }
    );
    
    let event = PluginEvent::DataReady {
        plugin_id: "test-publisher".to_string(),
        scan_id: "test-scan".to_string(),
        export: test_data,
    };
    
    // Publish event
    let publish_result = notification_manager.publish(event).await;
    assert!(publish_result.is_ok());
    
    // Give time for event processing
    tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    
    // Verify the subscriber received the event
    let coordinator = export_subscriber.data_coordinator.read().await;
    assert!(coordinator.has_data_from("test-publisher"));
}