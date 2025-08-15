//! Integration Tests for Data Export Pipeline
//! 
//! Tests the complete data export functionality including:
//! - Plugin data export creation
//! - Notification system integration
//! - ExportPlugin data collection and formatting
//! - End-to-end data flow from plugins to output

use std::sync::Arc;
use std::collections::HashMap;
use std::time::SystemTime;
use tokio::sync::RwLock;

use gstats::plugin::data_export::*;
use gstats::plugin::data_coordinator::DataCoordinator;
use gstats::notifications::events::PluginEvent;
use gstats::notifications::{AsyncNotificationManager, NotificationResult};
use gstats::notifications::traits::{Subscriber, NotificationManager};
use gstats::plugin::builtin::export::ExportPlugin;

/// Mock subscriber to capture exported data for testing
struct MockExportSubscriber {
    received_data: Arc<RwLock<Vec<Arc<PluginDataExport>>>>,
}

impl MockExportSubscriber {
    fn new() -> Self {
        Self {
            received_data: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    async fn get_received_data(&self) -> Vec<Arc<PluginDataExport>> {
        self.received_data.read().await.clone()
    }
}

#[async_trait::async_trait]
impl Subscriber<PluginEvent> for MockExportSubscriber {
    async fn handle_event(&self, event: PluginEvent) -> NotificationResult<()> {
        match event {
            PluginEvent::DataReady { export, .. } => {
                let mut data = self.received_data.write().await;
                data.push(export);
                Ok(())
            }
            _ => Ok(()),
        }
    }
    
    fn subscriber_id(&self) -> &str {
        "mock_export_subscriber"
    }
}

#[tokio::test]
async fn test_plugin_data_export_creation() {
    // Test creating a PluginDataExport with tabular data
    let schema = DataSchema {
        columns: vec![
            ColumnDef::new("Name", ColumnType::String),
            ColumnDef::new("Count", ColumnType::Integer),
            ColumnDef::new("Percentage", ColumnType::Float),
        ],
        metadata: HashMap::new(),
    };
    
    let rows = vec![
        Row::new(vec![
            Value::String("Alice".to_string()),
            Value::Integer(100),
            Value::Float(50.5),
        ]),
        Row::new(vec![
            Value::String("Bob".to_string()),
            Value::Integer(75),
            Value::Float(37.75),
        ]),
    ];
    
    let export = PluginDataExport {
        plugin_id: "test_plugin".to_string(),
        title: "Test Data".to_string(),
        description: Some("Test data for integration testing".to_string()),
        data_type: DataExportType::Tabular,
        schema,
        data: DataPayload::Rows(Arc::new(rows)),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    };
    
    assert_eq!(export.plugin_id, "test_plugin");
    assert_eq!(export.title, "Test Data");
    assert_eq!(export.data_type, DataExportType::Tabular);
    
    // Verify data payload
    if let DataPayload::Rows(rows) = &export.data {
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].values.len(), 3);
        assert_eq!(rows[1].values.len(), 3);
    } else {
        panic!("Expected Rows data payload");
    }
}

#[tokio::test]
async fn test_data_coordinator_functionality() {
    let mut coordinator = DataCoordinator::with_expected_plugins(vec![
        "commits".to_string(),
        "metrics".to_string(),
    ]);
    
    // Initially not complete
    assert!(!coordinator.is_complete());
    
    // Create test data exports
    let commits_export = Arc::new(PluginDataExport {
        plugin_id: "commits".to_string(),
        title: "Commit Statistics".to_string(),
        description: None,
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![ColumnDef::new("Author", ColumnType::String)],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(vec![
            Row::new(vec![Value::String("Alice".to_string())]),
        ])),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    });
    
    let metrics_export = Arc::new(PluginDataExport {
        plugin_id: "metrics".to_string(),
        title: "Code Metrics".to_string(),
        description: None,
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![ColumnDef::new("Metric", ColumnType::String)],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(vec![
            Row::new(vec![Value::String("Complexity".to_string())]),
        ])),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    });
    
    // Add data from plugins
    coordinator.add_data("commits".to_string(), commits_export.clone());
    assert!(!coordinator.is_complete()); // Still waiting for metrics
    
    coordinator.add_data("metrics".to_string(), metrics_export.clone());
    assert!(coordinator.is_complete()); // Now complete
    
    // Get all data
    let all_data = coordinator.get_all_data();
    assert_eq!(all_data.len(), 2);
    
    // Verify data content
    let commits_data = all_data.iter().find(|d| d.plugin_id == "commits").unwrap();
    let metrics_data = all_data.iter().find(|d| d.plugin_id == "metrics").unwrap();
    
    assert_eq!(commits_data.title, "Commit Statistics");
    assert_eq!(metrics_data.title, "Code Metrics");
}

#[tokio::test]
async fn test_notification_based_export_pipeline() {
    // Create notification manager
    let mut notification_manager = AsyncNotificationManager::new();
    
    // Create mock subscriber to capture exports
    let mock_subscriber = Arc::new(MockExportSubscriber::new());
    
    // Subscribe to plugin events
    notification_manager.subscribe(mock_subscriber.clone()).await.unwrap();
    
    // Simulate plugin publishing data ready events
    let export1 = Arc::new(PluginDataExport {
        plugin_id: "commits".to_string(),
        title: "Commit Analysis".to_string(),
        description: Some("Test commit data".to_string()),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![
                ColumnDef::new("Author", ColumnType::String),
                ColumnDef::new("Commits", ColumnType::Integer),
            ],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(vec![
            Row::new(vec![
                Value::String("Alice".to_string()),
                Value::Integer(50),
            ]),
            Row::new(vec![
                Value::String("Bob".to_string()),
                Value::Integer(30),
            ]),
        ])),
        export_hints: ExportHints {
            preferred_formats: vec![ExportFormat::Json, ExportFormat::Csv],
            sort_by: Some("Commits".to_string()),
            sort_ascending: false,
            limit: None,
            include_totals: true,
            include_row_numbers: false,
            custom_hints: HashMap::new(),
        },
        timestamp: SystemTime::now(),
    });
    
    let event1 = PluginEvent::DataReady {
        plugin_id: "commits".to_string(),
        scan_id: "test_scan_001".to_string(),
        export: export1.clone(),
    };
    
    // Publish the event
    notification_manager.publish(event1).await.unwrap();
    
    // Wait a moment for async processing
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    
    // Verify data was received
    let received_data = mock_subscriber.get_received_data().await;
    assert_eq!(received_data.len(), 1);
    
    let received_export = &received_data[0];
    assert_eq!(received_export.plugin_id, "commits");
    assert_eq!(received_export.title, "Commit Analysis");
    
    // Verify data content
    if let DataPayload::Rows(rows) = &received_export.data {
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].values[0], Value::String("Alice".to_string()));
        assert_eq!(rows[0].values[1], Value::Integer(50));
    } else {
        panic!("Expected Rows data payload");
    }
}

#[tokio::test]
async fn test_export_plugin_data_collection() {
    // This test verifies that ExportPlugin correctly collects data from multiple plugins
    let export_plugin = ExportPlugin::new();
    
    // Create test data from multiple plugins
    let commits_export = Arc::new(PluginDataExport {
        plugin_id: "commits".to_string(),
        title: "Commit Statistics".to_string(),
        description: Some("Git commit analysis".to_string()),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![
                ColumnDef::new("Author", ColumnType::String),
                ColumnDef::new("Commits", ColumnType::Integer),
            ],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(vec![
            Row::new(vec![
                Value::String("Alice".to_string()),
                Value::Integer(25),
            ]),
            Row::new(vec![
                Value::String("Bob".to_string()),
                Value::Integer(15),
            ]),
        ])),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    });
    
    let metrics_export = Arc::new(PluginDataExport {
        plugin_id: "metrics".to_string(),
        title: "Code Quality Metrics".to_string(),
        description: Some("Code analysis metrics".to_string()),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![
                ColumnDef::new("Metric", ColumnType::String),
                ColumnDef::new("Value", ColumnType::Float),
            ],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(vec![
            Row::new(vec![
                Value::String("Complexity".to_string()),
                Value::Float(7.5),
            ]),
            Row::new(vec![
                Value::String("Coverage".to_string()),
                Value::Float(85.2),
            ]),
        ])),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    });
    
    // Simulate DataReady events
    let event1 = PluginEvent::DataReady {
        plugin_id: "commits".to_string(),
        scan_id: "test_scan_002".to_string(),
        export: commits_export,
    };
    
    let event2 = PluginEvent::DataReady {
        plugin_id: "metrics".to_string(),
        scan_id: "test_scan_002".to_string(),
        export: metrics_export,
    };
    
    // Process events (this would normally trigger export when all expected plugins report)
    export_plugin.handle_event(event1).await.unwrap();
    export_plugin.handle_event(event2).await.unwrap();
    
    // Note: Full end-to-end testing would require a complete notification system setup
    // This test verifies the event handling works without errors
}

#[tokio::test]
async fn test_value_formatting_and_display() {
    // Test various value types and their string representations
    let test_values = vec![
        (Value::String("Hello World".to_string()), "Hello World"),
        (Value::Integer(42), "42"),
        (Value::Float(3.14159), "3.14"),
        (Value::Boolean(true), "true"),
        (Value::Boolean(false), "false"),
        (Value::Null, ""),
    ];
    
    for (value, expected) in test_values {
        assert_eq!(value.to_string(), expected);
    }
    
    // Test timestamp formatting
    let timestamp = SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(1234567890)).unwrap();
    let timestamp_value = Value::Timestamp(timestamp);
    assert_eq!(timestamp_value.to_string(), "1234567890");
    
    // Test duration formatting
    let duration = std::time::Duration::from_secs_f64(123.456);
    let duration_value = Value::Duration(duration);
    assert_eq!(duration_value.to_string(), "123.5s");
}

#[tokio::test]
async fn test_data_schema_validation() {
    // Test that schemas properly describe data structure
    let schema = DataSchema {
        columns: vec![
            ColumnDef::new("ID", ColumnType::Integer)
                .with_description("Unique identifier".to_string())
                .with_width(10),
            ColumnDef::new("Name", ColumnType::String)
                .with_description("Display name".to_string())
                .with_width(20),
            ColumnDef::new("Score", ColumnType::Float)
                .with_description("Performance score".to_string())
                .with_format_hint("percentage".to_string()),
        ],
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("version".to_string(), "1.0".to_string());
            meta.insert("generated_by".to_string(), "test".to_string());
            meta
        },
    };
    
    assert_eq!(schema.columns.len(), 3);
    assert_eq!(schema.columns[0].name, "ID");
    assert_eq!(schema.columns[0].data_type, ColumnType::Integer);
    assert_eq!(schema.columns[0].description, Some("Unique identifier".to_string()));
    assert_eq!(schema.columns[0].preferred_width, Some(10));
    
    assert_eq!(schema.columns[2].format_hint, Some("percentage".to_string()));
    assert_eq!(schema.metadata.get("version"), Some(&"1.0".to_string()));
}

#[tokio::test]
async fn test_export_hints_configuration() {
    // Test export hints for formatting guidance
    let mut custom_hints = HashMap::new();
    custom_hints.insert("highlight_top".to_string(), "3".to_string());
    custom_hints.insert("currency".to_string(), "USD".to_string());
    
    let hints = ExportHints {
        preferred_formats: vec![
            ExportFormat::Html,
            ExportFormat::Json,
            ExportFormat::Csv,
        ],
        sort_by: Some("Score".to_string()),
        sort_ascending: false,
        limit: Some(100),
        include_totals: true,
        include_row_numbers: true,
        custom_hints,
    };
    
    assert_eq!(hints.preferred_formats.len(), 3);
    assert_eq!(hints.preferred_formats[0], ExportFormat::Html);
    assert_eq!(hints.sort_by, Some("Score".to_string()));
    assert!(!hints.sort_ascending);
    assert_eq!(hints.limit, Some(100));
    assert!(hints.include_totals);
    assert!(hints.include_row_numbers);
    assert_eq!(hints.custom_hints.get("highlight_top"), Some(&"3".to_string()));
}

#[tokio::test]
async fn test_hierarchical_data_export() {
    // Test tree/hierarchical data structure
    let root = TreeNode::new("Root")
        .with_value(Value::String("root_value".to_string()))
        .add_child(
            TreeNode::new("Child1")
                .with_value(Value::Integer(100))
                .add_child(TreeNode::new("Grandchild1"))
        )
        .add_child(
            TreeNode::new("Child2")
                .with_value(Value::Float(3.14))
        );
    
    let export = PluginDataExport {
        plugin_id: "hierarchy_test".to_string(),
        title: "Hierarchical Data Test".to_string(),
        description: Some("Testing tree structure export".to_string()),
        data_type: DataExportType::Hierarchical,
        schema: DataSchema {
            columns: Vec::new(), // Not applicable for hierarchical data
            metadata: HashMap::new(),
        },
        data: DataPayload::Tree(Arc::new(root)),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    };
    
    assert_eq!(export.data_type, DataExportType::Hierarchical);
    
    if let DataPayload::Tree(tree) = &export.data {
        assert_eq!(tree.label, "Root");
        assert_eq!(tree.children.len(), 2);
        assert_eq!(tree.children[0].label, "Child1");
        assert_eq!(tree.children[0].children.len(), 1);
        assert_eq!(tree.children[0].children[0].label, "Grandchild1");
    } else {
        panic!("Expected Tree data payload");
    }
}

#[tokio::test]
async fn test_key_value_data_export() {
    // Test key-value data structure
    let mut data = HashMap::new();
    data.insert("total_files".to_string(), Value::Integer(1250));
    data.insert("total_lines".to_string(), Value::Integer(45000));
    data.insert("average_complexity".to_string(), Value::Float(6.7));
    data.insert("last_updated".to_string(), Value::String("2025-01-14".to_string()));
    data.insert("has_tests".to_string(), Value::Boolean(true));
    
    let export = PluginDataExport {
        plugin_id: "summary_stats".to_string(),
        title: "Project Summary".to_string(),
        description: Some("High-level project statistics".to_string()),
        data_type: DataExportType::KeyValue,
        schema: DataSchema {
            columns: Vec::new(), // Not applicable for key-value data
            metadata: HashMap::new(),
        },
        data: DataPayload::KeyValue(Arc::new(data)),
        export_hints: ExportHints {
            preferred_formats: vec![ExportFormat::Json, ExportFormat::Yaml],
            sort_by: None,
            sort_ascending: true,
            limit: None,
            include_totals: false,
            include_row_numbers: false,
            custom_hints: HashMap::new(),
        },
        timestamp: SystemTime::now(),
    };
    
    assert_eq!(export.data_type, DataExportType::KeyValue);
    
    if let DataPayload::KeyValue(kv_data) = &export.data {
        assert_eq!(kv_data.len(), 5);
        assert_eq!(kv_data.get("total_files"), Some(&Value::Integer(1250)));
        assert_eq!(kv_data.get("has_tests"), Some(&Value::Boolean(true)));
    } else {
        panic!("Expected KeyValue data payload");
    }
}

#[tokio::test]
async fn test_multi_plugin_data_coordination() {
    // Test coordinating data from multiple plugins in a realistic scenario
    let mut coordinator = DataCoordinator::with_expected_plugins(vec![
        "commits".to_string(),
        "metrics".to_string(),
        "debug".to_string(),
    ]);
    
    // Simulate commits plugin data
    let commits_data = create_test_commits_export();
    coordinator.add_data("commits".to_string(), commits_data);
    assert!(!coordinator.is_complete());
    
    // Simulate metrics plugin data
    let metrics_data = create_test_metrics_export();
    coordinator.add_data("metrics".to_string(), metrics_data);
    assert!(!coordinator.is_complete());
    
    // Simulate debug plugin data
    let debug_data = create_test_debug_export();
    coordinator.add_data("debug".to_string(), debug_data);
    assert!(coordinator.is_complete());
    
    // Verify all data is collected
    let all_data = coordinator.get_all_data();
    assert_eq!(all_data.len(), 3);
    
    let plugin_ids: Vec<String> = all_data.iter().map(|d| d.plugin_id.clone()).collect();
    assert!(plugin_ids.contains(&"commits".to_string()));
    assert!(plugin_ids.contains(&"metrics".to_string()));
    assert!(plugin_ids.contains(&"debug".to_string()));
    
    // Test clearing coordinator
    coordinator.clear();
    assert!(!coordinator.is_complete());
    let cleared_data = coordinator.get_all_data();
    assert_eq!(cleared_data.len(), 0);
}

// Helper functions for creating test data

fn create_test_commits_export() -> Arc<PluginDataExport> {
    Arc::new(PluginDataExport {
        plugin_id: "commits".to_string(),
        title: "Commit Analysis".to_string(),
        description: Some("Git commit statistics by author".to_string()),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![
                ColumnDef::new("Author", ColumnType::String),
                ColumnDef::new("Commits", ColumnType::Integer),
                ColumnDef::new("Percentage", ColumnType::Float)
                    .with_format_hint("percentage".to_string()),
            ],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(vec![
            Row::new(vec![
                Value::String("Alice Smith".to_string()),
                Value::Integer(45),
                Value::Float(56.25),
            ]),
            Row::new(vec![
                Value::String("Bob Johnson".to_string()),
                Value::Integer(35),
                Value::Float(43.75),
            ]),
        ])),
        export_hints: ExportHints {
            preferred_formats: vec![ExportFormat::Console, ExportFormat::Html],
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

fn create_test_metrics_export() -> Arc<PluginDataExport> {
    Arc::new(PluginDataExport {
        plugin_id: "metrics".to_string(),
        title: "Code Quality Metrics".to_string(),
        description: Some("Comprehensive code analysis metrics".to_string()),
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
                Value::String("cyclomatic_complexity".to_string()),
                Value::String("7.2".to_string()),
                Value::String("Complexity".to_string()),
            ]),
            Row::new(vec![
                Value::String("code_coverage".to_string()),
                Value::String("85.4%".to_string()),
                Value::String("General".to_string()),
            ]),
            Row::new(vec![
                Value::String("hotspot_files".to_string()),
                Value::String("12".to_string()),
                Value::String("Hotspot".to_string()),
            ]),
        ])),
        export_hints: ExportHints {
            preferred_formats: vec![ExportFormat::Json, ExportFormat::Html],
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

fn create_test_debug_export() -> Arc<PluginDataExport> {
    Arc::new(PluginDataExport {
        plugin_id: "debug".to_string(),
        title: "Debug Plugin Statistics".to_string(),
        description: Some("Message processing statistics from debug plugin".to_string()),
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
                Value::Integer(1250),
                Value::String("Total number of messages processed by debug plugin".to_string()),
            ]),
            Row::new(vec![
                Value::String("Commit Messages".to_string()),
                Value::Integer(80),
                Value::String("Number of git commit info messages".to_string()),
            ]),
            Row::new(vec![
                Value::String("Display Errors".to_string()),
                Value::Integer(0),
                Value::String("Number of message display errors encountered".to_string()),
            ]),
        ])),
        export_hints: ExportHints {
            preferred_formats: vec![ExportFormat::Console, ExportFormat::Csv],
            sort_by: Some("Metric".to_string()),
            sort_ascending: true,
            limit: None,
            include_totals: false,
            include_row_numbers: true,
            custom_hints: HashMap::new(),
        },
        timestamp: SystemTime::now(),
    })
}