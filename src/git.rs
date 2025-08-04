use git2::Repository;
use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use log::{debug, info, error};

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
    // Use the new function internally to avoid code duplication
    let repo = validate_git_repository_handle(path)?;
    
    // Get the canonical path from the repository
    let workdir = repo.workdir()
        .or_else(|| Some(repo.path()))
        .expect("Repository must have a path");
    
    let canonical_path = workdir.canonicalize()
        .with_context(|| format!("Failed to resolve canonical path for: {}", workdir.display()))?;
    
    let path_str = canonical_path.to_string_lossy().to_string();
    Ok(path_str)
}

/// Resolve repository path from optional argument
/// If no path provided, uses current directory and validates it's a git repository
pub fn resolve_repository_path(repository_arg: Option<String>) -> Result<String> {
    // Use the new function internally and convert back to string
    let handle = resolve_repository_handle(repository_arg)?;
    Ok(handle.path())
}

/// A wrapper around git2::Repository that provides scanner-friendly functionality
#[derive(Clone)]
pub struct RepositoryHandle {
    repository: Arc<Repository>,
    path: PathBuf,
}

impl RepositoryHandle {
    /// Open a repository from a path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let repo = Repository::open(path)
            .with_context(|| format!("Failed to open repository at: {}", path.display()))?;
        
        let canonical_path = path.canonicalize()
            .with_context(|| format!("Failed to resolve canonical path for: {}", path.display()))?;
        
        Ok(Self {
            repository: Arc::new(repo),
            path: canonical_path,
        })
    }
    
    /// Create a handle from an existing Repository
    pub fn from_repository(repository: Repository) -> Self {
        let path = repository
            .workdir()
            .unwrap_or_else(|| repository.path())
            .to_path_buf();
        
        Self {
            repository: Arc::new(repository),
            path,
        }
    }
    
    /// Get the repository path as a string
    pub fn path(&self) -> String {
        self.path.to_string_lossy().to_string()
    }
    
    /// Get the repository working directory
    pub fn workdir(&self) -> Option<&Path> {
        self.repository.workdir()
    }
    
    /// Check if this is a bare repository
    pub fn is_bare(&self) -> bool {
        self.repository.is_bare()
    }
    
    /// Convert to a path string
    pub fn to_path_string(&self) -> String {
        self.path()
    }
    
    /// Get a reference to the underlying Repository
    pub fn repository(&self) -> &Repository {
        &self.repository
    }
}

// Implement Send + Sync for thread safety
unsafe impl Send for RepositoryHandle {}
unsafe impl Sync for RepositoryHandle {}

/// Context for scanner initialization containing all necessary components
pub struct ScannerInitContext {
    repository: RepositoryHandle,
    config: crate::scanner::ScannerConfig,
    query: crate::scanner::QueryParams,
}

impl ScannerInitContext {
    /// Create a new scanner initialization context
    pub fn new(
        repository: RepositoryHandle,
        config: crate::scanner::ScannerConfig,
        query: crate::scanner::QueryParams,
    ) -> Self {
        Self {
            repository,
            config,
            query,
        }
    }
    
    /// Get the repository handle
    pub fn repository(&self) -> &RepositoryHandle {
        &self.repository
    }
    
    /// Get the scanner configuration
    pub fn config(&self) -> &crate::scanner::ScannerConfig {
        &self.config
    }
    
    /// Get the query parameters
    pub fn query(&self) -> &crate::scanner::QueryParams {
        &self.query
    }
}

/// Validate that the given path is an accessible git repository
/// Returns a Repository handle if valid, error otherwise
pub fn validate_git_repository_handle<P: AsRef<Path>>(path: P) -> Result<Repository> {
    let path = path.as_ref();
    debug!("Validating git repository handle at: {}", path.display());
    
    if !path.exists() {
        error!("Path does not exist: {}", path.display());
        anyhow::bail!("Path does not exist: {}", path.display());
    }
    
    let repo = Repository::open(path)
        .with_context(|| format!("Failed to open repository at: {}", path.display()))?;
    
    if repo.is_bare() {
        debug!("Repository is bare: {}", path.display());
    }
    
    debug!("Git repository handle validated successfully: {}", path.display());
    Ok(repo)
}

/// Resolve repository path and return a RepositoryHandle
/// If no path provided, uses current directory and validates it's a git repository
pub fn resolve_repository_handle(repository_arg: Option<String>) -> Result<RepositoryHandle> {
    match repository_arg {
        Some(path) => {
            debug!("Repository path provided: {}", path);
            let repo = validate_git_repository_handle(&path)?;
            Ok(RepositoryHandle::from_repository(repo))
        }
        None => {
            debug!("No repository path provided, using current directory");
            let current_dir = std::env::current_dir()
                .context("Failed to get current directory")?;
            
            let repo = Repository::open(&current_dir)
                .with_context(|| format!(
                    "Current directory '{}' is not a git repository. Please run this command from within a git repository or specify a repository path.",
                    current_dir.display()
                ))?;
            
            info!("Using current directory as git repository: {}", current_dir.display());
            Ok(RepositoryHandle::from_repository(repo))
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
        // The error message now comes from git2 library via validate_git_repository_handle
        assert!(error_msg.contains("Failed to open repository"));
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
