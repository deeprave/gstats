//! Application initialization and configuration

use anyhow::{Result, Context};
use std::path::PathBuf;
use log::{info, debug, error};
use crate::{cli, config, logging, display, plugin};

pub fn load_configuration(args: &cli::Args) -> Result<config::ConfigManager> {
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
pub fn handle_export_config(config_manager: &config::ConfigManager, export_path: &std::path::Path) -> Result<()> {
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

pub fn configure_logging(args: &cli::Args, config: &config::ConfigManager) -> Result<logging::LogConfig> {
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
pub fn create_colour_manager(args: &cli::Args, config: &config::ConfigManager) -> display::ColourManager {
    let colour_config = config.get_colour_config().ok();
    display::ColourManager::from_color_args(args.no_color, args.color, colour_config)
}

pub async fn initialize_builtin_plugins(
    plugin_registry: &plugin::SharedPluginRegistry,
) -> Result<()> {
    
    debug!("Initializing builtin plugins");
    
    // Register all plugins as inactive first
    {
        let mut registry = plugin_registry.inner().write().await;
        
        // Metrics Plugin  
        let metrics_plugin = Box::new(plugin::builtin::metrics::MetricsPlugin::new());
        registry.register_plugin_inactive(metrics_plugin).await
            .with_context(|| "Failed to register metrics plugin")?;
        
        // Commits Plugin
        let commits_plugin = Box::new(plugin::builtin::commits::CommitsPlugin::new());
        registry.register_plugin_inactive(commits_plugin).await
            .with_context(|| "Failed to register commits plugin")?;
        
        // Export Plugin
        let export_plugin = Box::new(plugin::builtin::export::ExportPlugin::new());
        registry.register_plugin_inactive(export_plugin).await
            .with_context(|| "Failed to register export plugin")?;
        
        // Auto-activate plugins marked with load_by_default = true
        registry.auto_activate_default_plugins().await
            .with_context(|| "Failed to auto-activate default plugins")?;
    }
    
    info!("Builtin plugins initialized successfully");
    Ok(())
}

pub fn create_plugin_context(_repo_path: &PathBuf) -> Result<plugin::PluginContext> {
    use crate::scanner::{ScannerConfig, QueryParams};
    use std::sync::Arc;
    
    // Create default scanner config and query params
    let scanner_config = Arc::new(ScannerConfig::default());
    let query_params = Arc::new(QueryParams::default());
    
    Ok(plugin::PluginContext::new(scanner_config, query_params))
}