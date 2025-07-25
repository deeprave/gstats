// Git repository detection and validation module
// Provides functionality to check if directories are git repositories

use git2::Repository;
use anyhow::{Result, Context};
use std::path::Path;

/// Check if the given path is a git repository
pub fn is_git_repository<P: AsRef<Path>>(path: P) -> bool {
    Repository::open(path).is_ok()
}

/// Validate that the given path is an accessible git repository
/// Returns the canonical path if valid, error otherwise
pub fn validate_git_repository<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = path.as_ref();
    
    // Check if path exists
    if !path.exists() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    // Try to open as git repository
    let _repo = Repository::open(path)
        .with_context(|| {
            format!(
                "Directory is not a git repository: {}\n\nHelp: Make sure you're in a git repository or specify a valid git repository path.",
                path.display()
            )
        })?;

    // Return canonical path
    let canonical_path = path.canonicalize()
        .with_context(|| format!("Failed to resolve path: {}", path.display()))?;

    Ok(canonical_path.to_string_lossy().to_string())
}

/// Get the repository path, using current directory if none specified
/// Validates that the resulting path is a git repository
pub fn resolve_repository_path(path_option: Option<String>) -> Result<String> {
    let path = path_option.unwrap_or_else(|| ".".to_string());
    validate_git_repository(path)
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
        // Test with a directory that definitely isn't a git repository
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
        assert!(error_msg.contains("Help:")); // Should contain helpful message
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
