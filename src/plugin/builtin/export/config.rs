//! Export configuration types and defaults

use std::path::PathBuf;
use super::formats::csv::QuotingStyle;

#[derive(Debug, Clone)]
pub struct ExportConfig {
    pub output_format: ExportFormat,
    pub output_file: Option<PathBuf>,
    pub csv_delimiter: String,
    pub csv_quote_char: String,
    pub csv_quoting_style: QuotingStyle,
    pub template_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportFormat {
    Console,
    Json,
    Csv,
    Xml,
    Yaml,
    Html,
    Markdown,
    Template,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            output_format: ExportFormat::Console,
            output_file: None,
            csv_delimiter: ",".to_string(),
            csv_quote_char: "\"".to_string(),
            csv_quoting_style: QuotingStyle::Minimal,
            template_file: None,
        }
    }
}
