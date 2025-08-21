//! Console table format for terminal output

use super::FormatExporter;
use crate::plugin::PluginResult;
use crate::plugin::data_export::{PluginDataExport, DataPayload};
use crate::display::{ColourManager, TableBuilder};
use std::sync::Arc;

/// Console table formatter
pub struct ConsoleFormatter {
    /// Optional color manager for styled output
    pub colour_manager: Option<Arc<ColourManager>>,
}

impl ConsoleFormatter {
    /// Create a new console formatter
    pub fn new() -> Self {
        Self {
            colour_manager: None,
        }
    }
    
    /// Create a console formatter with color support
    pub fn with_colors(colour_manager: Arc<ColourManager>) -> Self {
        Self {
            colour_manager: Some(colour_manager),
        }
    }
    
    /// Format with color support using stored ColourManager
    pub fn format_with_colors(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        if let Some(ref colour_manager) = self.colour_manager {
            self.format_with_colour_manager(data, colour_manager)
        } else {
            // Fall back to plain formatting if no color manager available
            self.format_data(data)
        }
    }
    
    /// Internal method to format with a specific colour manager
    fn format_with_colour_manager(&self, data: &[Arc<PluginDataExport>], colour_manager: &ColourManager) -> PluginResult<String> {
        let mut output = String::new();
        
        for export in data {
            // Add section header with colors
            let header_line = "=".repeat(export.title.len() + 4);
            output.push_str(&format!("\n{}\n", colour_manager.info(&header_line)));
            output.push_str(&format!("  {}  \n", colour_manager.highlight(&export.title)));
            output.push_str(&format!("{}\n", colour_manager.info(&header_line)));
            
            if let Some(ref desc) = export.description {
                output.push_str(&format!("{}\n\n", desc));
            }
            
            // Format data based on type using TableBuilder
            match &export.data {
                DataPayload::Rows(rows) => {
                    if !rows.is_empty() && !export.schema.columns.is_empty() {
                        // Create headers from schema
                        let headers: Vec<String> = export.schema.columns.iter()
                            .map(|col| col.name.clone())
                            .collect();
                        
                        // Build table using TableBuilder with colors
                        let mut table = TableBuilder::new().headers(headers);
                        
                        for row in rows.iter() {
                            let row_values: Vec<String> = row.values.iter()
                                .map(|v| v.to_string())
                                .collect();
                            table = table.add_row(row_values);
                        }
                        
                        let table_output = table.build_with_colors(colour_manager);
                        
                        // Add 2-space indent to each line to match existing format
                        for line in table_output.lines() {
                            output.push_str("  ");
                            output.push_str(line);
                            output.push('\n');
                        }
                    }
                }
                
                DataPayload::KeyValue(kv) => {
                    if !kv.is_empty() {
                        // Use TableBuilder for key-value pairs with colors
                        let mut table = TableBuilder::new()
                            .headers(vec!["Key".to_string(), "Value".to_string()]);
                        
                        for (key, value) in kv.iter() {
                            table = table.add_row(vec![key.clone(), value.to_string()]);
                        }
                        
                        let table_output = table.build_with_colors(colour_manager);
                        
                        // Add 2-space indent to each line
                        for line in table_output.lines() {
                            output.push_str("  ");
                            output.push_str(line);
                            output.push('\n');
                        }
                    }
                }
                
                DataPayload::Tree(root) => {
                    output.push_str(&format!("Tree: {}\n", colour_manager.success(&root.label)));
                    // TODO: Implement proper tree formatting with colors
                }
                
                DataPayload::Raw(raw) => {
                    output.push_str(&format!("Raw: {}\n", raw));
                }
                
                DataPayload::Empty => {
                    output.push_str(&colour_manager.warning("(no data)"));
                    output.push('\n');
                }
            }
            
            output.push('\n');
        }
        
        Ok(output)
    }
}

impl Default for ConsoleFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatExporter for ConsoleFormatter {
    fn format_data(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        // Use a no-color ColourManager to get consistent formatting with the color version
        let no_color_manager = ColourManager::new();
        self.format_with_colour_manager(data, &no_color_manager)
    }
}