use git2::Repository;
use anyhow::{Result, Context};
use std::path::Path;
use log::{debug, info, warn, error};

/// Check if the given path is a git repository
pub fn is_git_repository<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    debug!("Checking if path is git repository: {}", path.display());
    
    match Repository::open(path) {
        Ok(_) => {
            debug!("Git repository detected at: {}", path.display());
            true
        }
        Err(e) => {
            debug!("Not a git repository at {}: {}", path.display(), e);
            false
        }
    }
}

/// Validate that the given path is an accessible git repository
/// Returns the canonical path if valid, error otherwise
pub fn validate_git_repository<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = path.as_ref();
    info!("Validating git repository at: {}", path.display());
    
    if !path.exists() {
        error!("Path does not exist: {}", path.display());
        anyhow::bail!("Path does not exist: {}", path.display());
    }
    
    if !is_git_repository(path) {
        error!("Path is not a git repository: {}", path.display());
        anyhow::bail!(
            "The path '{}' is not a git repository. Please specify a valid git repository path.",
            path.display()
        );
    }
    
    let canonical_path = path.canonicalize()
        .with_context(|| format!("Failed to resolve canonical path for: {}", path.display()))?;
    
    let path_str = canonical_path.to_string_lossy().to_string();
    info!("Git repository validated successfully: {}", path_str);
    Ok(path_str)
}

/// Resolve repository path from optional argument
/// If no path provided, uses current directory and validates it's a git repository
pub fn resolve_repository_path(repository_arg: Option<String>) -> Result<String> {
    match repository_arg {
        Some(path) => {
            debug!("Repository path provided: {}", path);
            validate_git_repository(path)
        }
        None => {
            debug!("No repository path provided, using current directory");
            let current_dir = std::env::current_dir()
                .context("Failed to get current directory")?;
            
            if !is_git_repository(&current_dir) {
                warn!("Current directory is not a git repository: {}", current_dir.display());
                anyhow::bail!(
                    "Current directory '{}' is not a git repository. Please run this command from within a git repository or specify a repository path.",
                    current_dir.display()
                );
            }
            
            info!("Using current directory as git repository: {}", current_dir.display());
            Ok(current_dir.to_string_lossy().to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_git_repository_with_current_dir() {
        // Current directory should be a git repository (since we're in gstats repo)
        assert!(is_git_repository("."));
    }

    #[test]
    fn test_is_git_repository_with_non_git_dir() {
        // We'll use /tmp which should exist but not be a git repo
        assert!(!is_git_repository("/tmp"));
    }

    #[test]
    fn test_validate_git_repository_current_dir() {
        // Current directory should validate successfully
        let result = validate_git_repository(".");
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.contains("gstats")); // Should contain our project name
    }

    #[test]
    fn test_validate_git_repository_nonexistent_path() {
        // Non-existent path should return error
        let result = validate_git_repository("/definitely/does/not/exist");
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Path does not exist"));
    }

    #[test]
    fn test_validate_git_repository_non_git_dir() {
        // Existing directory that's not a git repository should return error
        let result = validate_git_repository("/tmp");
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("not a git repository"));
        assert!(error_msg.contains("Please specify")); // Should contain helpful message
    }

    #[test]
    fn test_resolve_repository_path_with_none() {
        // None should default to current directory and validate it
        let result = resolve_repository_path(None);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.contains("gstats"));
    }

    #[test]
    fn test_resolve_repository_path_with_current_dir() {
        // Explicit current directory should work
        let result = resolve_repository_path(Some(".".to_string()));
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.contains("gstats"));
    }

    #[test]
    fn test_resolve_repository_path_with_invalid_path() {
        // Invalid path should return error
        let result = resolve_repository_path(Some("/invalid/path".to_string()));
        assert!(result.is_err());
    }
}
