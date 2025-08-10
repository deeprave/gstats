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
use crate::display::{ColourManager, ColourConfig};

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
    pub colour_config: Option<ColourConfig>,
    pub enable_colours: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            console_level: LevelFilter::Info,
            file_level: None,
            format: LogFormat::Text,
            destination: LogDestination::Console,
            colour_config: None,
            enable_colours: true,
        }
    }
}

impl LogConfig {
    /// Create a new LogConfig with color support
    pub fn with_colors(mut self, enable: bool) -> Self {
        self.enable_colours = enable;
        self
    }
    
    /// Create a new LogConfig with specific color configuration
    pub fn with_color_config(mut self, color_config: ColourConfig) -> Self {
        self.colour_config = Some(color_config);
        self.enable_colours = true;
        self
    }
}

/// Custom logger implementation
pub struct GstatsLogger {
    config: LogConfig,
    colour_manager: Option<ColourManager>,
}

impl GstatsLogger {
    pub fn new(config: LogConfig) -> Self {
        let colour_manager = if config.enable_colours {
            Some(if let Some(colour_config) = config.colour_config.clone() {
                ColourManager::with_config(colour_config)
            } else {
                ColourManager::new()
            })
        } else {
            None
        };

        Self {
            config,
            colour_manager,
        }
    }

    fn format_timestamp() -> String {
        let now: DateTime<Local> = Local::now();
        now.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    fn format_text_message(&self, level: Level, message: &str) -> String {
        let timestamp = Self::format_timestamp();
        let level_abbr = Self::level_to_abbreviation(level);
        
        if let Some(colour_manager) = &self.colour_manager {
            // Apply italic/oblique styling to the level and strong color
            let coloured_level = match level {
                Level::Error => format!("\x1b[3m{}\x1b[23m", colour_manager.error(&level_abbr)),
                Level::Warn => format!("\x1b[3m{}\x1b[23m", colour_manager.warning(&level_abbr)),
                Level::Info => format!("\x1b[3m{}\x1b[23m", colour_manager.info(&level_abbr)),
                Level::Debug => format!("\x1b[3m{}\x1b[23m", colour_manager.debug(&level_abbr)),
                Level::Trace => format!("\x1b[3m{}\x1b[23m", colour_manager.debug(&level_abbr)), // Use debug color for trace
            };
            
            // Apply lighter shade to the message
            let coloured_message = match level {
                Level::Error => Self::apply_lighter_shade(&colour_manager.error(message)),
                Level::Warn => Self::apply_lighter_shade(&colour_manager.warning(message)),
                Level::Info => Self::apply_lighter_shade(&colour_manager.info(message)),
                Level::Debug => Self::apply_lighter_shade(&colour_manager.debug(message)),
                Level::Trace => Self::apply_lighter_shade(&colour_manager.debug(message)),
            };
            
            format!("{} {} {}", timestamp, coloured_level, coloured_message)
        } else {
            format!("{} {} {}", timestamp, level_abbr, message)
        }
    }
    
    /// Convert log level to 3-character abbreviation
    fn level_to_abbreviation(level: Level) -> String {
        match level {
            Level::Error => "ERR".to_string(),
            Level::Warn => "WRN".to_string(),
            Level::Info => "INF".to_string(),
            Level::Debug => "DBG".to_string(),
            Level::Trace => "TRC".to_string(),
        }
    }
    
    /// Apply a lighter shade effect to colored text by reducing intensity
    fn apply_lighter_shade(colored_text: &colored::ColoredString) -> String {
        let text_str = colored_text.to_string();
        
        // If the text contains ANSI codes, try to make it lighter
        if text_str.contains("\x1b[") {
            // Replace standard colors with their bright equivalents for lighter effect
            text_str
                .replace("\x1b[31m", "\x1b[91m")  // red -> bright red
                .replace("\x1b[33m", "\x1b[93m")  // yellow -> bright yellow  
                .replace("\x1b[34m", "\x1b[94m")  // blue -> bright blue
                .replace("\x1b[32m", "\x1b[92m")  // green -> bright green
                .replace("\x1b[36m", "\x1b[96m")  // cyan -> bright cyan
                .replace("\x1b[35m", "\x1b[95m")  // magenta -> bright magenta
                .replace("\x1b[37m", "\x1b[97m")  // white -> bright white
                .replace("\x1b[90m", "\x1b[37m")  // bright black -> white
        } else {
            text_str
        }
    }

    fn format_json_message(&self, level: Level, message: &str) -> Result<String> {
        let level_str = Self::level_to_abbreviation(level);
        
        // Add color information to JSON when colors are enabled
        let detail = if let Some(colour_manager) = &self.colour_manager {
            let color_name = match level {
                Level::Error => "error",
                Level::Warn => "warning", 
                Level::Info => "info",
                Level::Debug => "debug",
                Level::Trace => "debug",
            };
            Some(serde_json::json!({
                "color": color_name,
                "colored": colour_manager.colours_enabled()
            }))
        } else {
            None
        };
        
        let entry = JsonLogEntry {
            timestamp: Self::format_timestamp(),
            level: level_str,
            message: message.to_string(),
            detail,
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
        assert!(formatted.contains("INF"));
        assert!(formatted.contains("Test message"));
        assert!(formatted.contains("2025-")); // Should contain current year
        // Should not contain square brackets
        assert!(!formatted.contains("[INF]"));
    }

    #[test]
    fn test_text_message_formatting_no_colors() {
        let config = LogConfig::default().with_colors(false);
        let logger = GstatsLogger::new(config);
        
        let formatted = logger.format_text_message(Level::Info, "Test message");
        assert!(formatted.contains("INF"));
        assert!(formatted.contains("Test message"));
        assert!(formatted.contains("2025-")); // Should contain current year
        // Should not contain ANSI color codes
        assert!(!formatted.contains("\x1b["));
        // Should not contain square brackets
        assert!(!formatted.contains("[INF]"));
    }

    #[test]
    fn test_json_message_formatting() {
        let config = LogConfig::default();
        let logger = GstatsLogger::new(config);
        
        let formatted = logger.format_json_message(Level::Info, "Test message").unwrap();
        assert!(formatted.contains(r#""level":"INF""#));
        assert!(formatted.contains(r#""message":"Test message""#));
        assert!(formatted.contains(r#""timestamp":"#));
        // Should contain color information when colors are enabled
        assert!(formatted.contains(r#""detail""#));
        assert!(formatted.contains(r#""color":"info""#));
    }

    #[test]
    fn test_json_message_formatting_no_colors() {
        let config = LogConfig::default().with_colors(false);
        let logger = GstatsLogger::new(config);
        
        let formatted = logger.format_json_message(Level::Info, "Test message").unwrap();
        assert!(formatted.contains(r#""level":"INF""#));
        assert!(formatted.contains(r#""message":"Test message""#));
        assert!(formatted.contains(r#""timestamp":"#));
        // Should not contain color information when colors are disabled
        assert!(!formatted.contains(r#""detail""#));
        assert!(!formatted.contains(r#""color""#));
    }

    #[test]
    fn test_color_level_mapping() {
        let config = LogConfig::default();
        let logger = GstatsLogger::new(config);
        
        // Test all log levels have appropriate color mapping and abbreviations
        let error_json = logger.format_json_message(Level::Error, "Error").unwrap();
        assert!(error_json.contains(r#""level":"ERR""#));
        assert!(error_json.contains(r#""color":"error""#));
        
        let warn_json = logger.format_json_message(Level::Warn, "Warning").unwrap();
        assert!(warn_json.contains(r#""level":"WRN""#));
        assert!(warn_json.contains(r#""color":"warning""#));
        
        let info_json = logger.format_json_message(Level::Info, "Info").unwrap();
        assert!(info_json.contains(r#""level":"INF""#));
        assert!(info_json.contains(r#""color":"info""#));
        
        let debug_json = logger.format_json_message(Level::Debug, "Debug").unwrap();
        assert!(debug_json.contains(r#""level":"DBG""#));
        assert!(debug_json.contains(r#""color":"debug""#));
        
        let trace_json = logger.format_json_message(Level::Trace, "Trace").unwrap();
        assert!(trace_json.contains(r#""level":"TRC""#));
        assert!(trace_json.contains(r#""color":"debug""#)); // Trace uses debug color
    }
    
    #[test]
    fn test_level_abbreviations() {
        assert_eq!(GstatsLogger::level_to_abbreviation(Level::Error), "ERR");
        assert_eq!(GstatsLogger::level_to_abbreviation(Level::Warn), "WRN");
        assert_eq!(GstatsLogger::level_to_abbreviation(Level::Info), "INF");
        assert_eq!(GstatsLogger::level_to_abbreviation(Level::Debug), "DBG");
        assert_eq!(GstatsLogger::level_to_abbreviation(Level::Trace), "TRC");
    }
}
