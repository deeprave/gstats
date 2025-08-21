//! Template-based export format using Tera template engine
//! 
//! This formatter uses the Tera template engine to render data using custom templates.
//! Templates are specified via the --template flag and can access all plugin data and metadata.

use super::FormatExporter;
use crate::plugin::PluginResult;
use crate::plugin::data_export::PluginDataExport;
use crate::plugin::builtin::export::template_engine::TemplateEngine;
use std::sync::Arc;
use std::path::Path;

pub struct TemplateExporter {
    template_file: std::path::PathBuf,
}

impl TemplateExporter {
    pub fn new(template_file: &Path) -> Self {
        Self {
            template_file: template_file.to_path_buf(),
        }
    }
}

impl FormatExporter for TemplateExporter {
    fn format_data(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        // Create and configure template engine
        let mut engine = TemplateEngine::new();
        engine.load_template(&self.template_file)?;
        
        // Add global template variables
        engine.add_template_var("plugin_count".to_string(), data.len().to_string());
        engine.add_template_var("timestamp".to_string(), 
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string()
        );
        
        let mut output = String::new();
        
        for export in data {
            // Convert PluginDataExport to JSON for template rendering
            let json_data = match &export.data {
                crate::plugin::data_export::DataPayload::Rows(rows) => {
                    let mut json_obj = serde_json::Map::new();
                    
                    // Add metadata
                    json_obj.insert("title".to_string(), serde_json::Value::String(export.title.clone()));
                    if let Some(ref desc) = export.description {
                        json_obj.insert("description".to_string(), serde_json::Value::String(desc.clone()));
                    }
                    json_obj.insert("plugin_id".to_string(), serde_json::Value::String(export.plugin_id.clone()));
                    
                    // Add schema information
                    let schema_json = serde_json::json!({
                        "columns": export.schema.columns.iter().map(|col| {
                            serde_json::json!({
                                "name": col.name,
                                "type": format!("{:?}", col.data_type), // Fixed: use data_type instead of column_type
                                "format_hint": col.format_hint
                            })
                        }).collect::<Vec<_>>()
                    });
                    json_obj.insert("schema".to_string(), schema_json);
                    
                    // Add rows data
                    let rows_json: Vec<serde_json::Value> = rows.iter().map(|row| {
                        let values: Vec<serde_json::Value> = row.values.iter().map(|value| {
                            match value {
                                crate::plugin::data_export::Value::String(s) => serde_json::Value::String(s.clone()),
                                crate::plugin::data_export::Value::Integer(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
                                crate::plugin::data_export::Value::Float(f) => {
                                    serde_json::Number::from_f64(*f)
                                        .map(serde_json::Value::Number)
                                        .unwrap_or(serde_json::Value::Null)
                                },
                                crate::plugin::data_export::Value::Boolean(b) => serde_json::Value::Bool(*b),
                                crate::plugin::data_export::Value::Null => serde_json::Value::Null,
                                crate::plugin::data_export::Value::Timestamp(ts) => {
                                    serde_json::Value::String(format!("{:?}", ts))
                                },
                                crate::plugin::data_export::Value::Duration(dur) => serde_json::Value::String(format!("{:?}", dur)),
                            }
                        }).collect();
                        serde_json::Value::Array(values)
                    }).collect();
                    json_obj.insert("rows".to_string(), serde_json::Value::Array(rows_json));
                    
                    serde_json::Value::Object(json_obj)
                },
                crate::plugin::data_export::DataPayload::Raw(_) => {
                    serde_json::json!({
                        "title": export.title,
                        "description": export.description,
                        "plugin_id": export.plugin_id,
                        "data_type": "raw"
                    })
                },
                crate::plugin::data_export::DataPayload::Tree(_) => {
                    serde_json::json!({
                        "title": export.title,
                        "description": export.description,
                        "plugin_id": export.plugin_id,
                        "data_type": "tree"
                    })
                },
                crate::plugin::data_export::DataPayload::KeyValue(_) => {
                    serde_json::json!({
                        "title": export.title,
                        "description": export.description,
                        "plugin_id": export.plugin_id,
                        "data_type": "key_value"
                    })
                },
                crate::plugin::data_export::DataPayload::Empty => {
                    serde_json::json!({
                        "title": export.title,
                        "description": export.description,
                        "plugin_id": export.plugin_id,
                        "data_type": "empty"
                    })
                },
            };
            
            // Render template
            let rendered = engine.render(&json_data)?;
            output.push_str(&rendered);
            output.push('\n');
        }
        
        Ok(output.trim_end().to_string())
    }
    
}