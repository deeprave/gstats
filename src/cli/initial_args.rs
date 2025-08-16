//! Minimal initial argument parsing for configuration discovery
//!
//! This module provides a first-stage parser that uses clap to extract only
//! the configuration-related arguments needed before plugin discovery.
//! It uses clap's ability to ignore unknown arguments, ensuring we handle
//! all CLI edge cases (short flags, equals syntax, etc.) correctly.

use clap::{Parser, ArgAction};
use std::path::PathBuf;

/// Minimal clap parser for configuration discovery only
/// 
/// This parser uses clap's derive API but with settings that allow
/// it to ignore unknown arguments gracefully. Only configuration-related
/// arguments are captured here.
#[derive(Parser, Debug, Clone)]
#[command(name = "gstats")]
#[command(disable_help_flag = true)]  // We'll handle help ourselves
#[command(disable_version_flag = true)]  // We'll handle version ourselves
#[command(ignore_errors = true)]  // Ignore unknown arguments/subcommands
pub struct InitialArgs {
    /// Configuration file path
    #[arg(long = "config-file", value_name = "FILE")]
    pub config_file: Option<PathBuf>,
    
    /// Plugin directory override
    #[arg(long = "plugin-dir", value_name = "DIR")]
    pub plugin_dir: Option<String>,
    
    /// Additional plugin directories
    #[arg(long = "plugins-dir", value_name = "DIR", action = ArgAction::Append)]
    pub plugins_dir: Vec<String>,
    
    /// Plugin exclusion list
    #[arg(long = "plugin-exclude", value_name = "LIST")]
    pub plugin_exclude: Option<String>,
    
    /// Help flag detection
    #[arg(long = "help", short = 'h', action = ArgAction::SetTrue)]
    pub help_requested: bool,
    
    /// Version flag detection  
    #[arg(long = "version", short = 'V', action = ArgAction::SetTrue)]
    pub version_requested: bool,
}

impl InitialArgs {
    /// Parse minimal arguments from command line using clap with proper error handling
    /// 
    /// This builds a clap parser that only includes configuration-related arguments
    /// and uses clap's proper parsing with error handling for unknown arguments.
    pub fn parse_from_env() -> Self {
        use std::env;
        let args: Vec<String> = env::args().collect();
        Self::parse_from_args(&args)
    }
    
    /// Parse minimal arguments from a provided argument list using clap's try_parse_from
    pub fn parse_from_args(args: &[String]) -> Self {
        // Build a minimal clap Command that only knows about config arguments
        let cmd = clap::Command::new("gstats")
            .disable_help_flag(true)  // We'll handle help manually
            .disable_version_flag(true)  // We'll handle version manually
            .arg(clap::Arg::new("config-file")
                .long("config-file")
                .value_name("FILE")
                .help("Configuration file path"))
            .arg(clap::Arg::new("plugin-dir")
                .long("plugin-dir")
                .value_name("DIR")
                .help("Plugin directory override"))
            .arg(clap::Arg::new("plugins-dir")
                .long("plugins-dir")
                .value_name("DIR")
                .action(clap::ArgAction::Append)
                .help("Additional plugin directories"))
            .arg(clap::Arg::new("plugin-exclude")
                .long("plugin-exclude")
                .value_name("LIST")
                .help("Plugin exclusion list"))
            .arg(clap::Arg::new("help")
                .long("help")
                .short('h')
                .action(clap::ArgAction::SetTrue)
                .help("Show help"))
            .arg(clap::Arg::new("version")
                .long("version")
                .short('V')
                .action(clap::ArgAction::SetTrue)
                .help("Show version"))
            .allow_external_subcommands(true)
            .ignore_errors(true);
        
        match cmd.try_get_matches_from(args) {
            Ok(matches) => Self::from_matches(&matches),
            Err(_) => Self::create_minimal_fallback(),
        }
    }
    
    /// Create InitialArgs from clap ArgMatches
    fn from_matches(matches: &clap::ArgMatches) -> Self {
        Self {
            config_file: matches.get_one::<String>("config-file")
                .map(|s| std::path::PathBuf::from(s)),
            plugin_dir: matches.get_one::<String>("plugin-dir").cloned(),
            plugins_dir: matches.get_many::<String>("plugins-dir")
                .map(|vals| vals.cloned().collect())
                .unwrap_or_default(),
            plugin_exclude: matches.get_one::<String>("plugin-exclude").cloned(),
            help_requested: matches.get_flag("help"),
            version_requested: matches.get_flag("version"),
        }
    }
    
    /// Create a minimal fallback when initial parsing fails
    fn create_minimal_fallback() -> Self {
        Self {
            config_file: None,
            plugin_dir: None,
            plugins_dir: Vec::new(),
            plugin_exclude: None,
            help_requested: false,
            version_requested: false,
        }
    }
    
    /// Check if only basic help or version was requested
    pub fn is_early_exit(&self) -> bool {
        self.help_requested || self.version_requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_config_file() {
        let args = vec![
            "gstats".to_string(),
            "--config-file".to_string(),
            "custom.toml".to_string(),
            "commits".to_string(),  // Unknown subcommand, but should be ignored
        ];
        
        let initial = InitialArgs::parse_from_args(&args);
        assert_eq!(initial.config_file, Some(PathBuf::from("custom.toml")));
        assert!(!initial.help_requested);
    }
    
    #[test]
    fn test_parse_plugin_dir() {
        let args = vec![
            "gstats".to_string(),
            "--plugin-dir".to_string(),
            "/custom/plugins".to_string(),
            "--plugin-exclude".to_string(),
            "unwanted".to_string(),
            "output".to_string(),  // Unknown subcommand, should be ignored
        ];
        
        let initial = InitialArgs::parse_from_args(&args);
        assert_eq!(initial.plugin_dir, Some("/custom/plugins".to_string()));
        assert_eq!(initial.plugin_exclude, Some("unwanted".to_string()));
    }
    
    #[test]
    fn test_mixed_known_unknown_args() {
        let args = vec![
            "gstats".to_string(),
            "--config-file".to_string(),
            "test.toml".to_string(),
            "--verbose".to_string(),     // Unknown to initial parser
            "commits".to_string(),       // Unknown subcommand
            "--since".to_string(),       // Unknown to initial parser  
            "1 week".to_string(),        // Unknown argument
        ];
        
        let initial = InitialArgs::parse_from_args(&args);
        assert_eq!(initial.config_file, Some(PathBuf::from("test.toml")));
        assert!(!initial.help_requested);
        assert!(!initial.version_requested);
    }
    
    #[test]
    fn test_help_requested() {
        let args = vec![
            "gstats".to_string(),
            "--help".to_string(),
        ];
        
        let initial = InitialArgs::parse_from_args(&args);
        assert!(initial.help_requested);
        assert!(initial.is_early_exit());
    }
    
    #[test]
    fn test_version_requested() {
        let args = vec![
            "gstats".to_string(),
            "--version".to_string(),
        ];
        
        let initial = InitialArgs::parse_from_args(&args);
        assert!(initial.version_requested);
        assert!(initial.is_early_exit());
    }
    
    #[test]
    fn test_short_flags() {
        let args = vec![
            "gstats".to_string(),
            "-h".to_string(),
        ];
        
        let initial = InitialArgs::parse_from_args(&args);
        assert!(initial.help_requested);
        assert!(initial.is_early_exit());
    }
    
    #[test]
    fn test_equals_syntax() {
        let args = vec![
            "gstats".to_string(),
            "--config-file=custom.toml".to_string(),
            "--plugin-dir=/plugins".to_string(),
            "commits".to_string(),
        ];
        
        let initial = InitialArgs::parse_from_args(&args);
        assert_eq!(initial.config_file, Some(PathBuf::from("custom.toml")));
        assert_eq!(initial.plugin_dir, Some("/plugins".to_string()));
    }
    
    #[test]
    fn test_fallback_on_unknown_args() {
        let args = vec![
            "gstats".to_string(),
            "--some-unknown-flag".to_string(),
            "unknown-value".to_string(),
        ];
        
        // Should not panic, should return fallback
        let initial = InitialArgs::parse_from_args(&args);
        assert_eq!(initial.config_file, None);
        assert!(!initial.help_requested);
    }
}