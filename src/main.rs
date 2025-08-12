mod cli;
mod config;
mod display;
mod logging;
mod notifications;
mod scanner;
mod plugin;
mod stats;
mod app;
mod output;

use anyhow::Result;
use std::process;
use std::sync::Arc;
use log::error;
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
            eprintln!("Error: {}", e);
        }
        
        process::exit(1);
    }
    
    // Set up panic handler with better error reporting
    std::panic::set_hook(Box::new(|panic_info| {
        error!("Application panicked: {:?}", panic_info);
        eprintln!("Panic: {:?}", panic_info);
        process::exit(101);
    }));
}

fn run() -> Result<()> {
    let args = cli::args::parse_args();
    
    cli::args::validate_args(&args)?;
    
    let config_manager = app::load_configuration(&args)?;
    
    let log_config = app::configure_logging(&args, &config_manager)?;
    logging::init_logger(log_config)?;
    
    // Enhanced logging system is now ready
    
    // Handle configuration export command first (before creating runtime)
    if let Some(export_path) = &args.export_config {
        return app::initialization::handle_export_config(&config_manager, export_path);
    }
    
    // Create runtime for async operations - single runtime for the entire application
    // Using current_thread runtime to avoid nested runtime issues
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    
    // Handle plugin management commands
    if args.list_plugins || args.show_plugins || args.plugins_help || args.plugin_info.is_some() || args.list_by_type.is_some() || args.list_formats {
        return runtime.block_on(async {
            let config_manager = app::load_configuration(&args)?;
            app::handle_plugin_commands(&args, &config_manager).await
        });
    }
    
    // Resolve repository path (validates it's a git repository)
    let repo_path = app::resolve_repository_path(args.repository.clone())?;
    
    // Run scanner with existing runtime
    let runtime_arc = Arc::new(runtime);
    let runtime_clone = Arc::clone(&runtime_arc);
    let result = runtime_arc.block_on(async {
        app::run_scanner(repo_path, args, config_manager, runtime_clone).await
    });
    
    // Runtime will be dropped when runtime_arc goes out of scope
    
    result
}

/// Utility function to estimate line count from file size
pub fn estimate_line_count(size: usize) -> usize {
    (size / 35).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processed_statistics_compact_format() {
        let stats = ProcessedStatistics {
            files_processed: 42,
            commits_processed: 100,
            authors_processed: 5,
        };
        
        let compact = stats.to_compact_format();
        assert_eq!(compact, "Files: 42 | Commits: 100 | Authors: 5");
    }

    #[test]
    fn test_estimate_line_count() {
        assert_eq!(estimate_line_count(0), 1);
        assert_eq!(estimate_line_count(35), 1);
        assert_eq!(estimate_line_count(70), 2);
        assert_eq!(estimate_line_count(350), 10);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_repository_validation_at_cli_level() {
        // Test with current directory (should be a git repo)
        let result = app::validate_repository_path(None);
        assert!(result.is_ok(), "Current directory should be a valid git repository");
        
        // Test with explicit valid path
        let current_dir = std::env::current_dir().unwrap();
        let result = app::validate_repository_path(Some(current_dir.to_string_lossy().to_string()));
        assert!(result.is_ok(), "Explicit current directory should be valid");
    }

    #[tokio::test]
    async fn test_repository_validation_with_invalid_path() {
        // Test with non-existent path
        let result = app::validate_repository_path(Some("/nonexistent/path".to_string()));
        assert!(result.is_err(), "Non-existent path should fail validation");
        
        // Test with non-git directory
        let temp_dir = TempDir::new().unwrap();
        let result = app::validate_repository_path(Some(temp_dir.path().to_string_lossy().to_string()));
        assert!(result.is_err(), "Non-git directory should fail validation");
    }
}