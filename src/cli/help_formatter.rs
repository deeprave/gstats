//! Enhanced help formatter with colors and improved layout
//!
//! Provides better formatted help text with color coding and plugin/function tables.

use crate::display::{ColourManager, ColourConfig};
use std::fmt::Write;

/// Enhanced help formatter for CLI output
pub struct HelpFormatter {
    colour_manager: ColourManager,
}

impl HelpFormatter {
    /// Create a new help formatter with the given color configuration
    pub fn new(color_config: Option<ColourConfig>) -> Self {
        let colour_manager = if let Some(config) = color_config {
            ColourManager::with_config(config)
        } else {
            ColourManager::new()
        };
        
        Self { colour_manager }
    }
    
    /// Create a help formatter that respects the --no-color flag
    pub fn from_no_color_flag(no_color: bool) -> Self {
        let colour_manager = ColourManager::from_args_and_config(no_color, None);
        Self { colour_manager }
    }
    
    /// Create a help formatter that respects both --color and --no-color flags
    pub fn from_color_flags(no_color: bool, color: bool) -> Self {
        let colour_manager = ColourManager::from_color_args(no_color, color, None);
        Self { colour_manager }
    }
    
    /// Format the main help text with colors and improved layout
    pub fn format_main_help(&self) -> String {
        let mut output = String::new();
        
        // Header
        writeln!(output, "{}", self.colour_manager.highlight("gstats - Fast, local-first git analytics tool")).unwrap();
        writeln!(output).unwrap();
        
        // Usage
        writeln!(output, "{}", self.colour_manager.info("USAGE:")).unwrap();
        writeln!(output, "    {} {} {}", 
                self.colour_manager.command("gstats"),
                self.colour_manager.success("[OPTIONS]"),
                self.colour_manager.highlight("<COMMAND>")).unwrap();
        writeln!(output).unwrap();
        
        // Plugin/Function table
        writeln!(output, "{}", self.colour_manager.info("AVAILABLE PLUGINS AND FUNCTIONS:")).unwrap();
        self.write_plugin_table(&mut output);
        writeln!(output).unwrap();
        
        // Main options
        writeln!(output, "{}", self.colour_manager.info("OPTIONS:")).unwrap();
        self.write_main_options(&mut output);
        writeln!(output).unwrap();
        
        // Filtering options
        writeln!(output, "{}", self.colour_manager.info("FILTERING OPTIONS:")).unwrap();
        self.write_filtering_options(&mut output);
        writeln!(output).unwrap();
        
        // Performance options
        writeln!(output, "{}", self.colour_manager.info("PERFORMANCE OPTIONS:")).unwrap();
        self.write_performance_options(&mut output);
        writeln!(output).unwrap();
        
        // Plugin discovery options
        writeln!(output, "{}", self.colour_manager.info("PLUGIN DISCOVERY:")).unwrap();
        self.write_plugin_options(&mut output);
        writeln!(output).unwrap();
        
        // Usage examples
        writeln!(output, "{}", self.colour_manager.info("USAGE EXAMPLES:")).unwrap();
        self.write_usage_examples(&mut output);
        
        output
    }
    
    /// Format an option with consistent alignment and colors
    fn format_option_line(&self, flag_part: &str, arg_part: &str, description: &str) -> String {
        let colored_flag = self.colour_manager.success(flag_part);
        let colored_arg = if arg_part.is_empty() {
            String::new()
        } else {
            format!(" {}", self.colour_manager.highlight(arg_part))
        };
        
        // Calculate the plain text width for alignment (without ANSI codes)
        let plain_text_width = flag_part.len() + if arg_part.is_empty() { 0 } else { 1 + arg_part.len() };
        let padding_needed = if plain_text_width >= 35 { 1 } else { 35 - plain_text_width };
        
        format!("    {}{}{}{}", 
                colored_flag, 
                colored_arg, 
                " ".repeat(padding_needed),
                description)
    }
    
    /// Parse option string into flag and arg parts 
    fn parse_option(&self, option_str: &str) -> (String, String) {
        if let Some(space_pos) = option_str.find(' ') {
            let flag_part = option_str[..space_pos].to_string();
            let arg_part = option_str[space_pos + 1..].to_string();
            (flag_part, arg_part)
        } else {
            (option_str.to_string(), String::new())
        }
    }

    /// Write the plugin/function table
    /// Write dynamic plugin table using the same format as --plugins command
    fn write_dynamic_plugin_table(&self, output: &mut String, mappings: &[crate::cli::plugin_handler::FunctionMapping]) {
        use std::collections::HashMap;
        
        // Group functions by plugin
        let mut plugins_map: HashMap<String, Vec<_>> = HashMap::new();
        for mapping in mappings {
            plugins_map.entry(mapping.plugin_name.clone())
                .or_insert_with(Vec::new)
                .push(mapping);
        }
        
        // Sort plugins by name
        let mut plugin_names: Vec<_> = plugins_map.keys().collect();
        plugin_names.sort();
        
        // First pass: collect all data and calculate max widths
        let mut all_plugin_data = Vec::new();
        let mut max_function_width = "Function".len();
        let mut max_aliases_width = "Aliases".len();
        
        for plugin_name in &plugin_names {
            let plugin_functions = &plugins_map[*plugin_name];
            
            // Sort functions within plugin (default first, then alphabetically)
            let mut sorted_functions = plugin_functions.clone();
            sorted_functions.sort_by(|a, b| {
                match (a.is_default, b.is_default) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.function_name.cmp(&b.function_name),
                }
            });
            
            let mut plugin_rows = Vec::new();
            for function in sorted_functions {
                let aliases_str = if function.aliases.is_empty() {
                    "—".to_string() // Em dash for no aliases
                } else {
                    function.aliases.join(", ")
                };
                
                // Format function name with default highlighting
                let function_display = if function.is_default {
                    format!("{} (default)", function.function_name)
                } else {
                    function.function_name.clone()
                };
                
                // Track maximum widths (without color codes)
                max_function_width = max_function_width.max(function_display.len());
                max_aliases_width = max_aliases_width.max(aliases_str.len());
                
                plugin_rows.push((function_display, aliases_str, function.description.clone()));
            }
            
            all_plugin_data.push((plugin_name, plugin_rows));
        }
        
        // Generate output
        for (i, (plugin_name, plugin_rows)) in all_plugin_data.iter().enumerate() {
            if i > 0 {
                writeln!(output).unwrap(); // Single line between plugins
            }
            
            writeln!(output, "{}: {}",
                self.colour_manager.info("Plugin"),
                self.colour_manager.success(plugin_name)).unwrap();
            
            // Create table data without colors for proper alignment
            let mut table_lines = Vec::new();
            
            // Header
            table_lines.push(format!("  {:<width_fn$} {:<width_al$} {}",
                "Function", "Aliases", "Description",
                width_fn = max_function_width,
                width_al = max_aliases_width));
            
            // Separator line
            table_lines.push(format!("  {:-<width_fn$} {:-<width_al$} {}",
                "", "", "---",
                width_fn = max_function_width,
                width_al = max_aliases_width));
            
            // Data rows
            for (function_display, aliases_str, description) in plugin_rows {
                table_lines.push(format!("  {:<width_fn$} {:<width_al$} {}",
                    function_display, aliases_str, description,
                    width_fn = max_function_width,
                    width_al = max_aliases_width));
            }
            
            for line in table_lines {
                writeln!(output, "{}", line).unwrap();
            }
        }
        
        writeln!(output).unwrap();
        writeln!(output, "{}:",
            self.colour_manager.info("Usage Examples")).unwrap();
        writeln!(output, "  {} {}                {}",
            self.colour_manager.command("gstats"),
            self.colour_manager.success("<plugin>"),
            "Use plugin's default function").unwrap();
        writeln!(output, "  {} {}     {}",
            self.colour_manager.command("gstats"),
            self.colour_manager.success("<plugin>:<function>"),
            "Use specific plugin function").unwrap();
        writeln!(output).unwrap();
        write!(output, "{} = default function for plugin",
            self.colour_manager.orange("(default)")).unwrap();
    }
    
    /// Write static plugin table (fallback)
    fn write_plugin_table(&self, output: &mut String) {
        // Table header with colors
        writeln!(output, "    ┌─────────┬────────────────────────────────────────────────────────┐").unwrap();
        writeln!(output, "    │ {} │ {}                                              │", 
                self.colour_manager.highlight("Plugin "),
                self.colour_manager.highlight("Functions")).unwrap();
        writeln!(output, "    ├─────────┼────────────────────────────────────────────────────────┤").unwrap();
        
        // Plugin rows
        let plugins = vec![
            ("commits", "authors, contributors, committers, commits, history"),
            ("metrics", "metrics, complexity, quality"),
            ("export", "export, json, csv, xml"),
        ];
        
        for (plugin, functions) in plugins {
            writeln!(output, "    │ {} │ {:<54} │", 
                    self.colour_manager.success(&format!("{:<7}", plugin)),
                    functions).unwrap();
        }
        
        writeln!(output, "    └─────────┴────────────────────────────────────────────────────────┘").unwrap();
    }
    
    /// Write main options
    fn write_main_options(&self, output: &mut String) {
        let options = vec![
            ("-r, --repo <PATH>", "Repository path (default: current directory)"),
            ("-v, --verbose", "Verbose output (debug level logging)"),
            ("-q, --quiet", "Quiet output (errors only)"),
            ("--debug", "Debug output (trace level logging)"),
            ("--color", "Force colored output even when redirected"),
            ("--no-color", "Disable colored output"),
            ("--log-format <FORMAT>", "Log format: text or json [default: text]"),
            ("--log-file <FILE>", "Log file path for file output"),
            ("--config-file <FILE>", "Configuration file path"),
            ("-h, --help", "Print help information"),
            ("-V, --version", "Print version information"),
        ];
        
        for (option, desc) in options {
            let (flag_part, arg_part) = self.parse_option(option);
            writeln!(output, "{}", self.format_option_line(&flag_part, &arg_part, desc)).unwrap();
        }
    }
    
    /// Write filtering options
    fn write_filtering_options(&self, output: &mut String) {
        let options = vec![
            ("-S, --since <DATE>", "Start date filter (ISO 8601 or relative like '1 week ago')"),
            ("-U, --until <DATE>", "End date filter (ISO 8601 or relative like 'yesterday')"),
            ("-I, --include-path <PATH>", "Include specific paths (supports comma-separated)"),
            ("-X, --exclude-path <PATH>", "Exclude specific paths (supports comma-separated)"),
            ("-F, --include-file <PATTERN>", "Include file patterns (supports comma-separated)"),
            ("--exclude-file <PATTERN>", "Skip files matching these patterns"),
            ("--author <AUTHOR>", "Filter commits by author name or email"),
            ("--exclude-author <AUTHOR>", "Exclude commits by author name or email"),
        ];
        
        for (option, desc) in options {
            let (flag_part, arg_part) = self.parse_option(option);
            writeln!(output, "{}", self.format_option_line(&flag_part, &arg_part, desc)).unwrap();
        }
    }
    
    /// Write performance options
    fn write_performance_options(&self, output: &mut String) {
        let options = vec![
            ("--performance-mode", "Enable performance mode (optimized for speed over memory)"),
            ("--no-performance-mode", "Disable performance mode (prioritize memory over speed)"),
            ("--max-memory <SIZE>", "Maximum memory usage (supports units: MB, GB, K, T)"),
            ("--queue-size <N>", "Queue size for scanner operations"),
        ];
        
        for (option, desc) in options {
            let (flag_part, arg_part) = self.parse_option(option);
            writeln!(output, "{}", self.format_option_line(&flag_part, &arg_part, desc)).unwrap();
        }
    }
    
    /// Write plugin discovery options
    fn write_plugin_options(&self, output: &mut String) {
        let options = vec![
            ("--list-plugins", "List all available plugins"),
            ("--plugins", "Show all plugins with functions and descriptions"),
            ("--plugins-help", "Show detailed plugin functions and command mappings"),
            ("--plugin-info <PLUGIN>", "Show detailed information about specific plugin"),
            ("--check-plugin <PLUGIN>", "Check plugin compatibility with current API"),
            ("--list-by-type <TYPE>", "List plugins by type (scanner, output, etc.)"),
        ];
        
        for (option, desc) in options {
            let (flag_part, arg_part) = self.parse_option(option);
            writeln!(output, "{}", self.format_option_line(&flag_part, &arg_part, desc)).unwrap();
        }
    }

    /// Write usage examples with consistent alignment
    fn write_usage_examples(&self, output: &mut String) {
        let examples = vec![
            ("gstats commits", "Use plugin's default function"),
            ("gstats authors", "Use function if unambiguous"),
            ("gstats commits:authors", "Explicit plugin:function syntax"),
            ("gstats --plugins", "Show detailed plugin information"),
        ];
        
        for (command, desc) in examples {
            // Split command into 'gstats' and the rest
            let parts: Vec<&str> = command.splitn(2, ' ').collect();
            let colored_command = if parts.len() == 2 {
                format!("{} {}", 
                       self.colour_manager.command(parts[0]),
                       self.colour_manager.success(parts[1]))
            } else {
                self.colour_manager.command(command).to_string()
            };
            
            let plain_text_width = command.len();
            let padding_needed = if plain_text_width >= 35 { 1 } else { 35 - plain_text_width };
            
            writeln!(output, "    {}{}{}", 
                    colored_command,
                    " ".repeat(padding_needed),
                    desc).unwrap();
        }
    }
    
    /// Generate the invalid command error message with colored output
    pub async fn format_invalid_command(&self, command: &str, suggestions: &[String]) -> String {
        let mut output = String::new();
        
        writeln!(output, "{} '{}'.", 
                self.colour_manager.error("Unknown command"), command).unwrap();
        
        // Suggestions
        if !suggestions.is_empty() {
            writeln!(output).unwrap();
            if suggestions.len() == 1 {
                writeln!(output, "{} '{}'?", 
                        self.colour_manager.info("Did you mean"), 
                        self.colour_manager.highlight(&suggestions[0])).unwrap();
                writeln!(output, "Try: {} {} --help", 
                        self.colour_manager.command("gstats"),
                        self.colour_manager.highlight(&suggestions[0])).unwrap();
            } else {
                writeln!(output, "{}",
                        self.colour_manager.info("Did you mean one of these?")).unwrap();
                for suggestion in suggestions {
                    writeln!(output, "  • {}", self.colour_manager.highlight(suggestion)).unwrap();
                }
            }
        }
        
        // Plugin table using the same format as --plugins command
        writeln!(output).unwrap();
        writeln!(output, "{}", self.colour_manager.info("Available plugins and functions:")).unwrap();
        
        // Generate dynamic plugin table using the same logic as --plugins command
        if let Ok(mut handler) = crate::cli::plugin_handler::PluginHandler::new() {
            if let Err(_) = handler.build_command_mappings().await {
                // Fallback to hardcoded table if mapping fails
                self.write_plugin_table(&mut output);
            } else {
                let mappings = handler.get_function_mappings();
                if mappings.is_empty() {
                    writeln!(output, "No plugins available.").unwrap();
                } else {
                    // Use the same tabular format as --plugins command
                    self.write_dynamic_plugin_table(&mut output, &mappings);
                }
            }
        } else {
            // Fallback to hardcoded table if plugin handler creation fails
            self.write_plugin_table(&mut output);
        }
        
        // Usage
        writeln!(output, "{}:", self.colour_manager.info("Usage")).unwrap();
        writeln!(output, "  {} {}                      {}", 
                self.colour_manager.command("gstats"),
                self.colour_manager.highlight("<plugin>"),
                "Use plugin's default function").unwrap();
        writeln!(output, "  {} {}                     {}", 
                self.colour_manager.command("gstats"),
                self.colour_manager.highlight("<function>"),
                "Use function if unambiguous").unwrap();
        writeln!(output, "  {} {}             {}", 
                self.colour_manager.command("gstats"),
                self.colour_manager.highlight("<plugin>:<function>"),
                "Explicit plugin:function syntax").unwrap();
        writeln!(output).unwrap();
        writeln!(output, "For detailed help: {} {}", 
                self.colour_manager.command("gstats"),
                self.colour_manager.highlight("--help")).unwrap();
        
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::ColourConfig;
    
    #[test]
    fn test_help_formatter_creation() {
        // Use explicit config to ensure predictable behavior in tests
        let mut enabled_config = ColourConfig::new();
        enabled_config.set_enabled(true);
        enabled_config.set_color_forced(true); // Force enable to avoid environment detection
        let formatter = HelpFormatter::new(Some(enabled_config));
        assert!(formatter.colour_manager.colours_enabled());
        
        let disabled_config = ColourConfig::disabled();
        let formatter_disabled = HelpFormatter::new(Some(disabled_config));
        assert!(!formatter_disabled.colour_manager.colours_enabled());
    }
    
    #[test]
    fn test_help_formatter_from_no_color_flag() {
        let formatter = HelpFormatter::from_no_color_flag(true);
        assert!(!formatter.colour_manager.colours_enabled());
        
        // For the false case, we can't reliably assert colors are enabled
        // since it depends on terminal detection, so just verify creation works
        let _formatter = HelpFormatter::from_no_color_flag(false);
        // Don't assert enabled state - depends on test environment
    }
    
    #[test]
    fn test_format_main_help() {
        let formatter = HelpFormatter::from_no_color_flag(true); // Disable colors for predictable testing
        let help = formatter.format_main_help();
        
        assert!(help.contains("gstats - Fast, local-first git analytics tool"));
        assert!(help.contains("USAGE:"));
        assert!(help.contains("AVAILABLE PLUGINS AND FUNCTIONS:"));
        assert!(help.contains("commits"));
        assert!(help.contains("metrics"));
        assert!(help.contains("export"));
        assert!(help.contains("OPTIONS:"));
        assert!(help.contains("USAGE EXAMPLES:"));
    }
    
    #[tokio::test]
    async fn test_format_invalid_command() {
        let formatter = HelpFormatter::from_no_color_flag(true); // Disable colors for predictable testing
        let suggestions = vec!["commits".to_string(), "metrics".to_string()];
        let error = formatter.format_invalid_command("comits", &suggestions).await;
        
        assert!(error.contains("Unknown command 'comits'"));
        assert!(error.contains("Did you mean one of these?"));
        assert!(error.contains("commits"));
        assert!(error.contains("metrics"));
        assert!(error.contains("Available plugins and functions:"));
        assert!(error.contains("Usage:"));
    }
}