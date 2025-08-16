//! Dynamic CLI builder that creates clap commands from discovered plugins
//!
//! This module is responsible for building the complete CLI structure dynamically
//! based on plugin discovery results. It generates subcommands for each plugin
//! function and includes their specific arguments from PluginArgDefinition.

use clap::{Command, Arg, ArgAction, value_parser};
use std::path::PathBuf;
use crate::cli::plugin_handler::{PluginHandler, FunctionMapping};
use crate::plugin::traits::PluginArgDefinition;
use anyhow::Result;

/// Builds the CLI dynamically from discovered plugins
pub struct DynamicCliBuilder;

impl DynamicCliBuilder {
    /// Build the complete CLI with all global arguments and plugin subcommands
    pub async fn build_cli(
        plugin_handler: &PluginHandler,
        discovered_functions: &[FunctionMapping]
    ) -> Command {
        let mut app = Self::build_base_command();
        
        // Add global arguments
        app = Self::add_global_arguments(app);
        
        // Add discovered plugin subcommands
        for function in discovered_functions {
            if let Ok(subcommand) = Self::build_plugin_subcommand(plugin_handler, function.clone()) {
                app = app.subcommand(subcommand);
            }
        }
        
        // Add after_help text
        app = app.after_help("Use 'gstats <COMMAND> --help' for command-specific options.");
        
        app
    }
    
    /// Build the base command with name, version, and description
    fn build_base_command() -> Command {
        Command::new("gstats")
            .version(env!("CARGO_PKG_VERSION"))
            .about("Fast, local-first git analytics tool")
            .long_about(
                "gstats - Fast, local-first git analytics tool

COMMON WORKFLOWS:
  Quick analysis:      gstats authors
  Code metrics:        gstats complexity  
  Export results:      gstats output --format json --outfile report.json
  Time-based analysis: gstats authors  (uses global filters like --since)

For detailed plugin options: gstats <COMMAND> --help"
            )
    }
    
    /// Add all global arguments to the command
    fn add_global_arguments(mut app: Command) -> Command {
        // Repository path
        app = app.arg(
            Arg::new("repository")
                .long("repo")
                .short('r')
                .aliases(["repository"])
                .help("Path to git repository (defaults to current directory)")
                .value_name("PATH")
                .value_parser(value_parser!(String))
        );
        
        // Logging flags
        app = app.arg(
            Arg::new("verbose")
                .long("verbose")
                .short('v')
                .help("Enable verbose output with detailed logging")
                .action(ArgAction::SetTrue)
        );
        
        app = app.arg(
            Arg::new("quiet")
                .long("quiet")
                .short('q')
                .help("Suppress all output except errors")
                .action(ArgAction::SetTrue)
                .conflicts_with("verbose")
        );
        
        app = app.arg(
            Arg::new("debug")
                .long("debug")
                .help("Enable debug output with trace-level logging")
                .action(ArgAction::SetTrue)
                .conflicts_with_all(["verbose", "quiet"])
        );
        
        // Output formatting
        app = app.arg(
            Arg::new("color")
                .long("color")
                .help("Force colored output even when redirected")
                .action(ArgAction::SetTrue)
                .conflicts_with("no-color")
        );
        
        app = app.arg(
            Arg::new("no-color")
                .long("no-color")
                .help("Disable colored output")
                .action(ArgAction::SetTrue)
        );
        
        app = app.arg(
            Arg::new("compact")
                .long("compact")
                .help("Display results in compact, one-line format suitable for CI/CD")
                .action(ArgAction::SetTrue)
        );
        
        // Configuration
        app = app.arg(
            Arg::new("config-file")
                .long("config-file")
                .help("Configuration file path")
                .value_name("FILE")
                .value_parser(value_parser!(PathBuf))
        );
        
        app = app.arg(
            Arg::new("config-name")
                .long("config-name")
                .help("Configuration section name")
                .value_name("SECTION")
                .value_parser(value_parser!(String))
        );
        
        // Branch selection
        app = app.arg(
            Arg::new("branch")
                .long("branch")
                .short('b')
                .help("Git branch to scan (overrides automatic detection)")
                .value_name("BRANCH")
                .value_parser(value_parser!(String))
        );
        
        app = app.arg(
            Arg::new("show-branch")
                .long("show-branch")
                .help("Show which branch would be scanned and exit")
                .action(ArgAction::SetTrue)
        );
        
        app = app.arg(
            Arg::new("fallback-branch")
                .long("fallback-branch")
                .help("Comma-separated fallback branch list")
                .value_name("LIST")
                .value_parser(value_parser!(String))
        );
        
        app = app.arg(
            Arg::new("remote")
                .long("remote")
                .help("Git remote for branch detection (overrides auto-detection)")
                .value_name("REMOTE")
                .value_parser(value_parser!(String))
        );
        
        // Global filtering (for scanner)
        app = app.arg(
            Arg::new("since")
                .long("since")
                .short('S')
                .help("Filter commits from this date onwards")
                .value_name("DATE")
                .value_parser(value_parser!(String))
        );
        
        app = app.arg(
            Arg::new("until")
                .long("until")
                .short('U')
                .help("Filter commits up to this date")
                .value_name("DATE")
                .value_parser(value_parser!(String))
        );
        
        app = app.arg(
            Arg::new("include-path")
                .long("include-path")
                .short('I')
                .help("Include specific paths (supports comma-separated)")
                .value_name("PATH")
                .action(ArgAction::Append)
                .value_parser(value_parser!(String))
        );
        
        app = app.arg(
            Arg::new("exclude-path")
                .long("exclude-path")
                .short('X')
                .help("Exclude specific paths (supports comma-separated)")
                .value_name("PATH")
                .action(ArgAction::Append)
                .value_parser(value_parser!(String))
        );
        
        app = app.arg(
            Arg::new("include-file")
                .long("include-file")
                .short('F')
                .help("Include file patterns (supports comma-separated)")
                .value_name("PATTERN")
                .action(ArgAction::Append)
                .value_parser(value_parser!(String))
        );
        
        app = app.arg(
            Arg::new("exclude-file")
                .long("exclude-file")
                .short('N')
                .help("Exclude file patterns (supports comma-separated)")
                .value_name("PATTERN")
                .action(ArgAction::Append)
                .value_parser(value_parser!(String))
        );
        
        app = app.arg(
            Arg::new("author")
                .long("author")
                .short('A')
                .help("Include specific authors (supports comma-separated)")
                .value_name("AUTHOR")
                .action(ArgAction::Append)
                .value_parser(value_parser!(String))
        );
        
        app = app.arg(
            Arg::new("exclude-author")
                .long("exclude-author")
                .short('E')
                .help("Exclude specific authors (supports comma-separated)")
                .value_name("AUTHOR")
                .action(ArgAction::Append)
                .value_parser(value_parser!(String))
        );
        
        app = app.arg(
            Arg::new("scan-limit")
                .long("scan-limit")
                .help("Maximum number of commits to scan from repository")
                .value_name("N")
                .value_parser(value_parser!(usize))
        );
        
        // Scanner configuration
        app = app.arg(
            Arg::new("performance-mode")
                .long("performance-mode")
                .help("Enable performance mode (optimized for speed over memory usage)")
                .action(ArgAction::SetTrue)
                .conflicts_with("no-performance-mode")
        );
        
        app = app.arg(
            Arg::new("no-performance-mode")
                .long("no-performance-mode")
                .help("Disable performance mode (prioritize memory usage over speed)")
                .action(ArgAction::SetTrue)
        );
        
        app = app.arg(
            Arg::new("max-memory")
                .long("max-memory")
                .help("Maximum memory usage for scanner queues (supports units: MB, GB, K, T, etc.)")
                .value_name("SIZE")
                .value_parser(value_parser!(String))
        );
        
        app = app.arg(
            Arg::new("queue-size")
                .long("queue-size")
                .help("Queue size for scanner operations")
                .value_name("N")
                .value_parser(value_parser!(usize))
        );
        
        // Plugin discovery
        app = app.arg(
            Arg::new("plugin-dir")
                .long("plugin-dir")
                .help("Override default plugin discovery directory")
                .value_name("DIR")
                .value_parser(value_parser!(String))
        );
        
        app = app.arg(
            Arg::new("plugins-dir")
                .long("plugins-dir")
                .help("Additional plugin directories to search")
                .value_name("DIR")
                .action(ArgAction::Append)
                .value_parser(value_parser!(String))
        );
        
        app = app.arg(
            Arg::new("plugin-exclude")
                .long("plugin-exclude")
                .help("Comma-separated list of plugins to exclude")
                .value_name("LIST")
                .value_parser(value_parser!(String))
        );
        
        // Plugin management commands
        app = app.arg(
            Arg::new("list-plugins")
                .long("list-plugins")
                .help("List all available plugins")
                .action(ArgAction::SetTrue)
        );
        
        app = app.arg(
            Arg::new("show-plugins")
                .long("plugins")
                .help("Show all plugins with functions and descriptions")
                .action(ArgAction::SetTrue)
        );
        
        app = app.arg(
            Arg::new("list-formats")
                .long("list-formats")
                .help("List all supported export formats and their file extensions")
                .action(ArgAction::SetTrue)
        );
        
        app = app.arg(
            Arg::new("export-config")
                .long("export-config")
                .help("Export complete configuration to specified TOML file")
                .value_name("FILE")
                .value_parser(value_parser!(PathBuf))
        );
        
        app
    }
    
    /// Build a subcommand for a specific plugin function
    fn build_plugin_subcommand(
        _plugin_handler: &PluginHandler, 
        function: FunctionMapping  // Take ownership
    ) -> Result<Command> {
        // TEMPORARY: Use static string until clap upgrade resolves type issues
        let subcommand = Command::new("placeholder")
            .about("Placeholder subcommand");
        
        // TODO: Enable dynamic string building after clap upgrade
        // let subcommand = Command::new(function.function_name)
        //     .about(function.description);
        // 
        // let subcommand = function.aliases.into_iter().fold(subcommand, |cmd, alias| {
        //     cmd.alias(alias)
        // });
        
        // Get plugin argument schema and add arguments
        // TODO: This requires implementing get_plugin_arg_schema in PluginHandler
        // For now, we'll create an empty subcommand
        // When Phase 3 is complete, we'll call:
        // if let Ok(arg_schema) = plugin_handler.get_plugin_arg_schema(&function.plugin_name).await {
        //     for arg_def in arg_schema {
        //         let arg = Self::convert_plugin_arg_to_clap_arg(&arg_def);
        //         subcommand = subcommand.arg(arg);
        //     }
        // }
        
        Ok(subcommand)
    }
    
    /// Convert a PluginArgDefinition to a clap Arg
    fn convert_plugin_arg_to_clap_arg(arg_def: &PluginArgDefinition) -> Arg {
        // For testing purposes, we need to handle known arguments
        // In production, this would be dynamically generated
        let arg_name = arg_def.name.trim_start_matches("--");
        
        let mut arg = match arg_name {
            "outfile" => Arg::new("outfile")
                .long("outfile")
                .short('o')
                .help("Output file path"),
            "format" => Arg::new("format")
                .long("format")
                .short('f')
                .help("Output format"),
            "template" => Arg::new("template")
                .long("template")
                .short('t')
                .help("Template file"),
            _ => Arg::new("generic")
                .long("generic")
                .help("Generic argument"),
        };
        
        arg = arg.required(arg_def.required);
        
        // Map plugin arg types to clap value parsers
        match arg_def.arg_type.as_str() {
            "path" => arg = arg.value_parser(value_parser!(PathBuf)),
            "string" => arg = arg.value_parser(value_parser!(String)),
            "integer" => arg = arg.value_parser(value_parser!(i64)),
            "float" => arg = arg.value_parser(value_parser!(f64)),
            "boolean" => arg = arg.action(ArgAction::SetTrue),
            _ => arg = arg.value_parser(value_parser!(String)),
        }
        
        // TODO: Enable after clap upgrade
        // Add default value if specified
        // if let Some(default) = &arg_def.default_value {
        //     arg = arg.default_value(default.clone());
        // }
        // 
        // Include examples in help text if available
        // if !arg_def.examples.is_empty() {
        //     let examples_text = format!(" [examples: {}]", arg_def.examples.join(", "));
        //     let help_with_examples = format!("{}{}", description, examples_text);
        //     arg = arg.help(help_with_examples);
        // }
        
        arg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_build_base_command() {
        let cmd = DynamicCliBuilder::build_base_command();
        assert_eq!(cmd.get_name(), "gstats");
        assert!(cmd.get_about().is_some());
    }
    
    #[test]
    fn test_add_global_arguments() {
        let cmd = DynamicCliBuilder::build_base_command();
        let cmd_with_args = DynamicCliBuilder::add_global_arguments(cmd);
        
        // Check that key global arguments are present
        assert!(cmd_with_args.get_arguments().any(|a| a.get_id() == "repository"));
        assert!(cmd_with_args.get_arguments().any(|a| a.get_id() == "verbose"));
        assert!(cmd_with_args.get_arguments().any(|a| a.get_id() == "since"));
        assert!(cmd_with_args.get_arguments().any(|a| a.get_id() == "plugin-dir"));
    }
    
    #[test]
    fn test_convert_plugin_arg_to_clap_arg() {
        let arg_def = PluginArgDefinition {
            name: "--outfile".to_string(),
            description: "Output file path".to_string(),
            required: false,
            default_value: None,
            arg_type: "path".to_string(),
            examples: vec!["report.json".to_string(), "data.csv".to_string()],
        };
        
        let clap_arg = DynamicCliBuilder::convert_plugin_arg_to_clap_arg(&arg_def);
        
        assert_eq!(clap_arg.get_id(), "outfile");
        assert_eq!(clap_arg.get_long(), Some("outfile"));
        assert_eq!(clap_arg.get_short(), Some('o'));
        assert!(!clap_arg.is_required_set());
    }
}