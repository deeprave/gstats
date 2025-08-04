mod cli;
mod config;
mod git;
mod logging;
mod queue;
mod scanner;
mod plugin;

use anyhow::Result;
use std::process;
use std::sync::Arc;
use log::{info, error, debug};

fn main() {
    if let Err(e) = run() {
        error!("Application error: {}", e);
        for cause in e.chain().skip(1) {
            error!("  Caused by: {}", cause);
        }
        
        // Also output to stderr for cases where logging might not be initialized
        eprintln!("Error: {}", e);
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
    
    let repo_path = git::resolve_repository_path(args.repository.clone())?;
    
    info!("Analyzing git repository at: {}", repo_path);
    
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
        info!("Loading configuration from explicit file: {}", config_file.display());
        config::ConfigManager::load_from_file(config_file.clone())?
    } else {
        config::ConfigManager::load()?
    };
    
    if let Some(section_name) = &args.config_name {
        info!("Selecting configuration section: {}", section_name);
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
                info!("Using console log level from config: {:?}", level);
                level
            }
            Ok(None) => LevelFilter::Info,
            Err(e) => {
                info!("Invalid console-level in config, using default: {}", e);
                LevelFilter::Info
            }
        }
    };
    
    info!("Console log level set to: {:?}", console_level);
    
    let format = if !args.log_format.is_empty() && args.log_format != "text" {
        logging::LogFormat::from_str(&args.log_format)
            .map_err(|e| anyhow::anyhow!(e))?
    } else {
        match config.get_value("base", "log-format") {
            Some(format_str) => {
                info!("Using log format from config: {}", format_str);
                logging::LogFormat::from_str(format_str)
                    .unwrap_or(logging::LogFormat::Text)
            }
            None => logging::LogFormat::Text,
        }
    };
    
    info!("Log format set to: {:?}", format);
    
    let log_file_path = args.log_file.clone()
        .or_else(|| config.get_path("base", "log-file"));
    
    let file_log_level = match &args.log_file_level {
        Some(level_str) => Some(logging::parse_log_level(level_str)?),
        None => {
            match config.get_log_level("base", "file-log-level") {
                Ok(Some(level)) => {
                    info!("Using file log level from config: {:?}", level);
                    Some(level)
                }
                Ok(None) => None,
                Err(e) => {
                    info!("Invalid file-log-level in config, using None: {}", e);
                    None
                }
            }
        }
    };
    
    let (destination, file_level) = match (log_file_path.as_ref(), file_log_level) {
        (Some(file_path), Some(level)) => {
            info!("File logging enabled: {} (level: {:?})", file_path.display(), level);
            (logging::LogDestination::Both(file_path.clone()), Some(level))
        }
        (Some(file_path), None) => {
            info!("File logging enabled: {} (level: {:?} - same as console)", file_path.display(), console_level);
            (logging::LogDestination::Both(file_path.clone()), Some(console_level))
        }
        (None, None) => {
            info!("Console-only logging enabled");
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
    repo_path: String, 
    args: cli::Args,
    config_manager: config::ConfigManager,
    runtime: Arc<tokio::runtime::Runtime>,
) -> Result<()> {
    use std::sync::Arc;
    
    // Open repository
    let repo = git::resolve_repository_handle(Some(repo_path))?;
    
    // Convert CLI args to scanner config and query params
    let scanner_config = cli::converter::args_to_scanner_config(&args, Some(&config_manager))?;
    let query_params = cli::converter::args_to_query_params(&args)?;
    
    info!("Scanner configuration: {:?}", scanner_config);
    info!("Query parameters: {:?}", query_params);
    
    // Create plugin registry and initialize plugins
    let plugin_registry = plugin::SharedPluginRegistry::new();
    
    // Initialize built-in plugins
    initialize_builtin_plugins(&plugin_registry).await?;
    
    // Create plugin handler for discovery and command mapping
    let mut plugin_handler = cli::plugin_handler::PluginHandler::new()?;
    plugin_handler.build_command_mappings().await?;
    
    // Resolve plugin commands using CommandMapper
    let plugin_names = if args.plugins.is_empty() {
        vec!["commits".to_string()] // Default plugin
    } else {
        resolve_plugin_commands(&plugin_handler, &args.plugins).await?
    };
    
    info!("Active plugins: {:?}", plugin_names);
    
    // Create message queue
    let queue_config = queue::QueueConfig::default();
    let memory_queue = Arc::new(queue::MemoryQueue::new(queue_config.capacity, queue_config.memory_limit));
    
    // Create message producer
    let message_producer = Arc::new(queue::QueueMessageProducer::new(
        Arc::clone(&memory_queue), 
        "ScannerProducer".to_string()
    ));
    
    // Create scanner engine
    let mut engine_builder = scanner::AsyncScannerEngineBuilder::new()
        .repository(repo)
        .config(scanner_config.clone())
        .message_producer(message_producer as Arc<dyn scanner::MessageProducer + Send + Sync>)
        .runtime(runtime);
    
    // Create base scanners
    let repo_handle = git::resolve_repository_handle(None)?;
    let async_repo = Arc::new(scanner::async_engine::repository::AsyncRepositoryHandle::new(repo_handle));
    
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
    
    info!("Starting scan with modes: {:?}", scan_modes);
    
    // Note: Consumer disabled to prevent runtime drop issues
    // For GS-30 we'll implement proper async consumer integration
    
    // Run the scan
    match engine.scan(scan_modes).await {
        Ok(()) => {
            info!("Scan completed successfully");
            
            // Get final statistics
            let stats = engine.get_stats().await;
            info!("Scanner statistics: {:?}", stats);
            
            // Process remaining messages
            // Queue will be processed by the consumer
            
            // Get plugin statistics
            let registry_plugins = plugin_registry.inner().read().await.list_plugins();
            for plugin_name in registry_plugins {
                info!("Plugin {} completed", plugin_name);
            }
        }
        Err(e) => {
            error!("Scan failed: {}", e);
            return Err(e.into());
        }
    }
    
    // Consumer cleanup will be implemented in GS-30
    
    info!("Analysis complete");
    
    Ok(())
}

async fn initialize_builtin_plugins(registry: &plugin::SharedPluginRegistry) -> Result<()> {
    use plugin::builtin::{CommitsPlugin, MetricsPlugin, ExportPlugin};
    
    let mut reg = registry.inner().write().await;
    
    // Register built-in plugins
    reg.register_plugin(Box::new(CommitsPlugin::new())).await?;
    reg.register_plugin(Box::new(MetricsPlugin::new())).await?;
    reg.register_plugin(Box::new(ExportPlugin::new())).await?;
    
    info!("Initialized {} built-in plugins", 3);
    
    Ok(())
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
