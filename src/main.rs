mod cli;
mod config;
mod git;
mod logging;
mod queue;
mod scanner;
mod plugin;

use anyhow::Result;
use std::process;
use log::{info, error};

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
}

fn run() -> Result<()> {
    let args = cli::args::parse_args();
    
    cli::args::validate_args(&args)?;
    
    let config_manager = load_configuration(&args)?;
    
    let log_config = configure_logging(&args, &config_manager)?;
    logging::init_logger(log_config)?;
    
    // Handle plugin management commands
    if args.list_plugins || args.plugin_info.is_some() || args.check_plugin.is_some() || args.list_by_type.is_some() {
        return tokio::runtime::Runtime::new()?.block_on(async {
            handle_plugin_commands(&args).await
        });
    }
    
    let repo_path = git::resolve_repository_path(args.repository.clone())?;
    
    info!("Analyzing git repository at: {}", repo_path);
    
    // Create runtime for async operations
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        run_scanner(repo_path, args, config_manager).await
    })
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
    
    let handler = PluginHandler::new()?;
    
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
    
    // Create plugin handler for discovery
    let plugin_handler = cli::plugin_handler::PluginHandler::new()?;
    
    // Validate and get plugin names
    let plugin_names = if args.plugins.is_empty() {
        vec!["commits".to_string()] // Default plugin
    } else {
        plugin_handler.validate_plugin_names(&args.plugins).await?
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
        .message_producer(message_producer as Arc<dyn scanner::MessageProducer + Send + Sync>);
    
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
    
    // Create consumer for processing messages
    let listener_registry = Arc::new(std::sync::Mutex::new(queue::DefaultListenerRegistry::new()));
    let mut consumer = queue::MessageConsumer::with_config(
        Arc::clone(&memory_queue),
        listener_registry,
        queue::ConsumerConfig::default(),
    );
    
    // Start consumer in background
    let consumer_handle = tokio::task::spawn_blocking(move || {
        consumer.start()
    });
    
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
    
    // Stop consumer gracefully
    consumer_handle.abort();
    
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
