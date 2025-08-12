//! CLI Argument Converter
//! 
//! Converts parsed CLI arguments to QueryParams for integration with the GS-24 filtering system.

use crate::cli::date_parser::{parse_date, validate_date_range, DateParseError};
use crate::cli::memory_parser::{parse_memory_size, MemoryParseError};
use crate::scanner::query::{QueryParams, DateRange, FilePathFilter, AuthorFilter};
use crate::scanner::config::ScannerConfig;
use std::path::PathBuf;
use thiserror::Error;

/// CLI conversion errors
#[derive(Debug, Error)]
pub enum CliError {
    #[error("Date parsing error: {0}")]
    DateParse(#[from] DateParseError),
    
    #[error("Memory parsing error: {0}")]
    MemoryParse(#[from] MemoryParseError),
    
    #[error("Invalid path: {path}")]
    InvalidPath { path: String },
    
    
    #[error("Empty author name provided")]
    EmptyAuthor,
    
    #[error("Invalid limit: {limit} must be greater than 0")]
    InvalidLimit { limit: usize },
    
    #[error("Invalid queue size: {size} must be greater than 0")]
    InvalidQueueSize { size: usize },
    
    #[error("Conflicting performance mode options: cannot specify both --performance-mode and --no-performance-mode")]
    ConflictingPerformanceModes,
    
    #[error("Plugin validation error: {message}")]
    PluginValidation { message: String },
}

/// Convert CLI arguments to ScannerConfig with ConfigManager integration
/// 
/// This function takes CLI scanner configuration arguments and converts them
/// into a ScannerConfig for the scanning system. CLI arguments override config file settings.
pub fn args_to_scanner_config(args: &crate::cli::Args, config_manager: Option<&crate::config::ConfigManager>) -> Result<ScannerConfig, CliError> {
    // Validate performance mode conflicts
    if args.performance_mode && args.no_performance_mode {
        return Err(CliError::ConflictingPerformanceModes);
    }
    
    // Start with config from file (if available), otherwise use defaults
    let mut config = if let Some(manager) = config_manager {
        manager.get_scanner_config()
            .map_err(|e| CliError::PluginValidation { message: e.to_string() })?
    } else {
        ScannerConfig::default()
    };
    
    // Apply CLI performance mode settings (override config file)
    if args.performance_mode {
        // Performance mode: increase memory and queue size for speed
        config.max_memory_bytes = 256 * 1024 * 1024; // 256MB
        config.queue_size = 5000;
    } else if args.no_performance_mode {
        // Conservative mode: reduce memory usage
        config.max_memory_bytes = 32 * 1024 * 1024; // 32MB
        config.queue_size = 500;
    }
    
    // Override with specific CLI memory setting if provided
    if let Some(memory_str) = &args.max_memory {
        config.max_memory_bytes = parse_memory_size(memory_str)?; // Already returns bytes
    }
    
    // Override with specific CLI queue size if provided
    if let Some(queue_size) = args.queue_size {
        if queue_size == 0 {
            return Err(CliError::InvalidQueueSize { size: queue_size });
        }
        config.queue_size = queue_size;
    }
    
    // Validate the final configuration
    config.validate()
        .map_err(|e| CliError::PluginValidation { message: e.to_string() })?;
    
    Ok(config)
}


/// Convert CLI arguments to QueryParams
/// 
/// This function takes the parsed CLI arguments and converts them into the QueryParams
/// structure expected by the GS-24 filtering system.
/// 
/// # Examples
/// 
/// ```ignore
/// use gstats::cli::converter::args_to_query_params;
/// use gstats::cli::Args;
/// 
/// // Args would typically be created by clap CLI parsing
/// // let args = Args::parse(); // from clap
/// // let query_params = args_to_query_params(&args)?;
/// ```
pub fn args_to_query_params(args: &crate::cli::Args) -> Result<QueryParams, CliError> {
    // Convert date arguments
    let date_range = convert_date_arguments(&args.since, &args.until)?;
    
    // Convert path arguments
    let file_paths = convert_path_arguments(&args.include_path, &args.exclude_path)?;
    
    // Convert author arguments
    let authors = convert_author_arguments(&args.author, &args.exclude_author)?;
    
    // Validate limit
    let scan_limit = validate_scan_limit(args.scan_limit)?;
    
    Ok(QueryParams {
        date_range,
        file_paths,
        limit: scan_limit,
        authors,
    })
}

/// Convert CLI date arguments to DateRange
fn convert_date_arguments(since: &Option<String>, until: &Option<String>) -> Result<Option<DateRange>, CliError> {
    // Validate date range logic first
    if let (Some(since_str), Some(until_str)) = (since.as_ref(), until.as_ref()) {
        validate_date_range(Some(since_str), Some(until_str))?;
    }
    
    let start_time = if let Some(since_str) = since {
        Some(parse_date(since_str)?)
    } else {
        None
    };
    
    let end_time = if let Some(until_str) = until {
        Some(parse_date(until_str)?)
    } else {
        None
    };
    
    match (start_time, end_time) {
        (Some(start), Some(end)) => Ok(Some(DateRange::new(start, end))),
        (Some(start), None) => Ok(Some(DateRange::from(start))),
        (None, Some(end)) => Ok(Some(DateRange::until(end))),
        (None, None) => Ok(None),
    }
}

/// Convert CLI path arguments to FilePathFilter
fn convert_path_arguments(include_paths: &[String], exclude_paths: &[String]) -> Result<FilePathFilter, CliError> {
    let include = include_paths.iter()
        .map(|path| validate_and_convert_path(path))
        .collect::<Result<Vec<_>, _>>()?;
    
    let exclude = exclude_paths.iter()
        .map(|path| validate_and_convert_path(path))
        .collect::<Result<Vec<_>, _>>()?;
    
    Ok(FilePathFilter { include, exclude })
}

/// Validate and convert a path string to PathBuf
fn validate_and_convert_path(path: &str) -> Result<PathBuf, CliError> {
    if path.trim().is_empty() {
        return Err(CliError::InvalidPath { path: path.to_string() });
    }
    
    // Basic path validation - check for invalid characters
    if path.contains('\0') {
        return Err(CliError::InvalidPath { path: path.to_string() });
    }
    
    Ok(PathBuf::from(path.trim()))
}

/// Convert CLI author arguments to AuthorFilter
fn convert_author_arguments(include_authors: &[String], exclude_authors: &[String]) -> Result<AuthorFilter, CliError> {
    let include = include_authors.iter()
        .map(|author| validate_author(author))
        .collect::<Result<Vec<_>, _>>()?;
    
    let exclude = exclude_authors.iter()
        .map(|author| validate_author(author))
        .collect::<Result<Vec<_>, _>>()?;
    
    Ok(AuthorFilter { include, exclude })
}

/// Validate an author name/email
fn validate_author(author: &str) -> Result<String, CliError> {
    let trimmed = author.trim();
    if trimmed.is_empty() {
        return Err(CliError::EmptyAuthor);
    }
    Ok(trimmed.to_string())
}

/// Validate limit argument
fn validate_scan_limit(scan_limit: Option<usize>) -> Result<Option<usize>, CliError> {
    if let Some(limit_value) = scan_limit {
        if limit_value == 0 {
            return Err(CliError::InvalidLimit { limit: limit_value });
        }
    }
    Ok(scan_limit)
}

/// Plugin configuration merged from CLI arguments and config file
#[derive(Debug, Clone)]
pub struct PluginConfig {
    /// Plugin directories to search (CLI --plugins-dir + config directory)
    pub directories: Vec<String>,
    /// Plugins to explicitly load (CLI --plugin-load or config plugin-load)
    pub plugin_load: Vec<String>,
    /// Plugins to exclude (CLI --plugin-exclude or config plugin-exclude)
    pub plugin_exclude: Vec<String>,
}

/// Merge plugin configuration from CLI arguments and config file
/// Precedence: CLI > Config > Defaults
pub fn merge_plugin_config(args: &crate::cli::Args, config_manager: Option<&crate::config::ConfigManager>) -> PluginConfig {
    let mut config = PluginConfig {
        directories: Vec::new(),
        plugin_load: Vec::new(),
        plugin_exclude: Vec::new(),
    };

    // Start with configuration file values
    if let Some(manager) = config_manager {
        // Add config directory to search paths
        if let Some(config_dir) = manager.get_plugins_directory() {
            config.directories.push(config_dir.to_string());
        }
        
        // Add config plugin-load list
        config.plugin_load.extend(manager.get_plugins_load());
        
        // Add config plugin-exclude list  
        config.plugin_exclude.extend(manager.get_plugins_exclude());
    }

    // Override with CLI arguments (higher precedence)
    
    // Add CLI plugin directories
    config.directories.extend(args.plugins_dir.clone());

    // CLI plugin-load overrides config completely (as per requirements)
    if let Some(cli_load) = &args.plugin_load {
        config.plugin_load.clear(); // Override, don't merge
        config.plugin_load.extend(parse_comma_separated(cli_load));
    }

    // CLI plugin-exclude overrides config completely
    if let Some(cli_exclude) = &args.plugin_exclude {
        config.plugin_exclude.clear(); // Override, don't merge
        config.plugin_exclude.extend(parse_comma_separated(cli_exclude));
    }

    // Add CLI plugin directory if specified
    if let Some(cli_plugin_dir) = &args.plugin_directory {
        config.directories.push(cli_plugin_dir.clone());
    }

    // Add default plugin directory if no directories specified
    if config.directories.is_empty() {
        config.directories.push(resolve_default_plugin_directory());
    }

    config
}

/// Parse comma-separated string into Vec<String> (helper for CLI arguments)
fn parse_comma_separated(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Resolve default plugin directory with home expansion
fn resolve_default_plugin_directory() -> String {
    // Try to get home directory
    if let Some(home_dir) = dirs::home_dir() {
        home_dir.join(".config").join("gstats").join("plugins")
            .to_string_lossy()
            .to_string()
    } else {
        // Fallback to relative path if home directory is not available
        "plugins".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Args;

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
            compact: false,
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
            scan_limit: None,
            performance_mode: false,
            no_performance_mode: false,
            max_memory: None,
            queue_size: None,
            command: None,
            plugin_args: Vec::new(),
            list_plugins: false,
            plugin_info: None,
            list_by_type: None,
            plugin_dir: None,
            plugins_dir: Vec::new(),
            plugin_load: None,
            plugin_exclude: None,
            plugin_directory: None,
            show_plugins: false,
            plugins_help: false,
            export_config: None,
            list_formats: false,
        }
    }

    #[test]
    fn test_convert_date_arguments_both_dates() {
        let result = convert_date_arguments(
            &Some("2023-01-01".to_string()),
            &Some("2023-12-31".to_string())
        ).unwrap();
        
        assert!(result.is_some());
        let date_range = result.unwrap();
        assert!(date_range.is_bounded());
    }
    
    #[test]
    fn test_convert_date_arguments_start_only() {
        let result = convert_date_arguments(
            &Some("2023-01-01".to_string()),
            &None
        ).unwrap();
        
        assert!(result.is_some());
        let date_range = result.unwrap();
        assert!(date_range.start.is_some());
        assert!(date_range.end.is_none());
    }
    
    #[test]
    fn test_convert_date_arguments_end_only() {
        let result = convert_date_arguments(
            &None,
            &Some("2023-12-31".to_string())
        ).unwrap();
        
        assert!(result.is_some());
        let date_range = result.unwrap();
        assert!(date_range.start.is_none());
        assert!(date_range.end.is_some());
    }
    
    #[test]
    fn test_convert_date_arguments_none() {
        let result = convert_date_arguments(&None, &None).unwrap();
        assert!(result.is_none());
    }
    
    #[test]
    fn test_convert_date_arguments_invalid_range() {
        let result = convert_date_arguments(
            &Some("2023-12-31".to_string()),
            &Some("2023-01-01".to_string())
        );
        assert!(result.is_err());
    }
    
    #[test]
    fn test_convert_path_arguments() {
        let include = vec!["src/".to_string(), "tests/".to_string()];
        let exclude = vec!["target/".to_string()];
        
        let result = convert_path_arguments(&include, &exclude).unwrap();
        
        assert_eq!(result.include.len(), 2);
        assert_eq!(result.exclude.len(), 1);
        assert_eq!(result.include[0], PathBuf::from("src/"));
        assert_eq!(result.exclude[0], PathBuf::from("target/"));
    }
    
    #[test]
    fn test_convert_path_arguments_empty_path() {
        let include = vec!["".to_string()];
        let exclude = vec![];
        
        let result = convert_path_arguments(&include, &exclude);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_convert_author_arguments() {
        let include = vec!["alice@example.com".to_string(), "Bob Smith".to_string()];
        let exclude = vec!["spam@example.com".to_string()];
        
        let result = convert_author_arguments(&include, &exclude).unwrap();
        
        assert_eq!(result.include.len(), 2);
        assert_eq!(result.exclude.len(), 1);
        assert_eq!(result.include[0], "alice@example.com");
        assert_eq!(result.exclude[0], "spam@example.com");
    }
    
    #[test]
    fn test_convert_author_arguments_empty_author() {
        let include = vec!["".to_string()];
        let exclude = vec![];
        
        let result = convert_author_arguments(&include, &exclude);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_validate_scan_limit() {
        assert!(validate_scan_limit(Some(100)).unwrap() == Some(100));
        assert!(validate_scan_limit(None).unwrap().is_none());
        assert!(validate_scan_limit(Some(0)).is_err());
    }
    
    
    #[test]
    fn test_args_to_query_params_full() {
        let args = Args {
            repository: None,
            verbose: false,
            quiet: false,
            debug: false,
            log_format: "text".to_string(),
            log_file: None,
            log_file_level: None,
            color: false,
            no_color: false,
            compact: false,
            config_file: None,
            config_name: None,
            since: Some("2023-01-01".to_string()),
            until: Some("2023-12-31".to_string()),
            include_path: vec!["src/".to_string()],
            exclude_path: vec!["target/".to_string()],
            include_file: vec!["*.rs".to_string()],
            exclude_file: vec!["*.tmp".to_string()],
            author: vec!["alice@example.com".to_string()],
            exclude_author: vec!["spam@example.com".to_string()],
            scan_limit: Some(100),
            performance_mode: false,
            no_performance_mode: false,
            max_memory: None,
            queue_size: None,
            command: Some("commits".to_string()),
            plugin_args: Vec::new(),
            list_plugins: false,
            plugin_info: None,
            list_by_type: None,
            plugin_dir: None,
            plugins_dir: Vec::new(),
            plugin_load: None,
            plugin_exclude: None,
            plugin_directory: None,
            show_plugins: false,
            plugins_help: false,
            export_config: None,
            list_formats: false,
        };
        
        let result = args_to_query_params(&args).unwrap();
        
        assert!(result.date_range.is_some());
        assert!(!result.file_paths.include.is_empty());
        assert!(!result.file_paths.exclude.is_empty());
        assert!(!result.authors.include.is_empty());
        assert!(!result.authors.exclude.is_empty());
        assert_eq!(result.limit, Some(100));
    }
    
    #[test]
    fn test_args_to_query_params_minimal() {
        let args = create_test_args();
        
        let result = args_to_query_params(&args).unwrap();
        
        assert!(result.date_range.is_none());
        assert!(result.file_paths.include.is_empty());
        assert!(result.file_paths.exclude.is_empty());
        assert!(result.authors.include.is_empty());
        assert!(result.authors.exclude.is_empty());
        assert!(result.limit.is_none());
    }

    #[test]
    fn test_args_to_scanner_config_default() {
        let args = create_test_args();
        
        let result = args_to_scanner_config(&args, None).unwrap();
        
        // Should use default settings when no performance mode specified
        assert_eq!(result.max_memory_bytes, 64 * 1024 * 1024); // 64MB in bytes
        assert_eq!(result.queue_size, 1000);
    }

    #[test]
    fn test_args_to_scanner_config_performance_mode() {
        let args = Args {
            performance_mode: true,
            ..create_test_args()
        };
        
        let result = args_to_scanner_config(&args, None).unwrap();
        
        // Should use performance mode presets
        assert_eq!(result.max_memory_bytes, 256 * 1024 * 1024); // 256MB in bytes
        assert_eq!(result.queue_size, 5000);
    }

    #[test]
    fn test_args_to_scanner_config_conservative_mode() {
        let args = Args {
            no_performance_mode: true,
            ..create_test_args()
        };
        
        let result = args_to_scanner_config(&args, None).unwrap();
        
        // Should use conservative mode presets
        assert_eq!(result.max_memory_bytes, 32 * 1024 * 1024); // 32MB in bytes
        assert_eq!(result.queue_size, 500);
    }

    #[test]
    fn test_args_to_scanner_config_custom_memory() {
        let args = Args {
            max_memory: Some("512MB".to_string()),
            queue_size: Some(2000),
            ..create_test_args()
        };
        
        let result = args_to_scanner_config(&args, None).unwrap();
        
        // Should use custom settings
        assert_eq!(result.max_memory_bytes, 512 * 1024 * 1024); // 512MB in bytes
        assert_eq!(result.queue_size, 2000);
    }

    #[test]
    fn test_args_to_scanner_config_memory_units() {
        let test_cases = vec![
            ("1GB", 1024 * 1024 * 1024),      // 1GB in bytes
            ("1.5GB", 1536 * 1024 * 1024),    // 1.5GB in bytes  
            ("512MB", 512 * 1024 * 1024),     // 512MB in bytes
            ("2048K", 2 * 1024 * 1024),       // 2MB in bytes
            ("1T", 1024 * 1024 * 1024 * 1024), // 1TB in bytes
        ];
        
        for (memory_str, expected_bytes) in test_cases {
            let args = Args {
                repository: None,
                verbose: false,
                quiet: false,
                debug: false,
                log_format: "text".to_string(),
                log_file: None,
                log_file_level: None,
                color: false,
                no_color: false,
                compact: false,
                config_file: None,
                config_name: None,
                since: None,
                until: None,
                include_path: vec![],
                exclude_path: vec![],
                include_file: vec![],
                exclude_file: vec![],
                author: vec![],
                exclude_author: vec![],
                scan_limit: None,
                performance_mode: false,
                no_performance_mode: false,
                max_memory: Some(memory_str.to_string()),
                queue_size: None,
                command: None,
                plugin_args: Vec::new(),
                list_plugins: false,
                plugin_info: None,
                list_by_type: None,
                plugin_dir: None,
                plugins_dir: Vec::new(),
                plugin_load: None,
                plugin_exclude: None,
                plugin_directory: None,
                show_plugins: false,
                plugins_help: false,
                export_config: None,
                list_formats: false,
            };
            
            let result = args_to_scanner_config(&args, None).unwrap();
            assert_eq!(result.max_memory_bytes, expected_bytes, "Failed for input: {}", memory_str);
        }
    }

    #[test]
    fn test_merge_plugin_config_defaults() {
        let args = create_test_args();
        let config = merge_plugin_config(&args, None);
        
        // Should use expanded default directory
        assert_eq!(config.directories.len(), 1);
        let default_dir = &config.directories[0];
        assert!(default_dir.contains(".config/gstats/plugins"), 
               "Expected default directory to contain .config/gstats/plugins, got: {}", default_dir);
        assert!(config.plugin_load.is_empty());
        assert!(config.plugin_exclude.is_empty());
    }

    #[test]
    fn test_merge_plugin_config_cli_only() {
        let args = Args {
            plugins_dir: vec!["./my_plugins".to_string(), "./shared_plugins".to_string()],
            plugin_load: Some("plugin1,plugin2".to_string()),
            plugin_exclude: Some("unwanted1,unwanted2".to_string()),
            ..create_test_args()
        };
        
        let config = merge_plugin_config(&args, None);
        
        assert_eq!(config.directories, vec!["./my_plugins", "./shared_plugins"]);
        assert_eq!(config.plugin_load, vec!["plugin1", "plugin2"]);
        assert_eq!(config.plugin_exclude, vec!["unwanted1", "unwanted2"]);
    }

    #[test]
    fn test_merge_plugin_config_config_only() {
        use crate::config::{ConfigManager, Configuration};
        use std::collections::HashMap;
        
        let mut config_data = Configuration::new();
        let mut plugins_section = HashMap::new();
        plugins_section.insert("directory".to_string(), "/config/plugins".to_string());
        plugins_section.insert("plugin-load".to_string(), "[\"config_plugin1\", \"config_plugin2\"]".to_string());
        plugins_section.insert("plugin-exclude".to_string(), "[\"config_exclude1\"]".to_string());
        config_data.insert("plugins".to_string(), plugins_section);
        
        let config_manager = ConfigManager::from_config(config_data);
        let args = create_test_args();
        
        let config = merge_plugin_config(&args, Some(&config_manager));
        
        assert_eq!(config.directories, vec!["/config/plugins"]);
        assert_eq!(config.plugin_load, vec!["config_plugin1", "config_plugin2"]);
        assert_eq!(config.plugin_exclude, vec!["config_exclude1"]);
    }

    #[test]
    fn test_merge_plugin_config_cli_overrides_config() {
        use crate::config::{ConfigManager, Configuration};
        use std::collections::HashMap;
        
        let mut config_data = Configuration::new();
        let mut plugins_section = HashMap::new();
        plugins_section.insert("directory".to_string(), "/config/plugins".to_string());
        plugins_section.insert("plugin-load".to_string(), "[\"config_plugin1\", \"config_plugin2\"]".to_string());
        plugins_section.insert("plugin-exclude".to_string(), "[\"config_exclude1\"]".to_string());
        config_data.insert("plugins".to_string(), plugins_section);
        
        let config_manager = ConfigManager::from_config(config_data);
        
        let args = Args {
            plugins_dir: vec!["./cli_plugins".to_string()],
            plugin_load: Some("cli_plugin1,cli_plugin2".to_string()),
            plugin_exclude: Some("cli_exclude1".to_string()),
            ..create_test_args()
        };
        
        let config = merge_plugin_config(&args, Some(&config_manager));
        
        // CLI should override config
        assert_eq!(config.directories, vec!["/config/plugins", "./cli_plugins"]); // Combined
        assert_eq!(config.plugin_load, vec!["cli_plugin1", "cli_plugin2"]); // CLI overrides
        assert_eq!(config.plugin_exclude, vec!["cli_exclude1"]); // CLI overrides
    }

    #[test]
    fn test_parse_comma_separated() {
        assert_eq!(parse_comma_separated(""), Vec::<String>::new());
        assert_eq!(parse_comma_separated("plugin1"), vec!["plugin1"]);
        assert_eq!(parse_comma_separated("plugin1,plugin2"), vec!["plugin1", "plugin2"]);
        assert_eq!(parse_comma_separated("plugin1, plugin2, plugin3"), vec!["plugin1", "plugin2", "plugin3"]);
        assert_eq!(parse_comma_separated("plugin1,,plugin3"), vec!["plugin1", "plugin3"]); // Skip empty
    }

    #[test]
    fn test_plugin_directory_cli_with_config_integration() {
        use crate::config::{ConfigManager, Configuration};
        use std::collections::HashMap;
        
        // Create config with plugin directory
        let mut config_data = Configuration::new();
        let mut plugins_section = HashMap::new();
        plugins_section.insert("directory".to_string(), "/config/plugins".to_string());
        config_data.insert("plugins".to_string(), plugins_section);
        
        let config_manager = ConfigManager::from_config(config_data);
        
        // Test CLI plugin-directory overrides config
        let args = Args {
            plugin_directory: Some("/cli/plugins".to_string()),
            ..create_test_args()
        };
        
        let plugin_config = merge_plugin_config(&args, Some(&config_manager));
        
        // Should have both config directory and CLI directory
        assert_eq!(plugin_config.directories.len(), 2);
        assert!(plugin_config.directories.contains(&"/config/plugins".to_string()));
        assert!(plugin_config.directories.contains(&"/cli/plugins".to_string()));
    }

    #[test]
    fn test_default_plugin_directory_resolution() {
        // Test that when no directories are specified, default is ~/.config/gstats/plugins
        let args = create_test_args();
        
        let plugin_config = merge_plugin_config(&args, None);
        
        // Should resolve to ~/.config/gstats/plugins
        assert_eq!(plugin_config.directories.len(), 1);
        let default_dir = &plugin_config.directories[0];
        
        // Should contain the expanded home directory path
        assert!(default_dir.contains(".config/gstats/plugins"), 
               "Expected default directory to be ~/.config/gstats/plugins, got: {}", default_dir);
        
        // Should not be the literal "~" character
        assert!(!default_dir.starts_with("~"), 
               "Expected home directory to be expanded, got: {}", default_dir);
    }

    #[test]
    fn test_args_to_scanner_config_conflicting_performance_modes() {
        let args = Args {
            repository: None,
            verbose: false,
            quiet: false,
            debug: false,
            log_format: "text".to_string(),
            log_file: None,
            log_file_level: None,
            color: false,
            no_color: false,
            compact: false,
            config_file: None,
            config_name: None,
            since: None,
            until: None,
            include_path: vec![],
            exclude_path: vec![],
            include_file: vec![],
            exclude_file: vec![],
            author: vec![],
            exclude_author: vec![],
            scan_limit: None,
            performance_mode: true,
            no_performance_mode: true,
            max_memory: None,
            queue_size: None,
            command: None,
            plugin_args: Vec::new(),
            list_plugins: false,
            plugin_info: None,
            list_by_type: None,
            plugin_dir: None,
            plugins_dir: Vec::new(),
            plugin_load: None,
            plugin_exclude: None,
            plugin_directory: None,
            show_plugins: false,
            plugins_help: false,
            export_config: None,
            list_formats: false,
        };
        
        let result = args_to_scanner_config(&args, None);
        assert!(result.is_err());
        if let Err(CliError::ConflictingPerformanceModes) = result {
            // Expected error
        } else {
            panic!("Expected ConflictingPerformanceModes error");
        }
    }

    #[test]
    fn test_args_to_scanner_config_invalid_memory() {
        let args = Args {
            repository: None,
            verbose: false,
            quiet: false,
            debug: false,
            log_format: "text".to_string(),
            log_file: None,
            log_file_level: None,
            color: false,
            no_color: false,
            compact: false,
            config_file: None,
            config_name: None,
            since: None,
            until: None,
            include_path: vec![],
            exclude_path: vec![],
            include_file: vec![],
            exclude_file: vec![],
            author: vec![],
            exclude_author: vec![],
            scan_limit: None,
            performance_mode: false,
            no_performance_mode: false,
            max_memory: Some("invalid".to_string()),
            queue_size: None,
            command: None,
            plugin_args: Vec::new(),
            list_plugins: false,
            plugin_info: None,
            list_by_type: None,
            plugin_dir: None,
            plugins_dir: Vec::new(),
            plugin_load: None,
            plugin_exclude: None,
            plugin_directory: None,
            show_plugins: false,
            plugins_help: false,
            export_config: None,
            list_formats: false,
        };
        
        let result = args_to_scanner_config(&args, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_args_to_scanner_config_with_config() {
        use std::collections::HashMap;
        use crate::config::{ConfigManager, Configuration};
        
        // Create a config with scanner settings
        let mut config = Configuration::new();
        let mut scanner_section = HashMap::new();
        scanner_section.insert("max-memory".to_string(), "256MB".to_string());
        scanner_section.insert("queue-size".to_string(), "3000".to_string());
        config.insert("scanner".to_string(), scanner_section);
        
        let config_manager = ConfigManager::from_config(config);
        
        let args = create_test_args();
        let result = args_to_scanner_config(&args, Some(&config_manager)).unwrap();
        
        // Should use config values
        assert_eq!(result.max_memory_bytes, 256 * 1024 * 1024); // 256MB from config
        assert_eq!(result.queue_size, 3000); // 3000 from config
    }

    #[test]
    fn test_args_to_scanner_config_cli_overrides_config() {
        use std::collections::HashMap;
        use crate::config::{ConfigManager, Configuration};
        
        // Create a config with scanner settings
        let mut config = Configuration::new();
        let mut scanner_section = HashMap::new();
        scanner_section.insert("max-memory".to_string(), "128MB".to_string());
        scanner_section.insert("queue-size".to_string(), "2000".to_string());
        config.insert("scanner".to_string(), scanner_section);
        
        let config_manager = ConfigManager::from_config(config);
        
        // CLI args that should override config
        let args = Args {
            max_memory: Some("512MB".to_string()),
            queue_size: Some(4000),
            ..create_test_args()
        };
        
        let result = args_to_scanner_config(&args, Some(&config_manager)).unwrap();
        
        // Should use CLI values (override config)
        assert_eq!(result.max_memory_bytes, 512 * 1024 * 1024); // 512MB from CLI
        assert_eq!(result.queue_size, 4000); // 4000 from CLI
    }
}
