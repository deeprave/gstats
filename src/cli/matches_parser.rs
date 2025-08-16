//! Parser for extracting structured data from clap ArgMatches
//!
//! This module converts the untyped ArgMatches from clap's dynamic command
//! parsing into the structured Args type that the rest of the application expects.

use clap::ArgMatches;
use std::path::PathBuf;
use crate::cli::enhanced_parser::EnhancedParser;

/// Parsed arguments extracted from ArgMatches
/// 
/// This is a temporary structure used during the conversion from
/// ArgMatches to the full Args struct.
#[derive(Debug, Clone)]
pub struct ParsedArgs {
    pub global_args: GlobalArgs,
    pub subcommand_info: Option<SubcommandInfo>,
}

/// All global arguments extracted from ArgMatches
#[derive(Debug, Clone)]
pub struct GlobalArgs {
    // Repository and configuration
    pub repository: Option<String>,
    pub config_file: Option<PathBuf>,
    
    // Logging
    pub verbose: bool,
    pub quiet: bool,
    pub debug: bool,
    pub log_format: String,
    pub log_file: Option<PathBuf>,
    pub log_file_level: Option<String>,
    
    // Output formatting
    pub color: bool,
    pub no_color: bool,
    pub compact: bool,
    
    // Branch selection
    pub branch: Option<String>,
    pub show_branch: bool,
    pub fallback_branch: Option<String>,
    pub remote: Option<String>,
    
    // Filtering (global scanner filters)
    pub since: Option<String>,
    pub until: Option<String>,
    pub include_path: Vec<String>,
    pub exclude_path: Vec<String>,
    pub include_file: Vec<String>,
    pub exclude_file: Vec<String>,
    pub author: Vec<String>,
    pub exclude_author: Vec<String>,
    pub scan_limit: Option<usize>,
    
    // Scanner configuration
    pub performance_mode: bool,
    pub no_performance_mode: bool,
    pub max_memory: Option<String>,
    pub queue_size: Option<usize>,
    
    // Plugin management
    pub plugin_dir: Option<String>,
    pub plugins_dir: Vec<String>,
    pub plugin_exclude: Option<String>,
    pub list_plugins: bool,
    pub show_plugins: bool,
    pub list_formats: bool,
    pub export_config: Option<PathBuf>,
}

/// Information about the subcommand that was invoked
#[derive(Debug, Clone)]
pub struct SubcommandInfo {
    pub function_name: String,
    pub plugin_args: Vec<String>,
}

impl ParsedArgs {
    /// Extract structured arguments from clap ArgMatches
    pub fn from_matches(matches: &ArgMatches) -> anyhow::Result<Self> {
        let global_args = GlobalArgs::from_matches(matches)?;
        let subcommand_info = SubcommandInfo::from_matches(matches)?;
        
        Ok(ParsedArgs {
            global_args,
            subcommand_info,
        })
    }
}

impl GlobalArgs {
    /// Extract all global arguments from ArgMatches
    fn from_matches(matches: &ArgMatches) -> anyhow::Result<Self> {
        Ok(GlobalArgs {
            // Repository and configuration
            repository: matches.get_one::<String>("repository").cloned(),
            config_file: matches.get_one::<PathBuf>("config-file").cloned(),
            
            // Logging
            verbose: matches.get_flag("verbose"),
            quiet: matches.get_flag("quiet"),
            debug: matches.get_flag("debug"),
            log_format: matches.get_one::<String>("log-format")
                .cloned()
                .unwrap_or_else(|| "text".to_string()),
            log_file: matches.get_one::<PathBuf>("log-file").cloned(),
            log_file_level: matches.get_one::<String>("log-file-level").cloned(),
            
            // Output formatting
            color: matches.get_flag("color"),
            no_color: matches.get_flag("no-color"),
            compact: matches.get_flag("compact"),
            
            // Branch selection
            branch: matches.get_one::<String>("branch").cloned(),
            show_branch: matches.get_flag("show-branch"),
            fallback_branch: matches.get_one::<String>("fallback-branch").cloned(),
            remote: matches.get_one::<String>("remote").cloned(),
            
            // Filtering
            since: matches.get_one::<String>("since").cloned(),
            until: matches.get_one::<String>("until").cloned(),
            include_path: Self::get_string_vec(matches, "include-path"),
            exclude_path: Self::get_string_vec(matches, "exclude-path"),
            include_file: Self::get_string_vec(matches, "include-file"),
            exclude_file: Self::get_string_vec(matches, "exclude-file"),
            author: Self::get_string_vec(matches, "author"),
            exclude_author: Self::get_string_vec(matches, "exclude-author"),
            scan_limit: matches.get_one::<usize>("scan-limit").copied(),
            
            // Scanner configuration
            performance_mode: matches.get_flag("performance-mode"),
            no_performance_mode: matches.get_flag("no-performance-mode"),
            max_memory: matches.get_one::<String>("max-memory").cloned(),
            queue_size: matches.get_one::<usize>("queue-size").copied(),
            
            // Plugin management
            plugin_dir: matches.get_one::<String>("plugin-dir").cloned(),
            plugins_dir: Self::get_string_vec(matches, "plugins-dir"),
            plugin_exclude: matches.get_one::<String>("plugin-exclude").cloned(),
            list_plugins: matches.get_flag("list-plugins"),
            show_plugins: matches.get_flag("show-plugins"),
            list_formats: matches.get_flag("list-formats"),
            export_config: matches.get_one::<PathBuf>("export-config").cloned(),
        })
    }
    
    /// Helper to extract multiple string values from an argument
    fn get_string_vec(matches: &ArgMatches, arg_name: &str) -> Vec<String> {
        matches.get_many::<String>(arg_name)
            .map(|values| values.cloned().collect())
            .unwrap_or_default()
    }
    
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

impl SubcommandInfo {
    /// Extract subcommand information from ArgMatches
    fn from_matches(matches: &ArgMatches) -> anyhow::Result<Option<Self>> {
        if let Some((subcommand_name, subcommand_matches)) = matches.subcommand() {
            let plugin_args = Self::convert_subcommand_to_plugin_args(subcommand_matches)?;
            
            Ok(Some(SubcommandInfo {
                function_name: subcommand_name.to_string(),
                plugin_args,
            }))
        } else {
            Ok(None)
        }
    }
    
    /// Convert subcommand ArgMatches back to Vec<String> for plugin compatibility
    /// 
    /// This maintains backward compatibility with the existing plugin argument
    /// parsing interface that expects a Vec<String> of arguments.
    fn convert_subcommand_to_plugin_args(matches: &ArgMatches) -> anyhow::Result<Vec<String>> {
        let mut plugin_args = Vec::new();
        
        // Iterate through all argument IDs that have values
        for arg_id in matches.ids() {
            let arg_name = arg_id.as_str();
            
            // Skip the help flag as it's handled by clap
            if arg_name == "help" {
                continue;
            }
            
            // Check if it's a flag (boolean)
            if matches.try_contains_id(arg_name).unwrap_or(false) && 
               matches.value_source(arg_name).is_some() {
                if let Ok(flag_value) = matches.try_get_one::<bool>(arg_name) {
                    if flag_value == Some(&true) {
                        plugin_args.push(format!("--{}", arg_name));
                    }
                    continue;
                }
            }
            
            // Check for string values
            if let Some(values) = matches.get_many::<String>(arg_name) {
                plugin_args.push(format!("--{}", arg_name));
                for value in values {
                    plugin_args.push(value.clone());
                }
            } 
            // Check for PathBuf values
            else if let Some(values) = matches.get_many::<PathBuf>(arg_name) {
                plugin_args.push(format!("--{}", arg_name));
                for value in values {
                    plugin_args.push(value.to_string_lossy().to_string());
                }
            }
            // Check for numeric values
            else if let Some(value) = matches.get_one::<i64>(arg_name) {
                plugin_args.push(format!("--{}", arg_name));
                plugin_args.push(value.to_string());
            }
        }
        
        Ok(plugin_args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Command, Arg, ArgAction, value_parser};
    
    fn create_test_command() -> Command {
        Command::new("test")
            .arg(Arg::new("verbose").long("verbose").action(ArgAction::SetTrue))
            .arg(Arg::new("repository").long("repo").value_parser(value_parser!(String)))
            .arg(Arg::new("since").long("since").value_parser(value_parser!(String)))
            .subcommand(
                Command::new("output")
                    .arg(Arg::new("outfile").long("outfile").value_parser(value_parser!(PathBuf)))
                    .arg(Arg::new("format").long("format").value_parser(value_parser!(String)))
            )
    }
    
    #[test]
    fn test_parse_global_args() {
        let cmd = create_test_command();
        let matches = cmd.try_get_matches_from(vec![
            "test", "--verbose", "--repo", "/path/to/repo", "--since", "1 week"
        ]).unwrap();
        
        let global_args = GlobalArgs::from_matches(&matches).unwrap();
        assert!(global_args.verbose);
        assert_eq!(global_args.repository, Some("/path/to/repo".to_string()));
        assert_eq!(global_args.since, Some("1 week".to_string()));
    }
    
    #[test]
    fn test_parse_subcommand() {
        let cmd = create_test_command();
        let matches = cmd.try_get_matches_from(vec![
            "test", "output", "--outfile", "report.json", "--format", "json"
        ]).unwrap();
        
        let subcommand_info = SubcommandInfo::from_matches(&matches).unwrap();
        assert!(subcommand_info.is_some());
        
        let info = subcommand_info.unwrap();
        assert_eq!(info.function_name, "output");
        assert!(info.plugin_args.contains(&"--outfile".to_string()));
        assert!(info.plugin_args.contains(&"report.json".to_string()));
        assert!(info.plugin_args.contains(&"--format".to_string()));
        assert!(info.plugin_args.contains(&"json".to_string()));
    }
    
    #[test]
    fn test_enhanced_parsing() {
        let mut global_args = GlobalArgs {
            include_path: vec!["src/,tests/".to_string()],
            author: vec!["john@example.com,jane@example.com".to_string()],
            ..Default::default()
        };
        
        global_args = global_args.apply_enhanced_parsing();
        
        assert_eq!(global_args.include_path, vec!["src/", "tests/"]);
        assert_eq!(global_args.author, vec!["john@example.com", "jane@example.com"]);
    }
}

// Implement Default for testing
impl Default for GlobalArgs {
    fn default() -> Self {
        GlobalArgs {
            repository: None,
            config_file: None,
            verbose: false,
            quiet: false,
            debug: false,
            log_format: "text".to_string(),
            log_file: None,
            log_file_level: None,
            color: false,
            no_color: false,
            compact: false,
            branch: None,
            show_branch: false,
            fallback_branch: None,
            remote: None,
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
            plugin_dir: None,
            plugins_dir: Vec::new(),
            plugin_exclude: None,
            list_plugins: false,
            show_plugins: false,
            list_formats: false,
            export_config: None,
        }
    }
}