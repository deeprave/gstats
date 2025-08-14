//! Application execution and scanner management

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use log::{info, debug, error};
use crate::{cli, config, display, plugin, scanner};
use crate::scanner::branch_detection::BranchDetection;
use crate::scanner::traits::QueueMessageProducer;
use crate::scanner::async_traits::AsyncScanner;


/// Resolve plugin commands using CommandMapper
pub async fn resolve_single_plugin_command(
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
            // TEMPORARY FALLBACK: Since CLI handler doesn't register functions (see GS-73),
            // assume the command is a builtin plugin name
            debug!("Command resolution failed, trying fallback for builtin plugins: {}", e);
            
            match command {
                "commits" | "commit" | "history" => {
                    debug!("Fallback: Resolved '{}' to 'commits' plugin", command);
                    Ok("commits".to_string())
                }
                "metrics" | "metric" => {
                    debug!("Fallback: Resolved '{}' to 'metrics' plugin", command);
                    Ok("metrics".to_string())
                }
                "export" => {
                    debug!("Fallback: Resolved '{}' to 'export' plugin", command);
                    Ok("export".to_string())
                }
                _ => {
                    error!("Failed to resolve command '{}': {}", command, e);
                    Err(anyhow::anyhow!("Command resolution failed for '{}': {}", command, e))
                }
            }
        }
    }
}

pub async fn handle_plugin_commands(args: &cli::Args, config: &config::ConfigManager) -> Result<()> {
    use cli::plugin_handler::PluginHandler;

    let plugin_config = cli::converter::merge_plugin_config(&args, Some(config));
    let mut handler = PluginHandler::with_plugin_config(plugin_config)?;
    
    // Handle --plugins command (display plugins with their functions)
    if args.show_plugins {
        // Create a colour manager for enhanced output
        let colour_manager = super::initialization::create_colour_manager(args, config);
        
        println!("{}", colour_manager.highlight("Available Plugins:"));
        println!("{}", colour_manager.highlight("=================="));
        
        let table_output = "Plugin table generation is not yet implemented";
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
        
        let colour_manager = super::initialization::create_colour_manager(args, config);
        
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
        println!("Usage: --output file.ext (auto-detect) or --format <format> --output <file>");
        
        return Ok(());
    }
    
    Ok(())
}

pub async fn run_scanner(
    repo_path: PathBuf, 
    args: cli::Args,
    config_manager: config::ConfigManager,
    runtime: Arc<tokio::runtime::Runtime>,
) -> Result<()> {
    use std::sync::Arc;
    
    // Convert CLI args to scanner config and query params
    let scanner_config = cli::converter::args_to_scanner_config(&args, Some(&config_manager))?;
    let query_params = cli::converter::args_to_query_params(&args, Some(&config_manager))?;
    
    debug!("Scanner configuration: {:?}", scanner_config);
    debug!("Query parameters: {:?}", query_params);
    
    // Create plugin configuration
    let plugin_config = cli::converter::merge_plugin_config(&args, Some(&config_manager));
    
    // Create a plugin registry and initialise plugins
    let plugin_registry = plugin::SharedPluginRegistry::new();
    
    debug!("Initializing scanner system");
    
    // Initialise built-in plugins
    super::initialization::initialize_builtin_plugins(&plugin_registry).await?;
    
    // Create a plugin handler with enhanced configuration
    let mut plugin_handler = cli::plugin_handler::PluginHandler::with_plugin_config(plugin_config)?;
    plugin_handler.build_command_mappings().await?;
    
    // Resolve plugin command using CommandMapper
    let command = args.command.as_ref().map(|s| s.as_str()).unwrap_or("commits");
    let plugin_names = vec![resolve_single_plugin_command(&plugin_handler, command, &args).await?];
    
    debug!("Active plugins: {:?}", plugin_names);
    debug!("Plugin arguments (original): {:?}", args.plugin_args);

    // Filter out global flags from plugin arguments to improve UX
    let filtered_plugin_args = cli::filter_global_flags(&args.plugin_args);
    debug!("Plugin arguments (filtered): {:?}", filtered_plugin_args);
    
    // 1. CREATE THE QUEUE FIRST
    let queue = crate::queue::SharedMessageQueue::new("main-scan".to_string());
    queue.start().await?;
    
    debug!("Queue created and started");
    
    // 2. ADD CONSUMERS (register all active plugins BEFORE scanning starts)
    for plugin_name in &plugin_names {
        let consumer = queue.register_consumer(plugin_name.clone()).await?;
        
        // Get the plugin and start consuming
        let mut plugin_registry_guard = plugin_registry.inner().write().await;
        if let Some(plugin) = plugin_registry_guard.get_plugin_mut(plugin_name) {
            if let Some(consumer_plugin) = plugin.as_consumer_plugin_mut() {
                consumer_plugin.start_consuming(consumer).await
                    .map_err(|e| anyhow::anyhow!("Failed to start consuming for plugin {}: {}", plugin_name, e))?;
                debug!("Plugin {} registered as consumer and started consuming", plugin_name);
            }
        }
    }
    
    debug!("All active plugins registered as consumers");
    
    // 3. CREATE SCANNER WITH QUEUE-BASED MESSAGE PRODUCER
    let message_producer = Arc::new(QueueMessageProducer::new(
        queue.clone(),
        "ScannerProducer".to_string()
    ));
    
    // Create a scanner engine with the repository path and queue producer
    let mut engine_builder = scanner::AsyncScannerEngineBuilder::new()
        .repository_path(repo_path.clone())
        .config(scanner_config.clone())
        .message_producer(message_producer as Arc<dyn scanner::MessageProducer + Send + Sync>)
        .runtime(runtime);
    
    // Create an event-driven scanner - no plugin wrapping needed, uses queue directly
    let query_params = scanner::QueryParams::default();
    let event_scanner = Arc::new(scanner::async_engine::scanners::EventDrivenScanner::new(query_params));
    
    // Add scanner directly to engine (no plugin wrapper needed - queue handles distribution)
    engine_builder = engine_builder.add_scanner(event_scanner as Arc<dyn AsyncScanner>);
    
    // Build and run scanner engine
    let engine = engine_builder.build()?;
    
    debug!("Starting the repository scan");
    
    // Create progress indicators
    let colour_manager = super::initialization::create_colour_manager(&args, &config_manager);
    let progress = display::ProgressIndicator::new(colour_manager.clone());
    
    // Show repository information  
    progress.status(display::StatusType::Info, &format!("Using git repository: {}", repo_path.display()));
    
    progress.status(display::StatusType::Info, "Starting repository scan...");
    
    // Execute scan - no mode filtering needed
    match engine.scan().await {
        Ok(()) => {
            info!("Scanner execution completed successfully");
        }
        Err(e) => {
            error!("Scanner execution failed: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}

/// Handle --show-branch command
pub async fn handle_show_branch_command(
    args: &cli::Args,
    config_manager: &config::ConfigManager,
) -> Result<()> {
    // Resolve repository path (same logic as main.rs)
    let repo_path = match args.repository.as_deref() {
        Some(path) => {
            // Expand tilde if present
            let expanded_path = if path.starts_with("~") {
                if let Some(home_dir) = dirs::home_dir() {
                    home_dir.join(path.strip_prefix("~/").unwrap_or(&path[1..]))
                } else {
                    PathBuf::from(path)
                }
            } else {
                PathBuf::from(path)
            };
            
            // Return canonical path if possible, otherwise just the expanded path
            expanded_path.canonicalize()
                .unwrap_or(expanded_path)
        }
        None => {
            // Use current directory
            std::env::current_dir()?
        }
    };
    
    // Create colour manager for progress display
    let colour_manager = super::initialization::create_colour_manager(args, config_manager);
    let progress = display::ProgressIndicator::new(colour_manager.clone());
    
    // Show repository information
    progress.status(display::StatusType::Info, &format!("Repository: {}", repo_path.display()));
    
    // Get CLI branch parameters
    let cli_branch = args.branch.as_deref();
    let cli_remote = args.remote.as_deref(); 
    let cli_fallbacks: Option<Vec<String>> = args.fallback_branch.as_ref()
        .map(|s| s.split(',').map(|s| s.trim().to_string()).collect());
    
    // Create branch detection
    let branch_detection = BranchDetection::new();
    
    // Detect the branch
    match branch_detection.detect_branch(&repo_path, cli_branch, cli_remote, cli_fallbacks.as_deref()) {
        Ok(branch_result) => {
            progress.status(display::StatusType::Info, &format!(
                "Selected branch: {} ({})",
                branch_result.branch_name,
                branch_result.selection_source.debug()
            ));
            progress.status(display::StatusType::Info, &format!(
                "Commit ID: {}",
                branch_result.commit_id
            ));
        }
        Err(e) => {
            progress.status(display::StatusType::Warning, &format!("Branch detection failed: {}", e));
            return Err(e.into());
        }
    }
    
    Ok(())
}

