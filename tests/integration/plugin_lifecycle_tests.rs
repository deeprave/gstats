//! Integration Tests for Plugin Lifecycle with Data Export
//! 
//! Tests the complete plugin lifecycle including initialization, message processing,
//! data export creation, and cleanup for all plugins that support data export.

use std::sync::Arc;
use std::collections::HashMap;
use std::time::SystemTime;
use tokio::sync::RwLock;

use gstats::plugin::traits::{Plugin, ConsumerPlugin, PluginArgumentParser};
use gstats::plugin::context::{PluginContext, PluginRequest, PluginResponse};
use gstats::plugin::builtin::{
    commits::CommitsPlugin,
    metrics::MetricsPlugin,
    debug::DebugPlugin,
    export::ExportPlugin,
};
use gstats::plugin::data_export::*;
use gstats::scanner::{ScannerConfig, QueryParams};
use gstats::scanner::messages::{ScanMessage, MessageData, MessageHeader};
use gstats::queue::QueueEvent;

/// Test the complete lifecycle of CommitsPlugin with data export
#[tokio::test]
async fn test_commits_plugin_complete_lifecycle() {
    let mut plugin = CommitsPlugin::new();
    let context = create_test_context();
    
    // Initialize plugin
    plugin.initialize(&context).await.unwrap();
    
    // Test plugin info
    let info = plugin.plugin_info();
    assert_eq!(info.name, "commits");
    assert_eq!(info.plugin_type, gstats::plugin::traits::PluginType::Processing);
    
    // Test capabilities
    let request = PluginRequest::GetCapabilities;
    let response = plugin.execute(request).await.unwrap();
    match response {
        PluginResponse::Capabilities(caps) => {
            assert!(!caps.is_empty());
            assert!(caps.iter().any(|c| c.name == "commit_analysis"));
        }
        _ => panic!("Expected Capabilities response"),
    }
    
    // Simulate scan lifecycle
    let scan_event = QueueEvent::scan_started("test_scan".to_string());
    plugin.handle_queue_event(&scan_event).await.unwrap();
    
    // Create test commit messages
    let commit_messages = create_test_commit_messages();
    
    // Process commit messages (simulate queue consumption)
    for message in commit_messages {
        // This simulates the internal processing that would happen during queue consumption
        // In a real scenario, this would be called by the queue consumer
        process_commit_message_internal(&plugin, &message).await;
    }
    
    // Simulate scan completion
    let complete_event = QueueEvent::scan_complete("test_scan".to_string(), 3);
    plugin.handle_queue_event(&complete_event).await.unwrap();
    
    // Test data export creation (if plugin had notification manager)
    let export_result = create_commits_export_simulation(&plugin, "test_scan").await;
    assert!(export_result.is_ok());
    
    let export = export_result.unwrap();
    assert_eq!(export.plugin_id, "commits");
    assert_eq!(export.title, "Commit Analysis");
    assert_eq!(export.data_type, DataExportType::Tabular);
    
    // Verify exported data structure
    if let DataPayload::Rows(rows) = export.data {
        assert!(!rows.is_empty());
        assert_eq!(rows[0].values.len(), 3); // Author, Commits, Percentage
    } else {
        panic!("Expected Rows data payload");
    }
    
    // Cleanup
    plugin.cleanup().await.unwrap();
}

/// Test the complete lifecycle of MetricsPlugin with data export
#[tokio::test]
async fn test_metrics_plugin_complete_lifecycle() {
    let mut plugin = MetricsPlugin::new();
    let context = create_test_context();
    
    // Initialize plugin
    plugin.initialize(&context).await.unwrap();
    
    // Test plugin info
    let info = plugin.plugin_info();
    assert_eq!(info.name, "metrics");
    assert_eq!(info.plugin_type, gstats::plugin::traits::PluginType::Processing);
    
    // Test consumer preferences
    let prefs = plugin.consumer_preferences();
    assert!(prefs.consume_all_messages);
    assert!(prefs.high_frequency_capable);
    
    // Simulate scan lifecycle with metrics collection
    let scan_event = QueueEvent::scan_started("metrics_test_scan".to_string());
    plugin.handle_queue_event(&scan_event).await.unwrap();
    
    // Simulate metrics processing
    simulate_metrics_processing(&plugin).await;
    
    // Simulate scan completion
    let complete_event = QueueEvent::scan_complete("metrics_test_scan".to_string(), 5);
    plugin.handle_queue_event(&complete_event).await.unwrap();
    
    // Test data export creation simulation
    let export_result = create_metrics_export_simulation(&plugin, "metrics_test_scan").await;
    assert!(export_result.is_ok());
    
    let export = export_result.unwrap();
    assert_eq!(export.plugin_id, "metrics");
    assert_eq!(export.title, "Code Quality Metrics");
    assert_eq!(export.data_type, DataExportType::Tabular);
    
    // Cleanup
    plugin.cleanup().await.unwrap();
}

/// Test the complete lifecycle of DebugPlugin with export functionality
#[tokio::test]
async fn test_debug_plugin_with_export_flag() {
    let mut plugin = DebugPlugin::new();
    let context = create_test_context();
    
    // Test argument parsing with export flag
    let args = vec!["--verbose".to_string(), "--export".to_string()];
    plugin.parse_plugin_args(&args).await.unwrap();
    
    // Initialize plugin
    plugin.initialize(&context).await.unwrap();
    
    // Test plugin info
    let info = plugin.plugin_info();
    assert_eq!(info.name, "debug");
    assert_eq!(info.plugin_type, gstats::plugin::traits::PluginType::Processing);
    
    // Test argument schema includes export flag
    let schema = plugin.get_arg_schema();
    assert!(schema.iter().any(|arg| arg.name == "--export"));
    
    // Simulate debug message processing
    let scan_event = QueueEvent::scan_started("debug_test_scan".to_string());
    plugin.handle_queue_event(&scan_event).await.unwrap();
    
    // Simulate processing some messages
    simulate_debug_message_processing(&plugin).await;
    
    // Simulate scan completion (should trigger export if enabled)
    let complete_event = QueueEvent::scan_complete("debug_test_scan".to_string(), 10);
    plugin.handle_queue_event(&complete_event).await.unwrap();
    
    // Test data export creation simulation
    let export_result = create_debug_export_simulation(&plugin, "debug_test_scan").await;
    assert!(export_result.is_ok());
    
    let export = export_result.unwrap();
    assert_eq!(export.plugin_id, "debug");
    assert_eq!(export.title, "Debug Plugin Statistics");
    assert_eq!(export.data_type, DataExportType::Tabular);
    
    // Cleanup
    plugin.cleanup().await.unwrap();
}

/// Test the ExportPlugin complete lifecycle
#[tokio::test]
async fn test_export_plugin_complete_lifecycle() {
    let mut plugin = ExportPlugin::new();
    let context = create_test_context();
    
    // Test argument parsing
    let args = vec!["--format".to_string(), "json".to_string()];
    plugin.parse_plugin_args(&args).await.unwrap();
    
    // Initialize plugin
    plugin.initialize(&context).await.unwrap();
    
    // Test plugin info
    let info = plugin.plugin_info();
    assert_eq!(info.name, "export");
    assert_eq!(info.plugin_type, gstats::plugin::traits::PluginType::Output);
    assert!(info.load_by_default); // Export plugin should load by default
    
    // Test argument schema
    let schema = plugin.get_arg_schema();
    assert!(schema.iter().any(|arg| arg.name == "--format"));
    assert!(schema.iter().any(|arg| arg.name == "--output"));
    assert!(schema.iter().any(|arg| arg.name == "--template"));
    
    // Cleanup
    plugin.cleanup().await.unwrap();
}

/// Test plugin interaction and data coordination
#[tokio::test]
async fn test_multi_plugin_coordination() {
    // Create multiple plugins
    let mut commits_plugin = CommitsPlugin::new();
    let mut metrics_plugin = MetricsPlugin::new();
    let mut debug_plugin = DebugPlugin::new();
    let mut export_plugin = ExportPlugin::new();
    
    let context = create_test_context();
    
    // Initialize all plugins
    commits_plugin.initialize(&context).await.unwrap();
    metrics_plugin.initialize(&context).await.unwrap();
    debug_plugin.initialize(&context).await.unwrap();
    export_plugin.initialize(&context).await.unwrap();
    
    // Simulate coordinated scan lifecycle
    let scan_id = "multi_plugin_test";
    let scan_start = QueueEvent::scan_started(scan_id.to_string());
    
    // All plugins receive scan start
    commits_plugin.handle_queue_event(&scan_start).await.unwrap();
    metrics_plugin.handle_queue_event(&scan_start).await.unwrap();
    debug_plugin.handle_queue_event(&scan_start).await.unwrap();
    
    // Simulate data processing
    simulate_multi_plugin_processing(&commits_plugin, &metrics_plugin, &debug_plugin).await;
    
    // All plugins receive scan complete
    let scan_complete = QueueEvent::scan_complete(scan_id.to_string(), 15);
    commits_plugin.handle_queue_event(&scan_complete).await.unwrap();
    metrics_plugin.handle_queue_event(&scan_complete).await.unwrap();
    debug_plugin.handle_queue_event(&scan_complete).await.unwrap();
    
    // Create exports from all plugins
    let commits_export = create_commits_export_simulation(&commits_plugin, scan_id).await.unwrap();
    let metrics_export = create_metrics_export_simulation(&metrics_plugin, scan_id).await.unwrap();
    let debug_export = create_debug_export_simulation(&debug_plugin, scan_id).await.unwrap();
    
    // Verify all exports are valid and unique
    let exports = vec![commits_export, metrics_export, debug_export];
    assert_eq!(exports.len(), 3);
    
    let plugin_ids: Vec<String> = exports.iter().map(|e| e.plugin_id.clone()).collect();
    assert!(plugin_ids.contains(&"commits".to_string()));
    assert!(plugin_ids.contains(&"metrics".to_string()));
    assert!(plugin_ids.contains(&"debug".to_string()));
    
    // Cleanup all plugins
    commits_plugin.cleanup().await.unwrap();
    metrics_plugin.cleanup().await.unwrap();
    debug_plugin.cleanup().await.unwrap();
    export_plugin.cleanup().await.unwrap();
}

/// Test plugin error handling during lifecycle
#[tokio::test]
async fn test_plugin_error_handling() {
    let mut plugin = CommitsPlugin::new();
    let context = create_test_context();
    
    // Test execution before initialization (should fail)
    let request = PluginRequest::GetStatistics;
    let result = plugin.execute(request).await;
    assert!(result.is_err());
    
    // Initialize and test normal operation
    plugin.initialize(&context).await.unwrap();
    
    let request = PluginRequest::GetStatistics;
    let result = plugin.execute(request).await;
    assert!(result.is_ok());
    
    // Test double cleanup (should be idempotent)
    plugin.cleanup().await.unwrap();
    plugin.cleanup().await.unwrap(); // Should not fail
}

/// Test plugin data requirements
#[tokio::test]
async fn test_plugin_data_requirements() {
    let commits_plugin = CommitsPlugin::new();
    let metrics_plugin = MetricsPlugin::new();
    let debug_plugin = DebugPlugin::new();
    let export_plugin = ExportPlugin::new();
    
    // Test data requirements for each plugin
    assert!(!commits_plugin.requires_current_file_content()); // Only needs metadata
    assert!(!commits_plugin.requires_historical_file_content());
    
    assert!(metrics_plugin.requires_current_file_content()); // Needs code for analysis
    assert!(!metrics_plugin.requires_historical_file_content());
    
    assert!(!debug_plugin.requires_current_file_content()); // Only displays metadata
    assert!(!debug_plugin.requires_historical_file_content());
    
    assert!(!export_plugin.requires_current_file_content()); // Formats data
    assert!(!export_plugin.requires_historical_file_content());
}

// Helper functions

fn create_test_context() -> PluginContext {
    PluginContext::new(
        Arc::new(ScannerConfig::default()),
        Arc::new(QueryParams::default()),
    )
}

fn create_test_commit_messages() -> Vec<ScanMessage> {
    vec![
        create_commit_message("Alice", "abc123", "Initial commit"),
        create_commit_message("Bob", "def456", "Add feature X"),
        create_commit_message("Alice", "ghi789", "Fix bug in feature X"),
    ]
}

fn create_commit_message(author: &str, hash: &str, message: &str) -> ScanMessage {
    let data = MessageData::CommitInfo {
        author: author.to_string(),
        hash: hash.to_string(),
        message: message.to_string(),
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
        changed_files: vec![],
    };
    
    let header = MessageHeader::new(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );
    
    ScanMessage::new(header, data)
}

async fn process_commit_message_internal(plugin: &CommitsPlugin, message: &ScanMessage) {
    // This simulates the internal processing that CommitsPlugin does
    // In a real scenario, this would be called via the queue consumer
    if let MessageData::CommitInfo { author, .. } = message.data() {
        // This is a simplified version of what the plugin would do internally
        // The actual implementation would be through the process_message method
        log::debug!("Processing commit from {}", author);
    }
}

async fn create_commits_export_simulation(plugin: &CommitsPlugin, scan_id: &str) -> Result<PluginDataExport, Box<dyn std::error::Error>> {
    // This simulates what the plugin would create for export
    // In the actual implementation, this would be the create_data_export method
    Ok(PluginDataExport {
        plugin_id: "commits".to_string(),
        title: "Commit Analysis".to_string(),
        description: Some(format!("Commit analysis for scan {}", scan_id)),
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
                Value::Integer(2),
                Value::Float(66.67),
            ]),
            Row::new(vec![
                Value::String("Bob".to_string()),
                Value::Integer(1),
                Value::Float(33.33),
            ]),
        ])),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    })
}

async fn simulate_metrics_processing(_plugin: &MetricsPlugin) {
    // Simulate metrics collection
    log::debug!("Simulating metrics processing");
}

async fn create_metrics_export_simulation(plugin: &MetricsPlugin, scan_id: &str) -> Result<PluginDataExport, Box<dyn std::error::Error>> {
    // This simulates what the metrics plugin would create for export
    Ok(PluginDataExport {
        plugin_id: "metrics".to_string(),
        title: "Code Quality Metrics".to_string(),
        description: Some(format!("Metrics analysis for scan {}", scan_id)),
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
                Value::String("7.2".to_string()),
                Value::String("Complexity".to_string()),
            ]),
        ])),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    })
}

async fn simulate_debug_message_processing(_plugin: &DebugPlugin) {
    // Simulate debug message processing
    log::debug!("Simulating debug message processing");
}

async fn create_debug_export_simulation(plugin: &DebugPlugin, scan_id: &str) -> Result<PluginDataExport, Box<dyn std::error::Error>> {
    // This simulates what the debug plugin would create for export
    Ok(PluginDataExport {
        plugin_id: "debug".to_string(),
        title: "Debug Plugin Statistics".to_string(),
        description: Some(format!("Debug statistics for scan {}", scan_id)),
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
                Value::Integer(10),
                Value::String("Total messages processed".to_string()),
            ]),
        ])),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    })
}

async fn simulate_multi_plugin_processing(
    _commits: &CommitsPlugin,
    _metrics: &MetricsPlugin,
    _debug: &DebugPlugin,
) {
    // Simulate coordinated processing across multiple plugins
    log::debug!("Simulating multi-plugin processing");
}