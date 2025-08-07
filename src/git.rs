use git2::Repository;
use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use log::{debug, info, error};




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


/// Validate that the given path is an accessible git repository
/// Returns a Repository handle if valid, error otherwise
pub fn validate_git_repository_handle<P: AsRef<Path>>(path: P) -> Result<Repository> {
    let path = path.as_ref();
    debug!("Validating git repository handle at: {}", path.display());
    
    if !path.exists() {
        error!("Path does not exist: {}", path.display());
        anyhow::bail!(
            "Directory does not exist: {}\n\nPlease check the path and try again. Make sure you have permission to access the directory.", 
            path.display()
        );
    }
    
    let repo = Repository::open(path)
        .with_context(|| format!(
            "Not a valid git repository: {}\n\nMake sure this directory contains a git repository (initialized with 'git init' or cloned from a remote).\nIf this is the correct path, check that the .git directory exists and is accessible.", 
            path.display()
        ))?;
    
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
                    "Current directory '{}' is not a git repository.\n\nTo fix this:\n  • Navigate to a git repository directory\n  • Or specify a repository path: gstats --repository /path/to/repo\n  • Or initialize a git repository: git init", 
                    current_dir.display()
                ))?;
            
            info!("Using current directory as git repository: {}", current_dir.display());
            Ok(RepositoryHandle::from_repository(repo))
        }
    }
}

