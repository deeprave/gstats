mod cli;
mod config;
mod display;
mod logging;
mod notifications;
mod queue;
mod scanner;
mod plugin;
mod stats;
mod app;
mod output;

use anyhow::{Result, Context};
use std::process;
use std::sync::Arc;
use std::path::PathBuf;
use log::error;
use crate::display::CompactFormat;

/// Simple repository path resolution without validation
/// Validation will be handled by the scanner itself
fn resolve_repository_path(repository_arg: Option<&str>) -> Result<PathBuf> {
    match repository_arg {
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
                .or_else(|_| Ok(expanded_path))
        }
        None => {
            // Use current directory
            std::env::current_dir()
                .context("Failed to get current directory")
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
    // Set up panic handler with broken pipe handling
    std::panic::set_hook(Box::new(|panic_info| {
        let panic_str = panic_info.to_string();
        
        // Handle broken pipe errors gracefully (when piping to less, head, etc.)
        if panic_str.contains("Broken pipe") || panic_str.contains("os error 32") {
            // Silently exit on broken pipe - this is normal when piping to utilities
            process::exit(0);
        }
        
        // For other panics, show error and exit with error code
        error!("Application panicked: {:?}", panic_info);
        eprintln!("Panic: {:?}", panic_info);
        process::exit(101);
    }));
    
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
}

fn run() -> Result<()> {
    // Stage 1: Parse minimal configuration arguments for early initialization
    let initial_args = cli::initial_args::InitialArgs::parse_from_env();
    
    // Handle early exit cases (help/version) before any heavy initialization
    if initial_args.is_early_exit() {
        if initial_args.help_requested {
            // Use enhanced help system but fall back to traditional parsing for compatibility
            cli::args::display_enhanced_help(false, false);
            return Ok(());
        }
        if initial_args.version_requested {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
    }
    
    // Stage 1.5: Handle plugin help requests BEFORE clap parsing
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    
    // Set up early color override based on raw arguments for plugin help
    if raw_args.contains(&"--no-color".to_string()) {
        colored::control::set_override(false);
        std::env::set_var("NO_COLOR", "1");
    } else if raw_args.contains(&"--color".to_string()) {
        colored::control::set_override(true);
        std::env::set_var("CLICOLOR_FORCE", "1");
    }
    
    if raw_args.len() >= 2 && (raw_args.contains(&"--help".to_string()) || raw_args.contains(&"-h".to_string())) {
        // Check if this might be a plugin help request
        if let Some(help_result) = handle_potential_plugin_help(&raw_args, &initial_args)? {
            println!("{}", help_result);
            return Ok(());
        }
    }
    
    // Stage 2: Segment command line and parse with plugin-aware help
    let args = segment_and_parse_args(&initial_args)?;
    
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
    
    // Handle help command
    if args.help {
        cli::args::display_enhanced_help(args.no_color, args.color);
        return Ok(());
    }
    
    // Handle plugin management commands
    if args.list_plugins || args.show_plugins || args.plugins_help || args.plugin_info.is_some() || args.list_formats {
        return runtime.block_on(async {
            let config_manager = app::load_configuration(&args)?;
            app::handle_plugin_commands(&args, &config_manager).await
        });
    }
    
    // Handle --show-branch command
    if args.show_branch {
        return runtime.block_on(async {
            app::handle_show_branch_command(&args, &config_manager).await
        });
    }
    
    // Resolve repository path (scanner will validate it's a git repository)
    let repo_path = resolve_repository_path(args.repository.as_deref())?;
    
    // Run scanner with existing runtime
    let runtime_arc = Arc::new(runtime);
    let runtime_clone = Arc::clone(&runtime_arc);
    let result = runtime_arc.block_on(async {
        app::run_scanner(repo_path, args, config_manager, runtime_clone).await
    });
    
    // Runtime will be dropped when runtime_arc goes out of scope
    
    result
}

/// Parse command line arguments with plugin-aware segmentation
/// 
/// This function implements the two-stage parsing approach for GS-81:
/// 1. Uses initial_args for configuration discovery
/// 2. Segments command line by plugin boundaries 
/// 3. Handles plugin-specific help routing
/// 4. Parses global arguments using traditional clap derive
fn segment_and_parse_args(_initial_args: &cli::initial_args::InitialArgs) -> Result<cli::Args> {
    // Plugin help is now handled earlier in run(), so we can proceed with normal parsing
    
    // For now, fall back to traditional parsing for global arguments
    // The segmentation logic will be used in the next phase for plugin execution
    let args = cli::args::parse_args();
    Ok(args)
}

/// Handle potential plugin help requests
/// 
/// Checks if the command line contains a pattern like "plugin --help" and routes to plugin help
fn handle_potential_plugin_help(raw_args: &[String], initial_args: &cli::initial_args::InitialArgs) -> Result<Option<String>> {
    use crate::cli::converter::PluginConfig;
    use crate::cli::plugin_handler::PluginHandler;
    use crate::cli::command_segmenter::CommandSegmenter;
    
    // Create plugin configuration from initial_args
    let plugin_config = PluginConfig {
        directories: if let Some(dir) = &initial_args.plugin_dir {
            vec![dir.clone()]
        } else {
            vec!["plugins".to_string()]
        },
        plugin_exclude: if let Some(exclude) = &initial_args.plugin_exclude {
            exclude.split(',').map(|s| s.trim().to_string()).collect()
        } else {
            Vec::new()
        },
        plugin_load: Vec::new(),
    };
    
    // Create runtime for async operations
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
        
    runtime.block_on(async {
        // Create plugin handler and build command mappings
        let plugin_handler = PluginHandler::with_plugin_config(plugin_config)
            .map_err(|e| anyhow::anyhow!("Failed to create plugin handler: {}", e))?;
        
        let mut plugin_handler = plugin_handler;
        plugin_handler.build_command_mappings().await
            .map_err(|e| anyhow::anyhow!("Failed to build command mappings: {}", e))?;
        
        // Create command segmenter
        let segmenter = CommandSegmenter::new(plugin_handler).await
            .map_err(|e| anyhow::anyhow!("Failed to create command segmenter: {}", e))?;
        
        // Segment the arguments
        let segmented = segmenter.segment_arguments(raw_args)?;
        
        // Extract color flags from raw arguments
        let no_color = raw_args.contains(&"--no-color".to_string());
        let color = raw_args.contains(&"--color".to_string());
        
        // Check each plugin segment for help requests
        for segment in &segmented.plugin_segments {
            if segmenter.is_help_request(segment) {
                let help_text = segmenter.get_plugin_help_with_colors(&segment.plugin_name, segment.function_name.as_deref(), no_color, color).await
                    .map_err(|e| anyhow::anyhow!("Failed to get plugin help: {}", e))?;
                return Ok(Some(help_text));
            }
        }
        
        Ok(None)
    })
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
    async fn test_repository_path_resolution() {
        // Test with current directory
        let result = resolve_repository_path(None);
        assert!(result.is_ok(), "Should resolve current directory");
        
        // Test with explicit valid path
        let current_dir = std::env::current_dir().unwrap();
        let result = resolve_repository_path(Some(&current_dir.to_string_lossy()));
        assert!(result.is_ok(), "Should resolve explicit path");
        
        // Test with non-existent path (should still resolve the path, validation happens later)
        let result = resolve_repository_path(Some("/nonexistent/path"));
        assert!(result.is_ok(), "Should resolve path even if it doesn't exist - validation happens in scanner");
        
        // Test with directory that exists (even if not git)
        let temp_dir = TempDir::new().unwrap();
        let result = resolve_repository_path(Some(&temp_dir.path().to_string_lossy()));
        assert!(result.is_ok(), "Should resolve existing directory - git validation happens in scanner");
    }
}