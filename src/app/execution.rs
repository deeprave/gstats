//! Application execution and scanner management

use anyhow::Result;
use std::path::PathBuf;
use log::{info, debug, error};
use crate::{cli, config, display, plugin, scanner};
use crate::scanner::branch_detection::BranchDetection;
use crate::scanner::traits::QueueMessageProducer;


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
            error!("Failed to resolve command '{}': {}", command, e);
            Err(anyhow::anyhow!("Command resolution failed for '{}': {}", command, e))
        }
    }
}

pub async fn handle_plugin_commands(args: &cli::Args, config: &config::ConfigManager) -> Result<()> {
    use cli::plugin_handler::PluginHandler;

    let plugin_config = cli::converter::merge_plugin_config(&args, Some(config));
    let mut handler = PluginHandler::with_plugin_config(plugin_config)?;
    
    // Handle --plugins command (display plugins with their functions)
    if args.show_plugins {
        handler.build_command_mappings().await?;
        
        // Create a colour manager for enhanced output
        let colour_manager = super::initialization::create_colour_manager(args, config);
        
        println!("{}", colour_manager.highlight("Available Plugins:"));
        println!("{}", colour_manager.highlight("=================="));
        println!();
        
        let mappings = handler.get_function_mappings();
        if mappings.is_empty() {
            println!("No plugins available.");
        } else {
            use std::collections::HashMap;
            
            // Group functions by plugin for better organization
            let mut by_plugin: HashMap<String, Vec<_>> = HashMap::new();
            for mapping in &mappings {
                by_plugin.entry(mapping.plugin_name.clone())
                    .or_insert_with(Vec::new)
                    .push(mapping);
            }
            
            // Sort plugins alphabetically
            let mut plugin_names: Vec<_> = by_plugin.keys().cloned().collect();
            plugin_names.sort();
            
            // Prepare data for table formatting
            let mut plugin_data = Vec::new();
            for plugin_name in &plugin_names {
                if let Some(funcs) = by_plugin.get(plugin_name) {
                    // Sort functions, putting default first
                    let mut sorted_funcs = funcs.clone();
                    sorted_funcs.sort_by(|a, b| {
                        match (a.is_default, b.is_default) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => a.function_name.cmp(&b.function_name),
                        }
                    });
                    
                    // Build function list string
                    let function_strs: Vec<String> = sorted_funcs.iter().map(|f| {
                        let default_marker = if f.is_default { "*" } else { "" };
                        let aliases = if !f.aliases.is_empty() {
                            format!(", {}", f.aliases.join(", "))
                        } else {
                            String::new()
                        };
                        format!("{}{}{}", f.function_name, default_marker, aliases)
                    }).collect();
                    
                    let functions_str = function_strs.join(", ");
                    plugin_data.push((plugin_name.clone(), functions_str));
                }
            }
            
            // Use the display module's table formatting
            let table_output = crate::display::create_plugin_table(plugin_data, &colour_manager);
            print!("{}", table_output);
            
            println!();
            println!("{}", colour_manager.highlight("Usage:"));
            println!("  {}                # Use plugin's default function",
                colour_manager.command("gstats <plugin>"));
            println!("  {}              # Use function if unambiguous",
                colour_manager.command("gstats <function>"));
            println!("  {}     # Explicit plugin:function syntax",
                colour_manager.command("gstats <plugin>:<function>"));
            println!();
            println!("{}", colour_manager.orange("* = default function for plugin"));
        }
        
        return Ok(());
    }
    
    // Handle --plugins-help command (display functions with their providers)
    if args.plugins_help {
        handler.build_command_mappings().await?;
        
        // Create color manager for styled output
        let colour_manager = display::ColourManager::from_color_args(args.no_color, args.color, None);
        
        println!("{}", colour_manager.highlight("Available Plugin Functions and Commands:"));
        println!("{}", colour_manager.info("========================================"));
        
        let mappings = handler.get_function_mappings();
        if mappings.is_empty() {
            println!("No plugin functions available.");
        } else {
            use std::collections::HashMap;
            
            // Group functions by plugin for better organization
            let mut by_plugin: HashMap<String, Vec<_>> = HashMap::new();
            for mapping in &mappings {
                by_plugin.entry(mapping.plugin_name.clone())
                    .or_insert_with(Vec::new)
                    .push(mapping);
            }
            
            // Sort plugins alphabetically
            let mut plugin_names: Vec<_> = by_plugin.keys().cloned().collect();
            plugin_names.sort();
            
            // Calculate column widths for proper alignment
            let max_plugin_width = plugin_names.iter().map(|name| name.len()).max().unwrap_or(6).max(6); // "Plugin" length
            
            // Print sleek header with underline using colors
            println!(" {:<width$} {}", 
                colour_manager.highlight("Plugin"), 
                colour_manager.highlight("Functions & Description"), 
                width = max_plugin_width);
            println!(" {} {}", 
                colour_manager.info(&"-".repeat(max_plugin_width)), 
                colour_manager.info("--"));
            
            // Print each plugin row
            for plugin_name in &plugin_names {
                if let Some(funcs) = by_plugin.get(plugin_name) {
                    // Sort functions, putting default first
                    let mut sorted_funcs = funcs.clone();
                    sorted_funcs.sort_by(|a, b| {
                        match (a.is_default, b.is_default) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => a.function_name.cmp(&b.function_name),
                        }
                    });
                    
                    // Build function list string without brackets around aliases
                    let function_strs: Vec<String> = sorted_funcs.iter().map(|f| {
                        let default_marker = if f.is_default { "*" } else { "" };
                        let aliases = if !f.aliases.is_empty() {
                            format!(", {}", f.aliases.join(", "))
                        } else {
                            String::new()
                        };
                        format!("{}{}{}", f.function_name, default_marker, aliases)
                    }).collect();
                    
                    let functions_str = function_strs.join(", ");
                    
                    // Get plugin description (use first function's description or plugin name)
                    let description = sorted_funcs.first()
                        .and_then(|f| if !f.description.is_empty() { Some(f.description.as_str()) } else { None })
                        .unwrap_or("Plugin for data processing");
                    
                    // Print plugin row with functions on first line, description on second using colors
                    println!(" {:<width$} {}", 
                        colour_manager.command(plugin_name), 
                        colour_manager.success(&functions_str), 
                        width = max_plugin_width);
                    println!(" {:<width$} {}", 
                        "", 
                        colour_manager.info(description), 
                        width = max_plugin_width);
                }
            }
            
            println!();
            println!("{}", colour_manager.highlight("Usage Examples:"));
            println!("  {}              # Use function if unambiguous",
                colour_manager.command("gstats <function>"));
            println!("  {}                # Use plugin's default function",
                colour_manager.command("gstats <plugin>"));
            println!("  {}     # Explicit plugin:function syntax",
                colour_manager.command("gstats <plugin>:<function>"));
            println!();
            println!("{}", colour_manager.orange("* = default function for plugin"));
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
        // TODO: Move this functionality to export plugin (see GS-90)
        println!("Format listing moved to export plugin - functionality temporarily unavailable");
        return Ok(());
    }
    
    Ok(())
}

pub fn run_scanner(
    repo_path: PathBuf, 
    args: cli::Args,
    config_manager: config::ConfigManager,
) -> Result<()> {
    use std::sync::Arc;
    
    // Convert CLI args to scanner config and query params - these are sync
    let scanner_config = cli::converter::args_to_scanner_config(&args, Some(&config_manager))?;
    let query_params = cli::converter::args_to_query_params(&args, Some(&config_manager))?;
    
    debug!("Scanner configuration: {:?}", scanner_config);
    debug!("Query parameters: {:?}", query_params);
    
    // Create plugin configuration
    let plugin_config = cli::converter::merge_plugin_config(&args, Some(&config_manager));
    
    // Create a plugin registry and initialise plugins
    let plugin_registry = plugin::SharedPluginRegistry::new();
    
    debug!("Initializing scanner system");
    
    // Create colour manager early for plugin initialization
    let colour_manager = super::initialization::create_colour_manager(&args, &config_manager);
    
    // CREATE UNIFIED NOTIFICATION MANAGER
    let unified_notification_manager = Arc::new(crate::notifications::AsyncNotificationManager::<crate::notifications::events::UnifiedEvent>::new());
    
    // Create typed publishers for different event types
    use crate::notifications::typed_publishers::{ScanEventPublisher, QueueEventPublisher, PluginEventPublisher};
    let scan_publisher = Arc::new(ScanEventPublisher::new(unified_notification_manager.clone()));
    let queue_publisher = Arc::new(QueueEventPublisher::new(unified_notification_manager.clone()));
    let plugin_publisher = Arc::new(PluginEventPublisher::new(unified_notification_manager.clone()));
    
    // Initialize plugins using the proper UnifiedPluginDiscovery system
    // Get excluded plugins from configuration (if any)  
    let excluded_plugins = plugin_config.plugin_exclude.clone();
    
    // Plugin initialization is now sync - pass the plugin publisher
    super::initialization::initialize_plugins_via_discovery(&plugin_registry, &colour_manager, excluded_plugins, plugin_publisher)?;
    
    // Create a plugin handler with enhanced configuration
    let mut plugin_handler = cli::plugin_handler::PluginHandler::with_plugin_config(plugin_config)?;
    
    // Create minimal runtime for plugin handler async operations only
    let init_rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create initialization runtime: {}", e))?;
    
    init_rt.block_on(plugin_handler.build_command_mappings())?;
    
    // Resolve plugin command using CommandMapper
    let command = if let Some(cmd) = args.command.as_ref() {
        cmd.clone()
    } else {
        // No command specified - show plugin help and exit with error code
        let mappings = plugin_handler.get_function_mappings();
        if mappings.is_empty() {
            return Err(anyhow::anyhow!("No plugins available"));
        }
        
        // Create color manager for styled output (same as --plugins-help)
        let colour_manager = display::ColourManager::from_color_args(args.no_color, args.color, None);
        
        println!("{}", colour_manager.highlight("Available Plugin Functions and Commands:"));
        println!("{}", colour_manager.info("========================================"));
        
        if mappings.is_empty() {
            println!("No plugin functions available.");
        } else {
            use std::collections::HashMap;
            
            // Group functions by plugin for better organization
            let mut by_plugin: HashMap<String, Vec<_>> = HashMap::new();
            for mapping in &mappings {
                by_plugin.entry(mapping.plugin_name.clone())
                    .or_insert_with(Vec::new)
                    .push(mapping);
            }
            
            // Sort plugins alphabetically
            let mut plugin_names: Vec<_> = by_plugin.keys().cloned().collect();
            plugin_names.sort();
            
            // Calculate column widths for proper alignment
            let max_plugin_width = plugin_names.iter().map(|name| name.len()).max().unwrap_or(6).max(6);
            
            // Print sleek header with underline using colors
            println!(" {:<width$} {}", 
                colour_manager.highlight("Plugin"), 
                colour_manager.highlight("Functions & Description"), 
                width = max_plugin_width);
            println!(" {} {}", 
                colour_manager.info(&"-".repeat(max_plugin_width)), 
                colour_manager.info("--"));
            
            // Print each plugin row
            for plugin_name in &plugin_names {
                if let Some(funcs) = by_plugin.get(plugin_name) {
                    // Sort functions, putting default first
                    let mut sorted_funcs = funcs.clone();
                    sorted_funcs.sort_by(|a, b| {
                        match (a.is_default, b.is_default) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => a.function_name.cmp(&b.function_name),
                        }
                    });
                    
                    // Build function list string without brackets around aliases
                    let function_strs: Vec<String> = sorted_funcs.iter().map(|f| {
                        let default_marker = if f.is_default { "*" } else { "" };
                        let aliases = if !f.aliases.is_empty() {
                            format!(", {}", f.aliases.join(", "))
                        } else {
                            String::new()
                        };
                        format!("{}{}{}", f.function_name, default_marker, aliases)
                    }).collect();
                    
                    let functions_str = function_strs.join(", ");
                    
                    // Get plugin description
                    let description = sorted_funcs.first()
                        .and_then(|f| if !f.description.is_empty() { Some(f.description.as_str()) } else { None })
                        .unwrap_or("Plugin for data processing");
                    
                    // Print plugin row with functions on first line, description on second using colors
                    println!(" {:<width$} {}", 
                        colour_manager.command(plugin_name), 
                        colour_manager.success(&functions_str), 
                        width = max_plugin_width);
                    println!(" {:<width$} {}", 
                        "", 
                        colour_manager.info(description), 
                        width = max_plugin_width);
                }
            }
            
            println!();
            println!("{}", colour_manager.highlight("Usage Examples:"));
            println!("  {}              # Use function if unambiguous",
                colour_manager.command("gstats <function>"));
            println!("  {}                # Use plugin's default function",
                colour_manager.command("gstats <plugin>"));
            println!("  {}     # Explicit plugin:function syntax",
                colour_manager.command("gstats <plugin>:<function>"));
            println!();
            println!("{}", colour_manager.orange("* = default function for plugin"));
        }
        
        return Err(anyhow::anyhow!("No command specified. Please specify a plugin or function to execute."));
    };
    let resolved_plugin = init_rt.block_on(resolve_single_plugin_command(&plugin_handler, &command, &args))?;
    let plugin_names = vec![resolved_plugin.clone()];
    
    debug!("Active plugins: {:?}", plugin_names);
    debug!("Plugin arguments: {:?}", args.plugin_args);
    
    // CREATE THE QUEUE using typed publishers
    let queue = crate::queue::SharedMessageQueue::new(queue_publisher.clone(), scan_publisher.clone());
    init_rt.block_on(async {
        queue.start().await?;
        
        // Subscribe queue to ScanEvents for statistical tracking
        queue.subscribe_to_scan_events().await
    })?;
    
    debug!("Queue created and started");
    
    // 2. ADD CONSUMERS (register all active plugins BEFORE scanning starts)
    for plugin_name in &plugin_names {
        let consumer = init_rt.block_on(queue.register_consumer(plugin_name.clone()))?;
        
            // Get the plugin and configure it with arguments
        init_rt.block_on(async {
            let mut plugin_registry_guard = plugin_registry.inner().write().await;
            if let Some(plugin) = plugin_registry_guard.get_plugin_mut(plugin_name) {
                // Parse plugin arguments before starting consumption
                plugin.parse_plugin_arguments(&args.plugin_args).await
                    .map_err(|e| anyhow::anyhow!("Failed to parse plugin arguments for {}: {}", plugin_name, e))?;
                debug!("Plugin {} arguments parsed successfully", plugin_name);
                
                if let Some(consumer_plugin) = plugin.as_consumer_plugin_mut() {
                    consumer_plugin.start_consuming(consumer).await
                        .map_err(|e| anyhow::anyhow!("Failed to start consuming for plugin {}: {}", plugin_name, e))?;
                    debug!("Plugin {} registered as consumer and started consuming", plugin_name);
                }
            }
            Result::<_, anyhow::Error>::Ok(())
        })?
    }
    
    debug!("All active plugins registered as consumers");
    
    // 3. CREATE SCANNER WITH QUEUE-BASED MESSAGE PRODUCER
    let message_producer = Arc::new(QueueMessageProducer::new(
        queue.clone(),
        "ScannerProducer".to_string()
    ));
    
    // Create a scanner manager with the repository path, queue producer and shared notification manager
    // Let the manager create its own runtime to avoid nested runtime issues
    let mut engine_builder = scanner::AsyncScannerManagerBuilder::new()
        .repository_path(repo_path.clone())
        .config(scanner_config.clone())
        .message_producer(message_producer as Arc<dyn scanner::MessageProducer + Send + Sync>)
        .notification_manager(Arc::new(crate::notifications::AsyncNotificationManager::<crate::notifications::events::ScanEvent>::new()))
        .plugin_registry(plugin_registry.clone());
    
    // Create an event-driven scanner - no plugin wrapping needed, uses queue directly
    let query_params = scanner::QueryParams::default();
    let event_scanner = Arc::new(scanner::async_engine::scanners::EventDrivenScanner::new(query_params));
    
    // Add scanner directly to manager
    engine_builder = engine_builder.add_scanner(event_scanner);
    
    // Build and run scanner engine
    let engine = engine_builder.build()?;
    
    debug!("Starting the repository scan");
    
    // Create progress indicators (reusing colour_manager from plugin initialization)
    let progress = display::ProgressIndicator::new(colour_manager.clone());
    
    // Show repository information  
    progress.status(display::StatusType::Info, &format!("Using git repository: {}", repo_path.display()));
    
    progress.status(display::StatusType::Info, "Starting repository scan...");
    
    // Execute scan in scanner's own runtime - no mode filtering needed
    // Scanner creates its own runtime internally to avoid nested runtime conflicts
    init_rt.block_on(async {
        match engine.scan().await {
            Ok(()) => {
                info!("Scanner execution completed successfully");
                Ok(())
            }
            Err(e) => {
                error!("Scanner execution failed: {}", e);
                Err(anyhow::anyhow!("Scanner execution failed: {}", e))
            }
        }
    })?;
    
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

