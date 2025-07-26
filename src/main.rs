// Main entry point for gstats - Git Repository Analytics Tool

mod cli;
mod git;
mod logging;

use anyhow::Result;
use std::process;
use log::{info, error};

fn main() {
    if let Err(e) = run() {
        // Log error with full context chain
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
    let args = cli::parse_args();
    
    // Validate CLI arguments
    cli::validate_args(&args)?;
    
    // Configure logging based on CLI arguments
    let log_config = configure_logging(&args)?;
    logging::init_logger(log_config)?;
    
    // Validate git repository
    let repo_path = git::resolve_repository_path(args.repository)?;
    
    info!("Analyzing git repository at: {}", repo_path);
    
    Ok(())
}

fn configure_logging(args: &cli::Args) -> Result<logging::LogConfig> {
    use log::LevelFilter;
    use std::str::FromStr;
    
    // Determine console log level
    let console_level = if args.debug {
        LevelFilter::Trace
    } else if args.verbose {
        LevelFilter::Debug
    } else if args.quiet {
        LevelFilter::Error
    } else {
        LevelFilter::Info
    };
    
    info!("Console log level set to: {:?}", console_level);
    
    // Determine log format
    let format = logging::LogFormat::from_str(&args.log_format)
        .map_err(|e| anyhow::anyhow!(e))?;
    
    info!("Log format set to: {:?}", format);
    
    // Determine destination and file log level
    let (destination, file_level) = match (&args.log_file, &args.log_file_level) {
        (Some(file_path), Some(level_str)) => {
            let file_level = logging::parse_log_level(level_str)?;
            info!("File logging enabled: {} (level: {:?})", file_path.display(), file_level);
            (logging::LogDestination::Both(file_path.clone()), Some(file_level))
        }
        (Some(file_path), None) => {
            // Use console level for file if not specified
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
