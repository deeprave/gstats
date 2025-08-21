//! Integration Tests for Export Formatting
//! 
//! Tests the various export formatters (JSON, CSV, XML, YAML, HTML, Markdown)
//! to ensure they properly handle different data types and formatting options.

use std::sync::Arc;
use std::collections::HashMap;
use std::time::SystemTime;

use gstats::plugin::data_export::*;
use gstats::plugin::ExportPlugin;

/// Helper function to format data as console output using the new architecture
async fn format_as_console(plugin: &ExportPlugin, data: &[Arc<PluginDataExport>]) -> Result<String, Box<dyn std::error::Error>> {
    // Use the public format_as_console method
    Ok(plugin.format_as_console(data).await?)
}

#[tokio::test]
async fn test_json_formatter_with_tabular_data() {
    let export_plugin = ExportPlugin::new();
    
    let test_data = create_sample_tabular_data();
    let formatted = export_plugin.format_json(&[test_data]).await.unwrap();
    
    // Verify JSON structure
    let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();
    assert!(parsed.is_object());
    
    let obj = parsed.as_object().unwrap();
    assert!(obj.contains_key("sample_plugin"));
    
    let plugin_data = &obj["sample_plugin"];
    assert_eq!(plugin_data["title"], "Sample Data");
    assert_eq!(plugin_data["type"], "Tabular");
    assert!(plugin_data["data"].is_array());
    
    let data_array = plugin_data["data"].as_array().unwrap();
    assert_eq!(data_array.len(), 2);
    
    // Verify first row
    let first_row = &data_array[0];
    assert_eq!(first_row["Name"], "Alice");
    assert_eq!(first_row["Score"], "95");
}

#[tokio::test]
async fn test_csv_formatter_with_tabular_data() {
    let export_plugin = ExportPlugin::new();
    
    let test_data = create_sample_tabular_data();
    let formatted = export_plugin.format_csv(&[test_data]).await.unwrap();
    
    let lines: Vec<&str> = formatted.lines().collect();
    assert_eq!(lines.len(), 3); // Header + 2 data rows
    
    // Verify header
    assert_eq!(lines[0], "Name,Score,Active");
    
    // Verify data rows
    assert_eq!(lines[1], "Alice,95,true");
    assert_eq!(lines[2], "Bob,87,false");
}

#[tokio::test]
async fn test_xml_formatter_with_tabular_data() {
    let export_plugin = ExportPlugin::new();
    
    let test_data = create_sample_tabular_data();
    let formatted = export_plugin.format_xml(&[test_data]).await.unwrap();
    
    // Verify XML structure
    assert!(formatted.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(formatted.contains("<export>"));
    assert!(formatted.contains("</export>"));
    assert!(formatted.contains("<plugin id=\"sample_plugin\">"));
    assert!(formatted.contains("<title>Sample Data</title>"));
    assert!(formatted.contains("<data>"));
    assert!(formatted.contains("<row>"));
    assert!(formatted.contains("<Name>Alice</Name>"));
    assert!(formatted.contains("<Score>95</Score>"));
}

#[tokio::test]
async fn test_yaml_formatter_with_tabular_data() {
    let export_plugin = ExportPlugin::new();
    
    let test_data = create_sample_tabular_data();
    let formatted = export_plugin.format_yaml(&[test_data]).await.unwrap();
    
    // Verify YAML structure
    assert!(formatted.contains("sample_plugin:"));
    assert!(formatted.contains("title: Sample Data"));
    assert!(formatted.contains("data:"));
    assert!(formatted.contains("- Name: Alice"));
    assert!(formatted.contains("Score: 95"));
}

#[tokio::test]
async fn test_html_formatter_with_tabular_data() {
    let export_plugin = ExportPlugin::new();
    
    let test_data = create_sample_tabular_data();
    let formatted = export_plugin.format_html(&[test_data]).await.unwrap();
    
    // Verify HTML structure
    assert!(formatted.contains("<!DOCTYPE html>"));
    assert!(formatted.contains("<html>"));
    assert!(formatted.contains("<head>"));
    assert!(formatted.contains("<title>Export Report</title>"));
    assert!(formatted.contains("<body>"));
    assert!(formatted.contains("<h2>Sample Data</h2>"));
    assert!(formatted.contains("<table>"));
    assert!(formatted.contains("<thead>"));
    assert!(formatted.contains("<tbody>"));
    assert!(formatted.contains("<th>Name</th>"));
    assert!(formatted.contains("<td>Alice</td>"));
    assert!(formatted.contains("<td>95</td>"));
}

#[tokio::test]
async fn test_markdown_formatter_with_tabular_data() {
    let export_plugin = ExportPlugin::new();
    
    let test_data = create_sample_tabular_data();
    let formatted = export_plugin.format_markdown(&[test_data]).await.unwrap();
    
    // Verify Markdown structure
    assert!(formatted.contains("# Export Report"));
    assert!(formatted.contains("## Sample Data"));
    assert!(formatted.contains("| Name | Score | Active |"));
    assert!(formatted.contains("| --- | ---: | --- |")); // Right-aligned numbers
    assert!(formatted.contains("| Alice | 95 | true |"));
    assert!(formatted.contains("| Bob | 87 | false |"));
}

#[tokio::test]
async fn test_console_formatter_with_tabular_data() {
    let export_plugin = ExportPlugin::new();
    
    let test_data = create_sample_tabular_data();
    let formatted = format_as_console(&export_plugin, &[test_data]).await.unwrap();
    
    // Verify console table structure
    assert!(formatted.contains("========"));
    assert!(formatted.contains("Sample Data"));
    assert!(formatted.contains("Name"));
    assert!(formatted.contains("Score"));
    assert!(formatted.contains("Active"));
    assert!(formatted.contains("Alice"));
    assert!(formatted.contains("95"));
    assert!(formatted.contains("Bob"));
    assert!(formatted.contains("87"));
    
    // Verify table formatting with separators (TableBuilder uses spaces and dashes)
    assert!(formatted.contains("-")); // Table separators
    assert!(formatted.contains("Name")); // Headers are present
    assert!(formatted.contains("Score"));
}

#[tokio::test]
async fn test_console_formatter_with_colors() {
    // Test that console formatting produces consistent output
    let export_plugin = ExportPlugin::new();
    let test_data = create_sample_tabular_data();
    
    // Format using the public method (will use TableBuilder internally)
    let formatted = export_plugin.format_as_console(&[test_data]).await.unwrap();
    
    // Verify the output contains expected data
    assert!(formatted.contains("Sample Data"));
    assert!(formatted.contains("Alice"));
    assert!(formatted.contains("Bob"));
    assert!(formatted.contains("Name"));
    assert!(formatted.contains("Score"));
    
    // Verify table structure (TableBuilder output)
    assert!(formatted.contains("-")); // Table separators
}

#[tokio::test]
async fn test_multiple_plugins_json_formatting() {
    let export_plugin = ExportPlugin::new();
    
    let commits_data = create_commits_data();
    let metrics_data = create_metrics_data();
    
    let formatted = export_plugin.format_json(&[commits_data, metrics_data]).await.unwrap();
    
    let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();
    let obj = parsed.as_object().unwrap();
    
    // Verify both plugins' data are present
    assert!(obj.contains_key("commits"));
    assert!(obj.contains_key("metrics"));
    
    assert_eq!(obj["commits"]["title"], "Commit Statistics");
    assert_eq!(obj["metrics"]["title"], "Code Metrics");
}

#[tokio::test]
async fn test_key_value_data_formatting() {
    let export_plugin = ExportPlugin::new();
    
    let kv_data = create_key_value_data();
    
    // Test JSON formatting
    let json_formatted = export_plugin.format_json(&[kv_data.clone()]).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_formatted).unwrap();
    
    let obj = parsed.as_object().unwrap();
    assert!(obj.contains_key("summary"));
    
    let summary_data = &obj["summary"]["data"];
    assert!(summary_data.is_object());
    
    // Test console formatting
    let console_formatted = format_as_console(&export_plugin, &[kv_data]).await.unwrap();
    assert!(console_formatted.contains("Summary Statistics"));
    assert!(console_formatted.contains("total_files"));
    assert!(console_formatted.contains("1250"));
    assert!(console_formatted.contains(":"));
}

#[tokio::test]
async fn test_hierarchical_data_formatting() {
    let export_plugin = ExportPlugin::new();
    
    let tree_data = create_hierarchical_data();
    
    // Test console formatting for tree data
    let console_formatted = format_as_console(&export_plugin, &[tree_data.clone()]).await.unwrap();
    assert!(console_formatted.contains("Project Structure"));
    assert!(console_formatted.contains("Tree: Root Directory"));
    
    // Test JSON formatting
    let json_formatted = export_plugin.format_json(&[tree_data]).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_formatted).unwrap();
    
    let obj = parsed.as_object().unwrap();
    assert!(obj.contains_key("structure"));
}

#[tokio::test]
async fn test_empty_data_formatting() {
    let export_plugin = ExportPlugin::new();
    
    let empty_data = create_empty_data();
    
    // Test all formatters handle empty data gracefully
    let json_result = export_plugin.format_json(&[empty_data.clone()]).await;
    assert!(json_result.is_ok());
    
    let csv_result = export_plugin.format_csv(&[empty_data.clone()]).await;
    assert!(csv_result.is_ok());
    
    let console_result = format_as_console(&export_plugin, &[empty_data]).await;
    assert!(console_result.is_ok());
    let console_output = console_result.unwrap();
    assert!(console_output.contains("(no data)"));
}

#[tokio::test]
async fn test_special_characters_in_data() {
    let export_plugin = ExportPlugin::new();
    
    let special_data = create_data_with_special_characters();
    
    // Test CSV formatting handles commas and quotes
    let csv_formatted = export_plugin.format_csv(&[special_data.clone()]).await.unwrap();
    assert!(csv_formatted.contains("\"Smith, John\""));
    assert!(csv_formatted.contains("\"He said \"\"Hello\"\"\""));
    
    // Test JSON formatting handles quotes
    let json_formatted = export_plugin.format_json(&[special_data]).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_formatted).unwrap();
    assert!(parsed.is_object());
}

#[tokio::test]
async fn test_large_dataset_formatting_performance() {
    let export_plugin = ExportPlugin::new();
    
    let large_data = create_large_dataset(500);
    
    // Test that formatting large datasets completes in reasonable time
    let start = std::time::Instant::now();
    
    let json_result = export_plugin.format_json(&[large_data.clone()]).await;
    assert!(json_result.is_ok());
    
    let csv_result = export_plugin.format_csv(&[large_data.clone()]).await;
    assert!(csv_result.is_ok());
    
    let console_result = format_as_console(&export_plugin, &[large_data]).await;
    assert!(console_result.is_ok());
    
    let elapsed = start.elapsed();
    assert!(elapsed < std::time::Duration::from_secs(2), "Formatting should complete quickly");
}

#[tokio::test]
async fn test_export_hints_influence_formatting() {
    let export_plugin = ExportPlugin::new();
    
    let data_with_hints = create_data_with_export_hints();
    
    // Test console formatting respects column alignment hints
    let console_formatted = format_as_console(&export_plugin, &[data_with_hints]).await.unwrap();
    
    // Verify data is present and formatted
    assert!(console_formatted.contains("Sortable Data"));
    assert!(console_formatted.contains("Value"));
    assert!(console_formatted.contains("100"));
    assert!(console_formatted.contains("200"));
}

// Helper functions to create test data

fn create_sample_tabular_data() -> Arc<PluginDataExport> {
    Arc::new(PluginDataExport {
        plugin_id: "sample_plugin".to_string(),
        title: "Sample Data".to_string(),
        description: Some("Test data for formatting".to_string()),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![
                ColumnDef::new("Name", ColumnType::String),
                ColumnDef::new("Score", ColumnType::Integer),
                ColumnDef::new("Active", ColumnType::Boolean),
            ],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(vec![
            Row::new(vec![
                Value::String("Alice".to_string()),
                Value::Integer(95),
                Value::Boolean(true),
            ]),
            Row::new(vec![
                Value::String("Bob".to_string()),
                Value::Integer(87),
                Value::Boolean(false),
            ]),
        ])),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    })
}

fn create_commits_data() -> Arc<PluginDataExport> {
    Arc::new(PluginDataExport {
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
                Value::Integer(45),
            ]),
            Row::new(vec![
                Value::String("Bob".to_string()),
                Value::Integer(23),
            ]),
        ])),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    })
}

fn create_metrics_data() -> Arc<PluginDataExport> {
    Arc::new(PluginDataExport {
        plugin_id: "metrics".to_string(),
        title: "Code Metrics".to_string(),
        description: Some("Code quality metrics".to_string()),
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
                Value::Float(6.5),
            ]),
            Row::new(vec![
                Value::String("Coverage".to_string()),
                Value::Float(85.2),
            ]),
        ])),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    })
}

fn create_key_value_data() -> Arc<PluginDataExport> {
    let mut data = HashMap::new();
    data.insert("total_files".to_string(), Value::Integer(1250));
    data.insert("total_lines".to_string(), Value::Integer(45000));
    data.insert("avg_complexity".to_string(), Value::Float(6.7));
    data.insert("has_tests".to_string(), Value::Boolean(true));
    
    Arc::new(PluginDataExport {
        plugin_id: "summary".to_string(),
        title: "Summary Statistics".to_string(),
        description: Some("Project summary".to_string()),
        data_type: DataExportType::KeyValue,
        schema: DataSchema {
            columns: Vec::new(),
            metadata: HashMap::new(),
        },
        data: DataPayload::KeyValue(Arc::new(data)),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    })
}

fn create_hierarchical_data() -> Arc<PluginDataExport> {
    let root = TreeNode::new("Root Directory")
        .with_value(Value::String("project_root".to_string()))
        .add_child(
            TreeNode::new("src")
                .add_child(TreeNode::new("main.rs"))
                .add_child(TreeNode::new("lib.rs"))
        )
        .add_child(
            TreeNode::new("tests")
                .add_child(TreeNode::new("integration_tests.rs"))
        );
    
    Arc::new(PluginDataExport {
        plugin_id: "structure".to_string(),
        title: "Project Structure".to_string(),
        description: Some("Directory hierarchy".to_string()),
        data_type: DataExportType::Hierarchical,
        schema: DataSchema {
            columns: Vec::new(),
            metadata: HashMap::new(),
        },
        data: DataPayload::Tree(Arc::new(root)),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    })
}

fn create_empty_data() -> Arc<PluginDataExport> {
    Arc::new(PluginDataExport {
        plugin_id: "empty".to_string(),
        title: "Empty Data".to_string(),
        description: Some("No data available".to_string()),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: Vec::new(),
            metadata: HashMap::new(),
        },
        data: DataPayload::Empty,
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    })
}

fn create_data_with_special_characters() -> Arc<PluginDataExport> {
    Arc::new(PluginDataExport {
        plugin_id: "special_chars".to_string(),
        title: "Special Characters Test".to_string(),
        description: Some("Data with special characters".to_string()),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![
                ColumnDef::new("Name", ColumnType::String),
                ColumnDef::new("Quote", ColumnType::String),
            ],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(vec![
            Row::new(vec![
                Value::String("Smith, John".to_string()),
                Value::String("He said \"Hello\"".to_string()),
            ]),
            Row::new(vec![
                Value::String("O'Brien".to_string()),
                Value::String("It's working!".to_string()),
            ]),
        ])),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    })
}

fn create_large_dataset(size: usize) -> Arc<PluginDataExport> {
    let mut rows = Vec::new();
    
    for i in 0..size {
        rows.push(Row::new(vec![
            Value::Integer(i as i64),
            Value::String(format!("Item_{:04}", i)),
            Value::Float(i as f64 * 1.5),
        ]));
    }
    
    Arc::new(PluginDataExport {
        plugin_id: "large_dataset".to_string(),
        title: format!("Large Dataset ({} items)", size),
        description: Some("Performance test dataset".to_string()),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![
                ColumnDef::new("ID", ColumnType::Integer),
                ColumnDef::new("Name", ColumnType::String),
                ColumnDef::new("Value", ColumnType::Float),
            ],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(rows)),
        export_hints: ExportHints::default(),
        timestamp: SystemTime::now(),
    })
}

fn create_data_with_export_hints() -> Arc<PluginDataExport> {
    Arc::new(PluginDataExport {
        plugin_id: "hints_test".to_string(),
        title: "Sortable Data".to_string(),
        description: Some("Data with export hints".to_string()),
        data_type: DataExportType::Tabular,
        schema: DataSchema {
            columns: vec![
                ColumnDef::new("Item", ColumnType::String),
                ColumnDef::new("Value", ColumnType::Integer),
            ],
            metadata: HashMap::new(),
        },
        data: DataPayload::Rows(Arc::new(vec![
            Row::new(vec![
                Value::String("Second".to_string()),
                Value::Integer(200),
            ]),
            Row::new(vec![
                Value::String("First".to_string()),
                Value::Integer(100),
            ]),
        ])),
        export_hints: ExportHints {
            preferred_formats: vec![ExportFormat::Console],
            sort_by: Some("Value".to_string()),
            sort_ascending: true,
            limit: None,
            include_totals: false,
            include_row_numbers: true,
            custom_hints: HashMap::new(),
        },
        timestamp: SystemTime::now(),
    })
}