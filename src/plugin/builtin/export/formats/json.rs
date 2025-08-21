//! JSON export format implementation

use super::FormatExporter;
use crate::plugin::{PluginResult, PluginError};
use crate::plugin::data_export::{PluginDataExport, DataPayload};
use std::sync::Arc;
use serde_json::{json, Value};

/// JSON formatter
pub struct JsonFormatter;

impl JsonFormatter {
    /// Create a new JSON formatter
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatExporter for JsonFormatter {
    fn format_data(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        let mut json_data = json!({});
        let json_obj = json_data.as_object_mut().unwrap();
        
        for export in data {
            let mut plugin_data = json!({
                "title": export.title,
                "description": export.description,
                "type": export.data_type,
            });
            
            // Add data based on type
            match &export.data {
                DataPayload::Rows(rows) => {
                    let mut json_rows = Vec::new();
                    
                    for row in rows.iter() {
                        let mut json_row = serde_json::Map::new();
                        
                        for (i, value) in row.values.iter().enumerate() {
                            if let Some(column) = export.schema.columns.get(i) {
                                let json_value = match value {
                                    crate::plugin::data_export::Value::String(s) => json!(s),
                                    crate::plugin::data_export::Value::Integer(i) => json!(i),
                                    crate::plugin::data_export::Value::Float(f) => json!(f),
                                    crate::plugin::data_export::Value::Boolean(b) => json!(b),
                                    crate::plugin::data_export::Value::Timestamp(ts) => json!(format!("{:?}", ts)),
                                    crate::plugin::data_export::Value::Duration(d) => json!(format!("{:?}", d)),
                                    crate::plugin::data_export::Value::Null => json!(null),
                                };
                                json_row.insert(column.name.clone(), json_value);
                            }
                        }
                        
                        json_rows.push(Value::Object(json_row));
                    }
                    
                    plugin_data["rows"] = json!(json_rows);
                    plugin_data["schema"] = json!({
                        "columns": export.schema.columns.iter().map(|col| {
                            json!({
                                "name": col.name,
                                "type": format!("{:?}", col.data_type),
                                "description": col.description
                            })
                        }).collect::<Vec<_>>()
                    });
                }
                
                DataPayload::KeyValue(kv) => {
                    plugin_data["data"] = json!(kv);
                }
                
                DataPayload::Tree(root) => {
                    plugin_data["tree"] = json!({
                        "label": root.label,
                        "children": root.children.len()
                        // TODO: Recursively serialize tree structure
                    });
                }
                
                DataPayload::Raw(raw) => {
                    plugin_data["raw"] = json!(raw.as_str());
                }
                
                DataPayload::Empty => {
                    plugin_data["empty"] = json!(true);
                }
            }
            
            json_obj.insert(export.plugin_id.clone(), plugin_data);
        }
        
        serde_json::to_string_pretty(&json_data)
            .map_err(|e| PluginError::execution_failed(format!("JSON serialization failed: {}", e)))
    }
    
}