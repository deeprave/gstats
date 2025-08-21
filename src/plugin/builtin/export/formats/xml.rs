//! XML export format implementation

use super::FormatExporter;
use crate::plugin::PluginResult;
use crate::plugin::data_export::{PluginDataExport, DataPayload};
use std::sync::Arc;

/// XML formatter
pub struct XmlFormatter;

impl XmlFormatter {
    /// Create a new XML formatter
    pub fn new() -> Self {
        Self
    }
}

impl Default for XmlFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatExporter for XmlFormatter {
    fn format_data(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        let mut output = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gstats_export>\n");
        
        for export in data {
            output.push_str(&format!("  <plugin id=\"{}\">\n", export.plugin_id));
            output.push_str(&format!("    <title>{}</title>\n", escape_xml(&export.title)));
            
            if let Some(ref desc) = export.description {
                output.push_str(&format!("    <description>{}</description>\n", escape_xml(desc)));
            }
            
            match &export.data {
                DataPayload::Rows(rows) => {
                    output.push_str("    <data type=\"table\">\n");
                    for row in rows.iter() {
                        output.push_str("      <row>\n");
                        for (i, value) in row.values.iter().enumerate() {
                            if let Some(column) = export.schema.columns.get(i) {
                                output.push_str(&format!("        <{}>{}</{}>\n", 
                                    column.name, escape_xml(&value.to_string()), column.name));
                            }
                        }
                        output.push_str("      </row>\n");
                    }
                    output.push_str("    </data>\n");
                }
                
                DataPayload::KeyValue(kv) => {
                    output.push_str("    <data type=\"keyvalue\">\n");
                    for (key, value) in kv.iter() {
                        output.push_str(&format!("      <item key=\"{}\">{}</item>\n", 
                            escape_xml(key), escape_xml(&value.to_string())));
                    }
                    output.push_str("    </data>\n");
                }
                
                DataPayload::Tree(_) => {
                    output.push_str("    <data type=\"tree\">\n");
                    output.push_str("      <!-- Tree structure not yet implemented -->\n");
                    output.push_str("    </data>\n");
                }
                
                DataPayload::Raw(raw) => {
                    output.push_str(&format!("    <raw>{}</raw>\n", escape_xml(raw.as_str())));
                }
                
                DataPayload::Empty => {
                    output.push_str("    <empty>true</empty>\n");
                }
            }
            
            output.push_str("  </plugin>\n");
        }
        
        output.push_str("</gstats_export>\n");
        Ok(output)
    }
    
}

/// Escape text for XML output
fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}