//! HTML export format implementation

use super::FormatExporter;
use crate::plugin::PluginResult;
use crate::plugin::data_export::{PluginDataExport, DataPayload};
use std::sync::Arc;

/// HTML formatter
pub struct HtmlFormatter;

impl HtmlFormatter {
    /// Create a new HTML formatter
    pub fn new() -> Self {
        Self
    }
}

impl Default for HtmlFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatExporter for HtmlFormatter {
    fn format_data(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        let mut output = String::from("<!DOCTYPE html>\n<html>\n<head>\n");
        output.push_str("    <title>Git Statistics Report</title>\n");
        output.push_str("    <style>\n");
        output.push_str("        body { font-family: Arial, sans-serif; margin: 20px; }\n");
        output.push_str("        table { border-collapse: collapse; width: 100%; margin: 20px 0; }\n");
        output.push_str("        th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }\n");
        output.push_str("        th { background-color: #f2f2f2; }\n");
        output.push_str("        h2 { color: #333; border-bottom: 2px solid #333; }\n");
        output.push_str("    </style>\n");
        output.push_str("</head>\n<body>\n");
        output.push_str("    <h1>Git Statistics Report</h1>\n");
        
        for export in data {
            output.push_str(&format!("    <h2>{}</h2>\n", escape_html(&export.title)));
            
            if let Some(ref desc) = export.description {
                output.push_str(&format!("    <p>{}</p>\n", escape_html(desc)));
            }
            
            match &export.data {
                DataPayload::Rows(rows) => {
                    if !rows.is_empty() && !export.schema.columns.is_empty() {
                        output.push_str("    <table>\n        <thead>\n            <tr>\n");
                        for col in &export.schema.columns {
                            output.push_str(&format!("                <th>{}</th>\n", escape_html(&col.name)));
                        }
                        output.push_str("            </tr>\n        </thead>\n        <tbody>\n");
                        
                        for row in rows.iter() {
                            output.push_str("            <tr>\n");
                            for value in &row.values {
                                output.push_str(&format!("                <td>{}</td>\n", escape_html(&value.to_string())));
                            }
                            output.push_str("            </tr>\n");
                        }
                        output.push_str("        </tbody>\n    </table>\n");
                    }
                }
                
                DataPayload::KeyValue(kv) => {
                    output.push_str("    <table>\n");
                    for (key, value) in kv.iter() {
                        output.push_str(&format!("        <tr><td><strong>{}</strong></td><td>{}</td></tr>\n", 
                            escape_html(key), escape_html(&value.to_string())));
                    }
                    output.push_str("    </table>\n");
                }
                
                DataPayload::Tree(_) => {
                    output.push_str("    <p><em>Tree structure not yet implemented</em></p>\n");
                }
                
                DataPayload::Raw(raw) => {
                    output.push_str(&format!("    <p><strong>Raw:</strong> {}</p>\n", escape_html(raw.as_str())));
                }
                
                DataPayload::Empty => {
                    output.push_str("    <p><em>No data available</em></p>\n");
                }
            }
        }
        
        output.push_str("</body>\n</html>\n");
        Ok(output)
    }
    
}

/// Escape text for HTML output
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}