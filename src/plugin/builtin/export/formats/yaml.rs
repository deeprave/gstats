//! YAML export format implementation

use super::FormatExporter;
use crate::plugin::PluginResult;
use crate::plugin::data_export::{PluginDataExport, DataPayload};
use std::sync::Arc;

/// YAML formatter
pub struct YamlFormatter;

impl YamlFormatter {
    /// Create a new YAML formatter
    pub fn new() -> Self {
        Self
    }
}

impl Default for YamlFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatExporter for YamlFormatter {
    fn format_data(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        let mut output = String::new();
        
        for export in data {
            output.push_str(&format!("{}:\n", export.plugin_id));
            output.push_str(&format!("  title: {}\n", export.title));
            
            if let Some(ref desc) = export.description {
                output.push_str(&format!("  description: {}\n", desc));
            }
            
            match &export.data {
                DataPayload::Rows(rows) => {
                    output.push_str("  data:\n");
                    for row in rows.iter() {
                        output.push_str("    - ");
                        for (i, value) in row.values.iter().enumerate() {
                            if let Some(column) = export.schema.columns.get(i) {
                                if i > 0 { output.push_str(", "); }
                                output.push_str(&format!("{}: {}", column.name, value.to_string()));
                            }
                        }
                        output.push('\n');
                    }
                }
                
                DataPayload::KeyValue(kv) => {
                    output.push_str("  data:\n");
                    for (key, value) in kv.iter() {
                        output.push_str(&format!("    {}: {}\n", key, value.to_string()));
                    }
                }
                
                DataPayload::Tree(_) => {
                    output.push_str("  data: \"Tree structure not yet implemented\"\n");
                }
                
                DataPayload::Raw(raw) => {
                    output.push_str(&format!("  raw: \"{}\"\n", raw.as_str().replace("\"", "\\\"")));
                }
                
                DataPayload::Empty => {
                    output.push_str("  empty: true\n");
                }
            }
            
            output.push('\n');
        }
        
        Ok(output)
    }
    
}