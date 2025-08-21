//! Generic table formatting utilities for consistent display across the application
//!
//! This module provides reusable table formatting functionality that can be used
//! by different parts of the application to maintain consistent output styling.

use crate::display::ColourManager;

/// A generic table builder for consistent formatting
pub struct TableBuilder {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    max_plugin_width: Option<usize>,
}

impl TableBuilder {
    /// Create a new table builder
    pub fn new() -> Self {
        Self {
            headers: Vec::new(),
            rows: Vec::new(),
            max_plugin_width: None,
        }
    }
    
    /// Set the table headers
    pub fn headers(mut self, headers: Vec<String>) -> Self {
        self.headers = headers;
        self
    }
    
    /// Add a row to the table
    pub fn add_row(mut self, row: Vec<String>) -> Self {
        self.rows.push(row);
        self
    }
    
    /// Set explicit column width for the first column (used for plugin tables)
    pub fn with_plugin_width(mut self, width: usize) -> Self {
        self.max_plugin_width = Some(width);
        self
    }
    
    /// Build and format the table with colors
    pub fn build_with_colors(&self, colour_manager: &ColourManager) -> String {
        if self.headers.is_empty() && self.rows.is_empty() {
            return String::new();
        }
        
        let mut output = String::new();
        
        // Calculate column width for proper alignment
        let first_col_width = if let Some(width) = self.max_plugin_width {
            width
        } else if !self.headers.is_empty() {
            self.headers[0].len().max(6)
        } else {
            6
        };
        
        // Print headers if available
        if !self.headers.is_empty() {
            let header_line = if self.headers.len() >= 2 {
                format!(" {:<width$} {}", 
                    colour_manager.highlight(&self.headers[0]), 
                    colour_manager.highlight(&self.headers[1]), 
                    width = first_col_width)
            } else {
                format!(" {}", colour_manager.highlight(&self.headers[0]))
            };
            output.push_str(&header_line);
            output.push('\n');
            
            // Print separator line
            let separator = if self.headers.len() >= 2 {
                format!(" {} {}", 
                    colour_manager.info(&"-".repeat(first_col_width)), 
                    colour_manager.info("--"))
            } else {
                format!(" {}", colour_manager.info(&"-".repeat(first_col_width)))
            };
            output.push_str(&separator);
            output.push('\n');
        }
        
        // Print data rows
        for row in &self.rows {
            if row.len() >= 2 {
                let row_line = format!(" {:<width$} {}", 
                    colour_manager.command(&row[0]), 
                    colour_manager.success(&row[1]), 
                    width = first_col_width);
                output.push_str(&row_line);
            } else if !row.is_empty() {
                output.push_str(&format!(" {}", colour_manager.command(&row[0])));
            }
            output.push('\n');
        }
        
        output
    }
    
    /// Build and format the table without colors
    pub fn build(&self) -> String {
        let no_color_manager = ColourManager::new();
        self.build_with_colors(&no_color_manager)
    }
}

impl Default for TableBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to create a plugin listing table
pub fn create_plugin_table(plugin_data: Vec<(String, String)>, colour_manager: &ColourManager) -> String {
    if plugin_data.is_empty() {
        return "No plugins available.".to_string();
    }
    
    // Calculate max plugin width
    let max_plugin_width = plugin_data.iter()
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or(6)
        .max(6);
    
    let mut table = TableBuilder::new()
        .headers(vec!["Plugin".to_string(), "Available Functions".to_string()])
        .with_plugin_width(max_plugin_width);
    
    for (plugin_name, functions) in plugin_data {
        table = table.add_row(vec![plugin_name, functions]);
    }
    
    table.build_with_colors(colour_manager)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::ColourConfig;
    
    #[test]
    fn test_table_builder_basic() {
        let table = TableBuilder::new()
            .headers(vec!["Name".to_string(), "Value".to_string()])
            .add_row(vec!["test".to_string(), "123".to_string()])
            .build();
        
        assert!(table.contains("Name"));
        assert!(table.contains("Value"));
        assert!(table.contains("test"));
        assert!(table.contains("123"));
    }
    
    #[test]
    fn test_plugin_table_creation() {
        let plugin_data = vec![
            ("commits".to_string(), "commits*, authors".to_string()),
            ("export".to_string(), "export*".to_string()),
        ];
        
        let mut config = ColourConfig::default();
        config.set_enabled(false); // Test without colors
        let colour_manager = ColourManager::with_config(config);
        
        let table = create_plugin_table(plugin_data, &colour_manager);
        
        assert!(table.contains("Plugin"));
        assert!(table.contains("Available Functions"));
        assert!(table.contains("commits"));
        assert!(table.contains("export"));
    }
    
    #[test]
    fn test_empty_table() {
        let table = TableBuilder::new().build();
        assert!(table.is_empty());
    }
}