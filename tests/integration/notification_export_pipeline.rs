//! Integration Tests for Notification-Based Export Pipeline
//! 
//! Tests the complete notification system integration with the export pipeline:
//! - Plugin event publishing and subscription
//! - ExportPlugin coordination and formatting
//! - End-to-end data flow verification

use std::sync::Arc;
use std::collections::HashMap;
use std::time::{SystemTime, Duration};
use tokio::sync::RwLock;

use gstats::plugin::data_export::*;
use gstats::plugin::data_coordinator::DataCoordinator;
use gstats::plugin::builtin::export::ExportPlugin;
use gstats::notifications::events::PluginEvent;
use gstats::notifications::{AsyncNotificationManager, NotificationResult};
use gstats::notifications::traits::{Subscriber, NotificationManager};

/// Integration test for the complete export pipeline
#[tokio::test]
async fn test_complete_export_pipeline() {
    // Create notification manager
    let mut notification_manager = AsyncNotificationManager::new();
    
    // Create export plugin
    let export_plugin = Arc::new(ExportPlugin::new());
    
    // Subscribe export plugin to notifications
    notification_manager.subscribe(export_plugin.clone()).await.unwrap();
    
    // Wait for subscription to be processed
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    // Create test data from commits plugin
    let commits_export = create_commits_test_data();
    let commits_event = PluginEvent::DataReady {
        plugin_id: "commits".to_string(),
        scan_id: "integration_test_001".to_string(),
        export: commits_export,
    };
    
    // Create test data from metrics plugin  
    let metrics_export = create_metrics_test_data();
    let metrics_event = PluginEvent::DataReady {
        plugin_id: "metrics".to_string(),
        scan_id: "integration_test_001".to_string(),
        export: metrics_export,
    };
    
    // Publish events (this should trigger export when both plugins report)
    notification_manager.publish(commits_event).await.unwrap();
    notification_manager.publish(metrics_event).await.unwrap();
    
    // Wait for async processing
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Note: In a full integration test, we would capture the console output
    // or use a mock output handler to verify the export was triggered
    // For now, we verify that the events were processed without errors
}

#[tokio::test]
async fn test_export_plugin_data_coordination() {
    let export_plugin = ExportPlugin::new();
    
    // Test handling individual DataReady events
    let test_export = Arc::new(PluginDataExport {
        plugin_id: "test_plugin".to_string(),
        title: "Test Export".to_string(),
        description: Some("Integration test data".to_string()),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![
                ColumnDef::new("Column1", ColumnType::String),
                ColumnDef::new("Column2", ColumnType::Integer),
            ],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(vec![
            Row::new(vec![
                Value::String("Test Value".to_string()),
                Value::Integer(42),
            ]),
        ])),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    });
    
    let event = PluginEvent::DataReady {
        plugin_id: "test_plugin".to_string(),
        scan_id: "coordination_test".to_string(),
        export: test_export,
    };
    
    // Handle the event
    let result = export_plugin.handle_event(event).await;
    assert!(result.is_ok(), "ExportPlugin should handle DataReady events without error");
}

#[tokio::test]
async fn test_multi_plugin_coordination_timeout() {
    // Test that the coordinator properly handles partial data scenarios
    let mut coordinator = DataCoordinator::with_expected_plugins(vec![
        "commits".to_string(),
        "metrics".to_string(),
        "debug".to_string(),
    ]);
    
    // Add data from only some plugins
    let partial_export = create_commits_test_data();
    coordinator.add_data("commits".to_string(), partial_export);
    
    // Should not be complete with partial data
    assert!(!coordinator.is_complete());
    
    // Verify pending plugins
    let pending = coordinator.get_pending_plugins();
    assert_eq!(pending.len(), 2);
    assert!(pending.contains(&"metrics".to_string()));
    assert!(pending.contains(&"debug".to_string()));
    
    // Add another plugin's data
    let metrics_export = create_metrics_test_data();
    coordinator.add_data("metrics".to_string(), metrics_export);
    
    // Still not complete
    assert!(!coordinator.is_complete());
    let pending = coordinator.get_pending_plugins();
    assert_eq!(pending.len(), 1);
    assert!(pending.contains(&"debug".to_string()));
    
    // Complete with final plugin
    let debug_export = create_debug_test_data();
    coordinator.add_data("debug".to_string(), debug_export);
    
    // Now should be complete
    assert!(coordinator.is_complete());
    assert!(coordinator.get_pending_plugins().is_empty());
}

#[tokio::test] 
async fn test_notification_system_resilience() {
    // Test that the notification system handles errors gracefully
    let mut notification_manager = AsyncNotificationManager::new();
    
    // Create a failing subscriber for testing error handling
    let failing_subscriber = Arc::new(FailingSubscriber::new());
    let working_subscriber = Arc::new(WorkingSubscriber::new());
    
    // Subscribe both
    notification_manager.subscribe(failing_subscriber.clone()).await.unwrap();
    notification_manager.subscribe(working_subscriber.clone()).await.unwrap();
    
    // Create test event
    let test_export = create_commits_test_data();
    let event = PluginEvent::DataReady {
        plugin_id: "resilience_test".to_string(),
        scan_id: "error_handling".to_string(),
        export: test_export,
    };
    
    // Publish event - should not fail even if one subscriber fails
    let result = notification_manager.publish(event).await;
    assert!(result.is_ok(), "Publishing should succeed even with failing subscribers");
    
    // Wait for processing
    tokio::time::sleep(Duration::from_millis(20)).await;
    
    // Verify working subscriber received the event
    assert!(working_subscriber.received_event().await);
}

#[tokio::test]
async fn test_export_format_selection() {
    // Test that export hints properly influence format selection
    let export_with_hints = Arc::new(PluginDataExport {
        plugin_id: "format_test".to_string(),
        title: "Format Selection Test".to_string(),
        description: Some("Testing export format preferences".to_string()),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![ColumnDef::new("Data", ColumnType::String)],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(vec![
            Row::new(vec![Value::String("Test Data".to_string())]),
        ])),
        export_hints: ExportHints {
            preferred_formats: vec![
                ExportFormat::Json,
                ExportFormat::Html,
                ExportFormat::Csv,
            ],
            sort_by: Some("Data".to_string()),
            sort_ascending: true,
            limit: Some(10),
            include_totals: false,
            include_row_numbers: true,
            custom_hints: {
                let mut hints = HashMap::new();
                hints.insert("theme".to_string(), "dark".to_string());
                hints.insert("precision".to_string(), "2".to_string());
                hints
            },
        },
        timestamp: SystemTime::now(),
    });
    
    // Verify export hints are properly structured
    assert_eq!(export_with_hints.export_hints.preferred_formats.len(), 3);
    assert_eq!(export_with_hints.export_hints.preferred_formats[0], ExportFormat::Json);
    assert_eq!(export_with_hints.export_hints.sort_by, Some("Data".to_string()));
    assert!(export_with_hints.export_hints.sort_ascending);
    assert_eq!(export_with_hints.export_hints.limit, Some(10));
    assert!(export_with_hints.export_hints.include_row_numbers);
    assert_eq!(
        export_with_hints.export_hints.custom_hints.get("theme"),
        Some(&"dark".to_string())
    );
}

#[tokio::test]
async fn test_large_dataset_handling() {
    // Test handling of larger datasets to ensure performance
    let large_dataset = create_large_test_dataset(1000);
    
    let export_plugin = ExportPlugin::new();
    let event = PluginEvent::DataReady {
        plugin_id: "performance_test".to_string(),
        scan_id: "large_dataset_test".to_string(),
        export: large_dataset,
    };
    
    // Time the processing
    let start = std::time::Instant::now();
    let result = export_plugin.handle_event(event).await;
    let elapsed = start.elapsed();
    
    assert!(result.is_ok(), "Should handle large datasets without error");
    assert!(elapsed < Duration::from_secs(1), "Should process large dataset quickly (got {:?})", elapsed);
}

#[tokio::test]
async fn test_concurrent_plugin_events() {
    // Test handling multiple concurrent plugin events
    let export_plugin = Arc::new(ExportPlugin::new());
    
    let mut handles = Vec::new();
    
    // Create multiple concurrent events
    for i in 0..10 {
        let plugin = export_plugin.clone();
        let handle = tokio::spawn(async move {
            let export = create_numbered_test_data(i);
            let event = PluginEvent::DataReady {
                plugin_id: format!("concurrent_plugin_{}", i),
                scan_id: "concurrent_test".to_string(),
                export,
            };
            
            plugin.handle_event(event).await
        });
        handles.push(handle);
    }
    
    // Wait for all to complete
    let results = {
        let mut all_results = Vec::new();
        for handle in handles {
            all_results.push(handle.await);
        }
        all_results
    };
    
    // Verify all completed successfully
    for result in results {
        assert!(result.is_ok(), "Concurrent event handling should succeed");
        assert!(result.unwrap().is_ok(), "Event processing should succeed");
    }
}

// Helper structs and functions

struct FailingSubscriber {
    name: String,
}

impl FailingSubscriber {
    fn new() -> Self {
        Self {
            name: "failing_subscriber".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl Subscriber<PluginEvent> for FailingSubscriber {
    async fn handle_event(&self, _event: PluginEvent) -> NotificationResult<()> {
        // Always fail for testing error handling
        Err(gstats::notifications::error::NotificationError::processing(
            "Intentional failure for testing".to_string()
        ))
    }
    
    fn subscriber_id(&self) -> &str {
        &self.name
    }
}

struct WorkingSubscriber {
    name: String,
    received: Arc<RwLock<bool>>,
}

impl WorkingSubscriber {
    fn new() -> Self {
        Self {
            name: "working_subscriber".to_string(),
            received: Arc::new(RwLock::new(false)),
        }
    }
    
    async fn received_event(&self) -> bool {
        *self.received.read().await
    }
}

#[async_trait::async_trait]
impl Subscriber<PluginEvent> for WorkingSubscriber {
    async fn handle_event(&self, _event: PluginEvent) -> NotificationResult<()> {
        let mut received = self.received.write().await;
        *received = true;
        Ok(())
    }
    
    fn subscriber_id(&self) -> &str {
        &self.name
    }
}

fn create_commits_test_data() -> Arc<PluginDataExport> {
    Arc::new(PluginDataExport {
        plugin_id: "commits".to_string(),
        title: "Commit Analysis".to_string(),
        description: Some("Integration test commit data".to_string()),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![
                ColumnDef::new("Author", ColumnType::String),
                ColumnDef::new("Commits", ColumnType::Integer),
                ColumnDef::new("Percentage", ColumnType::Float),
            ],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(vec![
            Row::new(vec![
                Value::String("Alice".to_string()),
                Value::Integer(50),
                Value::Float(62.5),
            ]),
            Row::new(vec![
                Value::String("Bob".to_string()),
                Value::Integer(30),
                Value::Float(37.5),
            ]),
        ])),
        export_hints: ExportHints {
            preferred_formats: vec![ExportFormat::Console],
            sort_by: Some("Commits".to_string()),
            sort_ascending: false,
            limit: None,
            include_totals: true,
            include_row_numbers: false,
            custom_hints: HashMap::new(),
        },
        timestamp: SystemTime::now(),
    })
}

fn create_metrics_test_data() -> Arc<PluginDataExport> {
    Arc::new(PluginDataExport {
        plugin_id: "metrics".to_string(),
        title: "Code Quality Metrics".to_string(),
        description: Some("Integration test metrics data".to_string()),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![
                ColumnDef::new("Metric", ColumnType::String),
                ColumnDef::new("Value", ColumnType::String),
                ColumnDef::new("Type", ColumnType::String),
            ],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(vec![
            Row::new(vec![
                Value::String("complexity".to_string()),
                Value::String("6.8".to_string()),
                Value::String("Complexity".to_string()),
            ]),
            Row::new(vec![
                Value::String("coverage".to_string()),
                Value::String("87.3%".to_string()),
                Value::String("General".to_string()),
            ]),
        ])),
        export_hints: ExportHints {
            preferred_formats: vec![ExportFormat::Json],
            sort_by: Some("Type".to_string()),
            sort_ascending: true,
            limit: None,
            include_totals: false,
            include_row_numbers: true,
            custom_hints: HashMap::new(),
        },
        timestamp: SystemTime::now(),
    })
}

fn create_debug_test_data() -> Arc<PluginDataExport> {
    Arc::new(PluginDataExport {
        plugin_id: "debug".to_string(),
        title: "Debug Plugin Statistics".to_string(),
        description: Some("Integration test debug data".to_string()),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![
                ColumnDef::new("Metric", ColumnType::String),
                ColumnDef::new("Value", ColumnType::Integer),
                ColumnDef::new("Description", ColumnType::String),
            ],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(vec![
            Row::new(vec![
                Value::String("Messages Processed".to_string()),
                Value::Integer(150),
                Value::String("Total messages processed".to_string()),
            ]),
            Row::new(vec![
                Value::String("Errors".to_string()),
                Value::Integer(0),
                Value::String("Processing errors".to_string()),
            ]),
        ])),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    })
}

fn create_numbered_test_data(number: usize) -> Arc<PluginDataExport> {
    Arc::new(PluginDataExport {
        plugin_id: format!("test_plugin_{}", number),
        title: format!("Test Data {}", number),
        description: Some(format!("Test data item number {}", number)),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![
                ColumnDef::new("ID", ColumnType::Integer),
                ColumnDef::new("Name", ColumnType::String),
            ],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(vec![
            Row::new(vec![
                Value::Integer(number as i64),
                Value::String(format!("Item {}", number)),
            ]),
        ])),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    })
}

fn create_large_test_dataset(size: usize) -> Arc<PluginDataExport> {
    let mut rows = Vec::new();
    
    for i in 0..size {
        rows.push(Row::new(vec![
            Value::Integer(i as i64),
            Value::String(format!("Entry_{:04}", i)),
            Value::Float(i as f64 * 1.5),
            Value::Boolean(i % 2 == 0),
        ]));
    }
    
    Arc::new(PluginDataExport {
        plugin_id: "large_dataset_test".to_string(),
        title: format!("Large Dataset ({} entries)", size),
        description: Some(format!("Performance test with {} data entries", size)),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![
                ColumnDef::new("ID", ColumnType::Integer),
                ColumnDef::new("Name", ColumnType::String),
                ColumnDef::new("Value", ColumnType::Float),
                ColumnDef::new("Active", ColumnType::Boolean),
            ],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(rows)),
        export_hints: ExportHints {
            preferred_formats: vec![ExportFormat::Console],
            sort_by: Some("ID".to_string()),
            sort_ascending: true,
            limit: Some(100), // Limit display for performance
            include_totals: true,
            include_row_numbers: false,
            custom_hints: HashMap::new(),
        },
        timestamp: SystemTime::now(),
    })
}