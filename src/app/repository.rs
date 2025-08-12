//! Repository path resolution and validation

use anyhow::{Result, Context};
use std::path::PathBuf;
use log::{info, debug};

/// Resolve repository path from CLI arguments
/// If no path provided, uses current directory and validates it's a git repository
pub fn resolve_repository_path(repository_arg: Option<String>) -> Result<PathBuf> {
    match repository_arg {
        Some(path) => {
            debug!("Repository path provided: {}", path);
            let path_buf = PathBuf::from(path);
            
            if !path_buf.exists() {
                anyhow::bail!(
                    "Directory does not exist: {}\n\nPlease check the path and try again. Make sure you have permission to access the directory.", 
                    path_buf.display()
                );
            }
            
            // Validate it's a git repository
            gix::discover(&path_buf)
                .with_context(|| format!(
                    "Not a valid git repository: {}\n\nMake sure this directory contains a git repository (initialized with 'git init' or cloned from a remote).\nIf this is the correct path, check that the .git directory exists and is accessible.", 
                    path_buf.display()
                ))?;
            
            path_buf.canonicalize()
                .with_context(|| format!("Failed to resolve canonical path for: {}", path_buf.display()))
        }
        None => {
            debug!("No repository path provided, using current directory");
            let current_dir = std::env::current_dir()
                .context("Failed to get current directory")?;
            
            gix::discover(&current_dir)
                .with_context(|| format!(
                    "Current directory '{}' is not a git repository.\n\nTo fix this:\n  • Navigate to a git repository directory\n  • Or specify a repository path: gstats --repository /path/to/repo\n  • Or initialize a git repository: git init", 
                    current_dir.display()
                ))?;
            
            info!("Using current directory as git repository: {}", current_dir.display());
            Ok(current_dir)
        }
    }
}

/// Validate a repository path (used for testing and CLI validation)
pub fn validate_repository_path(path: Option<String>) -> Result<PathBuf> {
    resolve_repository_path(path)
}