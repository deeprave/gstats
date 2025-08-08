//! Export configuration types and defaults

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ExportConfig {
    pub output_format: ExportFormat,
    pub output_path: String,
    pub include_metadata: bool,
    pub max_entries: Option<usize>,
    pub output_all: bool,
    pub csv_delimiter: String,
    pub csv_quote_char: String,
    pub template_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportFormat {
    Json,
    Csv,
    Xml,
    Yaml,
    Html,
    Markdown,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            output_format: ExportFormat::Json,
            output_path: String::new(),
            include_metadata: false,
            max_entries: None,
            output_all: false,
            csv_delimiter: ",".to_string(),
            csv_quote_char: "\"".to_string(),
            template_file: None,
        }
    }
}
