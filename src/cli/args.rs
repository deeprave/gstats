use clap::{Parser, ArgAction};
use anyhow::Result;
use std::path::PathBuf;
use log::debug;

use super::enhanced_parser::EnhancedParser;
use super::help_formatter::HelpFormatter;

/// Git Repository Analytics Tool
#[derive(Parser, Debug)]
#[command(name = "gstats")]
#[command(about = "Fast, local-first git analytics tool")]
#[command(long_about = "gstats - Fast, local-first git analytics tool

COMMON WORKFLOWS:
  Quick analysis:     gstats commits
  Code metrics:       gstats metrics  
  Team insights:      gstats commits --author \"John Doe\"
  Export results:     gstats commits | gstats export --format csv
  Time-based analysis: gstats commits --since \"1 month ago\"

COMMANDS:
  commits             Analyze commit history and contributors
                      Example: gstats commits --since \"1 week ago\"
  
  metrics             Generate code complexity metrics
                      Example: gstats metrics --include-path src/
  
  export              Export analysis in various formats
                      Example: gstats export --format json > report.json

For command-specific help: gstats <command> --help
For plugin discovery: gstats --plugins")]
#[command(version)]
pub struct Args {
    /// Path to git repository (defaults to current directory)
    /// Examples: -r /path/to/repo, --repo ~/project
    #[arg(short = 'r', long = "repo", alias = "repository", value_name = "PATH", help = "Repository path (default: current directory)")]
    pub repository: Option<String>,
    
    /// Enable verbose output with detailed logging
    #[arg(short, long, help = "Verbose output (debug level logging)")]
    pub verbose: bool,
    
    /// Suppress all output except errors
    #[arg(short, long, help = "Quiet output (errors only)")]
    pub quiet: bool,
    
    /// Enable debug output with trace-level logging
    #[arg(long, help = "Debug output (trace level logging)")]
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
    
    /// Force colored output (overrides TTY detection and NO_COLOR)
    #[arg(long = "color", help = "Force colored output even when redirected")]
    pub color: bool,
    
    /// Disable colored output (overrides configuration and NO_COLOR)
    #[arg(long = "no-color", help = "Disable colored output")]
    pub no_color: bool,
    
    /// Configuration file path
    #[arg(long, value_name = "FILE")]
    pub config_file: Option<PathBuf>,
    
    /// Configuration section name
    #[arg(long, value_name = "SECTION")]
    pub config_name: Option<String>,

    // ============ FILTERING FLAGS ============
    
    /// Filter commits from this date onwards
    /// Examples: "2023-01-01", "1 week ago", "last month"
    #[arg(short = 'S', long = "since", value_name = "DATE", help = "Start date filter (ISO 8601 or relative)")]
    pub since: Option<String>,
    
    /// Filter commits up to this date
    /// Examples: "2023-12-31", "yesterday", "1 week ago"
    #[arg(short = 'U', long = "until", value_name = "DATE", help = "End date filter (ISO 8601 or relative)")]
    pub until: Option<String>,
    
    /// Only analyze specific directories
    /// Examples: --include-path src/ --include-path tests/
    #[arg(short = 'I', long = "include-path", value_name = "PATH", action = ArgAction::Append, help = "Include specific paths (supports comma-separated)")]
    pub include_path: Vec<String>,
    
    /// Skip specific directories from analysis
    /// Examples: --exclude-path target/ --exclude-path node_modules/
    #[arg(short = 'X', long = "exclude-path", value_name = "PATH", action = ArgAction::Append, help = "Exclude specific paths (supports comma-separated)")]
    pub exclude_path: Vec<String>,
    
    /// Only analyze files matching these patterns
    /// Examples: --include-file "*.rs" --include-file "*.toml"
    #[arg(short = 'F', long = "include-file", value_name = "PATTERN", action = ArgAction::Append, help = "Include file patterns (supports comma-separated)")]
    pub include_file: Vec<String>,
    
    /// Skip files matching these patterns
    /// Examples: --exclude-file "*.tmp" --exclude-file "*.bak"
    #[arg(short = 'N', long = "exclude-file", value_name = "PATTERN", action = ArgAction::Append, help = "Exclude file patterns (supports comma-separated)")]
    pub exclude_file: Vec<String>,
    
    /// Only show commits from specific authors
    /// Examples: --author "john@example.com" --author "Jane Doe"
    #[arg(short = 'A', long = "author", value_name = "AUTHOR", action = ArgAction::Append, help = "Include specific authors (supports comma-separated)")]
    pub author: Vec<String>,
    
    /// Hide commits from specific authors
    /// Examples: --exclude-author "bot@automated.com"
    #[arg(short = 'E', long = "exclude-author", value_name = "AUTHOR", action = ArgAction::Append, help = "Exclude specific authors (supports comma-separated)")]
    pub exclude_author: Vec<String>,
    
    /// Limit the number of results returned
    /// Example: --limit 100
    #[arg(short = 'L', long = "limit", value_name = "N", help = "Maximum number of results")]
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
    
    /// Plugin commands to execute
    /// Examples: commits, metrics, export, commits:authors
    #[arg(value_name = "COMMAND", help = "Plugin commands to execute (e.g., commits, metrics, export)")]
    pub plugins: Vec<String>,
    
    // ============ PLUGIN DISCOVERY & HELP ============
    
    /// List all available plugins
    #[arg(long = "list-plugins", help = "List all available plugins")]
    pub list_plugins: bool,
    
    /// Show plugins with their functions and descriptions
    #[arg(long = "plugins", help = "Show all plugins with functions and descriptions")]
    pub show_plugins: bool,
    
    /// Display comprehensive plugin help and command mappings
    #[arg(long = "plugins-help", help = "Show detailed plugin functions and command mappings")]
    pub plugins_help: bool,
    
    /// Get detailed information about a specific plugin
    /// Example: --plugin-info commits
    #[arg(long = "plugin-info", value_name = "PLUGIN", help = "Show detailed information about specific plugin")]
    pub plugin_info: Option<String>,
    
    
    /// List plugins by category
    /// Examples: --list-by-type scanner, --list-by-type output
    #[arg(long = "list-by-type", value_name = "TYPE", help = "List plugins by type (scanner, output, etc.)")]
    pub list_by_type: Option<String>,
    
    /// Override default plugin discovery directory
    /// Example: --plugin-dir ./custom_plugins
    #[arg(long = "plugin-dir", value_name = "DIR", help = "Custom plugin directory path")]
    pub plugin_dir: Option<String>,
    
    /// Additional plugin directories to search (adds to discovery process)
    /// Example: --plugins-dir ./my_plugins
    #[arg(long = "plugins-dir", value_name = "DIR", action = ArgAction::Append, help = "Additional plugin directories to search")]
    pub plugins_dir: Vec<String>,
    
    /// Explicitly load specific plugins by name or path (bypasses discovery)
    /// Example: --plugin-load plugin1,plugin2 or --plugin-load ./path/to/plugin.yaml
    #[arg(long = "plugin-load", value_name = "LIST", help = "Comma-separated list of plugins to load explicitly")]
    pub plugin_load: Option<String>,
    
    /// Exclude specific plugins by name or path
    /// Example: --plugin-exclude plugin1,plugin2
    #[arg(long = "plugin-exclude", value_name = "LIST", help = "Comma-separated list of plugins to exclude")]
    pub plugin_exclude: Option<String>,
    
    /// Export complete configuration to TOML file
    /// Example: --export-config gstats-config.toml
    #[arg(long = "export-config", value_name = "FILE", help = "Export complete configuration to specified TOML file")]
    pub export_config: Option<PathBuf>,
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
        
        // Parse plugin load/exclude lists (comma-separated)
        if let Some(load_list) = self.plugin_load.take() {
            self.plugin_load = Some(load_list); // Keep as string for now, will parse in converter
        }
        if let Some(exclude_list) = self.plugin_exclude.take() {
            self.plugin_exclude = Some(exclude_list); // Keep as string for now, will parse in converter
        }
        
        self
    }
}

/// Parse command line arguments
pub fn parse_args() -> Args {
    debug!("Parsing command line arguments");
    
    // Check for help flag before parsing to intercept it
    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        let no_color = args.contains(&"--no-color".to_string());
        let color = args.contains(&"--color".to_string());
        display_enhanced_help(no_color, color);
        std::process::exit(0);
    }
    
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
    
    debug!("CLI arguments validated successfully");
    Ok(())
}

/// Display enhanced help with colors and better formatting
pub fn display_enhanced_help(no_color: bool, color: bool) {
    let formatter = HelpFormatter::from_color_flags(no_color, color);
    println!("{}", formatter.format_main_help());
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
            color: false,
            no_color: false,
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
            list_plugins: false,
            show_plugins: false,
            plugins_help: false,
            plugin_info: None,
            list_by_type: None,
            plugin_dir: None,
            plugins_dir: Vec::new(),
            plugin_load: None,
            plugin_exclude: None,
            export_config: None,
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
        assert!(validate_args(&args).is_ok());
        assert_eq!(args.repository, Some("/some/path".to_string()));
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
        
        assert!(validate_args(&args).is_ok());
        assert!(args.repository.is_none());
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

    #[test]
    fn test_plugin_arguments_default_values() {
        let args = create_test_args();
        
        assert!(args.plugins_dir.is_empty());
        assert!(args.plugin_load.is_none());
        assert!(args.plugin_exclude.is_none());
    }

    #[test]
    fn test_plugin_arguments_with_values() {
        let args = Args {
            plugins_dir: vec!["./my_plugins".to_string(), "./shared_plugins".to_string()],
            plugin_load: Some("plugin1,plugin2,./custom/plugin.yaml".to_string()),
            plugin_exclude: Some("unwanted_plugin,./path/to/exclude.yaml".to_string()),
            ..create_test_args()
        };
        
        assert_eq!(args.plugins_dir, vec!["./my_plugins", "./shared_plugins"]);
        assert_eq!(args.plugin_load, Some("plugin1,plugin2,./custom/plugin.yaml".to_string()));
        assert_eq!(args.plugin_exclude, Some("unwanted_plugin,./path/to/exclude.yaml".to_string()));
    }
}
