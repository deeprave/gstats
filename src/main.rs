mod cli;
mod config;
mod display;
mod git;
mod logging;
mod scanner;
mod plugin;
mod stats;

use anyhow::Result;
use std::process;
use std::sync::Arc;
use std::collections::HashMap;
use log::{info, error, debug};
use crate::stats::RepositoryFileStats;
use crate::scanner::async_engine::repository::AsyncRepositoryHandle;

use crate::display::CompactFormat;

/// Statistics about what was actually processed during scanning
#[derive(Debug, Clone)]
pub struct ProcessedStatistics {
    pub files_processed: usize,
    pub commits_processed: usize,
    pub authors_processed: usize,
}

impl CompactFormat for ProcessedStatistics {
    fn to_compact_format(&self) -> String {
        format!(
            "Files: {} | Commits: {} | Authors: {}",
            self.files_processed,
            self.commits_processed,
            self.authors_processed
        )
    }
}

fn main() {
    if let Err(e) = run() {
        let error_msg = e.to_string();
        
        // Check if this is a user-facing command error (not a system error)
        let is_user_error = error_msg.contains("Command resolution failed") || 
                           error_msg.contains("Unknown command") ||
                           error_msg.contains("Plugin") && error_msg.contains("is not available") ||
                           error_msg.contains("not a git repository") ||
                           error_msg.contains("Directory does not exist");
        
        if is_user_error {
            // For user errors, only show to stderr (no logging noise)
            eprintln!("{}", e);
        } else {
            // For system errors, log and show to stderr
            error!("Application error: {}", e);
            for cause in e.chain().skip(1) {
                error!("  Caused by: {}", cause);
            }
            eprintln!("Error: {}", e);
        }
        
        process::exit(1);
    }
    
    // Application completed successfully - set exit handler to avoid panic on runtime drop
    std::panic::set_hook(Box::new(|panic_info| {
        // Check if this is the known runtime drop panic
        if let Some(payload) = panic_info.payload().downcast_ref::<&str>() {
            if payload.contains("Cannot drop a runtime in a context where blocking is not allowed") {
                // This is the known cleanup issue - exit cleanly 
                eprintln!("Runtime cleanup completed");
                std::process::exit(0);
            }
        }
        // For other panics, use default behavior
        eprintln!("Panic: {:?}", panic_info);
        std::process::exit(101);
    }));
}

fn run() -> Result<()> {
    let args = cli::args::parse_args();
    
    cli::args::validate_args(&args)?;
    
    let config_manager = load_configuration(&args)?;
    
    let log_config = configure_logging(&args, &config_manager)?;
    logging::init_logger(log_config)?;
    
    // Enhanced logging system is now ready
    
    // Handle configuration export command first (before creating runtime)
    if let Some(export_path) = &args.export_config {
        return handle_export_config(&config_manager, export_path);
    }
    
    // Create runtime for async operations - single runtime for the entire application
    // Using current_thread runtime to avoid nested runtime issues
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    
    // Handle plugin management commands
    if args.list_plugins || args.show_plugins || args.plugins_help || args.plugin_info.is_some() || args.list_by_type.is_some() || args.list_formats {
        return runtime.block_on(async {
            let config_manager = load_configuration(&args)?;
            handle_plugin_commands(&args, &config_manager).await
        });
    }
    
    // Open repository once
    let repo = git::resolve_repository_handle(args.repository.clone())?;
    
    // Run scanner with existing runtime
    let runtime_arc = Arc::new(runtime);
    let runtime_clone = Arc::clone(&runtime_arc);
    let result = runtime_arc.block_on(async {
        run_scanner(repo, args, config_manager, runtime_clone).await
    });
    
    // Runtime will be dropped when runtime_arc goes out of scope
    
    result
}

fn load_configuration(args: &cli::Args) -> Result<config::ConfigManager> {
    let mut manager = if let Some(config_file) = &args.config_file {
        debug!("Loading configuration from explicit file: {}", config_file.display());
        config::ConfigManager::load_from_file(config_file.clone())?
    } else {
        config::ConfigManager::load()?
    };
    
    if let Some(section_name) = &args.config_name {
        debug!("Selecting configuration section: {}", section_name);
        manager.select_section(section_name.clone());
    }
    
    Ok(manager)
}

/// Handle configuration export command
fn handle_export_config(config_manager: &config::ConfigManager, export_path: &std::path::Path) -> Result<()> {
    use std::fs;
    
    info!("Exporting configuration to: {}", export_path.display());
    
    // Generate complete configuration
    let config_content = config_manager.export_complete_config()?;
    
    // Write to file
    fs::write(export_path, config_content)
        .map_err(|e| anyhow::anyhow!("Failed to write configuration to {}: {}", export_path.display(), e))?;
    
    println!("Configuration exported to: {}", export_path.display());
    
    Ok(())
}

fn configure_logging(args: &cli::Args, config: &config::ConfigManager) -> Result<logging::LogConfig> {
    use log::LevelFilter;
    use std::str::FromStr;
    
    let console_level = if args.debug {
        LevelFilter::Trace
    } else if args.verbose {
        LevelFilter::Debug
    } else if args.quiet {
        LevelFilter::Error
    } else {
        match config.get_log_level("base", "console-level") {
            Ok(Some(level)) => {
                debug!("Using console log level from config: {:?}", level);
                level
            }
            Ok(None) => LevelFilter::Info,
            Err(e) => {
                debug!("Invalid console-level in config, using default: {}", e);
                LevelFilter::Info
            }
        }
    };
    
    debug!("Console log level set to: {:?}", console_level);
    
    let format = if !args.log_format.is_empty() && args.log_format != "text" {
        logging::LogFormat::from_str(&args.log_format)
            .map_err(|e| anyhow::anyhow!(e))?
    } else {
        match config.get_value_root("log-format") {
            Some(format_str) => {
                debug!("Using log format from config: {}", format_str);
                logging::LogFormat::from_str(format_str)
                    .unwrap_or(logging::LogFormat::Text)
            }
            None => logging::LogFormat::Text,
        }
    };
    
    debug!("Log format set to: {:?}", format);
    
    let log_file_path = args.log_file.clone()
        .or_else(|| config.get_path("base", "log-file"));
    
    let file_log_level = match &args.log_file_level {
        Some(level_str) => Some(logging::parse_log_level(level_str)?),
        None => {
            match config.get_log_level("base", "file-log-level") {
                Ok(Some(level)) => {
                    debug!("Using file log level from config: {:?}", level);
                    Some(level)
                }
                Ok(None) => None,
                Err(e) => {
                    debug!("Invalid file-log-level in config, using None: {}", e);
                    None
                }
            }
        }
    };
    
    let (destination, file_level) = match (log_file_path.as_ref(), file_log_level) {
        (Some(file_path), Some(level)) => {
            debug!("File logging enabled: {} (level: {:?})", file_path.display(), level);
            (logging::LogDestination::Both(file_path.clone()), Some(level))
        }
        (Some(file_path), None) => {
            debug!("File logging enabled: {} (level: {:?} - same as console)", file_path.display(), console_level);
            (logging::LogDestination::Both(file_path.clone()), Some(console_level))
        }
        (None, None) => {
            debug!("Console-only logging enabled");
            (logging::LogDestination::Console, None)
        }
        (None, Some(_)) => {
            // This case is handled by validate_args, but just in case
            error!("Log file level specified without log file - this should have been caught during validation");
            return Err(anyhow::anyhow!("Log file level specified without log file"));
        }
    };
    
    // Create color configuration based on args and config file
    // Precedence: --no-color > --color > config file > default behavior
    let (colour_config, enable_colours) = if args.no_color {
        (None, false)
    } else {
        // Start with config file settings
        let mut colour_config = config.get_colour_config()
            .unwrap_or_else(|_| display::ColourConfig::default());
            
        if args.color {
            // Force colors even when redirected (override config file)
            colour_config.set_color_forced(true);
        }
        
        let enabled = colour_config.should_use_colours();
        (Some(colour_config), enabled)
    };
    
    Ok(logging::LogConfig {
        console_level,
        file_level,
        format,
        destination,
        colour_config,
        enable_colours,
    })
}

/// Create a ColourManager from CLI arguments and configuration file
fn create_colour_manager(args: &cli::Args, config: &config::ConfigManager) -> display::ColourManager {
    let colour_config = config.get_colour_config().ok();
    display::ColourManager::from_color_args(args.no_color, args.color, colour_config)
}

/// Format a compact table with headers and rows
fn format_compact_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    
    // Calculate column widths
    let mut column_widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < column_widths.len() {
                column_widths[i] = column_widths[i].max(cell.len());
            }
        }
    }
    
    let mut result = String::new();
    
    // Header row
    result.push_str("  ");
    for (i, header) in headers.iter().enumerate() {
        if i > 0 {
            result.push(' ');
        }
        result.push_str(&format!("{:<width$}", header, width = column_widths[i]));
    }
    result.push('\n');
    
    // Separator row
    result.push_str("  ");
    for (i, width) in column_widths.iter().enumerate() {
        if i > 0 {
            result.push(' ');
        }
        result.push_str(&"-".repeat(*width));
    }
    result.push('\n');
    
    // Data rows
    for row in rows {
        result.push_str("  ");
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                result.push(' ');
            }
            if i < column_widths.len() {
                result.push_str(&format!("{:<width$}", cell, width = column_widths[i]));
            } else {
                result.push_str(cell);
            }
        }
        result.push('\n');
    }
    
    result
}

/// Generate formatted plugin table for display purposes
pub async fn generate_plugin_table(handler: &mut cli::plugin_handler::PluginHandler, colour_manager: &display::ColourManager) -> Result<String> {
    handler.build_command_mappings().await?;
    
    let mappings = handler.get_function_mappings();
    if mappings.is_empty() {
        return Ok("No plugins available.".to_string());
    }

    use std::fmt::Write;
    
    // Group functions by plugin
    let mut plugins_map: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for mapping in &mappings {
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
    
    // Generate output string
    let mut output = String::new();
    
    for (i, (plugin_name, plugin_rows)) in all_plugin_data.iter().enumerate() {
        if i > 0 {
            writeln!(output)?; // Single line between plugins
        }
        
        writeln!(output, "{}: {}",
            colour_manager.info("Plugin"),
            colour_manager.success(plugin_name))?;
        
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
            writeln!(output, "{}", line)?;
        }
    }
    
    writeln!(output)?;
    writeln!(output, "{}:",
        colour_manager.info("Usage Examples"))?;
    writeln!(output, "  {} {}                {}",
        colour_manager.command("gstats"),
        colour_manager.success("<plugin>"),
        "Use plugin's default function")?;
    writeln!(output, "  {} {}     {}",
        colour_manager.command("gstats"),
        colour_manager.success("<plugin>:<function>"),
        "Use specific plugin function")?;
    writeln!(output)?;
    write!(output, "{} = default function for plugin",
        colour_manager.orange("(default)"))?;
    
    Ok(output)
}

async fn handle_plugin_commands(args: &cli::Args, config: &config::ConfigManager) -> Result<()> {
    use cli::plugin_handler::PluginHandler;
    use crate::plugin::traits::PluginType;
    
    let plugin_config = cli::converter::merge_plugin_config(&args, Some(config));
    let mut handler = PluginHandler::with_plugin_config(plugin_config)?;
    
    // Handle --plugins command (display plugins with their functions)
    if args.show_plugins {
        // Create color manager for enhanced output
        let colour_manager = create_colour_manager(args, config);
        
        println!("{}", colour_manager.highlight("Available Plugins:"));
        println!("{}", colour_manager.highlight("=================="));
        
        let table_output = generate_plugin_table(&mut handler, &colour_manager).await?;
        print!("{}", table_output);
        
        return Ok(());
    }
    
    // Handle --plugins-help command (display functions with their providers)
    if args.plugins_help {
        handler.build_command_mappings().await?;
        
        println!("Available Plugin Functions and Commands:");
        println!("========================================");
        
        let mappings = handler.get_function_mappings();
        if mappings.is_empty() {
            println!("No plugin functions available.");
        } else {
            // Sort functions alphabetically
            let mut sorted_mappings = mappings.clone();
            sorted_mappings.sort_by(|a, b| a.function_name.cmp(&b.function_name));
            
            for mapping in &sorted_mappings {
                let aliases_str = if mapping.aliases.is_empty() {
                    String::new()
                } else {
                    format!(" (aliases: {})", mapping.aliases.join(", "))
                };
                
                let default_marker = if mapping.is_default { " *" } else { "" };
                
                println!("{}{}{} → {} plugin{}", 
                    mapping.function_name,
                    default_marker,
                    aliases_str,
                    mapping.plugin_name,
                    if !mapping.description.is_empty() { 
                        format!(" - {}", mapping.description) 
                    } else { 
                        String::new() 
                    }
                );
            }
            
            println!();
            println!("Usage Examples:");
            println!("  gstats <function>              # Use function if unambiguous");
            println!("  gstats <plugin>                # Use plugin's default function");  
            println!("  gstats <plugin>:<function>     # Explicit plugin:function syntax");
            println!();
            println!("* = default function for plugin");
        }
        
        // Show any ambiguities as warnings
        let ambiguities = handler.get_ambiguity_reports();
        if !ambiguities.is_empty() {
            println!();
            println!("Ambiguous Functions (require plugin:function syntax):");
            for ambiguity in ambiguities {
                println!("  ⚠️  {}", ambiguity);
            }
        }
        
        return Ok(());
    }
    
    if args.list_plugins {
        let plugins = handler.list_plugins().await?;
        for plugin in plugins {
            println!("{}: {} ({})", plugin.name, plugin.description, plugin.version);
        }
        return Ok(());
    }
    
    if args.list_formats {
        use crate::plugin::builtin::utils::format_detection::FormatDetector;
        use crate::plugin::builtin::export::config::ExportFormat;
        
        let colour_manager = create_colour_manager(args, config);
        
        println!("{}", colour_manager.highlight("Supported Export Formats:"));
        println!("{}", colour_manager.highlight("========================="));
        println!();
        
        let detector = FormatDetector::new();
        let formats = [
            (ExportFormat::Json, "JSON data format"),
            (ExportFormat::Csv, "Comma-separated values"),
            (ExportFormat::Xml, "XML markup format"),
            (ExportFormat::Yaml, "YAML data format"),
            (ExportFormat::Html, "HTML web format"),
            (ExportFormat::Markdown, "Markdown documentation format"),
        ];
        
        for (format, description) in formats {
            let extensions = detector.get_extensions_for_format(&format);
            let extensions_str = extensions.join(", ");
            println!("  {:<12} {} ({})", 
                colour_manager.success(&format!("{:?}", format).to_lowercase()),
                description,
                colour_manager.info(&format!("extensions: {}", extensions_str))
            );
        }
        
        println!();
        println!("{}:", colour_manager.highlight("Format Auto-Detection"));
        println!("  • File extensions automatically determine output format");
        println!("  • Use --format to override auto-detection");
        println!("  • Templates can generate any supported format");
        println!();
        println!("{}:", colour_manager.highlight("Examples"));
        println!("  gstats export data.json     # Auto-detects JSON format");
        println!("  gstats export data.csv      # Auto-detects CSV format");
        println!("  gstats export --format yaml # Force YAML format");
        
        return Ok(());
    }
    
    if let Some(plugin_name) = &args.plugin_info {
        if let Some(info) = handler.get_plugin_info(plugin_name).await? {
            println!("Plugin: {}", info.name);
            println!("Version: {}", info.version);
            println!("Description: {}", info.description);
            println!("Author: {}", info.author);
            println!("Type: {:?}", info.plugin_type);
        } else {
            println!("Plugin '{}' not found", plugin_name);
        }
        return Ok(());
    }
    
    
    if let Some(plugin_type_str) = &args.list_by_type {
        let plugin_type = match plugin_type_str.as_str() {
            "scanner" => PluginType::Scanner,
            "processing" => PluginType::Processing,
            "output" => PluginType::Output,
            "notification" => PluginType::Notification,
            _ => return Err(anyhow::anyhow!("Unknown plugin type: {}", plugin_type_str)),
        };
        let plugins = handler.get_plugins_by_type(plugin_type).await?;
        for plugin in plugins {
            println!("{}: {}", plugin.name, plugin.description);
        }
        return Ok(());
    }
    
    Ok(())
}

async fn run_scanner(
    repo: git::RepositoryHandle, 
    args: cli::Args,
    config_manager: config::ConfigManager,
    runtime: Arc<tokio::runtime::Runtime>,
) -> Result<()> {
    use std::sync::Arc;
    
    // Convert CLI args to scanner config and query params
    let scanner_config = cli::converter::args_to_scanner_config(&args, Some(&config_manager))?;
    let query_params = cli::converter::args_to_query_params(&args)?;
    
    debug!("Scanner configuration: {:?}", scanner_config);
    debug!("Query parameters: {:?}", query_params);
    
    // Create plugin configuration
    let plugin_config = cli::converter::merge_plugin_config(&args, Some(&config_manager));
    
    // Create plugin registry and initialize plugins
    let plugin_registry = plugin::SharedPluginRegistry::new();
    
    // Initialize built-in plugins (respecting exclusion configuration)
    initialize_builtin_plugins(&plugin_registry, repo.clone(), &plugin_config).await?;
    
    // Create plugin handler with enhanced configuration
    let mut plugin_handler = cli::plugin_handler::PluginHandler::with_plugin_config(plugin_config)?;
    plugin_handler.build_command_mappings().await?;
    
    // Resolve plugin command using CommandMapper
    let command = args.command.as_ref().map(|s| s.as_str()).unwrap_or("commits");
    let plugin_names = vec![resolve_single_plugin_command(&plugin_handler, command, &args).await?];
    let original_commands = vec![command.to_string()];
    
    debug!("Active plugins: {:?}", plugin_names);
    debug!("Plugin arguments (original): {:?}", args.plugin_args);

    // Filter out global flags from plugin arguments to improve UX
    let filtered_plugin_args = cli::filter_global_flags(&args.plugin_args);
    debug!("Plugin arguments (filtered): {:?}", filtered_plugin_args);
    
    // Create callback-based message producer (queue bypassed via plugin callbacks)
    let message_producer = Arc::new(scanner::CallbackMessageProducer::new(
        "ScannerProducer".to_string()
    ));
    
    // Create scanner engine
    let mut engine_builder = scanner::AsyncScannerEngineBuilder::new()
        .repository(repo.clone())
        .config(scanner_config.clone())
        .message_producer(message_producer as Arc<dyn scanner::MessageProducer + Send + Sync>)
        .runtime(runtime);
    
    // Create base scanners
    let async_repo = Arc::new(scanner::async_engine::repository::AsyncRepositoryHandle::new(repo.clone()));
    
    // Create file scanner
    let file_scanner = Arc::new(scanner::async_engine::scanners::AsyncFileScanner::new(
        Arc::clone(&async_repo),
    ));
    
    // Create history scanner  
    let history_scanner = Arc::new(scanner::async_engine::scanners::AsyncHistoryScanner::new(
        Arc::clone(&async_repo),
    ));
    
    // Create combined scanner
    let combined_scanner = Arc::new(scanner::async_engine::scanners::AsyncCombinedScanner::new(
        Arc::clone(&async_repo),
    ));
    
    // Wrap scanners with plugin processing
    let plugin_scanner_builder = scanner::PluginScannerBuilder::new()
        .add_scanner(file_scanner)
        .add_scanner(history_scanner)
        .add_scanner(combined_scanner)
        .plugin_registry(plugin_registry.clone());
    
    let plugin_scanners = plugin_scanner_builder.build()?;
    
    // Add plugin-enabled scanners to engine
    for scanner in plugin_scanners {
        engine_builder = engine_builder.add_scanner(scanner);
    }
    
    // Build and run scanner engine
    let engine = engine_builder.build()?;
    
    // Determine scan modes based on active plugins
    let scan_modes = determine_scan_modes(&plugin_names);
    
    debug!("Starting scan with modes: {:?}", scan_modes);
    
    // Create progress indicators
    let colour_manager = create_colour_manager(&args, &config_manager);
    let progress = display::ProgressIndicator::new(colour_manager.clone());
    
    // Show repository information  
    progress.status(display::StatusType::Info, &format!("Using current directory as git repository: {}", repo.to_path_string()));
    
    // Start scanning with progress feedback and spinner
    let scan_spinner = progress.start_spinner("Scanning repository...");
    
    // Note: Consumer disabled to prevent runtime drop issues
    // For GS-30 we'll implement proper async consumer integration
    
    // Run the scan
    match engine.scan(scan_modes).await {
        Ok(()) => {
            debug!("Scan completed successfully");
            
            // Stop spinner and show success
            scan_spinner.complete(display::StatusType::Success, "Repository scan complete").await;
            
            // Get comprehensive statistics
            let stats = engine.get_comprehensive_stats().await?;
            
            // Active plugins (show before analysis)
            let registry_plugins = plugin_registry.inner().read().await.list_plugins();
            if !registry_plugins.is_empty() {
                progress.status(display::StatusType::Info, &format!("Active plugins: {}", registry_plugins.join(", ")));
            }
            
            // Basic scan results with progress status
            let results_message = format!("Scan results: {} tasks completed successfully", stats.completed_tasks);
            progress.status(display::StatusType::Success, &results_message);
            
            if stats.errors > 0 {
                let error_message = format!("Encountered {} errors during scan", stats.errors);
                progress.status(display::StatusType::Warning, &error_message);
            }
            
            debug!("Detailed statistics: {:?}", stats);
            
            // Execute plugins to generate reports and get processed statistics
            let plugin_spinner = progress.start_spinner("Processing plugin analysis...");
            let _processed_stats = execute_plugins_analysis(&plugin_registry, &plugin_names, repo.clone(), &stats, &args, &config_manager).await?;
            plugin_spinner.complete(display::StatusType::Success, "Plugin analysis complete").await;
            
            // Display the plugin reports
            display_plugin_reports(&plugin_registry, &original_commands, repo.clone(), &stats, &args, &config_manager).await?;
            
            // Note: Analysis Summary is now displayed before the plugin reports in execute_plugins_and_display_reports
        }
        Err(e) => {
            error!("Scan failed: {}", e);
            scan_spinner.complete(display::StatusType::Error, &format!("Repository scan failed: {}", e)).await;
            return Err(e.into());
        }
    }
    
    Ok(())
}

async fn initialize_builtin_plugins(
    registry: &plugin::SharedPluginRegistry, 
    repo: git::RepositoryHandle,
    plugin_config: &cli::converter::PluginConfig,
) -> Result<()> {
    use plugin::builtin::{CommitsPlugin, MetricsPlugin, ExportPlugin};
    
    let mut reg = registry.inner().write().await;
    
    // Register built-in plugins (respecting exclusion configuration)
    let mut registered_count = 0;
    
    // Check and register CommitsPlugin
    if !plugin_config.plugin_exclude.contains(&"commits".to_string()) {
        reg.register_plugin(Box::new(CommitsPlugin::new())).await?;
        registered_count += 1;
        debug!("Registered built-in plugin: commits");
    } else {
        debug!("Excluded built-in plugin: commits");
    }
    
    // Check and register MetricsPlugin
    if !plugin_config.plugin_exclude.contains(&"metrics".to_string()) {
        reg.register_plugin(Box::new(MetricsPlugin::new())).await?;
        registered_count += 1;
        debug!("Registered built-in plugin: metrics");
    } else {
        debug!("Excluded built-in plugin: metrics");
    }
    
    // Check and register ExportPlugin
    if !plugin_config.plugin_exclude.contains(&"export".to_string()) {
        reg.register_plugin(Box::new(ExportPlugin::new())).await?;
        registered_count += 1;
        debug!("Registered built-in plugin: export");
    } else {
        debug!("Excluded built-in plugin: export");
    }
    
    // Create plugin context for initialization
    let context = create_plugin_context(repo)?;
    
    // Initialize all plugins
    let results = reg.initialize_all(&context).await;
    
    // Check for initialization errors
    for (plugin_name, result) in results {
        if let Err(e) = result {
            error!("Failed to initialize plugin '{}': {}", plugin_name, e);
            return Err(anyhow::anyhow!(
                "Plugin '{}' failed to initialize: {}\n\nTry:\n  • Running 'gstats commits' for basic analysis\n  • Checking your configuration file\n  • Reinstalling gstats if the issue persists",
                plugin_name, e
            ));
        }
    }
    
    debug!("Initialized {} built-in plugins", registered_count);
    
    Ok(())
}

/// Create a plugin context for plugin operations
fn create_plugin_context(repo_handle: git::RepositoryHandle) -> Result<plugin::PluginContext> {
    use plugin::context::RuntimeInfo;
    use scanner::{ScannerConfig, QueryParams};
    use std::collections::HashMap;
    
    // Create minimal context for plugin initialization
    let scanner_config = Arc::new(ScannerConfig::default());
    let query_params = Arc::new(QueryParams::default());
    
    // Use the provided repository handle
    
    let runtime_info = RuntimeInfo {
        api_version: scanner::version::get_api_version() as u32,
        runtime_version: "tokio-1.0".to_string(),
        cpu_cores: num_cpus::get(),
        available_memory: 0, // Not critical for initialization
        working_directory: std::env::current_dir()?.to_string_lossy().to_string(),
    };
    
    Ok(plugin::PluginContext {
        scanner_config,
        query_params,
        plugin_config: HashMap::new(),
        runtime_info,
        capabilities: Vec::new(),
        aggregated_data: None,
    })
}

fn determine_scan_modes(plugin_names: &[String]) -> scanner::ScanMode {
    use scanner::ScanMode;
    
    let mut modes = ScanMode::empty();
    
    for name in plugin_names {
        match name.as_str() {
            "commits" => modes |= ScanMode::HISTORY,
            "metrics" => modes |= ScanMode::FILES | ScanMode::METRICS,
            "export" => modes |= ScanMode::FILES | ScanMode::HISTORY,
            _ => modes |= ScanMode::FILES, // Default to files
        }
    }
    
    if modes.is_empty() {
        modes = ScanMode::FILES; // Default mode
    }
    
    modes
}

/// Resolve plugin commands using CommandMapper
async fn resolve_single_plugin_command(
    plugin_handler: &cli::plugin_handler::PluginHandler,
    command: &str,
    args: &cli::Args,
) -> Result<String> {
    use cli::command_mapper::CommandResolution;
    
    debug!("Resolving command: '{}'", command);
    
    match plugin_handler.resolve_command_with_colors(command, args.no_color, args.color).await {
        Ok(resolution) => {
            match resolution {
                CommandResolution::Function { plugin_name, function_name, .. } => {
                    debug!("Resolved '{}' to plugin '{}' function '{}'", command, plugin_name, function_name);
                    Ok(plugin_name)
                }
                CommandResolution::DirectPlugin { plugin_name, default_function } => {
                    debug!("Resolved '{}' to plugin '{}' (default: {:?})", command, plugin_name, default_function);
                    Ok(plugin_name)
                }
                CommandResolution::Explicit { plugin_name, function_name } => {
                    debug!("Resolved '{}' to plugin '{}' function '{}'", command, plugin_name, function_name);
                    Ok(plugin_name)
                }
            }
        }
        Err(e) => {
            error!("Failed to resolve command '{}': {}", command, e);
            Err(anyhow::anyhow!("Command resolution failed for '{}': {}", command, e))
        }
    }
}



/// Collect scan data directly from repository for plugin processing
async fn collect_scan_data_for_plugin(plugin_name: &str, repo: git::RepositoryHandle) -> Result<Vec<scanner::messages::ScanMessage>> {
    use scanner::messages::{ScanMessage, MessageHeader, MessageData};
    use scanner::modes::ScanMode;
    
    // Only collect data for plugins that need it
    if plugin_name != "commits" && plugin_name != "metrics" && plugin_name != "export" {
        return Ok(Vec::new());
    }
    
    debug!("Collecting scan data for {} plugin", plugin_name);
    
    // Create progress indicator for data collection
    let colour_manager = display::ColourManager::new(); // Use defaults for internal operations
    let progress = display::ProgressIndicator::new(colour_manager);
    
    let collection_message = match plugin_name {
        "commits" => "Scanning commit history...",
        "metrics" => "Analyzing file metrics...",
        "export" => "Collecting data for export...",
        _ => "Collecting repository data...",
    };
    
    // Start spinner for data collection
    let collection_spinner = progress.start_spinner(collection_message);
    
    // Create async repository handle for data collection
    let async_repo = Arc::new(scanner::async_engine::repository::AsyncRepositoryHandle::new(repo));
    let mut scan_messages = Vec::new();
    
    // Collect data based on plugin requirements
    match plugin_name {
        "commits" => {
            // Collect commit history (limit to reasonable number)
            let commits = async_repo.get_commit_history(Some(1000)).await
                .map_err(|e| anyhow::anyhow!("Failed to get commit history: {}", e))?;
            
            debug!("Collected {} commits from repository", commits.len());
            
            // Convert commits to scan messages
            for (index, commit_info) in commits.into_iter().enumerate() {
                let header = MessageHeader::new(ScanMode::HISTORY, index as u64);
                let data = MessageData::CommitInfo {
                    hash: commit_info.id,
                    author: commit_info.author,
                    message: commit_info.message,
                    timestamp: commit_info.timestamp,
                    changed_files: commit_info.changed_files.into_iter()
                        .map(|fc| scanner::messages::FileChangeData {
                            path: fc.path,
                            lines_added: fc.lines_added,
                            lines_removed: fc.lines_removed,
                        })
                        .collect(),
                };
                scan_messages.push(ScanMessage::new(header, data));
            }
        }
        "metrics" => {
            // Collect file data for metrics analysis
            let files = async_repo.list_files().await
                .map_err(|e| anyhow::anyhow!("Failed to get file list: {}", e))?;
            
            debug!("Collected {} files from repository", files.len());
            
            // Convert files to scan messages
            for (index, file_info) in files.into_iter().enumerate() {
                let header = MessageHeader::new(ScanMode::FILES, index as u64);
                let data = MessageData::FileInfo {
                    path: file_info.path,
                    size: file_info.size as u64,
                    lines: estimate_line_count(file_info.size) as u32,
                };
                scan_messages.push(ScanMessage::new(header, data));
            }
        }
        "export" => {
            // Export plugin needs both commits and file data
            
            // First collect commit history
            let commits = async_repo.get_commit_history(Some(1000)).await
                .map_err(|e| anyhow::anyhow!("Failed to get commit history: {}", e))?;
            
            debug!("Collected {} commits for export", commits.len());
            
            // Convert commits to scan messages
            for (index, commit_info) in commits.into_iter().enumerate() {
                let header = MessageHeader::new(ScanMode::HISTORY, index as u64);
                let data = MessageData::CommitInfo {
                    hash: commit_info.id,
                    author: commit_info.author,
                    message: commit_info.message,
                    timestamp: commit_info.timestamp,
                    changed_files: commit_info.changed_files.into_iter()
                        .map(|f| scanner::messages::FileChangeData {
                            path: f.path,
                            lines_added: f.lines_added,
                            lines_removed: f.lines_removed,
                        })
                        .collect(),
                };
                scan_messages.push(ScanMessage::new(header, data));
            }
            
            // Then collect file data
            let files = async_repo.list_files().await
                .map_err(|e| anyhow::anyhow!("Failed to get file list: {}", e))?;
            
            debug!("Collected {} files for export", files.len());
            
            // Convert files to scan messages
            let file_index_offset = scan_messages.len();
            for (index, file_info) in files.into_iter().enumerate() {
                let header = MessageHeader::new(ScanMode::FILES, (file_index_offset + index) as u64);
                let data = MessageData::FileInfo {
                    path: file_info.path,
                    size: file_info.size as u64,
                    lines: estimate_line_count(file_info.size) as u32,
                };
                scan_messages.push(ScanMessage::new(header, data));
            }
        }
        _ => {
            // For other plugins, collect no data
            debug!("No specific data collection implemented for plugin: {}", plugin_name);
        }
    }
    
    // Stop spinner and show completion status
    if !scan_messages.is_empty() {
        let completion_message = format!("Collected {} items for analysis", scan_messages.len());
        collection_spinner.complete(display::StatusType::Success, &completion_message).await;
    } else {
        collection_spinner.complete(display::StatusType::Info, "No data collection required for this plugin").await;
    }
    
    Ok(scan_messages)
}

/// Estimate line count from file size (rough heuristic)
fn estimate_line_count(size: usize) -> usize {
    if size == 0 {
        0
    } else {
        // Assume average of 50 characters per line
        (size / 50).max(1)
    }
}

/// Create a summary of file statistics for JSON output
fn create_file_statistics_summary(file_stats: &RepositoryFileStats, output_all: bool, output_limit: Option<usize>) -> serde_json::Value {
    let top_files_by_commits = file_stats.files_by_commit_count();
    let top_files_by_changes = file_stats.files_by_net_change();
    
    // Determine how many files to show based on flags
    let files_to_show = if output_all {
        top_files_by_commits.len()
    } else if let Some(limit) = output_limit {
        limit.min(top_files_by_commits.len())
    } else {
        10.min(top_files_by_commits.len()) // Default limit
    };
    
    serde_json::json!({
        "total_files": file_stats.file_count(),
        "total_commits_across_files": file_stats.total_commits(),
        "top_files_by_commits": top_files_by_commits.iter().take(files_to_show).map(|(path, stats)| {
            serde_json::json!({
                "path": path,
                "commits": stats.commit_count,
                "lines_added": stats.lines_added,
                "lines_removed": stats.lines_removed,
                "net_change": stats.net_change,
                "current_lines": stats.current_lines,
                "current_lines_display": stats.current_lines_display(),
                "status": format!("{:?}", stats.status),
                "authors": stats.author_count()
            })
        }).collect::<Vec<_>>(),
        "top_files_by_changes": top_files_by_changes.iter().take(files_to_show).map(|(path, stats)| {
            serde_json::json!({
                "path": path,
                "commits": stats.commit_count,
                "lines_added": stats.lines_added,
                "lines_removed": stats.lines_removed,
                "net_change": stats.net_change,
                "current_lines": stats.current_lines,
                "current_lines_display": stats.current_lines_display(),
                "status": format!("{:?}", stats.status),
                "authors": stats.author_count()
            })
        }).collect::<Vec<_>>()
    })
}

/// Execute plugins analysis (without displaying output)
async fn execute_plugins_analysis(
    _plugin_registry: &plugin::SharedPluginRegistry,
    requested_commands: &[String],
    _repo: git::RepositoryHandle,
    stats: &scanner::async_engine::EngineStats,
    _args: &cli::Args,
    _config: &config::ConfigManager,
) -> Result<Option<ProcessedStatistics>> {
    
    let commands = if requested_commands.is_empty() {
        vec!["authors".to_string()] // Default to authors report
    } else {
        requested_commands.to_vec()
    };
    
    // For simplicity, just execute the first command
    let command = &commands[0];
    debug!("Executing plugin command: '{}'", command);
    
    // For now, we'll estimate processed stats based on the command being run
    if let Some(repo_stats) = &stats.repository_stats {
        let processed_stats = match command.as_str() {
            "authors" | "contributors" | "committers" | "commits" | "commit" | "history" => {
                ProcessedStatistics {
                    files_processed: repo_stats.total_files as usize, // Commits do include file data
                    commits_processed: repo_stats.total_commits as usize,
                    authors_processed: repo_stats.total_authors as usize,
                }
            }
            "metrics" => {
                ProcessedStatistics {
                    files_processed: repo_stats.total_files as usize,
                    commits_processed: 0, // No commit analysis for metrics
                    authors_processed: 0,
                }
            }
            _ => ProcessedStatistics {
                files_processed: repo_stats.total_files as usize,
                commits_processed: repo_stats.total_commits as usize,
                authors_processed: repo_stats.total_authors as usize,
            }
        };
        
        Ok(Some(processed_stats))
    } else {
        Ok(None)
    }
}

/// Display plugin reports
async fn display_plugin_reports(
    plugin_registry: &plugin::SharedPluginRegistry,
    requested_commands: &[String],
    repo: git::RepositoryHandle,
    stats: &scanner::async_engine::EngineStats,
    args: &cli::Args,
    config: &config::ConfigManager,
) -> Result<()> {
    
    let commands = if requested_commands.is_empty() {
        vec!["authors".to_string()] // Default to authors report
    } else {
        requested_commands.to_vec()
    };
    
    // For simplicity, just execute the first command
    let command = &commands[0];
    debug!("Executing plugin command: '{}'", command);
    
    // Display Analysis Summary before plugin reports
    if let Some(repo_stats) = &stats.repository_stats {
        let processed_stats = match command.as_str() {
            "authors" | "contributors" | "committers" | "commits" | "commit" | "history" => {
                ProcessedStatistics {
                    files_processed: repo_stats.total_files as usize,
                    commits_processed: repo_stats.total_commits as usize,
                    authors_processed: repo_stats.total_authors as usize,
                }
            }
            "metrics" => {
                ProcessedStatistics {
                    files_processed: repo_stats.total_files as usize,
                    commits_processed: 0,
                    authors_processed: 0,
                }
            }
            _ => ProcessedStatistics {
                files_processed: repo_stats.total_files as usize,
                commits_processed: repo_stats.total_commits as usize,
                authors_processed: repo_stats.total_authors as usize,
            }
        };
        
        // Use compact format if requested
        if args.compact {
            use crate::display::CompactFormat;
            println!("{}", processed_stats.to_compact_format());
        } else {
            let mut summary_parts = Vec::new();
            
            // Only include file metrics if file analysis was performed
            if processed_stats.files_processed > 0 {
                summary_parts.push(format!("{}/{} files", processed_stats.files_processed, repo_stats.total_files));
            }
            
            // Always include commit metrics if they were processed
            if processed_stats.commits_processed > 0 {
                summary_parts.push(format!("{}/{} commits", processed_stats.commits_processed, repo_stats.total_commits));
            }
            
            // Always include author metrics if they were processed
            if processed_stats.authors_processed > 0 {
                summary_parts.push(format!("{}/{} authors", processed_stats.authors_processed, repo_stats.total_authors));
            }
            
            if !summary_parts.is_empty() {
                info!("Analysis Summary: {}", summary_parts.join(", "));
            }
        }
    }
    
    // Map command to plugin and function
    let (plugin_name, function_name) = match command.as_str() {
        "authors" | "contributors" | "committers" => ("commits", "authors"),
        "commits" | "commit" | "history" => ("commits", "commits"), 
        "metrics" => ("metrics", "metrics"),
        "export" => ("export", "export"),
        _ => ("commits", "authors"), // Default
    };
    
    // Collect scan data for the plugin
    let scan_data = collect_scan_data_for_plugin(plugin_name, repo.clone()).await?;
    
    execute_plugin_function_with_data(plugin_registry, plugin_name, function_name, scan_data.clone(), repo.clone(), &args, config).await?;
    
    Ok(())
}

/// Execute a specific plugin function with provided scan data and display its results
async fn execute_plugin_function_with_data(
    plugin_registry: &plugin::SharedPluginRegistry,
    plugin_name: &str,
    function_name: &str,
    scan_data: Vec<scanner::messages::ScanMessage>,
    repo: git::RepositoryHandle,
    args: &cli::Args,
    config: &config::ConfigManager,
) -> Result<()> {
    use plugin::{PluginRequest, InvocationType, Plugin};
    use plugin::traits::PluginArgumentParser;
    use plugin::context::RequestPriority;
    use scanner::ScanMode;
    use std::sync::Arc;
    
    // Filter out global flags from plugin arguments to improve UX
    let filtered_plugin_args = cli::filter_global_flags(&args.plugin_args);

    debug!("Executing plugin '{}' function '{}' with {} scan messages", plugin_name, function_name, scan_data.len());
    
    // Get plugin from registry
    let registry = plugin_registry.inner().read().await;
    let plugin = match registry.get_plugin(plugin_name) {
        Some(plugin) => plugin,
        None => {
            error!("Plugin '{}' not found in registry", plugin_name);
            
            // Generate dynamic plugin table for error message
            let plugin_config = cli::converter::merge_plugin_config(args, Some(config));
            let mut handler = cli::plugin_handler::PluginHandler::with_plugin_config(plugin_config)?;
            let colour_manager = display::ColourManager::from_color_args(false, false, None); // No colors for error messages
            let plugin_table = generate_plugin_table(&mut handler, &colour_manager).await?;
            
            return Err(anyhow::anyhow!(
                "Plugin '{}' is not available.\n\nAvailable plugins:\n\n{}\n\nRun 'gstats --help' to see all available commands.",
                plugin_name,
                plugin_table
            ));
        }
    };
    
    // Check if template arguments are present - if so, route to export plugin
    let has_template_args = filtered_plugin_args.iter().any(|arg|
        arg.starts_with("--template") || arg == "--output" || arg == "-o"
    );
    
    if has_template_args {
        // Template arguments detected - route to export plugin instead
        debug!("Template arguments detected, routing to export plugin");
        
        // Create a new export plugin instance directly
        let mut export_instance = plugin::builtin::ExportPlugin::new();
        
        // Create plugin context
        let scanner_config = std::sync::Arc::new(scanner::ScannerConfig::default());
        let query_params = std::sync::Arc::new(scanner::QueryParams::default());
        let plugin_context = plugin::context::PluginContext::new(
            scanner_config,
            query_params,
        );
        
        export_instance.initialize(&plugin_context).await?;
        
        // Filter out global flags from plugin arguments to improve UX
        let filtered_plugin_args = cli::filter_global_flags(&args.plugin_args);

        // Parse template-specific arguments using the export plugin's argument parser
        if let Err(e) = export_instance.parse_plugin_args(&filtered_plugin_args).await {
            error!("Failed to parse export plugin arguments: {}", e);
            return Err(e.into());
        }
        
        // Add all scan data to export plugin
        for message in &scan_data {
            export_instance.add_data(message.clone())?;
        }
        
        // Execute the export
        let export_response = export_instance.execute(PluginRequest::Export).await?;
        debug!("Export plugin response: {:?}", export_response);
        
        // If output file is specified, don't display console output
        if filtered_plugin_args.iter().any(|arg| arg == "--output" || arg == "-o") {
            info!("Export completed successfully");
            return Ok(());
        }
        
        return Ok(());
    }
    
    // For the commits plugin specifically, create a new instance and process the scan data
    if plugin_name == "commits" && !scan_data.is_empty() {
        
        // Filter out global flags from plugin arguments to improve UX
        let filtered_plugin_args = cli::filter_global_flags(&args.plugin_args);

        // Parse plugin-specific arguments for commits plugin
        let mut output_all = false;
        let mut output_limit: Option<usize> = None;
        
        // Simple argument parsing for commits plugin arguments
        let mut i = 0;
        while i < filtered_plugin_args.len() {
            match filtered_plugin_args[i].as_str() {
                "--all" => {
                    output_all = true;
                    i += 1;
                }
                "--output-limit" | "--limit" => {
                    if i + 1 < filtered_plugin_args.len() {
                        if let Ok(limit) = filtered_plugin_args[i + 1].parse::<usize>() {
                            output_limit = Some(limit);
                            output_all = false; // --output-limit/--limit overrides --all
                        }
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                _ => {
                    i += 1;
                }
            }
        }
        
        
        // Process scan data directly to extract detailed author and file statistics
        let mut author_stats: HashMap<String, usize> = HashMap::new();
        let mut commit_count = 0;
        let mut file_stats = RepositoryFileStats::new();
        
        for message in &scan_data {
            if let scanner::messages::MessageData::CommitInfo { author, changed_files, timestamp, .. } = &message.data {
                commit_count += 1;
                *author_stats.entry(author.clone()).or_insert(0) += 1;
                
                // Track detailed file statistics across all commits
                for file_change in changed_files {
                    file_stats.update_file(
                        &file_change.path,
                        author,
                        file_change.lines_added,
                        file_change.lines_removed,
                        *timestamp,
                    );
                }
            }
        }
        
        // Check file existence and get current line counts using direct git queries
        let async_repo = AsyncRepositoryHandle::new(repo.clone());
        
        // Get all files that need existence checking
        let unknown_files = file_stats.get_unknown_file_paths();
        
        debug!("Checking existence for {} files from commit history", unknown_files.len());
        
        // Check each file's existence in the current HEAD commit
        for file_path in unknown_files {
            match async_repo.get_file_info(&file_path).await {
                Ok(Some((line_count, _is_binary))) => {
                    // File exists in current repository
                    file_stats.set_file_current_lines(&file_path, line_count);
                    debug!("File '{}' exists with {} lines", file_path, line_count);
                }
                Ok(None) => {
                    // File doesn't exist in current repository - it's been deleted
                    if let Some(file_stat) = file_stats.files.get_mut(&file_path) {
                        file_stat.set_deleted();
                        debug!("File '{}' has been deleted", file_path);
                    }
                }
                Err(e) => {
                    // Error checking file - treat as unknown
                    debug!("Error checking file '{}': {}", file_path, e);
                }
            }
        }
        
        debug!("Processed {} commits, found {} unique authors, {} unique files", 
               commit_count, author_stats.len(), file_stats.file_count());
        
        // Create the report data with detailed author information
        let mut authors: Vec<_> = author_stats.iter().collect();
        authors.sort_by(|a, b| b.1.cmp(a.1)); // Sort by commit count descending
        
        // Determine how many items to show based on flags
        let authors_to_show = if output_all {
            authors.len()
        } else if let Some(limit) = output_limit {
            limit.min(authors.len())
        } else {
            10.min(authors.len()) // Default limit
        };

        let report_data = match function_name {
            "authors" | "contributors" | "committers" => {
                serde_json::json!({
                    "total_authors": author_stats.len(),
                    "top_authors": authors.iter().take(authors_to_show).map(|(name, count)| {
                        serde_json::json!({ "name": name, "commits": count })
                    }).collect::<Vec<_>>(),
                    "author_stats": author_stats,
                    "unique_files": file_stats.file_count(),
                    "file_statistics": create_file_statistics_summary(&file_stats, output_all, output_limit),
                    "function": "authors",
                    "output_all": output_all,
                    "output_limit": output_limit
                })
            }
            "commits" | "commit" | "history" => {
                let avg_commits_per_author = if author_stats.is_empty() {
                    0.0
                } else {
                    commit_count as f64 / author_stats.len() as f64
                };
                
                serde_json::json!({
                    "total_commits": commit_count,
                    "unique_authors": author_stats.len(),
                    "unique_files": file_stats.file_count(),
                    "file_statistics": create_file_statistics_summary(&file_stats, output_all, output_limit),
                    "avg_commits_per_author": avg_commits_per_author,
                    "function": "commits",
                    "output_all": output_all,
                    "output_limit": output_limit
                })
            }
            _ => {
                serde_json::json!({
                    "total_authors": author_stats.len(),
                    "top_authors": authors.iter().take(authors_to_show).map(|(name, count)| {
                        serde_json::json!({ "name": name, "commits": count })
                    }).collect::<Vec<_>>(),
                    "author_stats": author_stats,
                    "unique_files": file_stats.file_count(),
                    "file_statistics": create_file_statistics_summary(&file_stats, output_all, output_limit),
                    "function": function_name,
                    "output_all": output_all,
                    "output_limit": output_limit
                })
            }
        };
        
        // Display the report directly
        display_plugin_data(plugin_name, function_name, &report_data, args, config).await?;
        return Ok(()); // Success, return early
    }
    
    // For the metrics plugin specifically, process file data using the actual MetricsPlugin
    if plugin_name == "metrics" && !scan_data.is_empty() {
        debug!("Processing {} scan messages for metrics plugin", scan_data.len());
        
        // Create a new metrics plugin instance and initialize it
        let mut metrics_plugin = plugin::builtin::MetricsPlugin::new();
        
        // Initialize the plugin with a minimal context
        let context = plugin::context::PluginContext::new(
            Arc::new(scanner::config::ScannerConfig::default()),
            Arc::new(scanner::query::QueryParams::new()),
        );
        metrics_plugin.initialize(&context).await?;
        
        // Process scan data through the metrics plugin
        use plugin::traits::ScannerPlugin;
        let mut processed_messages = Vec::new();
        for message in &scan_data {
            let results = metrics_plugin.process_scan_data(message).await?;
            processed_messages.extend(results);
        }
        
        // Execute the metrics function to get calculated complexity
        let invocation_type = if function_name == "default" || function_name == "metrics" {
            InvocationType::Default
        } else {
            InvocationType::Function(function_name.to_string())
        };
        
        let request = PluginRequest::Execute {
            request_id: uuid::Uuid::now_v7().to_string(),
            scan_modes: scanner::modes::ScanMode::FILES,
            parameters: HashMap::new(),
            timeout_ms: None,
            priority: RequestPriority::Normal,
            invoked_as: function_name.to_string(),
            invocation_type,
        };
        
        match metrics_plugin.execute(request).await? {
            plugin::PluginResponse::Execute { data, .. } => {
                // Display the report directly
                display_plugin_data(plugin_name, function_name, &data, args, config).await?;
                return Ok(());
            }
            _ => {
                return Err(anyhow::anyhow!("Unexpected response from metrics plugin"));
            }
        }
    }
    
    // For the export plugin specifically, we need to pass the scan data
    if plugin_name == "export" && !scan_data.is_empty() {
        debug!("Processing {} scan messages for export plugin", scan_data.len());
        
        // Create a new export plugin instance and initialize it
        let mut export_plugin = plugin::builtin::ExportPlugin::new();
        
        // Initialize the plugin with a minimal context
        let context = plugin::context::PluginContext::new(
            Arc::new(scanner::config::ScannerConfig::default()),
            Arc::new(scanner::query::QueryParams::new()),
        );
        export_plugin.initialize(&context).await?;
        
        // Add all scan messages to the export plugin
        for message in scan_data {
            export_plugin.add_data(message)?;
        }
        
        // Execute the export function
        let invocation_type = if function_name == "default" || function_name == "export" {
            InvocationType::Default
        } else {
            InvocationType::Function(function_name.to_string())
        };
        
        let request = PluginRequest::Execute {
            request_id: uuid::Uuid::now_v7().to_string(),
            scan_modes: ScanMode::HISTORY | ScanMode::FILES,
            invoked_as: plugin_name.to_string(),
            invocation_type,
            parameters: std::collections::HashMap::new(),
            timeout_ms: None,
            priority: RequestPriority::Normal,
        };
        
        // Execute the export
        match export_plugin.execute(request).await {
            Ok(response) => {
                display_plugin_response(plugin_name, function_name, response, args, config).await?;
            }
            Err(e) => {
                error!("Export plugin execution failed: {}", e);
                return Err(anyhow::anyhow!("Export failed: {}", e));
            }
        }
        
        return Ok(()); // Success, return early
    }
    
    // Fallback: execute plugin from registry without scan data (for other plugins or empty data)
    let invocation_type = if function_name == "default" || plugin.default_function() == Some(function_name) {
        InvocationType::Default
    } else {
        InvocationType::Function(function_name.to_string())
    };
    
    let request = PluginRequest::Execute {
        request_id: uuid::Uuid::now_v7().to_string(),
        scan_modes: ScanMode::HISTORY, // Default to history mode
        invoked_as: plugin_name.to_string(),
        invocation_type,
        parameters: std::collections::HashMap::new(),
        timeout_ms: None,
        priority: RequestPriority::Normal,
    };
    
    // Execute plugin from registry
    match plugin.execute(request).await {
        Ok(response) => {
            display_plugin_response(plugin_name, function_name, response, args, config).await?;
        }
        Err(e) => {
            error!("Plugin '{}' execution failed: {}", plugin_name, e);
            return Err(anyhow::anyhow!(
                "Analysis failed with plugin '{}': {}\n\nThis could be due to:\n  • Invalid repository data\n  • Configuration issues\n  • Resource limitations\n\nTry using a different plugin or check your repository for issues.",
                plugin_name, e
            ));
        }
    }
    
    Ok(())
}


/// Display plugin response to the user
async fn display_plugin_response(
    plugin_name: &str,
    function_name: &str,
    response: plugin::PluginResponse,
    args: &cli::Args,
    config: &config::ConfigManager,
) -> Result<()> {
    // Check if compact format is requested
    if args.compact {
        use crate::display::CompactFormat;
        println!("{}", response.to_compact_format());
        return Ok(());
    }
    
    match response {
        plugin::PluginResponse::Execute { data, errors, .. } => {
            // Display any errors first
            if !errors.is_empty() {
                for error in &errors {
                    error!("Plugin '{}' error: {}", plugin_name, error);
                }
            }
            
            // Display the main report
            display_plugin_data(plugin_name, function_name, &data, args, config).await?;
        }
        plugin::PluginResponse::Data(data_str) => {
            // Parse and display JSON data
            match serde_json::from_str::<serde_json::Value>(&data_str) {
                Ok(data) => display_plugin_data(plugin_name, function_name, &data, args, config).await?,
                Err(_) => {
                    // Display as plain text if not valid JSON
                    println!("{}", data_str);
                }
            }
        }
        plugin::PluginResponse::Statistics(stats) => {
            info!("Plugin '{}' statistics: {:?}", plugin_name, stats);
        }
        plugin::PluginResponse::Capabilities(_) => {
            // Capabilities are not typically displayed in normal operation
            debug!("Plugin '{}' capabilities retrieved", plugin_name);
        }
        plugin::PluginResponse::ProcessedData(messages) => {
            debug!("Plugin '{}' returned {} processed messages", plugin_name, messages.len());
        }
    }
    
    Ok(())
}

/// Display plugin data in a user-friendly format with colors
async fn display_plugin_data(
    plugin_name: &str,
    function_name: &str,
    data: &serde_json::Value,
    args: &cli::Args,
    config: &config::ConfigManager,
) -> Result<()> {
    // Create color manager based on CLI args and config
    let colour_manager = create_colour_manager(args, config);
    
    // Handle specific plugin output formats
    match (plugin_name, function_name) {
        ("commits", "authors" | "contributors" | "committers") => {
            display_author_report(data, &colour_manager, args.compact).await?;
        }
        ("commits", "commits" | "commit" | "history") => {
            display_commit_report(data, &colour_manager, args.compact).await?;
        }
        ("metrics", _) => {
            display_metrics_report(data, &colour_manager, args.compact).await?;
        }
        ("export", _) => {
            display_export_report(data, &colour_manager, args.compact).await?;
        }
        _ => {
            // Generic JSON display for unknown plugins
            println!("{}", serde_json::to_string_pretty(data)?);
        }
    }
    
    Ok(())
}

/// Display author analysis report with colors
async fn display_author_report(data: &serde_json::Value, colour_manager: &display::ColourManager, compact: bool) -> Result<()> {
    if compact {
        // Compact format: single-line output
        let total_authors = data.get("total_authors").and_then(|v| v.as_u64()).unwrap_or(0);
        let unique_files = data.get("unique_files").and_then(|v| v.as_u64()).unwrap_or(0);
        let top_author = data.get("top_authors")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|author| author.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("N/A");
        let top_commits = data.get("top_authors")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|author| author.get("commits"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        
        println!("Authors: {} | Files: {} | Top: {} ({} commits)",
                 total_authors, unique_files, top_author, top_commits);
        return Ok(());
    }
    
    // Standard format
    println!("\n{}", colour_manager.highlight("=== Author Analysis Report ==="));
    
    if let Some(total_authors) = data.get("total_authors").and_then(|v| v.as_u64()) {
        println!("{}: {}", colour_manager.info("Total Authors"), colour_manager.success(&total_authors.to_string()));
    }
    
    if let Some(unique_files) = data.get("unique_files").and_then(|v| v.as_u64()) {
        println!("{}: {}", colour_manager.info("Files Modified"), colour_manager.success(&unique_files.to_string()));
    }
    
    if let Some(top_authors) = data.get("top_authors").and_then(|v| v.as_array()) {
        println!("\n{}", colour_manager.info("Top Contributors:"));
        
        let headers = &["Rank", "Author", "Commits", "Percentage"];
        let mut rows = Vec::new();
        
        let total_commits: u64 = top_authors.iter()
            .filter_map(|author| author.get("commits").and_then(|v| v.as_u64()))
            .sum();
        
        for (i, author) in top_authors.iter().enumerate() {
            if let (Some(name), Some(commits)) = (
                author.get("name").and_then(|v| v.as_str()),
                author.get("commits").and_then(|v| v.as_u64())
            ) {
                let percentage = if total_commits > 0 {
                    (commits as f64 / total_commits as f64) * 100.0
                } else {
                    0.0
                };
                
                rows.push(vec![
                    format!("{}", i + 1),
                    name.to_string(),
                    commits.to_string(),
                    format!("{:.1}%", percentage)
                ]);
            }
        }
        
        print!("{}", format_compact_table(headers, &rows));
    }
    
    // Display author-specific insights instead of file analysis for author reports
    display_author_insights(data.as_object().unwrap_or(&serde_json::Map::new()), colour_manager, data).await?;
    
    Ok(())
}

/// Display author-specific insights showing top files by author
async fn display_author_insights(data: &serde_json::Map<String, serde_json::Value>, colour_manager: &display::ColourManager, full_data: &serde_json::Value) -> Result<()> {
    
    // Extract output limit settings from full data
    let output_all = full_data.get("output_all").and_then(|v| v.as_bool()).unwrap_or(false);
    let output_limit = full_data.get("output_limit").and_then(|v| v.as_u64()).map(|v| v as usize);
    
    // Get author stats and file statistics
    let author_stats = data.get("author_stats").and_then(|v| v.as_object());
    let file_stats = data.get("file_statistics");
    
    if let (Some(author_stats), Some(file_stats)) = (author_stats, file_stats) {
        // Convert author stats to a sorted vector
        let mut authors: Vec<_> = author_stats.iter()
            .filter_map(|(name, commits)| {
                commits.as_u64().map(|count| (name.clone(), count))
            })
            .collect();
        
        // Sort by commit count descending and apply limits
        authors.sort_by(|a, b| b.1.cmp(&a.1));
        let authors_to_show = if output_all {
            authors.len()
        } else if let Some(limit) = output_limit {
            limit.min(authors.len())
        } else {
            5.min(authors.len()) // Default limit for insights
        };
        let top_authors = authors.into_iter().take(authors_to_show).collect::<Vec<_>>();
        
        // Extract top files data for each author from file statistics
        if let Some(top_files_by_commits) = file_stats.get("top_files_by_commits").and_then(|v| v.as_array()) {
            
            for (author_name, _author_commits) in &top_authors {
                println!("\n{} {}:", colour_manager.info("Top Contributions by"), colour_manager.success(author_name));
                
                let headers = &["File", "Author Commits", "Author Impact"];
                let mut rows = Vec::new();
                
                // Find files this author has worked on (simplified - showing top files from overall data)
                // Note: This is showing overall file stats, not per-author stats yet
                // TODO: Implement proper per-author file tracking in the future
                let files_to_show = if output_all {
                    top_files_by_commits.len()
                } else if let Some(limit) = output_limit {
                    limit.min(top_files_by_commits.len())
                } else {
                    10.min(top_files_by_commits.len()) // Default limit for file insights
                };
                
                let mut file_count = 0;
                for file_data in top_files_by_commits.iter().take(files_to_show) {
                    if let (
                        Some(path),
                        Some(total_commits),
                        Some(net_change)
                    ) = (
                        file_data.get("path").and_then(|v| v.as_str()),
                        file_data.get("commits").and_then(|v| v.as_u64()),
                        file_data.get("net_change").and_then(|v| v.as_i64())
                    ) {
                        // Truncate long paths for better display
                        let display_path = if path.len() > 50 {
                            format!("...{}", &path[path.len()-47..])
                        } else {
                            path.to_string()
                        };
                        
                        // Estimate author's portion (this is approximate - in reality we'd track per-author)
                        // For now, assume author contributed proportionally to their overall commit percentage
                        let total_all_commits: u64 = top_authors.iter().map(|(_, c)| c).sum();
                        let author_commit_ratio = (*_author_commits as f64) / (total_all_commits as f64);
                        let estimated_author_commits = (total_commits as f64 * author_commit_ratio).round() as u64;
                        let estimated_author_impact = (net_change as f64 * author_commit_ratio).round() as i64;
                        
                        let impact_str = if estimated_author_impact >= 0 {
                            format!("+{}", estimated_author_impact)
                        } else {
                            format!("{}", estimated_author_impact)
                        };
                        
                        rows.push(vec![
                            display_path,
                            format!("{}", estimated_author_commits.max(1)), // At least 1 if they worked on it
                            impact_str
                        ]);
                        
                        file_count += 1;
                        if file_count >= 5 { // Show top 5 files per author
                            break;
                        }
                    }
                }
                
                if file_count > 0 {
                    print!("{}", format_compact_table(headers, &rows));
                } else {
                    println!("  No file data available");
                }
            }
        }
    }
    
    Ok(())
}

/// Display commit analysis report with colors
async fn display_commit_report(data: &serde_json::Value, colour_manager: &display::ColourManager, compact: bool) -> Result<()> {
    if compact {
        // Compact format: single-line output
        let total_commits = data.get("total_commits").and_then(|v| v.as_u64()).unwrap_or(0);
        let unique_authors = data.get("unique_authors").and_then(|v| v.as_u64()).unwrap_or(0);
        let unique_files = data.get("unique_files").and_then(|v| v.as_u64()).unwrap_or(0);
        let avg_commits = data.get("avg_commits_per_author").and_then(|v| v.as_f64()).unwrap_or(0.0);
        
        println!("Commits: {} | Authors: {} | Files: {} | Avg/Author: {:.1}",
                 total_commits, unique_authors, unique_files, avg_commits);
        return Ok(());
    }
    
    // Standard format
    println!("\n{}", colour_manager.highlight("=== Commit Analysis Report ==="));
    
    if let Some(total_commits) = data.get("total_commits").and_then(|v| v.as_u64()) {
        println!("{}: {}", colour_manager.info("Total Commits"), colour_manager.success(&total_commits.to_string()));
    }
    
    if let Some(unique_authors) = data.get("unique_authors").and_then(|v| v.as_u64()) {
        println!("{}: {}", colour_manager.info("Unique Authors"), colour_manager.success(&unique_authors.to_string()));
    }
    
    if let Some(unique_files) = data.get("unique_files").and_then(|v| v.as_u64()) {
        println!("{}: {}", colour_manager.info("Files Modified"), colour_manager.success(&unique_files.to_string()));
    }
    
    if let Some(avg_commits) = data.get("avg_commits_per_author").and_then(|v| v.as_f64()) {
        println!("{}: {}", colour_manager.info("Average Commits per Author"), colour_manager.success(&format!("{:.1}", avg_commits)));
    }
    
    // Display Path Analysis Report if file statistics are available
    if let Some(file_stats) = data.get("file_statistics") {
        display_path_analysis_report(file_stats, colour_manager, data).await?;
    }
    
    Ok(())
}

/// Display metrics report with colors
async fn display_metrics_report(data: &serde_json::Value, colour_manager: &display::ColourManager, compact: bool) -> Result<()> {
    if compact {
        // Compact format: single-line output
        let total_files = data.get("total_files").and_then(|v| v.as_u64()).unwrap_or(0);
        let total_lines = data.get("total_lines").and_then(|v| v.as_u64()).unwrap_or(0);
        let avg_lines = data.get("average_lines_per_file").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let total_complexity = data.get("total_complexity").and_then(|v| v.as_f64()).unwrap_or(0.0);

        // Calculate average complexity and risk assessment
        let avg_complexity = if total_files > 0 { total_complexity / total_files as f64 } else { 0.0 };
        let complexity_risk = if avg_complexity <= 10.0 {
            "Low"
        } else if avg_complexity <= 20.0 {
            "Moderate"
        } else if avg_complexity <= 50.0 {
            "High"
        } else {
            "Very High"
        };

        println!("Source Code Files: {} | Lines: {} | Lines/File(avg): ({:.1}) | Cyclomatic Complexity(avg): {} ({:.1})",
                 total_files, total_lines, avg_lines, complexity_risk, avg_complexity);
        return Ok(());
    }
    
    // Standard format
    println!("\n{}", colour_manager.highlight("=== Metrics Report ==="));
    
    // Display basic metrics with colors if available
    if let Some(total_files) = data.get("total_files").and_then(|v| v.as_u64()) {
        println!("{}: {}", colour_manager.info("Total Files"), colour_manager.success(&total_files.to_string()));
    }
    
    if let Some(total_lines) = data.get("total_lines").and_then(|v| v.as_u64()) {
        println!("{}: {}", colour_manager.info("Total Lines"), colour_manager.success(&total_lines.to_string()));
    }
    
    if let Some(avg_lines) = data.get("average_lines_per_file").and_then(|v| v.as_f64()) {
        println!("{}: {}", colour_manager.info("Average Lines per File"), colour_manager.success(&format!("{:.1}", avg_lines)));
    }
    
    if let Some(complexity) = data.get("total_complexity").and_then(|v| v.as_f64()) {
        println!("{}: {}", colour_manager.info("Total Complexity"), colour_manager.success(&format!("{:.1}", complexity)));
    }
    
    Ok(())
}

/// Display export report with colors
async fn display_export_report(data: &serde_json::Value, colour_manager: &display::ColourManager, compact: bool) -> Result<()> {
    if compact {
        // Compact format: single-line JSON output
        println!("Export: {}", serde_json::to_string(data)?);
    } else {
        // Standard format
        println!("\n{}", colour_manager.highlight("=== Export Report ==="));
        println!("{}", serde_json::to_string_pretty(data)?);
    }
    Ok(())
}

/// Display path analysis report showing file commit statistics with colors
async fn display_path_analysis_report(file_stats: &serde_json::Value, colour_manager: &display::ColourManager, full_data: &serde_json::Value) -> Result<()> {
    
    // Extract output limit settings from full data
    let output_all = full_data.get("output_all").and_then(|v| v.as_bool()).unwrap_or(false);
    let output_limit = full_data.get("output_limit").and_then(|v| v.as_u64()).map(|v| v as usize);
    
    println!("\n{}", colour_manager.highlight("=== Path Analysis Report ==="));
    
    if let Some(total_files) = file_stats.get("total_files").and_then(|v| v.as_u64()) {
        println!("{}: {}", colour_manager.info("Total Files"), colour_manager.success(&total_files.to_string()));
    }
    
    if let Some(total_commits) = file_stats.get("total_commits_across_files").and_then(|v| v.as_u64()) {
        println!("{}: {}", colour_manager.info("Total File Modifications"), colour_manager.success(&total_commits.to_string()));
    }
    
    // Display top files by commit count
    if let Some(top_files) = file_stats.get("top_files_by_commits").and_then(|v| v.as_array()) {
        if !top_files.is_empty() {
            println!("\n{}", colour_manager.info("Most Frequently Modified Files (by commit count):"));
            
            let files_to_show = if output_all {
                top_files.len()
            } else if let Some(limit) = output_limit {
                limit.min(top_files.len())
            } else {
                10.min(top_files.len()) // Default limit
            };
            
            let headers = &["Rank", "File Path", "Commits", "Lines +", "Lines -", "Net Change", "Current Lines", "Authors"];
            let mut rows = Vec::new();
            
            for (i, file) in top_files.iter().take(files_to_show).enumerate() {
                if let (
                    Some(path),
                    Some(commits),
                    Some(lines_added),
                    Some(lines_removed), 
                    Some(net_change),
                    Some(current_lines_display),
                    Some(authors)
                ) = (
                    file.get("path").and_then(|v| v.as_str()),
                    file.get("commits").and_then(|v| v.as_u64()),
                    file.get("lines_added").and_then(|v| v.as_u64()),
                    file.get("lines_removed").and_then(|v| v.as_u64()),
                    file.get("net_change").and_then(|v| v.as_i64()),
                    file.get("current_lines_display").and_then(|v| v.as_str()),
                    file.get("authors").and_then(|v| v.as_u64())
                ) {
                    // Truncate long paths
                    let display_path = if path.len() > 40 {
                        format!("...{}", &path[path.len()-37..])
                    } else {
                        path.to_string()
                    };
                    
                    let net_change_str = if net_change >= 0 {
                        format!("+{}", net_change)
                    } else {
                        format!("{}", net_change)
                    };
                    
                    rows.push(vec![
                        format!("{}", i + 1),
                        display_path,
                        format!("{}", commits),
                        format!("{}", lines_added),
                        format!("{}", lines_removed),
                        net_change_str,
                        current_lines_display.to_string(),
                        format!("{}", authors)
                    ]);
                }
            }
            
            print!("{}", format_compact_table(headers, &rows));
        }
    }
    
    // Display top files by net change
    if let Some(top_changes) = file_stats.get("top_files_by_changes").and_then(|v| v.as_array()) {
        if !top_changes.is_empty() {
            println!("\n{}", colour_manager.info("Largest Net Changes (by lines changed):"));
            
            let changes_to_show = if output_all {
                top_changes.len()
            } else if let Some(limit) = output_limit {
                limit.min(top_changes.len())
            } else {
                10.min(top_changes.len()) // Default limit
            };
            
            let headers = &["Rank", "File Path", "Net Change", "Lines +", "Lines -", "Current Lines", "Commits"];
            let mut rows = Vec::new();
            
            for (i, file) in top_changes.iter().take(changes_to_show).enumerate() {
                if let (
                    Some(path),
                    Some(net_change),
                    Some(lines_added),
                    Some(lines_removed),
                    Some(current_lines_display),
                    Some(commits)
                ) = (
                    file.get("path").and_then(|v| v.as_str()),
                    file.get("net_change").and_then(|v| v.as_i64()),
                    file.get("lines_added").and_then(|v| v.as_u64()),
                    file.get("lines_removed").and_then(|v| v.as_u64()),
                    file.get("current_lines_display").and_then(|v| v.as_str()),
                    file.get("commits").and_then(|v| v.as_u64())
                ) {
                    // Truncate long paths
                    let display_path = if path.len() > 40 {
                        format!("...{}", &path[path.len()-37..])
                    } else {
                        path.to_string()
                    };
                    
                    let net_change_str = if net_change >= 0 {
                        format!("+{}", net_change)
                    } else {
                        format!("{}", net_change)
                    };
                    
                    rows.push(vec![
                        format!("{}", i + 1),
                        display_path,
                        net_change_str,
                        format!("{}", lines_added),
                        format!("{}", lines_removed),
                        current_lines_display.to_string(),
                        format!("{}", commits)
                    ]);
                }
            }
            
            print!("{}", format_compact_table(headers, &rows));
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processed_statistics_compact_format() {
        let stats = ProcessedStatistics {
            files_processed: 42,
            commits_processed: 156,
            authors_processed: 7,
        };
        
        let compact = stats.to_compact_format();
        assert_eq!(compact, "Files: 42 | Commits: 156 | Authors: 7");
        assert!(!compact.contains('\n'), "Compact format should not contain newlines");
    }
}
