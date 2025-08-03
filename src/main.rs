mod cli;
mod config;
mod git;
mod logging;
mod queue;
mod scanner;

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
    
    let repo_path = git::resolve_repository_path(args.repository)?;
    
    info!("Analyzing git repository at: {}", repo_path);
    
    Ok(())
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
