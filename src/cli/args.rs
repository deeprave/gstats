use clap::{Parser, ArgAction};
use anyhow::Result;
use std::path::PathBuf;
use log::{debug, info};

use super::enhanced_parser::EnhancedParser;

/// Git Repository Analytics Tool
#[derive(Parser, Debug)]
#[command(name = "gstats")]
#[command(about = "A fast, local-first git analytics tool providing code complexity trends, contributor statistics, performance metrics, and native macOS widgets")]
#[command(version)]
pub struct Args {
    /// Path to git repository (defaults to current directory if it's a git repository)
    #[arg(short = 'r', long = "repo", alias = "repository", value_name = "PATH")]
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
    
    /// Configuration file path
    #[arg(long, value_name = "FILE")]
    pub config_file: Option<PathBuf>,
    
    /// Configuration section name
    #[arg(long, value_name = "SECTION")]
    pub config_name: Option<String>,

    // ============ FILTERING FLAGS ============
    
    /// Start date filter (ISO 8601 or relative like "1 week ago")
    #[arg(short = 'S', long = "since", value_name = "DATE")]
    pub since: Option<String>,
    
    /// End date filter (ISO 8601 or relative like "1 week ago")
    #[arg(short = 'U', long = "until", value_name = "DATE")]
    pub until: Option<String>,
    
    /// Include paths (directories) - supports comma-separated values
    #[arg(short = 'I', long = "include-path", value_name = "PATH", action = ArgAction::Append)]
    pub include_path: Vec<String>,
    
    /// Exclude paths (directories) - supports comma-separated values  
    #[arg(short = 'X', long = "exclude-path", value_name = "PATH", action = ArgAction::Append)]
    pub exclude_path: Vec<String>,
    
    /// Include file patterns (glob patterns) - supports comma-separated values
    #[arg(short = 'F', long = "include-file", value_name = "PATTERN", action = ArgAction::Append)]
    pub include_file: Vec<String>,
    
    /// Exclude file patterns (glob patterns) - supports comma-separated values
    #[arg(short = 'N', long = "exclude-file", value_name = "PATTERN", action = ArgAction::Append)]
    pub exclude_file: Vec<String>,
    
    /// Include authors (name or email) - supports comma-separated values
    #[arg(short = 'A', long = "author", value_name = "AUTHOR", action = ArgAction::Append)]
    pub author: Vec<String>,
    
    /// Exclude authors (name or email) - supports comma-separated values
    #[arg(short = 'E', long = "exclude-author", value_name = "AUTHOR", action = ArgAction::Append)]
    pub exclude_author: Vec<String>,
    
    /// Maximum number of results to return
    #[arg(short = 'L', long = "limit", value_name = "N")]
    pub limit: Option<usize>,
    
    // ============ SCANNER CONFIGURATION ============
    
    /// Enable performance mode (optimized for speed over memory usage)
    #[arg(long = "performance-mode")]
    pub performance_mode: bool,
    
    /// Disable performance mode (prioritize memory usage over speed)
    #[arg(long = "no-performance-mode")]
    pub no_performance_mode: bool,
    
    /// Maximum memory usage for scanner queues (supports units: MB, GB, K, T, etc.)
    #[arg(long = "max-memory", value_name = "SIZE")]
    pub max_memory: Option<String>,
    
    /// Queue size for scanner operations
    #[arg(long = "queue-size", value_name = "N")]
    pub queue_size: Option<usize>,
    
    /// Plugin names to execute (positional arguments)
    #[arg(value_name = "PLUGIN")]
    pub plugins: Vec<String>,
}

impl Args {
    /// Apply enhanced parsing to vector fields that support comma-separated values
    pub fn apply_enhanced_parsing(mut self) -> Self {
        self.include_path = EnhancedParser::parse_paths(self.include_path);
        self.exclude_path = EnhancedParser::parse_paths(self.exclude_path);
        self.include_file = EnhancedParser::parse_file_patterns(self.include_file);
        self.exclude_file = EnhancedParser::parse_file_patterns(self.exclude_file);
        self.author = EnhancedParser::parse_authors(self.author);
        self.exclude_author = EnhancedParser::parse_authors(self.exclude_author);
        self
    }
}

/// Parse command line arguments
pub fn parse_args() -> Args {
    debug!("Parsing command line arguments");
    let args = Args::parse().apply_enhanced_parsing();
    debug!("Parsed CLI arguments with enhanced parsing: {:?}", args);
    args
}

/// Validate CLI argument combinations
pub fn validate_args(args: &Args) -> Result<()> {
    debug!("Validating CLI argument combinations");
    
    let log_flags_count = [args.verbose, args.quiet, args.debug]
        .iter()
        .filter(|&&flag| flag)
        .count();
    
    if log_flags_count > 1 {
        return Err(anyhow::anyhow!(
            "Conflicting log level flags: only one of --verbose, --quiet, or --debug may be specified"
        ));
    }
    
    match args.log_format.to_lowercase().as_str() {
        "text" | "json" => {},
        _ => return Err(anyhow::anyhow!(
            "Invalid log format '{}'. Valid options: text, json", args.log_format
        )),
    }
    
    if let Some(ref level) = args.log_file_level {
        match level.to_lowercase().as_str() {
            "error" | "warn" | "info" | "debug" | "trace" => {},
            _ => return Err(anyhow::anyhow!(
                "Invalid log file level '{}'. Valid levels: error, warn, info, debug, trace", level
            )),
        }
    }
    
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

    // TODO: Replace with actual git analysis implementation
    println!("Analyzing git repository at: {}", repo_path);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create Args with default values for testing
    fn create_test_args() -> Args {
        Args {
            repository: None,
            verbose: false,
            quiet: false,
            debug: false,
            log_format: "text".to_string(),
            log_file: None,
            log_file_level: None,
            config_file: None,
            config_name: None,
            since: None,
            until: None,
            include_path: Vec::new(),
            exclude_path: Vec::new(),
            include_file: Vec::new(),
            exclude_file: Vec::new(),
            author: Vec::new(),
            exclude_author: Vec::new(),
            limit: None,
            performance_mode: false,
            no_performance_mode: false,
            max_memory: None,
            queue_size: None,
            plugins: Vec::new(),
        }
    }

    #[test]
    fn test_args_parsing_with_repository() {
        let args = Args {
            repository: Some("/path/to/repo".to_string()),
            ..create_test_args()
        };
        assert_eq!(args.repository, Some("/path/to/repo".to_string()));
    }

    #[test]
    fn test_args_parsing_without_repository() {
        let args = Args {
            repository: None,
            ..create_test_args()
        };
        assert_eq!(args.repository, None);
    }

    #[test]
    fn test_validate_args_success() {
        let args = Args {
            verbose: true,
            log_format: "json".to_string(),
            ..create_test_args()
        };
        assert!(validate_args(&args).is_ok());
    }

    #[test]
    fn test_validate_args_conflicting_flags() {
        let args = Args {
            verbose: true,
            quiet: true,
            ..create_test_args()
        };
        assert!(validate_args(&args).is_err());
    }

    #[test]
    fn test_validate_args_invalid_format() {
        let args = Args {
            log_format: "invalid".to_string(),
            ..create_test_args()
        };
        assert!(validate_args(&args).is_err());
    }

    #[test]
    fn test_validate_args_file_level_without_file() {
        let args = Args {
            log_file_level: Some("debug".to_string()),
            ..create_test_args()
        };
        assert!(validate_args(&args).is_err());
    }

    #[test]
    fn test_cli_run_with_path() {
        let args = Args {
            repository: Some("/some/path".to_string()),
            ..create_test_args()
        };
        
        // Should not panic or error for basic path handling
        // Note: This doesn't validate git repository, just CLI processing
        let result = run(args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_repo_flag() {
        // Test that the repository flag aliases work correctly
        // Note: We can't easily test clap parsing in unit tests without command line input,
        // but we can verify the struct accepts the repository field correctly
        let args = Args {
            repository: Some("/test/repo/path".to_string()),
            ..create_test_args()
        };
        
        assert_eq!(args.repository, Some("/test/repo/path".to_string()));
        
        // Test that validation accepts repository paths
        assert!(validate_args(&args).is_ok());
    }

    #[test]
    fn test_error_handling() {
        // Test that CLI provides helpful error messages for common scenarios
        let args = Args {
            repository: Some("/nonexistent/path".to_string()),
            ..create_test_args()
        };
        
        // Basic validation should pass (repository paths are validated later)
        assert!(validate_args(&args).is_ok());
    }

    #[test]
    fn test_cli_run_without_path() {
        let args = create_test_args();
        
        let result = run(args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_filtering_flags_default_values() {
        let args = create_test_args();
        
        assert!(args.since.is_none());
        assert!(args.until.is_none());
        assert!(args.include_path.is_empty());
        assert!(args.exclude_path.is_empty());
        assert!(args.include_file.is_empty());
        assert!(args.exclude_file.is_empty());
        assert!(args.author.is_empty());
        assert!(args.exclude_author.is_empty());
        assert!(args.limit.is_none());
        assert!(args.plugins.is_empty());
    }

    #[test]
    fn test_filtering_flags_with_values() {
        let args = Args {
            since: Some("2023-01-01".to_string()),
            until: Some("2023-12-31".to_string()),
            include_path: vec!["src/".to_string(), "tests/".to_string()],
            exclude_path: vec!["target/".to_string()],
            include_file: vec!["*.rs".to_string(), "*.toml".to_string()],
            exclude_file: vec!["*.tmp".to_string()],
            author: vec!["alice@example.com".to_string()],
            exclude_author: vec!["bot@automated.com".to_string()],
            limit: Some(100),
            plugins: vec!["commits".to_string(), "metrics".to_string()],
            ..create_test_args()
        };
        
        assert_eq!(args.since, Some("2023-01-01".to_string()));
        assert_eq!(args.until, Some("2023-12-31".to_string()));
        assert_eq!(args.include_path, vec!["src/", "tests/"]);
        assert_eq!(args.exclude_path, vec!["target/"]);
        assert_eq!(args.include_file, vec!["*.rs", "*.toml"]);
        assert_eq!(args.exclude_file, vec!["*.tmp"]);
        assert_eq!(args.author, vec!["alice@example.com"]);
        assert_eq!(args.exclude_author, vec!["bot@automated.com"]);
        assert_eq!(args.limit, Some(100));
        assert_eq!(args.plugins, vec!["commits", "metrics"]);
    }

    #[test]
    fn test_enhanced_parsing_comma_separated() {
        let args = Args {
            include_path: vec!["src/,tests/".to_string()],
            exclude_path: vec!["target/,build/".to_string()],
            include_file: vec!["*.rs,*.toml".to_string()],
            exclude_file: vec!["*.tmp,*.bak".to_string()],
            author: vec!["john@example.com,jane@example.com".to_string()],
            exclude_author: vec!["bot@example.com,noreply@github.com".to_string()],
            ..create_test_args()
        }.apply_enhanced_parsing();
        
        assert_eq!(args.include_path, vec!["src/", "tests/"]);
        assert_eq!(args.exclude_path, vec!["target/", "build/"]);
        assert_eq!(args.include_file, vec!["*.rs", "*.toml"]);
        assert_eq!(args.exclude_file, vec!["*.tmp", "*.bak"]);
        assert_eq!(args.author, vec!["john@example.com", "jane@example.com"]);
        assert_eq!(args.exclude_author, vec!["bot@example.com", "noreply@github.com"]);
    }

    #[test]
    fn test_enhanced_parsing_mixed_formats() {
        let args = Args {
            include_path: vec!["src/,tests/".to_string(), "lib/".to_string()],
            author: vec!["john@example.com,jane@example.com".to_string(), "bob@example.com".to_string()],
            ..create_test_args()
        }.apply_enhanced_parsing();
        
        assert_eq!(args.include_path, vec!["src/", "tests/", "lib/"]);
        assert_eq!(args.author, vec!["john@example.com", "jane@example.com", "bob@example.com"]);
    }

    #[test]
    fn test_enhanced_parsing_with_spaces() {
        let args = Args {
            include_path: vec!["src/ , tests/ ".to_string()],
            author: vec![" john@example.com , jane@example.com ".to_string()],
            ..create_test_args()
        }.apply_enhanced_parsing();
        
        assert_eq!(args.include_path, vec!["src/", "tests/"]);
        assert_eq!(args.author, vec!["john@example.com", "jane@example.com"]);
    }
}
