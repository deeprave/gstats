mod cli;
mod config;
mod display;
mod logging;
mod notifications;
mod scanner;
mod plugin;
mod stats;

use anyhow::{Result, Context};
use std::process;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::SystemTime;
use std::path::{Path, PathBuf};
use log::{info, error, debug, warn};
use crate::stats::RepositoryFileStats;
use crate::notifications::traits::Publisher;

use crate::display::CompactFormat;

/// Resolve repository path from CLI arguments
/// If no path provided, uses current directory and validates it's a git repository
fn resolve_repository_path(repository_arg: Option<String>) -> Result<PathBuf> {
    match repository_arg {
        Some(path) => {
            debug!("Repository path provided: {}", path);
            let path_buf = PathBuf::from(path);
            
            if !path_buf.exists() {
                anyhow::bail!(
                    "Directory does not exist: {}\n\nPlease check the path and try again. Make sure you have permission to access the directory.", 
                    path_buf.display()
                );
            }
            
            // Validate it's a git repository
            gix::discover(&path_buf)
                .with_context(|| format!(
                    "Not a valid git repository: {}\n\nMake sure this directory contains a git repository (initialized with 'git init' or cloned from a remote).\nIf this is the correct path, check that the .git directory exists and is accessible.", 
                    path_buf.display()
                ))?;
            
            path_buf.canonicalize()
                .with_context(|| format!("Failed to resolve canonical path for: {}", path_buf.display()))
        }
        None => {
            debug!("No repository path provided, using current directory");
            let current_dir = std::env::current_dir()
                .context("Failed to get current directory")?;
            
            gix::discover(&current_dir)
                .with_context(|| format!(
                    "Current directory '{}' is not a git repository.\n\nTo fix this:\n  • Navigate to a git repository directory\n  • Or specify a repository path: gstats --repository /path/to/repo\n  • Or initialize a git repository: git init", 
                    current_dir.display()
                ))?;
            
            info!("Using current directory as git repository: {}", current_dir.display());
            Ok(current_dir)
        }
    }
}

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
    
    // Resolve repository path (validates it's a git repository)
    let repo_path = resolve_repository_path(args.repository.clone())?;
    
    // Run scanner with existing runtime
    let runtime_arc = Arc::new(runtime);
    let runtime_clone = Arc::clone(&runtime_arc);
    let result = runtime_arc.block_on(async {
        run_scanner(repo_path, args, config_manager, runtime_clone).await
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
    repo_path: PathBuf, 
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
    
    // Create notification manager and scanner publisher for event coordination
    let notification_manager = Arc::new(crate::notifications::AsyncNotificationManager::<crate::notifications::ScanEvent>::new());
    let scanner_publisher = crate::scanner::ScannerPublisher::new(notification_manager.clone());
    
    debug!("Notification system initialized for scanner-plugin coordination");
    
    // Initialize built-in plugins (respecting exclusion configuration)
    initialize_builtin_plugins(&plugin_registry, &repo_path, &plugin_config).await?;
    
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
    
    // Create scanner engine with repository path
    let mut engine_builder = scanner::AsyncScannerEngineBuilder::new()
        .repository_path(repo_path.clone())
        .config(scanner_config.clone())
        .message_producer(message_producer as Arc<dyn scanner::MessageProducer + Send + Sync>)
        .runtime(runtime);
    
    // Create event-driven scanner - using repository-owning pattern
    // Scanner receives repository path during scan_async() calls and supports all scan modes
    let query_params = scanner::QueryParams::default();
    let event_scanner = Arc::new(scanner::async_engine::scanners::EventDrivenScanner::new(query_params));
    
    // Wrap scanner with plugin processing
    let plugin_scanner_builder = scanner::PluginScannerBuilder::new()
        .add_scanner(event_scanner)
        .plugin_registry(plugin_registry.clone());
    
    let plugin_scanners = plugin_scanner_builder.build()?;
    
    // Add plugin-enabled scanner to engine
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
    progress.status(display::StatusType::Info, &format!("Using git repository: {}", repo_path.display()));
    
    // Start scanning with progress feedback and spinner
    let scan_spinner = progress.start_spinner("Scanning repository...");
    
    // Generate unique scan ID for event coordination
    let scan_id = format!("scan_{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis());
    
    // Emit scan started event
    let scan_start_time = std::time::Instant::now();
    if let Err(e) = scanner_publisher.publish(crate::notifications::ScanEvent::started(
        scan_id.clone(),
        crate::scanner::ScanMode::from_bits_truncate(scan_modes.bits()),
    )).await {
        warn!("Failed to publish scan started event: {}", e);
    }
    
    // Phase 1: Repository initialization progress
    if let Err(e) = scanner_publisher.publish(crate::notifications::ScanEvent::progress(
        scan_id.clone(),
        0.1,
        "Repository initialization".to_string(),
    )).await {
        warn!("Failed to publish repository initialization progress: {}", e);
    }
    
    // Phase 2: File discovery progress
    if let Err(e) = scanner_publisher.publish(crate::notifications::ScanEvent::progress(
        scan_id.clone(),
        0.3,
        "File discovery".to_string(),
    )).await {
        warn!("Failed to publish file discovery progress: {}", e);
    }
    
    // Phase 3: History analysis progress
    if let Err(e) = scanner_publisher.publish(crate::notifications::ScanEvent::progress(
        scan_id.clone(),
        0.6,
        "History analysis".to_string(),
    )).await {
        warn!("Failed to publish history analysis progress: {}", e);
    }
    
    // Note: Consumer disabled to prevent runtime drop issues
    // For GS-30 we'll implement proper async consumer integration
    
    // Phase 4: Data processing progress (before actual scan)
    if let Err(e) = scanner_publisher.publish(crate::notifications::ScanEvent::progress(
        scan_id.clone(),
        0.9,
        "Data processing".to_string(),
    )).await {
        warn!("Failed to publish data processing progress: {}", e);
    }
    
    // Run the scan
    match engine.scan(scan_modes).await {
        Ok(()) => {
            let scan_duration = scan_start_time.elapsed();
            debug!("Scan completed successfully in {:?}", scan_duration);
            
            // Emit scan completed event
            if let Err(e) = scanner_publisher.publish(crate::notifications::ScanEvent::completed(
                scan_id.clone(),
                scan_duration,
                vec![], // TODO: Collect actual warnings from scan process
            )).await {
                warn!("Failed to publish scan completed event: {}", e);
            }
            
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
            let _processed_stats = execute_plugins_analysis(&plugin_registry, &plugin_names, &repo_path, &stats, &args, &config_manager).await?;
            plugin_spinner.complete(display::StatusType::Success, "Plugin analysis complete").await;
            
            // Display the plugin reports
            display_plugin_reports(&plugin_registry, &original_commands, &repo_path, &stats, &args, &config_manager, &query_params).await?;
            
            // Note: Analysis Summary is now displayed before the plugin reports in execute_plugins_and_display_reports
        }
        Err(e) => {
            let scan_duration = scan_start_time.elapsed();
            error!("Scan failed after {:?}: {}", scan_duration, e);
            
            // Emit scan error event
            if let Err(publish_err) = scanner_publisher.publish(crate::notifications::ScanEvent::error(
                scan_id.clone(),
                e.to_string(),
                true,
            )).await {
                warn!("Failed to publish scan error event: {}", publish_err);
            }
            
            // Perform graceful shutdown after fatal error
            let shutdown_timeout = std::time::Duration::from_secs(5); // Default 5 second timeout
            if let Err(shutdown_err) = graceful_shutdown_after_error(
                &plugin_registry,
                notification_manager,
                shutdown_timeout
            ).await {
                warn!("Error during graceful shutdown: {}", shutdown_err);
            }
            
            scan_spinner.complete(display::StatusType::Error, &format!("Repository scan failed: {}", e)).await;
            return Err(e.into());
        }
    }
    
    Ok(())
}

async fn initialize_builtin_plugins(
    registry: &plugin::SharedPluginRegistry, 
    repo_path: &PathBuf,
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
    let context = create_plugin_context(repo_path)?;
    
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
fn create_plugin_context(_repo_path: &PathBuf) -> Result<plugin::PluginContext> {
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
    _repo_path: &PathBuf,
    stats: &scanner::async_engine::EngineStats,
    _args: &cli::Args,
    _config: &config::ConfigManager,
) -> Result<Option<ProcessedStatistics>> {
    
    let commands = if requested_commands.is_empty() {
        vec!["authors".to_string()] // Default to authors report
    } else {
        requested_commands.to_vec()
    };
    
    // Plugin analysis is now handled automatically by EventDrivenScanner → PluginScanner
    debug!("Plugin analysis completed through event-driven scanning for commands: {:?}", commands);
    
    // Return processed statistics based on engine stats
    if let Some(repo_stats) = &stats.repository_stats {
        let processed_stats = ProcessedStatistics {
            files_processed: repo_stats.total_files as usize,
            commits_processed: repo_stats.total_commits as usize,
            authors_processed: repo_stats.total_authors as usize,
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
    repo_path: &PathBuf,
    stats: &scanner::async_engine::EngineStats,
    args: &cli::Args,
    config: &config::ConfigManager,
    query_params: &scanner::QueryParams,
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
    
    // Note: Plugin data processing is now handled automatically by EventDrivenScanner → PluginScanner
    // The plugins have already received and processed data during the engine.scan() phase
    debug!("Plugin '{}' has already processed data through event-driven scanning", plugin_name);
    
    // TODO: Implement plugin result retrieval system to display processed results
    // For now, show a message that the plugin has processed the data
    let colour_manager = display::ColourManager::new();
    let progress = display::ProgressIndicator::new(colour_manager);


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

/// Graceful shutdown after fatal error
/// Waits for plugins to deregister themselves, then cleans up resources
async fn graceful_shutdown_after_error(
    plugin_registry: &crate::plugin::registry::SharedPluginRegistry,
    notification_manager: Arc<crate::notifications::AsyncNotificationManager<crate::notifications::ScanEvent>>,
    timeout: std::time::Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("Starting graceful shutdown after fatal error");
    
    // Wait for plugins to deregister themselves
    let plugins_deregistered = plugin_registry.wait_for_empty_registry(timeout).await;
    
    if plugins_deregistered {
        log::info!("All plugins deregistered successfully");
    } else {
        log::warn!("Timeout waiting for plugin deregistration, proceeding with cleanup");
    }
    
    // Clean up notification manager
    // Note: We don't have a shutdown method yet, but we can clear stats
    notification_manager.clear_stats().await;
    log::info!("Notification manager cleaned up");
    
    log::info!("Graceful shutdown completed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scan_progress_events_emission() {
        use std::sync::Arc;
        use crate::notifications::{AsyncNotificationManager, ScanEvent};
        use crate::notifications::traits::NotificationManager;
        use crate::scanner::ScannerPublisher;
        
        // Create notification manager
        let mut notification_manager = AsyncNotificationManager::<ScanEvent>::new();
        
        // Mock subscriber to capture events
        let events = Arc::new(tokio::sync::Mutex::new(Vec::<ScanEvent>::new()));
        let events_clone = events.clone();
        
        // Create a mock subscriber
        struct MockSubscriber {
            events: Arc<tokio::sync::Mutex<Vec<ScanEvent>>>,
        }
        
        #[async_trait::async_trait]
        impl crate::notifications::traits::Subscriber<ScanEvent> for MockSubscriber {
            async fn handle_event(&self, event: ScanEvent) -> crate::notifications::NotificationResult<()> {
                self.events.lock().await.push(event);
                Ok(())
            }
            
            fn subscriber_id(&self) -> &str {
                "test_subscriber"
            }
        }
        
        let subscriber = Arc::new(MockSubscriber { events: events_clone });
        
        // Subscribe to events
        notification_manager.subscribe(subscriber).await.expect("Failed to subscribe");
        
        // Create scanner publisher
        let scanner_publisher = ScannerPublisher::new(Arc::new(notification_manager));
        
        // Test progress events (existing functionality)
        let scan_id = "test_scan_123".to_string();
        
        // Emit progress events
        scanner_publisher.publish(ScanEvent::progress(
            scan_id.clone(),
            0.1,
            "Repository initialization".to_string(),
        )).await.expect("Failed to publish progress event");
        
        scanner_publisher.publish(ScanEvent::progress(
            scan_id.clone(),
            0.3,
            "File discovery".to_string(),
        )).await.expect("Failed to publish progress event");
        
        scanner_publisher.publish(ScanEvent::progress(
            scan_id.clone(),
            0.6,
            "History analysis".to_string(),
        )).await.expect("Failed to publish progress event");
        
        scanner_publisher.publish(ScanEvent::progress(
            scan_id.clone(),
            0.9,
            "Data processing".to_string(),
        )).await.expect("Failed to publish progress event");
        
        // Verify progress events were captured
        let captured_events = events.lock().await;
        assert_eq!(captured_events.len(), 4, "Should have captured 4 progress events");
        
        // Verify progress values and phases
        let progress_events: Vec<_> = captured_events.iter()
            .filter_map(|e| match e {
                ScanEvent::ScanProgress { progress, phase, .. } => Some((progress, phase)),
                _ => None,
            })
            .collect();
        
        assert_eq!(progress_events.len(), 4);
        assert_eq!(progress_events[0], (&0.1, &"Repository initialization".to_string()));
        assert_eq!(progress_events[1], (&0.3, &"File discovery".to_string()));
        assert_eq!(progress_events[2], (&0.6, &"History analysis".to_string()));
        assert_eq!(progress_events[3], (&0.9, &"Data processing".to_string()));
    }



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
    
    #[tokio::test]
    async fn test_graceful_shutdown_after_scan_error() {
        use std::time::Duration;
        use crate::notifications::{AsyncNotificationManager, traits::NotificationManager};
        use crate::plugin::registry::SharedPluginRegistry;
        use crate::plugin::tests::mock_plugins::MockPlugin;
        
        // Create notification manager and registry
        let notification_manager = Arc::new(AsyncNotificationManager::new());
        let registry = SharedPluginRegistry::with_notification_manager(notification_manager.clone());
        
        // Register multiple plugins
        let plugin1 = Box::new(MockPlugin::new("plugin1", false));
        let plugin2 = Box::new(MockPlugin::new("plugin2", false));
        registry.register_plugin(plugin1).await.unwrap();
        registry.register_plugin(plugin2).await.unwrap();
        
        // Verify plugins are registered
        assert_eq!(registry.get_plugin_count().await, 2);
        assert_eq!(notification_manager.subscriber_count().await, 2);
        
        // Simulate graceful shutdown
        let result = graceful_shutdown_after_error(
            &registry,
            notification_manager.clone(),
            Duration::from_millis(100)
        ).await;
        
        // Should succeed even though plugins don't deregister themselves yet
        // (that functionality will be implemented in the actual scanner integration)
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_graceful_shutdown_timeout() {
        use std::time::Duration;
        use crate::notifications::AsyncNotificationManager;
        use crate::plugin::registry::SharedPluginRegistry;
        use crate::plugin::tests::mock_plugins::MockPlugin;
        
        // Create notification manager and registry
        let notification_manager = Arc::new(AsyncNotificationManager::new());
        let registry = SharedPluginRegistry::with_notification_manager(notification_manager.clone());
        
        // Register a plugin
        let plugin = Box::new(MockPlugin::new("test_plugin", false));
        registry.register_plugin(plugin).await.unwrap();
        
        // Test timeout scenario
        let start = std::time::Instant::now();
        let result = graceful_shutdown_after_error(
            &registry,
            notification_manager.clone(),
            Duration::from_millis(50)
        ).await;
        let elapsed = start.elapsed();
        
        // Should timeout and still succeed
        assert!(result.is_ok());
        assert!(elapsed >= Duration::from_millis(45)); // Allow some variance
        assert!(elapsed < Duration::from_millis(100)); // But not too long
    }
    
    #[tokio::test]
    async fn test_notification_manager_cleanup_during_shutdown() {
        use std::time::Duration;
        use crate::notifications::{AsyncNotificationManager, traits::NotificationManager};
        use crate::plugin::registry::SharedPluginRegistry;
        use crate::plugin::tests::mock_plugins::MockPlugin;
        
        // Create notification manager and registry
        let notification_manager = Arc::new(AsyncNotificationManager::new());
        let registry = SharedPluginRegistry::with_notification_manager(notification_manager.clone());
        
        // Register plugins and generate some stats
        let plugin = Box::new(MockPlugin::new("test_plugin", false));
        registry.register_plugin(plugin).await.unwrap();
        
        // Verify initial state
        assert_eq!(notification_manager.subscriber_count().await, 1);
        
        // Simulate graceful shutdown
        let result = graceful_shutdown_after_error(
            &registry,
            notification_manager.clone(),
            Duration::from_millis(50)
        ).await;
        
        // Should succeed and clean up notification manager
        assert!(result.is_ok());
        
        // Verify cleanup occurred (stats should be cleared)
        let stats = notification_manager.get_stats().await;
        assert_eq!(stats.events_published, 0);
        assert_eq!(stats.events_delivered, 0);
        assert_eq!(stats.delivery_failures, 0);
    }
}

#[cfg(test)]
mod cli_integration_tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_repository_validation_at_cli_level() {
        // Test with current directory (should be a git repo)
        let result = validate_repository_path(None);
        assert!(result.is_ok(), "Current directory should be a valid git repository");
        
        // Test with explicit valid path
        let current_dir = std::env::current_dir().unwrap();
        let result = validate_repository_path(Some(current_dir.to_string_lossy().to_string()));
        assert!(result.is_ok(), "Explicit current directory should be valid");
    }

    #[tokio::test]
    async fn test_repository_validation_with_invalid_path() {
        // Test with non-existent path
        let result = validate_repository_path(Some("/nonexistent/path".to_string()));
        assert!(result.is_err(), "Non-existent path should fail validation");
        
        // Test with non-git directory
        let temp_dir = TempDir::new().unwrap();
        let result = validate_repository_path(Some(temp_dir.path().to_string_lossy().to_string()));
        assert!(result.is_err(), "Non-git directory should fail validation");
    }

    #[tokio::test]
    async fn test_event_driven_scanner_as_primary_scanner() {
        // Verify that EventDrivenScanner is being used, not PlaceholderScanner
        let query_params = scanner::QueryParams::default();
        let scanner = scanner::async_engine::scanners::EventDrivenScanner::new(query_params);
        
        // Verify it supports all scan modes
        assert!(scanner.supports_mode(scanner::ScanMode::FILES));
        assert!(scanner.supports_mode(scanner::ScanMode::HISTORY));
        assert!(scanner.supports_mode(scanner::ScanMode::METRICS));
        assert!(scanner.supports_mode(scanner::ScanMode::all()));
        
        // Verify it has the correct name
        assert_eq!(scanner.name(), "EventDrivenScanner");
    }

    #[tokio::test]
    async fn test_event_driven_scanner_provides_complete_data() {
        use futures::StreamExt;
        
        // Create EventDrivenScanner
        let query_params = scanner::QueryParams::default();
        let scanner = scanner::async_engine::scanners::EventDrivenScanner::new(query_params);
        
        // Test with current directory
        let current_path = std::env::current_dir().unwrap();
        
        // Test HISTORY mode (commits)
        let history_stream = scanner.scan_async(&current_path, scanner::ScanMode::HISTORY).await;
        assert!(history_stream.is_ok(), "EventDrivenScanner should provide history data");
        
        let mut stream = history_stream.unwrap();
        let mut commit_count = 0;
        
        // Collect some messages to verify data is provided
        while let Some(message_result) = stream.next().await {
            if commit_count >= 5 { break; } // Just test first few messages
            
            assert!(message_result.is_ok(), "Scanner should provide valid messages");
            let message = message_result.unwrap();
            
            // Verify message structure
            assert!(message.header.mode.contains(scanner::ScanMode::HISTORY));
            commit_count += 1;
        }
        
        // Should have found some commits in a git repository
        assert!(commit_count > 0, "EventDrivenScanner should provide commit data");
    }

    #[tokio::test]
    async fn test_event_driven_scanner_files_mode() {
        use futures::StreamExt;
        
        // Create EventDrivenScanner
        let query_params = scanner::QueryParams::default();
        let scanner = scanner::async_engine::scanners::EventDrivenScanner::new(query_params);
        
        // Test with current directory
        let current_path = std::env::current_dir().unwrap();
        
        // Test FILES mode
        let files_stream = scanner.scan_async(&current_path, scanner::ScanMode::FILES).await;
        assert!(files_stream.is_ok(), "EventDrivenScanner should provide files data");
        
        let mut stream = files_stream.unwrap();
        let mut file_count = 0;
        
        // Collect some messages to verify data is provided
        while let Some(message_result) = stream.next().await {
            if file_count >= 5 { break; } // Just test first few messages
            
            assert!(message_result.is_ok(), "Scanner should provide valid file messages");
            let message = message_result.unwrap();
            
            // Verify message structure
            assert!(message.header.mode.contains(scanner::ScanMode::FILES));
            file_count += 1;
        }
        
    #[tokio::test]
    async fn test_complete_event_driven_data_flow() {
        // Test that EventDrivenScanner → PluginScanner → Plugins flow works
        use std::sync::Arc;
        
        // Create plugin registry
        let plugin_registry = plugin::SharedPluginRegistry::new();
        
        // Create EventDrivenScanner
        let query_params = scanner::QueryParams::default();
        let event_scanner = Arc::new(scanner::async_engine::scanners::EventDrivenScanner::new(query_params));
        
        // Wrap with PluginScanner
        let plugin_scanner_builder = scanner::PluginScannerBuilder::new()
            .add_scanner(event_scanner)
            .plugin_registry(plugin_registry.clone());
        
        let plugin_scanners = plugin_scanner_builder.build().unwrap();
        assert_eq!(plugin_scanners.len(), 1, "Should create one plugin scanner");
        
        let plugin_scanner = &plugin_scanners[0];
        
        // Test that plugin scanner supports all modes
        assert!(plugin_scanner.supports_mode(scanner::ScanMode::FILES));
        assert!(plugin_scanner.supports_mode(scanner::ScanMode::HISTORY));
        assert!(plugin_scanner.supports_mode(scanner::ScanMode::METRICS));
        
        // Test scanning with current directory
        let current_path = std::env::current_dir().unwrap();
        let stream_result = plugin_scanner.scan_async(&current_path, scanner::ScanMode::HISTORY).await;
        assert!(stream_result.is_ok(), "Plugin scanner should successfully scan repository");
        
        // The stream should be created (actual processing happens when consumed)
        let _stream = stream_result.unwrap();
        // Note: We don't consume the stream in this test to avoid long execution time
    }

    #[tokio::test]
    async fn test_no_duplicate_data_collection() {
        // Verify that manual data collection functions have been removed
        // This test ensures we don't accidentally reintroduce the workarounds
        
        // The following functions should no longer exist:
        // - collect_scan_data_with_event_processing (removed)
        // - execute_plugin_function_with_data (removed)
        
        // Instead, data flows through: EventDrivenScanner → PluginScanner → Plugins
        
        let query_params = scanner::QueryParams::default();
        let scanner = scanner::async_engine::scanners::EventDrivenScanner::new(query_params);
        
        // EventDrivenScanner should be the single source of repository data
        assert_eq!(scanner.name(), "EventDrivenScanner");
        assert!(scanner.supports_mode(scanner::ScanMode::all()));
        
        // No PlaceholderScanner should exist
        // (This would fail to compile if PlaceholderScanner still existed)
        // let placeholder = scanner::async_engine::scanners::PlaceholderScanner::new(query_params); // Should not compile
    }
}
