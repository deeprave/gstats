// Command line interface module for gstats
// Handles argument parsing and CLI structure

use clap::Parser;
use anyhow::Result;
use std::path::PathBuf;
use log::{debug, info};

/// Git Repository Analytics Tool
#[derive(Parser, Debug)]
#[command(name = "gstats")]
#[command(about = "A fast, local-first git analytics tool providing code complexity trends, contributor statistics, performance metrics, and native macOS widgets")]
#[command(version)]
pub struct Args {
    /// Path to git repository (defaults to current directory if it's a git repository)
    pub repository: Option<String>,
    
    /// Verbose output (debug level logging)
    #[arg(short, long)]
    pub verbose: bool,
    
    /// Quiet output (error level logging only)
    #[arg(short, long)]
    pub quiet: bool,
    
    /// Debug output (trace level logging)
    #[arg(long)]
    pub debug: bool,
    
    /// Log format: text or json
    #[arg(long, value_name = "FORMAT", default_value = "text")]
    pub log_format: String,
    
    /// Log file path for file output
    #[arg(long, value_name = "FILE")]
    pub log_file: Option<PathBuf>,
    
    /// Log level for file output (independent of console level)
    #[arg(long, value_name = "LEVEL")]
    pub log_file_level: Option<String>,
}

/// Parse command line arguments
pub fn parse_args() -> Args {
    debug!("Parsing command line arguments");
    let args = Args::parse();
    debug!("Parsed CLI arguments: {:?}", args);
    args
}

/// Validate CLI argument combinations
pub fn validate_args(args: &Args) -> Result<()> {
    debug!("Validating CLI argument combinations");
    
    // Check for conflicting log level flags
    let log_flags_count = [args.verbose, args.quiet, args.debug]
        .iter()
        .filter(|&&flag| flag)
        .count();
    
    if log_flags_count > 1 {
        return Err(anyhow::anyhow!(
            "Conflicting log level flags: only one of --verbose, --quiet, or --debug may be specified"
        ));
    }
    
    // Validate log format
    match args.log_format.to_lowercase().as_str() {
        "text" | "json" => {},
        _ => return Err(anyhow::anyhow!(
            "Invalid log format '{}'. Valid options: text, json", args.log_format
        )),
    }
    
    // Validate log file level if provided
    if let Some(ref level) = args.log_file_level {
        match level.to_lowercase().as_str() {
            "error" | "warn" | "info" | "debug" | "trace" => {},
            _ => return Err(anyhow::anyhow!(
                "Invalid log file level '{}'. Valid levels: error, warn, info, debug, trace", level
            )),
        }
    }
    
    // Log file level requires log file
    if args.log_file_level.is_some() && args.log_file.is_none() {
        return Err(anyhow::anyhow!(
            "--log-file-level requires --log-file to be specified"
        ));
    }
    
    info!("CLI arguments validated successfully");
    Ok(())
}

/// Main CLI logic - processes parsed arguments and coordinates repository operations
pub fn run(args: Args) -> Result<()> {
    let repo_path = match args.repository {
        Some(path) => path,
        None => ".".to_string(), // Default to current directory
    };

    // For now, just print what we would analyze
    // This will be replaced with actual git analysis in future iterations
    println!("Analyzing git repository at: {}", repo_path);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_parsing_with_repository() {
        // Test that Args can be created with repository path
        let args = Args {
            repository: Some("/path/to/repo".to_string()),
            verbose: false,
            quiet: false,
            debug: false,
            log_format: "text".to_string(),
            log_file: None,
            log_file_level: None,
        };
        assert_eq!(args.repository, Some("/path/to/repo".to_string()));
    }

    #[test]
    fn test_args_parsing_without_repository() {
        // Test that Args can be created without repository path
        let args = Args {
            repository: None,
            verbose: false,
            quiet: false,
            debug: false,
            log_format: "text".to_string(),
            log_file: None,
            log_file_level: None,
        };
        assert_eq!(args.repository, None);
    }

    #[test]
    fn test_validate_args_success() {
        let args = Args {
            repository: None,
            verbose: true,
            quiet: false,
            debug: false,
            log_format: "json".to_string(),
            log_file: None,
            log_file_level: None,
        };
        assert!(validate_args(&args).is_ok());
    }

    #[test]
    fn test_validate_args_conflicting_flags() {
        let args = Args {
            repository: None,
            verbose: true,
            quiet: true,
            debug: false,
            log_format: "text".to_string(),
            log_file: None,
            log_file_level: None,
        };
        assert!(validate_args(&args).is_err());
    }

    #[test]
    fn test_validate_args_invalid_format() {
        let args = Args {
            repository: None,
            verbose: false,
            quiet: false,
            debug: false,
            log_format: "invalid".to_string(),
            log_file: None,
            log_file_level: None,
        };
        assert!(validate_args(&args).is_err());
    }

    #[test]
    fn test_validate_args_file_level_without_file() {
        let args = Args {
            repository: None,
            verbose: false,
            quiet: false,
            debug: false,
            log_format: "text".to_string(),
            log_file: None,
            log_file_level: Some("debug".to_string()),
        };
        assert!(validate_args(&args).is_err());
    }

    #[test]
    fn test_cli_run_with_path() {
        // Test that run function handles repository path correctly
        let args = Args {
            repository: Some("/some/path".to_string()),
            verbose: false,
            quiet: false,
            debug: false,
            log_format: "text".to_string(),
            log_file: None,
            log_file_level: None,
        };
        
        // Should not panic or error for basic path handling
        // Note: This doesn't validate git repository, just CLI processing
        let result = run(args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_run_without_path() {
        // Test that run function handles no repository path correctly
        let args = Args {
            repository: None,
            verbose: false,
            quiet: false,
            debug: false,
            log_format: "text".to_string(),
            log_file: None,
            log_file_level: None,
        };
        
        // Should not panic or error for basic path handling
        let result = run(args);
        assert!(result.is_ok());
    }
}
