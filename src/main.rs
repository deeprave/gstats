mod cli;
mod config;
mod display;
mod logging;
mod notifications;
mod queue;
mod scanner;
mod plugin;
mod app;

use anyhow::{Result, Context};
use std::process;
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
    
    // Stage 1.5: Create plugin settings from initial args and setup plugin system
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    
    // Create plugin settings early from parsed initial args
    let plugin_settings = crate::plugin::PluginSettings::from_initial_args(&initial_args);
    
    // Set up early color override based on initial args (not global args parsing)
    if initial_args.no_color {
        colored::control::set_override(false);
    } else if initial_args.color {
        colored::control::set_override(true);
    }
    
    // Create a segmenter and extract global args
    use crate::cli::converter::PluginConfig;
    use crate::cli::plugin_handler::PluginHandler;
    use crate::cli::command_segmenter::CommandSegmenter;
    
    let plugin_config = PluginConfig {
        directories: if let Some(dir) = &initial_args.plugin_dir {
            vec![dir.clone()]
        } else {
            vec![]
        },
        plugin_load: Vec::new(),
        plugin_exclude: if let Some(exclude_list) = &initial_args.plugin_exclude {
            exclude_list.split(',').map(|s| s.trim().to_string()).collect()
        } else {
            Vec::new()
        },
    };
    
    // Create a minimal runtime for plugin initialization
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to create tokio runtime for plugin initialization")?;
    
    let segmenter = rt.block_on(async {
        let plugin_handler = PluginHandler::with_plugin_config_and_settings(plugin_config, plugin_settings.clone())
            .map_err(|e| anyhow::anyhow!("Failed to create plugin handler: {}", e))?;
        let mut plugin_handler = plugin_handler;
        plugin_handler.build_command_mappings().await
            .map_err(|e| anyhow::anyhow!("Failed to build command mappings: {}", e))?;
        
        let segmenter = CommandSegmenter::new(plugin_handler).await
            .map_err(|e| anyhow::anyhow!("Failed to create command segmenter: {}", e))?;
        
        Result::<_, anyhow::Error>::Ok(segmenter)
    })?;
    
    let segmented = segmenter.segment_arguments(&raw_args)?;
    
    // Save plugin arguments to pass to the plugin later
    let plugin_args = if let Some(first_segment) = segmented.plugin_segments.first() {
        first_segment.args.clone()
    } else {
        Vec::new()
    };
    
    // Reconstruct args for clap parsing: global_args + first plugin command (NOT plugin args)
    let mut clap_args = segmented.global_args;
    
    // Add ONLY the first plugin command as the positional command argument
    // Do NOT include plugin arguments - they should be handled by the plugin itself
    if let Some(first_segment) = segmented.plugin_segments.first() {
        clap_args.push(first_segment.plugin_name.clone());
    }
    
    // Stage 2: Parse global arguments with clap
    let mut args = segment_and_parse_args(&clap_args, &initial_args)?;
    
    // Manually populate plugin_args since we didn't pass them to clap
    args.plugin_args = plugin_args;
    
    cli::args::validate_args(&args)?;
    
    let config_manager = app::load_configuration(&args)?;
    
    let log_config = app::configure_logging(&args, &config_manager)?;
    logging::init_logger(log_config)?;
    
    // Enhanced logging system is now ready
    
    // Handle configuration export command first (before creating runtime)
    if let Some(export_path) = &args.export_config {
        return app::initialization::handle_export_config(&config_manager, export_path);
    }
    
    // Main is no longer async - components can create their own runtimes cleanly
    
    // Handle help command
    if args.help {
        cli::args::display_enhanced_help(args.no_color, args.color);
        return Ok(());
    }
    
    // Handle plugin management commands
    if args.list_plugins || args.show_plugins || args.plugins_help || args.plugin_info.is_some() || args.list_formats {
        let config_manager = app::load_configuration(&args)?;
        return rt.block_on(app::handle_plugin_commands(&args, &config_manager));
    }
    
    // Handle --show-branch command
    if args.show_branch {
        return rt.block_on(app::handle_show_branch_command(&args, &config_manager));
    }
    
    // Resolve repository path (scanner will validate it's a git repository)
    let repo_path = resolve_repository_path(args.repository.as_deref())?;
    
    // Scanner handles its own runtime internally - clean sync interface
    app::run_scanner(repo_path, args, config_manager)
}

/// Parse command line arguments with plugin-aware segmentation
/// 
/// This function implements the two-stage parsing approach for GS-81:
/// 1. Uses initial_args for configuration discovery
/// 2. Segments command line by plugin boundaries 
/// 3. Handles plugin-specific help routing
/// 4. Parses global arguments using traditional clap derive
fn segment_and_parse_args(global_args: &[String], _initial_args: &cli::initial_args::InitialArgs) -> Result<cli::Args> {
    use clap::Parser;
    
    // Parse only the global arguments extracted from segmentation
    // Each plugin will handle its own argument parsing
    let mut args_with_program = vec!["gstats".to_string()];
    args_with_program.extend_from_slice(global_args);
    
    let args = cli::Args::try_parse_from(&args_with_program)
        .map_err(|e| anyhow::anyhow!("Failed to parse global arguments: {}", e))?;
    let args = args.apply_enhanced_parsing();
    
    Ok(args)
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