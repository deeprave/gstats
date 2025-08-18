//! Plugin Data Export Protocol
//! 
//! Native Rust data transfer protocol for efficient, type-safe data sharing
//! between plugins and the export system.

#![allow(dead_code)]

use std::sync::Arc;
use std::collections::HashMap;
use std::time::{SystemTime, Duration};
use serde::{Serialize, Deserialize};

/// Main data export structure that plugins create
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDataExport {
    /// Unique identifier of the source plugin
    pub plugin_id: String,
    
    /// Human-readable title for the data
    pub title: String,
    
    /// Optional description of the data
    pub description: Option<String>,
    
    /// Type of data being exported
    pub data_type: DataExportType,
    
    /// Schema definition for the data
    pub schema: DataSchema,
    
    /// Actual data payload
    pub data: DataPayload,
    
    /// Hints for export formatting
    pub export_hints: ExportHints,
    
    /// Timestamp when data was created
    pub timestamp: SystemTime,
}

/// Types of data that can be exported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataExportType {
    /// Rows and columns (most common)
    Tabular,
    
    /// Tree/nested structure
    Hierarchical,
    
    /// Simple key-value pairs
    KeyValue,
    
    /// Unstructured data
    Raw,
}

/// Schema definition for structured data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSchema {
    /// Column definitions for tabular data
    pub columns: Vec<ColumnDef>,
    
    /// Optional metadata about the schema
    pub metadata: HashMap<String, String>,
}

/// Definition of a single column
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    /// Column name
    pub name: String,
    
    /// Data type of the column
    pub data_type: ColumnType,
    
    /// Optional description
    pub description: Option<String>,
    
    /// Format hint for rendering (e.g., "percentage", "bytes", "timestamp")
    pub format_hint: Option<String>,
    
    /// Whether this column should be hidden by default
    pub hidden: bool,
    
    /// Preferred width for console output (characters)
    pub preferred_width: Option<usize>,
}

/// Data types for columns
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnType {
    String,
    Integer,
    Float,
    Boolean,
    Timestamp,
    Duration,
}

/// Container for actual data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataPayload {
    /// Tabular data as rows
    Rows(Arc<Vec<Row>>),
    
    /// Hierarchical tree structure
    Tree(Arc<TreeNode>),
    
    /// Key-value pairs
    KeyValue(Arc<HashMap<String, Value>>),
    
    /// Raw unstructured data
    Raw(Arc<String>),
    
    /// Empty payload (no data)
    Empty,
}

/// A single row of data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    /// Values in the row, matching schema column order
    pub values: Vec<Value>,
    
    /// Optional metadata for this row
    pub metadata: Option<HashMap<String, String>>,
}

/// Individual data values
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Timestamp(SystemTime),
    Duration(Duration),
    Null,
}

/// Tree node for hierarchical data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    /// Node label
    pub label: String,
    
    /// Node value
    pub value: Option<Value>,
    
    /// Child nodes
    pub children: Vec<Arc<TreeNode>>,
    
    /// Node metadata
    pub metadata: Option<HashMap<String, String>>,
}

/// Hints for how to export/format the data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportHints {
    /// Preferred output formats in order of preference
    pub preferred_formats: Vec<ExportFormat>,
    
    /// Column to sort by (for tabular data)
    pub sort_by: Option<String>,
    
    /// Sort order
    pub sort_ascending: bool,
    
    /// Maximum number of rows to display
    pub limit: Option<usize>,
    
    /// Whether to include summary/totals
    pub include_totals: bool,
    
    /// Whether to include row numbers
    pub include_row_numbers: bool,
    
    /// Custom hints for specific formatters
    pub custom_hints: HashMap<String, String>,
}

/// Supported export formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
    /// Console table output
    Console,
    
    /// JSON format
    Json,
    
    /// CSV format
    Csv,
    
    /// XML format
    Xml,
    
    /// YAML format
    Yaml,
    
    /// HTML format
    Html,
    
    /// Markdown format
    Markdown,
    
    /// Template-based format
    Template,
}

// Builder implementation for PluginDataExport
impl PluginDataExport {
    /// Create a new builder for PluginDataExport
    pub fn builder() -> PluginDataExportBuilder {
        PluginDataExportBuilder::new()
    }
}

/// Builder for PluginDataExport
pub struct PluginDataExportBuilder {
    plugin_id: Option<String>,
    title: Option<String>,
    description: Option<String>,
    data_type: Option<DataExportType>,
    schema: Option<DataSchema>,
    data: Option<DataPayload>,
    export_hints: Option<ExportHints>,
}

impl PluginDataExportBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            plugin_id: None,
            title: None,
            description: None,
            data_type: None,
            schema: None,
            data: None,
            export_hints: None,
        }
    }
    
    /// Set the plugin ID
    pub fn plugin_id(mut self, id: impl Into<String>) -> Self {
        self.plugin_id = Some(id.into());
        self
    }
    
    /// Set the title
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
    
    /// Set the description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
    
    /// Set the data type
    pub fn data_type(mut self, dtype: DataExportType) -> Self {
        self.data_type = Some(dtype);
        self
    }
    
    /// Set the schema
    pub fn schema(mut self, schema: DataSchema) -> Self {
        self.schema = Some(schema);
        self
    }
    
    /// Set the data payload
    pub fn data(mut self, data: DataPayload) -> Self {
        self.data = Some(data);
        self
    }
    
    /// Set export hints
    pub fn export_hints(mut self, hints: ExportHints) -> Self {
        self.export_hints = Some(hints);
        self
    }
    
    /// Build the PluginDataExport
    pub fn build(self) -> Result<PluginDataExport, String> {
        Ok(PluginDataExport {
            plugin_id: self.plugin_id.ok_or("plugin_id is required")?,
            title: self.title.ok_or("title is required")?,
            description: self.description,
            data_type: self.data_type.unwrap_or(DataExportType::Tabular),
            schema: self.schema.unwrap_or_else(|| DataSchema {
                columns: Vec::new(),
                metadata: HashMap::new(),
            }),
            data: self.data.unwrap_or(DataPayload::Empty),
            export_hints: self.export_hints.unwrap_or_else(|| ExportHints {
                preferred_formats: vec![ExportFormat::Console],
                sort_by: None,
                sort_ascending: true,
                limit: None,
                include_totals: false,
                include_row_numbers: false,
                custom_hints: HashMap::new(),
            }),
            timestamp: SystemTime::now(),
        })
    }
}

// Helper implementations
impl Row {
    /// Create a new row with values
    pub fn new(values: Vec<Value>) -> Self {
        Self {
            values,
            metadata: None,
        }
    }
    
    /// Create a row with metadata
    pub fn with_metadata(values: Vec<Value>, metadata: HashMap<String, String>) -> Self {
        Self {
            values,
            metadata: Some(metadata),
        }
    }
}

impl TreeNode {
    /// Create a new tree node
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: None,
            children: Vec::new(),
            metadata: None,
        }
    }
    
    /// Add a child node
    pub fn add_child(mut self, child: TreeNode) -> Self {
        self.children.push(Arc::new(child));
        self
    }
    
    /// Set the node value
    pub fn with_value(mut self, value: Value) -> Self {
        self.value = Some(value);
        self
    }
}

impl DataSchema {
    /// Create a new schema with columns
    pub fn new(columns: Vec<ColumnDef>) -> Self {
        Self {
            columns,
            metadata: HashMap::new(),
        }
    }
    
    /// Add metadata to the schema
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

impl ColumnDef {
    /// Create a new column definition
    pub fn new(name: impl Into<String>, data_type: ColumnType) -> Self {
        Self {
            name: name.into(),
            data_type,
            description: None,
            format_hint: None,
            hidden: false,
            preferred_width: None,
        }
    }
    
    /// Set description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
    
    /// Set format hint
    pub fn with_format_hint(mut self, hint: impl Into<String>) -> Self {
        self.format_hint = Some(hint.into());
        self
    }
    
    /// Set preferred width
    pub fn with_width(mut self, width: usize) -> Self {
        self.preferred_width = Some(width);
        self
    }
}

impl Value {
    /// Convert value to string representation
    pub fn to_string(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => format!("{:.2}", f),
            Value::Boolean(b) => b.to_string(),
            Value::Timestamp(t) => {
                // Format as ISO-like string
                if let Ok(duration) = t.duration_since(SystemTime::UNIX_EPOCH) {
                    format!("{}", duration.as_secs())
                } else {
                    "invalid".to_string()
                }
            }
            Value::Duration(d) => format!("{:.1}s", d.as_secs_f64()),
            Value::Null => String::new(),
        }
    }
    
    /// Check if value is null
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

impl Default for ExportHints {
    fn default() -> Self {
        Self {
            preferred_formats: vec![ExportFormat::Console],
            sort_by: None,
            sort_ascending: true,
            limit: None,
            include_totals: false,
            include_row_numbers: false,
            custom_hints: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_plugin_data_export_builder() {
        let export = PluginDataExport::builder()
            .plugin_id("test_plugin")
            .title("Test Data")
            .description("Test description")
            .data_type(DataExportType::Tabular)
            .build()
            .unwrap();
        
        assert_eq!(export.plugin_id, "test_plugin");
        assert_eq!(export.title, "Test Data");
        assert_eq!(export.description, Some("Test description".to_string()));
        assert_eq!(export.data_type, DataExportType::Tabular);
    }
    
    #[test]
    fn test_row_creation() {
        let row = Row::new(vec![
            Value::String("test".to_string()),
            Value::Integer(42),
            Value::Boolean(true),
        ]);
        
        assert_eq!(row.values.len(), 3);
        assert!(row.metadata.is_none());
    }
    
    #[test]
    fn test_tree_node() {
        let root = TreeNode::new("root")
            .with_value(Value::String("root_value".to_string()))
            .add_child(TreeNode::new("child1"))
            .add_child(TreeNode::new("child2"));
        
        assert_eq!(root.label, "root");
        assert_eq!(root.children.len(), 2);
    }
    
    #[test]
    fn test_value_to_string() {
        assert_eq!(Value::String("test".to_string()).to_string(), "test");
        assert_eq!(Value::Integer(42).to_string(), "42");
        assert_eq!(Value::Float(3.14).to_string(), "3.14");
        assert_eq!(Value::Boolean(true).to_string(), "true");
        assert_eq!(Value::Null.to_string(), "");
    }
}