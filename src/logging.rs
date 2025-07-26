// Logging module for gstats
// Provides structured logging with timestamp formatting and multiple output formats
//
// This module implements a comprehensive logging system that supports:
// - Multiple output formats: Text and JSON
// - Multiple destinations: Console, File, or Both
// - Independent log levels for console and file output
// - Structured timestamp formatting (YYYY-MM-DD HH:mm:ss)
// - Extensible JSON structure with optional detail field for future enhancements
//
// Example usage:
// ```
// let config = LogConfig {
//     console_level: LevelFilter::Info,
//     file_level: Some(LevelFilter::Debug),
//     format: LogFormat::Json,
//     destination: LogDestination::Both(PathBuf::from("app.log")),
// };
// init_logger(config)?;
// log::info!("Application started");
// ```

use log::{Level, LevelFilter};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local};
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::PathBuf;
use anyhow::{Context, Result};

/// Log output format options
#[derive(Debug, Clone, PartialEq)]
pub enum LogFormat {
    Text,
    Json,
}

impl std::str::FromStr for LogFormat {
    type Err = String;
    
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" => Ok(LogFormat::Text),
            "json" => Ok(LogFormat::Json),
            _ => Err(format!("Invalid log format: {}. Valid options: text, json", s)),
        }
    }
}

/// Log destination options
#[derive(Debug, Clone, PartialEq)]
pub enum LogDestination {
    Console,
    File(PathBuf),
    Both(PathBuf),
}

/// JSON log entry structure
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonLogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

/// Logging configuration
#[derive(Debug, Clone)]
pub struct LogConfig {
    pub console_level: LevelFilter,
    pub file_level: Option<LevelFilter>,
    pub format: LogFormat,
    pub destination: LogDestination,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            console_level: LevelFilter::Info,
            file_level: None,
            format: LogFormat::Text,
            destination: LogDestination::Console,
        }
    }
}

/// Custom logger implementation
pub struct GstatsLogger {
    config: LogConfig,
}

impl GstatsLogger {
    pub fn new(config: LogConfig) -> Self {
        Self { config }
    }

    fn format_timestamp() -> String {
        let now: DateTime<Local> = Local::now();
        now.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    fn format_text_message(&self, level: Level, message: &str) -> String {
        let timestamp = Self::format_timestamp();
        format!("{} [{}] {}", timestamp, level.to_string().to_uppercase(), message)
    }

    fn format_json_message(&self, level: Level, message: &str) -> Result<String> {
        let entry = JsonLogEntry {
            timestamp: Self::format_timestamp(),
            level: level.to_string().to_uppercase(),
            message: message.to_string(),
            detail: None, // Initially empty as specified
        };
        
        serde_json::to_string(&entry)
            .context("Failed to serialize log entry to JSON")
    }

    fn should_log_to_console(&self, level: Level) -> bool {
        level <= self.config.console_level
    }

    fn should_log_to_file(&self, level: Level) -> bool {
        if let Some(file_level) = self.config.file_level {
            level <= file_level
        } else {
            false
        }
    }

    fn write_to_console(&self, formatted_message: &str) -> Result<()> {
        writeln!(io::stderr(), "{}", formatted_message)
            .context("Failed to write to console")
    }

    fn write_to_file(&self, formatted_message: &str, file_path: &PathBuf) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)
            .with_context(|| format!("Failed to open log file: {}", file_path.display()))?;
        
        writeln!(file, "{}", formatted_message)
            .context("Failed to write to log file")
    }
}

impl log::Log for GstatsLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.should_log_to_console(metadata.level()) || 
        self.should_log_to_file(metadata.level())
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let message = record.args().to_string();
        let level = record.level();

        // Format message based on configured format
        let formatted_message = match self.config.format {
            LogFormat::Text => self.format_text_message(level, &message),
            LogFormat::Json => {
                match self.format_json_message(level, &message) {
                    Ok(json) => json,
                    Err(e) => {
                        // Fallback to text format if JSON serialization fails
                        eprintln!("JSON formatting error: {}. Falling back to text format.", e);
                        self.format_text_message(level, &message)
                    }
                }
            }
        };

        // Write to appropriate destinations
        match &self.config.destination {
            LogDestination::Console => {
                if self.should_log_to_console(level) {
                    if let Err(e) = self.write_to_console(&formatted_message) {
                        eprintln!("Console logging error: {}", e);
                    }
                }
            }
            LogDestination::File(path) => {
                if self.should_log_to_file(level) {
                    if let Err(e) = self.write_to_file(&formatted_message, path) {
                        eprintln!("File logging error: {}. Falling back to console.", e);
                        if let Err(console_err) = self.write_to_console(&formatted_message) {
                            eprintln!("Console fallback error: {}", console_err);
                        }
                    }
                }
            }
            LogDestination::Both(path) => {
                if self.should_log_to_console(level) {
                    if let Err(e) = self.write_to_console(&formatted_message) {
                        eprintln!("Console logging error: {}", e);
                    }
                }
                if self.should_log_to_file(level) {
                    if let Err(e) = self.write_to_file(&formatted_message, path) {
                        eprintln!("File logging error: {}", e);
                    }
                }
            }
        }
    }

    fn flush(&self) {
        let _ = io::stderr().flush();
    }
}

/// Initialize the logging system with the given configuration
pub fn init_logger(config: LogConfig) -> Result<()> {
    let logger = GstatsLogger::new(config.clone());
    
    // Set the maximum log level based on configuration
    let max_level = match (&config.file_level, config.console_level) {
        (Some(file_level), console_level) => {
            if *file_level > console_level {
                *file_level
            } else {
                console_level
            }
        }
        (None, console_level) => console_level,
    };

    log::set_boxed_logger(Box::new(logger))
        .context("Failed to set global logger")?;
    
    log::set_max_level(max_level);
    
    Ok(())
}

/// Convert string to LevelFilter
pub fn parse_log_level(level_str: &str) -> Result<LevelFilter> {
    match level_str.to_lowercase().as_str() {
        "error" => Ok(LevelFilter::Error),
        "warn" => Ok(LevelFilter::Warn),
        "info" => Ok(LevelFilter::Info),
        "debug" => Ok(LevelFilter::Debug),
        "trace" => Ok(LevelFilter::Trace),
        "off" => Ok(LevelFilter::Off),
        _ => Err(anyhow::anyhow!("Invalid log level: {}. Valid levels: error, warn, info, debug, trace, off", level_str)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_format_parsing() {
        assert_eq!("text".parse::<LogFormat>().unwrap(), LogFormat::Text);
        assert_eq!("json".parse::<LogFormat>().unwrap(), LogFormat::Json);
        assert_eq!("TEXT".parse::<LogFormat>().unwrap(), LogFormat::Text);
        assert_eq!("JSON".parse::<LogFormat>().unwrap(), LogFormat::Json);
        assert!("invalid".parse::<LogFormat>().is_err());
    }

    #[test]
    fn test_log_level_parsing() {
        assert_eq!(parse_log_level("error").unwrap(), LevelFilter::Error);
        assert_eq!(parse_log_level("warn").unwrap(), LevelFilter::Warn);
        assert_eq!(parse_log_level("info").unwrap(), LevelFilter::Info);
        assert_eq!(parse_log_level("debug").unwrap(), LevelFilter::Debug);
        assert_eq!(parse_log_level("trace").unwrap(), LevelFilter::Trace);
        assert_eq!(parse_log_level("ERROR").unwrap(), LevelFilter::Error);
        assert!(parse_log_level("invalid").is_err());
    }

    #[test]
    fn test_timestamp_format() {
        let timestamp = GstatsLogger::format_timestamp();
        // Should match YYYY-MM-DD HH:MM:SS format
        assert!(timestamp.len() >= 19);
        assert!(timestamp.contains("-"));
        assert!(timestamp.contains(":"));
        assert!(timestamp.chars().nth(4) == Some('-'));
        assert!(timestamp.chars().nth(7) == Some('-'));
        assert!(timestamp.chars().nth(10) == Some(' '));
        assert!(timestamp.chars().nth(13) == Some(':'));
        assert!(timestamp.chars().nth(16) == Some(':'));
    }

    #[test]
    fn test_json_log_entry_serialization() {
        let entry = JsonLogEntry {
            timestamp: "2025-07-26 14:30:45".to_string(),
            level: "INFO".to_string(),
            message: "Test message".to_string(),
            detail: None,
        };
        
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains(r#""timestamp":"2025-07-26 14:30:45""#));
        assert!(json.contains(r#""level":"INFO""#));
        assert!(json.contains(r#""message":"Test message""#));
        // detail field should be omitted when None
        assert!(!json.contains(r#""detail""#));
    }

    #[test]
    fn test_text_message_formatting() {
        let config = LogConfig::default();
        let logger = GstatsLogger::new(config);
        
        let formatted = logger.format_text_message(Level::Info, "Test message");
        assert!(formatted.contains("[INFO]"));
        assert!(formatted.contains("Test message"));
        assert!(formatted.contains("2025-")); // Should contain current year
    }

    #[test]
    fn test_json_message_formatting() {
        let config = LogConfig::default();
        let logger = GstatsLogger::new(config);
        
        let formatted = logger.format_json_message(Level::Info, "Test message").unwrap();
        assert!(formatted.contains(r#""level":"INFO""#));
        assert!(formatted.contains(r#""message":"Test message""#));
        assert!(formatted.contains(r#""timestamp":"#));
    }
}
