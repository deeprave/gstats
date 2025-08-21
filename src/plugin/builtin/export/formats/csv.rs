//! CSV/TSV export format implementation

use super::FormatExporter;
use crate::plugin::PluginResult;
use crate::plugin::data_export::{PluginDataExport, DataPayload};
use std::sync::Arc;

/// CSV quoting style
#[derive(Debug, Clone, Copy)]
pub enum QuotingStyle {
    /// RFC 4180: Quote only when necessary (contains delimiter, quote, or newline)
    Minimal,
    /// Always quote string fields
    AlwaysQuote,
    /// Never quote, escape with backslash (common in Unix-style CSV)
    NoQuotes,
}

/// CSV formatter
pub struct CsvFormatter {
    delimiter: char,
    quote_char: char,
    quoting_style: QuotingStyle,
}

impl CsvFormatter {
    /// Create a new CSV formatter (comma-separated, RFC 4180)
    pub fn new() -> Self {
        Self { 
            delimiter: ',', 
            quote_char: '"',
            quoting_style: QuotingStyle::Minimal,
        }
    }
    
    /// Create a CSV formatter with no quotes (backslash escaping)
    pub fn with_no_quotes(delimiter: char) -> Self {
        Self { 
            delimiter, 
            quote_char: '"',  // Still needed for the escaping logic
            quoting_style: QuotingStyle::NoQuotes,
        }
    }
    
    /// Create a formatter with custom delimiter and quote char
    pub fn with_delimiter_and_quote(delimiter: char, quote_char: char) -> Self {
        let quoting_style = if delimiter == '\t' {
            QuotingStyle::Minimal
        } else {
            QuotingStyle::AlwaysQuote
        };
        
        Self { delimiter, quote_char, quoting_style }
    }
    
    /// Create a formatter with full configuration
    pub fn with_config(delimiter: char, quote_char: char, quoting_style: QuotingStyle) -> Self {
        Self { delimiter, quote_char, quoting_style }
    }
    
    /// Escape a field based on the configured quoting style
    fn escape_field(&self, field: &str) -> String {
        match self.quoting_style {
            QuotingStyle::Minimal => {
                if self.needs_quoting(field) {
                    self.quote_field(field)
                } else {
                    // Special handling for TSV tabs
                    if self.delimiter == '\t' && field.contains('\t') {
                        field.replace('\t', "\\t")
                    } else {
                        field.to_string()
                    }
                }
            }
            QuotingStyle::AlwaysQuote => {
                if field.chars().any(|c| c.is_alphabetic()) {
                    self.quote_field(field)
                } else {
                    field.to_string()  // Don't quote pure numbers
                }
            }
            QuotingStyle::NoQuotes => {
                field.replace('\\', "\\\\")
                     .replace(&self.delimiter.to_string(), &format!("\\{}", self.delimiter))
                     .replace(&self.quote_char.to_string(), &format!("\\{}", self.quote_char))
                     .replace('\n', "\\n")
                     .replace('\r', "\\r")
            }
        }
    }
    
    /// Check if a field needs quoting in minimal mode
    fn needs_quoting(&self, field: &str) -> bool {
        field.contains(self.delimiter) || 
        field.contains(self.quote_char) || 
        field.contains('\n') || 
        field.contains('\r')
    }
    
    /// Quote a field with proper escaping
    fn quote_field(&self, field: &str) -> String {
        let escaped = field.replace(&self.quote_char.to_string(), 
                                  &format!("{}{}", self.quote_char, self.quote_char));
        format!("{}{}{}", self.quote_char, escaped, self.quote_char)
    }
}

impl Default for CsvFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatExporter for CsvFormatter {
    fn format_data(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String> {
        let mut output = String::new();
        
        for export in data {
            // Add section header as comment
            output.push_str(&format!("# {}\n", export.title));
            if let Some(ref desc) = export.description {
                output.push_str(&format!("# {}\n", desc));
            }
            
            match &export.data {
                DataPayload::Rows(rows) => {
                    if !rows.is_empty() && !export.schema.columns.is_empty() {
                        // Headers
                        let headers: Vec<String> = export.schema.columns.iter()
                            .map(|col| self.escape_field(&col.name))
                            .collect();
                        output.push_str(&headers.join(&self.delimiter.to_string()));
                        output.push('\n');
                        
                        // Data rows
                        for row in rows.iter() {
                            let fields: Vec<String> = row.values.iter()
                                .map(|value| self.escape_field(&value.to_string()))
                                .collect();
                            output.push_str(&fields.join(&self.delimiter.to_string()));
                            output.push('\n');
                        }
                    }
                }
                
                DataPayload::KeyValue(kv) => {
                    // Convert key-value to two-column format
                    output.push_str(&format!("Key{}Value\n", self.delimiter));
                    for (key, value) in kv.iter() {
                        output.push_str(&format!("{}{}{}\n", 
                            self.escape_field(key),
                            self.delimiter,
                            self.escape_field(&value.to_string())
                        ));
                    }
                }
                
                DataPayload::Tree(_) => {
                    // Trees don't translate well to CSV - provide a note
                    output.push_str("# Tree data not supported in CSV format\n");
                }
                
                DataPayload::Raw(raw) => {
                    output.push_str(&format!("Raw\n{}\n", self.escape_field(raw.as_str())));
                }
                
                DataPayload::Empty => {
                    output.push_str("Empty\ntrue\n");
                }
            }
            
            output.push('\n');
        }
        
        Ok(output)
    }
    
}

