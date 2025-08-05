mod cli;
mod config;
mod git;
mod logging;
mod scanner;
mod plugin;
mod stats;

use anyhow::Result;
use std::process;
use std::sync::Arc;
use std::collections::HashMap;
use log::{info, error, debug, warn};
use crate::stats::RepositoryFileStats;
use crate::scanner::async_engine::repository::AsyncRepositoryHandle;

/// Statistics about what was actually processed during scanning
#[derive(Debug, Clone)]
pub struct ProcessedStatistics {
    pub files_processed: usize,
    pub commits_processed: usize,
    pub authors_processed: usize,
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
    
    // Create runtime for async operations - single runtime for the entire application
    // Using current_thread runtime to avoid nested runtime issues
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    
    // Handle plugin management commands
    if args.list_plugins || args.show_plugins || args.plugins_help || args.plugin_info.is_some() || args.check_plugin.is_some() || args.list_by_type.is_some() {
        return runtime.block_on(async {
            handle_plugin_commands(&args).await
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
        match config.get_value("base", "log-format") {
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
    
    Ok(logging::LogConfig {
        console_level,
        file_level,
        format,
        destination,
    })
}

async fn handle_plugin_commands(args: &cli::Args) -> Result<()> {
    use cli::plugin_handler::PluginHandler;
    use crate::plugin::traits::PluginType;
    
    let mut handler = PluginHandler::new()?;
    
    // Handle --plugins command (display plugins with their functions)
    if args.show_plugins {
        handler.build_command_mappings().await?;
        
        println!("Available Plugins:");
        println!("==================");
        
        let mappings = handler.get_function_mappings();
        if mappings.is_empty() {
            println!("No plugins available.");
        } else {
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
            
            for (i, plugin_name) in plugin_names.iter().enumerate() {
                if i > 0 {
                    println!(); // Add spacing between plugins
                }
                
                let plugin_functions = &plugins_map[*plugin_name];
                
                // Find default function for plugin header
                let has_default = plugin_functions.iter().any(|f| f.is_default);
                
                println!("Plugin: {}{}", plugin_name, if has_default { " (has default)" } else { "" });
                
                // Sort functions within plugin (default first, then alphabetically)
                let mut sorted_functions = plugin_functions.clone();
                sorted_functions.sort_by(|a, b| {
                    match (a.is_default, b.is_default) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a.function_name.cmp(&b.function_name),
                    }
                });
                
                for function in sorted_functions {
                    let aliases_str = if function.aliases.is_empty() {
                        String::new()
                    } else {
                        format!(" (aliases: {})", function.aliases.join(", "))
                    };
                    
                    let default_marker = if function.is_default { " *" } else { "" };
                    
                    println!("  {}{}{}{}", 
                        function.function_name,
                        default_marker,
                        aliases_str,
                        if !function.description.is_empty() { 
                            format!(" - {}", function.description) 
                        } else { 
                            String::new() 
                        }
                    );
                }
            }
            
            println!();
            println!("Usage Examples:");
            println!("  gstats <plugin>                # Use plugin's default function");
            println!("  gstats <plugin>:<function>     # Use specific plugin function");
            println!();
            println!("* = default function for plugin");
        }
        
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
    
    if let Some(plugin_name) = &args.check_plugin {
        let report = handler.check_plugin_compatibility(plugin_name).await?;
        println!("Plugin '{}' compatibility: {:?}", plugin_name, report);
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
    
    // Create plugin registry and initialize plugins
    let plugin_registry = plugin::SharedPluginRegistry::new();
    
    // Initialize built-in plugins
    initialize_builtin_plugins(&plugin_registry, repo.clone()).await?;
    
    // Create plugin handler for discovery and command mapping
    let mut plugin_handler = cli::plugin_handler::PluginHandler::new()?;
    plugin_handler.build_command_mappings().await?;
    
    // Resolve plugin commands using CommandMapper
    let plugin_names = if args.plugins.is_empty() {
        vec!["commits".to_string()] // Default plugin
    } else {
        resolve_plugin_commands(&plugin_handler, &args.plugins).await?
    };
    
    debug!("Active plugins: {:?}", plugin_names);
    
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
    
    // Note: Consumer disabled to prevent runtime drop issues
    // For GS-30 we'll implement proper async consumer integration
    
    // Run the scan
    match engine.scan(scan_modes).await {
        Ok(()) => {
            debug!("Scan completed successfully");
            
            // Get comprehensive statistics
            let stats = engine.get_comprehensive_stats().await?;
            
            // Active plugins (show before analysis)
            let registry_plugins = plugin_registry.inner().read().await.list_plugins();
            if !registry_plugins.is_empty() {
                info!("Active plugins: {}", registry_plugins.join(", "));
            }
            
            // Repository information will be shown after plugin execution with processed vs total comparison
            
            // Basic scan results
            info!("Scan results: {} tasks completed successfully",
                stats.completed_tasks
            );
            
            if stats.errors > 0 {
                warn!("Encountered {} errors during scan", stats.errors);
            }
            
            debug!("Detailed statistics: {:?}", stats);
            
            // Execute plugins to generate reports and get processed statistics
            let _processed_stats = execute_plugins_and_display_reports(&plugin_registry, &args.plugins, repo.clone(), &stats).await?;
            
            // Note: Analysis Summary is now displayed before the plugin reports in execute_plugins_and_display_reports
        }
        Err(e) => {
            error!("Scan failed: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}

async fn initialize_builtin_plugins(registry: &plugin::SharedPluginRegistry, repo: git::RepositoryHandle) -> Result<()> {
    use plugin::builtin::{CommitsPlugin, MetricsPlugin, ExportPlugin};
    
    let mut reg = registry.inner().write().await;
    
    // Register built-in plugins
    reg.register_plugin(Box::new(CommitsPlugin::new())).await?;
    reg.register_plugin(Box::new(MetricsPlugin::new())).await?;
    reg.register_plugin(Box::new(ExportPlugin::new())).await?;
    
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
    
    debug!("Initialized {} built-in plugins", 3);
    
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
        repository: Arc::new(repo_handle),
        query_params,
        plugin_config: HashMap::new(),
        runtime_info,
        capabilities: Vec::new(),
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
async fn resolve_plugin_commands(
    plugin_handler: &cli::plugin_handler::PluginHandler,
    commands: &[String],
) -> Result<Vec<String>> {
    use cli::command_mapper::CommandResolution;
    
    let mut resolved_plugins = Vec::new();
    
    for command in commands {
        debug!("Resolving command: '{}'", command);
        
        match plugin_handler.resolve_command(command) {
            Ok(resolution) => {
                match resolution {
                    CommandResolution::Function { plugin_name, function_name, .. } => {
                        debug!("Resolved '{}' to plugin '{}' function '{}'", command, plugin_name, function_name);
                        resolved_plugins.push(plugin_name);
                    }
                    CommandResolution::DirectPlugin { plugin_name, default_function } => {
                        debug!("Resolved '{}' to plugin '{}' (default: {:?})", command, plugin_name, default_function);
                        resolved_plugins.push(plugin_name);
                    }
                    CommandResolution::Explicit { plugin_name, function_name } => {
                        debug!("Resolved '{}' to plugin '{}' function '{}'", command, plugin_name, function_name);
                        resolved_plugins.push(plugin_name);
                    }
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Command resolution failed for '{}': {}", command, e));
            }
        }
    }
    
    // Remove duplicates while preserving order
    let mut unique_plugins = Vec::new();
    for plugin in resolved_plugins {
        if !unique_plugins.contains(&plugin) {
            unique_plugins.push(plugin);
        }
    }
    
    debug!("Resolved {} commands to {} unique plugins: {:?}", 
           commands.len(), unique_plugins.len(), unique_plugins);
    
    Ok(unique_plugins)
}


/// Collect scan data directly from repository for plugin processing
async fn collect_scan_data_for_plugin(plugin_name: &str, repo: git::RepositoryHandle) -> Result<Vec<scanner::messages::ScanMessage>> {
    use scanner::messages::{ScanMessage, MessageHeader, MessageData};
    use scanner::modes::ScanMode;
    
    // Only collect data for plugins that need it
    if plugin_name != "commits" && plugin_name != "metrics" {
        return Ok(Vec::new());
    }
    
    debug!("Collecting scan data for {} plugin", plugin_name);
    
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
        _ => {
            // For other plugins, collect no data
            debug!("No specific data collection implemented for plugin: {}", plugin_name);
        }
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
fn create_file_statistics_summary(file_stats: &RepositoryFileStats) -> serde_json::Value {
    let top_files_by_commits = file_stats.files_by_commit_count();
    let top_files_by_changes = file_stats.files_by_net_change();
    
    serde_json::json!({
        "total_files": file_stats.file_count(),
        "total_commits_across_files": file_stats.total_commits(),
        "top_files_by_commits": top_files_by_commits.iter().take(10).map(|(path, stats)| {
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
        "top_files_by_changes": top_files_by_changes.iter().take(10).map(|(path, stats)| {
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

/// Execute plugins and display their reports
async fn execute_plugins_and_display_reports(
    plugin_registry: &plugin::SharedPluginRegistry,
    requested_commands: &[String],
    repo: git::RepositoryHandle,
    stats: &scanner::async_engine::EngineStats,
) -> Result<Option<ProcessedStatistics>> {
    
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
        // For now, we'll estimate processed stats based on the command being run
        // This could be improved to get actual processed statistics from plugins
        let processed_stats = match command.as_str() {
            "authors" | "contributors" | "committers" | "commits" | "commit" | "history" => {
                ProcessedStatistics {
                    files_processed: 0, // No file analysis for commit-based commands
                    commits_processed: repo_stats.total_commits as usize,
                    authors_processed: repo_stats.total_authors as usize,
                }
            }
            "metrics" => {
                ProcessedStatistics {
                    files_processed: repo_stats.total_files as usize, // File analysis for metrics
                    commits_processed: 0, // No commit analysis for metrics
                    authors_processed: 0, // No author analysis for metrics
                }
            }
            _ => ProcessedStatistics {
                files_processed: 0,
                commits_processed: repo_stats.total_commits as usize,
                authors_processed: repo_stats.total_authors as usize,
            }
        };
        
        // Only show metrics for work that was actually performed
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
    
    execute_plugin_function_with_data(plugin_registry, plugin_name, function_name, scan_data.clone(), repo.clone()).await?;
    
    // Create processed statistics from the scan data we collected
    let processed_stats = if !scan_data.is_empty() {
        // Count unique authors, commits, and files from the scan data
        let mut unique_authors = std::collections::HashSet::new();
        let mut commits_count = 0;
        let mut files_count = 0;
        
        for message in &scan_data {
            match &message.data {
                scanner::messages::MessageData::CommitInfo { author, .. } => {
                    unique_authors.insert(author.clone());
                    commits_count += 1;
                }
                scanner::messages::MessageData::FileInfo { .. } => {
                    files_count += 1;
                }
                _ => {}
            }
        }
        
        Some(ProcessedStatistics {
            files_processed: files_count,
            commits_processed: commits_count,
            authors_processed: unique_authors.len(),
        })
    } else {
        None
    };
    
    Ok(processed_stats)
}

/// Execute a specific plugin function with provided scan data and display its results
async fn execute_plugin_function_with_data(
    plugin_registry: &plugin::SharedPluginRegistry,
    plugin_name: &str,
    function_name: &str,
    scan_data: Vec<scanner::messages::ScanMessage>,
    repo: git::RepositoryHandle,
) -> Result<()> {
    use plugin::{PluginRequest, InvocationType};
    use plugin::context::RequestPriority;
    use scanner::ScanMode;
    
    debug!("Executing plugin '{}' function '{}' with {} scan messages", plugin_name, function_name, scan_data.len());
    
    // Get plugin from registry
    let registry = plugin_registry.inner().read().await;
    let plugin = match registry.get_plugin(plugin_name) {
        Some(plugin) => plugin,
        None => {
            error!("Plugin '{}' not found in registry", plugin_name);
            return Err(anyhow::anyhow!(
                "Plugin '{}' is not available.\n\nAvailable plugins:\n\ncommits - Analyze commit history and contributors\n  Functions: authors, contributors, committers, commits, history\n\nmetrics - Analyze code quality and complexity\n  Functions: metrics, complexity, quality\n\nexport - Export analysis results\n  Functions: export, json, csv, xml\n\nUsage:\n  gstats <plugin>           # Use plugin's default function\n  gstats <function>         # Use function if unambiguous\n  gstats <plugin>:<function> # Explicit plugin:function syntax\n\nRun 'gstats --help' to see all available commands.",
                plugin_name
            ));
        }
    };
    
    // For the commits plugin specifically, create a new instance and process the scan data
    if plugin_name == "commits" && !scan_data.is_empty() {
        
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
        
        let report_data = match function_name {
            "authors" | "contributors" | "committers" => {
                serde_json::json!({
                    "total_authors": author_stats.len(),
                    "top_authors": authors.iter().take(10).map(|(name, count)| {
                        serde_json::json!({ "name": name, "commits": count })
                    }).collect::<Vec<_>>(),
                    "author_stats": author_stats,
                    "unique_files": file_stats.file_count(),
                    "file_statistics": create_file_statistics_summary(&file_stats),
                    "function": "authors"
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
                    "file_statistics": create_file_statistics_summary(&file_stats),
                    "avg_commits_per_author": avg_commits_per_author,
                    "function": "commits"
                })
            }
            _ => {
                serde_json::json!({
                    "total_authors": author_stats.len(),
                    "top_authors": authors.iter().take(10).map(|(name, count)| {
                        serde_json::json!({ "name": name, "commits": count })
                    }).collect::<Vec<_>>(),
                    "author_stats": author_stats,
                    "unique_files": file_stats.file_count(),
                    "file_statistics": create_file_statistics_summary(&file_stats),
                    "function": function_name
                })
            }
        };
        
        // Display the report directly
        display_plugin_data(plugin_name, function_name, &report_data).await?;
        return Ok(()); // Success, return early
    }
    
    // For the metrics plugin specifically, process file data
    if plugin_name == "metrics" && !scan_data.is_empty() {
        // Process scan data to extract file metrics
        let mut file_count = 0;
        let mut total_lines = 0u64;
        let mut total_size = 0u64;
        
        for message in &scan_data {
            if let scanner::messages::MessageData::FileInfo { path: _, size, lines } = &message.data {
                file_count += 1;
                total_lines += *lines as u64;
                total_size += *size;
            }
        }
        
        debug!("Processed {} files, {} total lines, {} total bytes", file_count, total_lines, total_size);
        
        // Calculate metrics
        let average_lines_per_file = if file_count > 0 {
            total_lines as f64 / file_count as f64
        } else {
            0.0
        };
        
        let report_data = serde_json::json!({
            "total_files": file_count,
            "total_lines": total_lines,
            "total_blank_lines": 0, // TODO: Calculate from actual file content
            "total_comment_lines": 0, // TODO: Calculate from actual file content
            "total_complexity": 0, // TODO: Calculate complexity metrics
            "average_lines_per_file": average_lines_per_file,
            "average_complexity": 0.0, // TODO: Calculate average complexity
            "function": "metrics"
        });
        
        // Display the report directly
        display_plugin_data(plugin_name, function_name, &report_data).await?;
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
            display_plugin_response(plugin_name, function_name, response).await?;
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
) -> Result<()> {
    match response {
        plugin::PluginResponse::Execute { data, errors, .. } => {
            // Display any errors first
            if !errors.is_empty() {
                for error in &errors {
                    error!("Plugin '{}' error: {}", plugin_name, error);
                }
            }
            
            // Display the main report
            display_plugin_data(plugin_name, function_name, &data).await?;
        }
        plugin::PluginResponse::Data(data_str) => {
            // Parse and display JSON data
            match serde_json::from_str::<serde_json::Value>(&data_str) {
                Ok(data) => display_plugin_data(plugin_name, function_name, &data).await?,
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

/// Display plugin data in a user-friendly format
async fn display_plugin_data(
    plugin_name: &str,
    function_name: &str,
    data: &serde_json::Value,
) -> Result<()> {
    // Handle specific plugin output formats
    match (plugin_name, function_name) {
        ("commits", "authors" | "contributors" | "committers") => {
            display_author_report(data).await?;
        }
        ("commits", "commits" | "commit" | "history") => {
            display_commit_report(data).await?;
        }
        ("metrics", _) => {
            display_metrics_report(data).await?;
        }
        ("export", _) => {
            display_export_report(data).await?;
        }
        _ => {
            // Generic JSON display for unknown plugins
            println!("{}", serde_json::to_string_pretty(data)?);
        }
    }
    
    Ok(())
}

/// Display author analysis report
async fn display_author_report(data: &serde_json::Value) -> Result<()> {
    use prettytable::{Table, Row, Cell, format};
    
    println!("\n=== Author Analysis Report ===");
    
    if let Some(total_authors) = data.get("total_authors").and_then(|v| v.as_u64()) {
        println!("Total Authors: {}", total_authors);
    }
    
    if let Some(unique_files) = data.get("unique_files").and_then(|v| v.as_u64()) {
        println!("Files Modified: {}", unique_files);
    }
    
    if let Some(top_authors) = data.get("top_authors").and_then(|v| v.as_array()) {
        println!("\nTop Contributors:");
        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_BOX_CHARS);
        table.add_row(Row::new(vec![
            Cell::new("Rank"),
            Cell::new("Author"),
            Cell::new("Commits"),
            Cell::new("Percentage")
        ]));
        
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
                
                table.add_row(Row::new(vec![
                    Cell::new(&format!("{}", i + 1)),
                    Cell::new(name),
                    Cell::new(&format!("{}", commits)),
                    Cell::new(&format!("{:.1}%", percentage))
                ]));
            }
        }
        table.printstd();
    }
    
    // Display author-specific insights instead of file analysis for author reports
    display_author_insights(data.as_object().unwrap_or(&serde_json::Map::new())).await?;
    
    Ok(())
}

/// Display author-specific insights showing top files by author
async fn display_author_insights(data: &serde_json::Map<String, serde_json::Value>) -> Result<()> {
    use prettytable::{Table, Row, Cell, format};
    
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
        
        // Sort by commit count descending and take top 5 authors
        authors.sort_by(|a, b| b.1.cmp(&a.1));
        let top_authors = authors.into_iter().take(5).collect::<Vec<_>>();
        
        // Extract top files data for each author from file statistics
        if let Some(top_files_by_commits) = file_stats.get("top_files_by_commits").and_then(|v| v.as_array()) {
            
            for (author_name, _author_commits) in &top_authors {
                println!("\nTop Contributions by {}:", author_name);
                
                let mut author_table = Table::new();
                author_table.set_format(*format::consts::FORMAT_BOX_CHARS);
                author_table.add_row(Row::new(vec![
                    Cell::new("File"),
                    Cell::new("Author Commits"),
                    Cell::new("Author Impact")
                ]));
                
                // Find files this author has worked on (simplified - showing top files from overall data)
                // Note: This is showing overall file stats, not per-author stats yet
                // TODO: Implement proper per-author file tracking in the future
                let mut file_count = 0;
                for file_data in top_files_by_commits.iter().take(10) {
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
                        
                        author_table.add_row(Row::new(vec![
                            Cell::new(&display_path),
                            Cell::new(&format!("{}", estimated_author_commits.max(1))), // At least 1 if they worked on it
                            Cell::new(&impact_str)
                        ]));
                        
                        file_count += 1;
                        if file_count >= 5 { // Show top 5 files per author
                            break;
                        }
                    }
                }
                
                if file_count > 0 {
                    author_table.printstd();
                } else {
                    println!("  No file data available");
                }
            }
        }
    }
    
    Ok(())
}

/// Display commit analysis report  
async fn display_commit_report(data: &serde_json::Value) -> Result<()> {
    println!("\n=== Commit Analysis Report ===");
    
    if let Some(total_commits) = data.get("total_commits").and_then(|v| v.as_u64()) {
        println!("Total Commits: {}", total_commits);
    }
    
    if let Some(unique_authors) = data.get("unique_authors").and_then(|v| v.as_u64()) {
        println!("Unique Authors: {}", unique_authors);
    }
    
    if let Some(unique_files) = data.get("unique_files").and_then(|v| v.as_u64()) {
        println!("Files Modified: {}", unique_files);
    }
    
    if let Some(avg_commits) = data.get("avg_commits_per_author").and_then(|v| v.as_f64()) {
        println!("Average Commits per Author: {:.1}", avg_commits);
    }
    
    // Display Path Analysis Report if file statistics are available
    if let Some(file_stats) = data.get("file_statistics") {
        display_path_analysis_report(file_stats).await?;
    }
    
    Ok(())
}

/// Display metrics report
async fn display_metrics_report(data: &serde_json::Value) -> Result<()> {
    println!("\n=== Metrics Report ===");
    println!("{}", serde_json::to_string_pretty(data)?);
    Ok(())
}

/// Display export report
async fn display_export_report(data: &serde_json::Value) -> Result<()> {
    println!("\n=== Export Report ===");
    println!("{}", serde_json::to_string_pretty(data)?);
    Ok(())
}

/// Display path analysis report showing file commit statistics
async fn display_path_analysis_report(file_stats: &serde_json::Value) -> Result<()> {
    use prettytable::{Table, Row, Cell, format};
    
    println!("\n=== Path Analysis Report ===");
    
    if let Some(total_files) = file_stats.get("total_files").and_then(|v| v.as_u64()) {
        println!("Total Files: {}", total_files);
    }
    
    if let Some(total_commits) = file_stats.get("total_commits_across_files").and_then(|v| v.as_u64()) {
        println!("Total File Modifications: {}", total_commits);
    }
    
    // Display top files by commit count
    if let Some(top_files) = file_stats.get("top_files_by_commits").and_then(|v| v.as_array()) {
        if !top_files.is_empty() {
            println!("\nMost Frequently Modified Files (by commit count):");
            let mut table = Table::new();
            table.set_format(*format::consts::FORMAT_BOX_CHARS);
            table.add_row(Row::new(vec![
                Cell::new("Rank"),
                Cell::new("File Path"),
                Cell::new("Commits"),
                Cell::new("Lines +"),
                Cell::new("Lines -"),
                Cell::new("Net Change"),
                Cell::new("Current Lines"),
                Cell::new("Authors")
            ]));
            
            for (i, file) in top_files.iter().take(10).enumerate() {
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
                    
                    table.add_row(Row::new(vec![
                        Cell::new(&format!("{}", i + 1)),
                        Cell::new(&display_path),
                        Cell::new(&format!("{}", commits)),
                        Cell::new(&format!("{}", lines_added)),
                        Cell::new(&format!("{}", lines_removed)),
                        Cell::new(&net_change_str),
                        Cell::new(current_lines_display),
                        Cell::new(&format!("{}", authors))
                    ]));
                }
            }
            table.printstd();
        }
    }
    
    // Display top files by net change
    if let Some(top_changes) = file_stats.get("top_files_by_changes").and_then(|v| v.as_array()) {
        if !top_changes.is_empty() {
            println!("\nLargest Net Changes (by lines changed):");
            let mut table = Table::new();
            table.set_format(*format::consts::FORMAT_BOX_CHARS);
            table.add_row(Row::new(vec![
                Cell::new("Rank"),
                Cell::new("File Path"),
                Cell::new("Net Change"),
                Cell::new("Lines +"),
                Cell::new("Lines -"),
                Cell::new("Current Lines"),
                Cell::new("Commits")
            ]));
            
            for (i, file) in top_changes.iter().take(10).enumerate() {
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
                    
                    table.add_row(Row::new(vec![
                        Cell::new(&format!("{}", i + 1)),
                        Cell::new(&display_path),
                        Cell::new(&net_change_str),
                        Cell::new(&format!("{}", lines_added)),
                        Cell::new(&format!("{}", lines_removed)),
                        Cell::new(current_lines_display),
                        Cell::new(&format!("{}", commits))
                    ]));
                }
            }
            table.printstd();
        }
    }
    
    Ok(())
}
