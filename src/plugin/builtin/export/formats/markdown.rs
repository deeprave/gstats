//! Markdown export format implementation

use super::FormatExporter;
use crate::plugin::PluginResult;
use crate::plugin::data_export::{PluginDataExport, DataPayload};
use std::sync::Arc;

/// Markdown formatter
pub struct MarkdownFormatter;

impl MarkdownFormatter {
    /// Create a new Markdown formatter
    pub fn new() -> Self {
        Self
    }
}

impl Default for MarkdownFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatExporter for MarkdownFormatter {
    fn format_data(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        let mut output = String::from("# Git Statistics Report\n\n");
        
        for export in data {
            output.push_str(&format!("## {}\n\n", export.title));
            
            if let Some(ref desc) = export.description {
                output.push_str(&format!("{}\n\n", desc));
            }
            
            match &export.data {
                DataPayload::Rows(rows) => {
                    if !rows.is_empty() && !export.schema.columns.is_empty() {
                        // Table headers
                        output.push('|');
                        for col in &export.schema.columns {
                            output.push_str(&format!(" {} |", escape_markdown(&col.name)));
                        }
                        output.push('\n');
                        
                        // Table separator
                        output.push('|');
                        for _ in &export.schema.columns {
                            output.push_str(" --- |");
                        }
                        output.push('\n');
                        
                        // Table rows
                        for row in rows.iter() {
                            output.push('|');
                            for value in &row.values {
                                output.push_str(&format!(" {} |", escape_markdown(&value.to_string())));
                            }
                            output.push('\n');
                        }
                        output.push('\n');
                    }
                }
                
                DataPayload::KeyValue(kv) => {
                    for (key, value) in kv.iter() {
                        output.push_str(&format!("- **{}**: {}\n", 
                            escape_markdown(key), escape_markdown(&value.to_string())));
                    }
                    output.push('\n');
                }
                
                DataPayload::Tree(_) => {
                    output.push_str("*Tree structure not yet implemented*\n\n");
                }
                
                DataPayload::Raw(raw) => {
                    output.push_str(&format!("**Raw:** {}\n\n", raw.as_str()));
                }
                
                DataPayload::Empty => {
                    output.push_str("*No data available*\n\n");
                }
            }
        }
        
        Ok(output)
    }
    
}

/// Escape text for Markdown output
fn escape_markdown(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .replace('#', "\\#")
        .replace('+', "\\+")
        .replace('-', "\\-")
        .replace('.', "\\.")
        .replace('!', "\\!")
        .replace('|', "\\|")
}