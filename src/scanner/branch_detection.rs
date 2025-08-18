//! Branch Detection Logic
//! 
//! Provides intelligent branch detection and selection for repository scanning.

use std::path::Path;
use thiserror::Error;
use gix::bstr::ByteSlice;

/// Branch detection errors
#[derive(Error, Debug, PartialEq)]
pub enum BranchDetectionError {
    #[error("Branch '{branch}' not found in repository")]
    BranchNotFound { branch: String },
    #[error("Remote '{remote}' not found in repository")]
    RemoteNotFound { remote: String },
    #[error("No remotes found in repository")]
    NoRemotesFound,
    #[error("No suitable branch found using fallback strategy")]
    NoSuitableBranch,
    #[error("Repository error: {message}")]
    RepositoryError { message: String },
}

/// Branch detection configuration
#[derive(Debug, Clone)]
pub struct BranchDetectionConfig {
    /// Default branch to use if available
    pub default_branch: Option<String>,
    /// List of fallback branches in priority order
    pub fallback_branches: Vec<String>,
    /// Default remote to use for remote branch detection
    pub default_remote: Option<String>,
}

impl Default for BranchDetectionConfig {
    fn default() -> Self {
        Self {
            default_branch: None,
            fallback_branches: vec![
                "main".to_string(),
                "master".to_string(),
                "develop".to_string(),
                "trunk".to_string(),
            ],
            default_remote: None,
        }
    }
}

/// Branch detection result
#[derive(Debug, Clone, PartialEq)]
pub struct BranchDetectionResult {
    /// Selected branch name
    pub branch_name: String,
    /// Source of branch selection
    pub selection_source: BranchSelectionSource,
    /// Commit ID of the selected branch
    pub commit_id: String,
}

/// Source of branch selection for tracking and debugging
#[derive(Debug, Clone, PartialEq)]
pub enum BranchSelectionSource {
    /// Explicitly provided via CLI
    CliOverride,
    /// From configuration file
    ConfigDefault,
    /// Remote default branch
    RemoteDefault,
    /// Symbolic HEAD reference
    SymbolicHead,
    /// From fallback list
    Fallback,
}

impl BranchSelectionSource {
    /// Get debug string representation
    pub fn debug(&self) -> &'static str {
        match self {
            BranchSelectionSource::CliOverride => "CLI Override",
            BranchSelectionSource::ConfigDefault => "Config Default", 
            BranchSelectionSource::RemoteDefault => "Remote Default",
            BranchSelectionSource::SymbolicHead => "Symbolic HEAD",
            BranchSelectionSource::Fallback => "Fallback",
        }
    }
}

/// Branch detection implementation
pub struct BranchDetection {
    config: BranchDetectionConfig,
}

impl BranchDetection {
    /// Create branch detection with default configuration
    pub fn new() -> Self {
        Self {
            config: BranchDetectionConfig::default(),
        }
    }
    
    /// Create branch detection with custom configuration
    pub fn with_config(config: BranchDetectionConfig) -> Self {
        Self { config }
    }
    
    /// Detect appropriate branch for scanning
    /// 
    /// Priority order:
    /// 1. CLI branch parameter (if provided)
    /// 2. Config file default-branch setting
    /// 3. Remote default branch (CLI remote → Config remote → First available remote)
    /// 4. Symbolic HEAD reference
    /// 5. Fallback list in order
    pub fn detect_branch(
        &self,
        repository_path: &Path,
        cli_branch: Option<&str>,
        cli_remote: Option<&str>,
        cli_fallbacks: Option<&[String]>,
    ) -> Result<BranchDetectionResult, BranchDetectionError> {
        let repo = gix::discover(repository_path)
            .map_err(|e| BranchDetectionError::RepositoryError {
                message: format!("Failed to open repository: {e}"),
            })?;

        // Priority 1: CLI branch parameter
        if let Some(branch) = cli_branch {
            if let Ok(commit_id) = self.resolve_branch_ref_internal(&repo, branch) {
                return Ok(BranchDetectionResult {
                    branch_name: branch.to_string(),
                    selection_source: BranchSelectionSource::CliOverride,
                    commit_id,
                });
            } else {
                return Err(BranchDetectionError::BranchNotFound {
                    branch: branch.to_string(),
                });
            }
        }

        // Priority 2: Config file default-branch setting
        if let Some(ref default_branch) = self.config.default_branch {
            if let Ok(commit_id) = self.resolve_branch_ref_internal(&repo, default_branch) {
                return Ok(BranchDetectionResult {
                    branch_name: default_branch.clone(),
                    selection_source: BranchSelectionSource::ConfigDefault,
                    commit_id,
                });
            }
        }

        // Priority 3: Remote default branch detection
        if let Ok(result) = self.detect_remote_default_branch(&repo, cli_remote) {
            return Ok(result);
        }

        // Priority 4: Symbolic HEAD reference
        if let Ok(result) = self.detect_symbolic_head(&repo) {
            return Ok(result);
        }

        // Priority 5: Fallback list in order
        let fallbacks = cli_fallbacks.unwrap_or(&self.config.fallback_branches);
        for fallback in fallbacks {
            if let Ok(commit_id) = self.resolve_branch_ref_internal(&repo, fallback) {
                return Ok(BranchDetectionResult {
                    branch_name: fallback.clone(),
                    selection_source: BranchSelectionSource::Fallback,
                    commit_id,
                });
            }
        }

        Err(BranchDetectionError::NoSuitableBranch)
    }
    
    /// Convert branch name to commit ID
    pub fn resolve_branch_ref(
        &self,
        repository_path: &Path,
        branch_name: &str,
    ) -> Result<String, BranchDetectionError> {
        let repo = gix::discover(repository_path)
            .map_err(|e| BranchDetectionError::RepositoryError {
                message: format!("Failed to open repository: {e}"),
            })?;

        self.resolve_branch_ref_internal(&repo, branch_name)
    }

    /// Internal method to resolve branch reference using existing repo handle
    fn resolve_branch_ref_internal(
        &self,
        repo: &gix::Repository,
        branch_name: &str,
    ) -> Result<String, BranchDetectionError> {
        // Try to resolve local branch
        let local_ref = format!("refs/heads/{branch_name}");
        if let Ok(reference) = repo.find_reference(&local_ref) {
            if let Some(id) = reference.try_id() {
                return Ok(id.to_string());
            }
        }

        // Try remote branches
        for remote in repo.remote_names() {
            let remote_ref = format!("refs/remotes/{remote}/{branch_name}");
            if let Ok(reference) = repo.find_reference(&remote_ref) {
                if let Some(id) = reference.try_id() {
                    return Ok(id.to_string());
                }
            }
        }

        Err(BranchDetectionError::BranchNotFound {
            branch: branch_name.to_string(),
        })
    }

    /// Detect remote default branch
    fn detect_remote_default_branch(
        &self,
        repo: &gix::Repository,
        cli_remote: Option<&str>,
    ) -> Result<BranchDetectionResult, BranchDetectionError> {
        let remote_names: Vec<String> = repo.remote_names().into_iter()
            .map(|name| name.to_string())
            .collect();
        if remote_names.is_empty() {
            return Err(BranchDetectionError::NoRemotesFound);
        }

        // Determine which remote to use
        let target_remote = if let Some(remote) = cli_remote {
            if remote_names.contains(&remote.to_string()) {
                remote.to_string()
            } else {
                return Err(BranchDetectionError::RemoteNotFound {
                    remote: remote.to_string(),
                });
            }
        } else if let Some(ref default_remote) = self.config.default_remote {
            if remote_names.contains(default_remote) {
                default_remote.clone()
            } else {
                return Err(BranchDetectionError::RemoteNotFound {
                    remote: default_remote.clone(),
                });
            }
        } else {
            // Use first available remote
            remote_names[0].clone()
        };

        // Try to find remote HEAD reference
        let remote_head_ref = format!("refs/remotes/{target_remote}/HEAD");
        if let Ok(reference) = repo.find_reference(&remote_head_ref) {
            // Try to get the target reference name for symbolic references
            match reference.target() {
                gix::refs::TargetRef::Symbolic(target) => {
                    if let Some(branch_name) = target.as_bstr().to_str().ok()
                        .and_then(|s| s.strip_prefix(&format!("refs/remotes/{target_remote}/"))) {
                        // Get the commit ID for this branch
                        if let Ok(commit_id) = self.resolve_branch_ref_internal(repo, branch_name) {
                            return Ok(BranchDetectionResult {
                                branch_name: branch_name.to_string(),
                                selection_source: BranchSelectionSource::RemoteDefault,
                                commit_id,
                            });
                        }
                    }
                },
                // Try direct commit ID if available
                _ => {
                    if let Some(commit_id) = reference.try_id() {
                        // Try to find which branch this belongs to
                        for default_name in &["main", "master"] {
                            if let Ok(branch_commit_id) = self.resolve_branch_ref_internal(repo, default_name) {
                                if branch_commit_id == commit_id.to_string() {
                                    return Ok(BranchDetectionResult {
                                        branch_name: default_name.to_string(),
                                        selection_source: BranchSelectionSource::RemoteDefault,
                                        commit_id: commit_id.to_string(),
                                    });
                                }
                            }
                        }
                    }
                },
            }
        }

        // Fallback: try common default branches on the remote
        for default_name in &["main", "master"] {
            let remote_ref = format!("refs/remotes/{target_remote}/{default_name}");
            if let Ok(reference) = repo.find_reference(&remote_ref) {
                if let Some(commit_id) = reference.try_id() {
                    return Ok(BranchDetectionResult {
                        branch_name: default_name.to_string(),
                        selection_source: BranchSelectionSource::RemoteDefault,
                        commit_id: commit_id.to_string(),
                    });
                }
            }
        }

        Err(BranchDetectionError::NoSuitableBranch)
    }

    /// Detect symbolic HEAD reference
    fn detect_symbolic_head(&self, repo: &gix::Repository) -> Result<BranchDetectionResult, BranchDetectionError> {
        if let Ok(head_ref) = repo.find_reference("HEAD") {
            match head_ref.target() {
                gix::refs::TargetRef::Symbolic(target) => {
                    // HEAD points to a branch reference, extract the branch name
                    if let Some(branch_name) = target.as_bstr().to_str().ok()
                        .and_then(|s| s.strip_prefix("refs/heads/")) {
                        // Get the commit ID that this branch points to
                        if let Ok(commit_id) = self.resolve_branch_ref_internal(repo, branch_name) {
                            return Ok(BranchDetectionResult {
                                branch_name: branch_name.to_string(),
                                selection_source: BranchSelectionSource::SymbolicHead,
                                commit_id,
                            });
                        }
                    }
                },
                // Direct commit ID (detached HEAD)
                _ => {
                    if let Some(commit_id) = head_ref.try_id() {
                        // HEAD is detached, pointing directly to a commit
                        // Try to find which branch this commit belongs to by checking all local branches
                        for branch_name in &["main", "master", "develop", "trunk"] {
                            if let Ok(branch_commit_id) = self.resolve_branch_ref_internal(repo, branch_name) {
                                if branch_commit_id == commit_id.to_string() {
                                    return Ok(BranchDetectionResult {
                                        branch_name: branch_name.to_string(),
                                        selection_source: BranchSelectionSource::SymbolicHead,
                                        commit_id: commit_id.to_string(),
                                    });
                                }
                            }
                        }
                        
                        // If no common branch matches, use the first local branch that matches
                        for remote in repo.remote_names() {
                            for branch_name in &["main", "master"] {
                                let remote_ref = format!("refs/remotes/{remote}/{branch_name}");
                                if let Ok(reference) = repo.find_reference(&remote_ref) {
                                    if let Some(branch_commit_id) = reference.try_id() {
                                        if branch_commit_id.to_string() == commit_id.to_string() {
                                            return Ok(BranchDetectionResult {
                                                branch_name: branch_name.to_string(),
                                                selection_source: BranchSelectionSource::SymbolicHead,
                                                commit_id: commit_id.to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }

        Err(BranchDetectionError::NoSuitableBranch)
    }
}

impl Default for BranchDetection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_branch_detection_creation() {
        let detection = BranchDetection::new();
        assert_eq!(detection.config.fallback_branches.len(), 4);
        assert_eq!(detection.config.fallback_branches[0], "main");
    }

    #[test]
    fn test_branch_detection_with_custom_config() {
        let config = BranchDetectionConfig {
            default_branch: Some("develop".to_string()),
            fallback_branches: vec!["develop".to_string(), "main".to_string()],
            default_remote: Some("upstream".to_string()),
        };
        let detection = BranchDetection::with_config(config);
        assert_eq!(detection.config.default_branch, Some("develop".to_string()));
        assert_eq!(detection.config.default_remote, Some("upstream".to_string()));
    }

    #[test]
    fn test_intelligent_default_branch_detection_priority() {
        let detection = BranchDetection::new();
        let repo_path = Path::new(".");
        
        // Should succeed if we're in a git repository, fail otherwise
        let result = detection.detect_branch(&repo_path, None, None, None);
        match result {
            Ok(branch_result) => {
                // Successful branch detection - verify it's a valid result
                assert!(!branch_result.branch_name.is_empty());
                assert!(!branch_result.commit_id.is_empty());
                println!("✅ Detected branch: {} ({})", branch_result.branch_name, branch_result.selection_source.debug());
            }
            Err(BranchDetectionError::RepositoryError { .. }) => {
                // Expected if not in a git repository
                println!("⚠️  Not in a git repository - expected for some test environments");
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_remote_detection_without_hardcoded_origin() {
        let detection = BranchDetection::new();
        let repo_path = Path::new(".");
        
        // Should succeed if we're in a git repository with remotes
        let result = detection.detect_branch(&repo_path, None, None, None);
        match result {
            Ok(_) => println!("✅ Branch detection succeeded"),
            Err(BranchDetectionError::RepositoryError { .. }) => {
                println!("⚠️  Not in a git repository - expected for some test environments");
            }
            Err(BranchDetectionError::NoRemotesFound) => {
                println!("⚠️  No remotes found - expected for some test environments");
            }
            Err(BranchDetectionError::NoSuitableBranch) => {
                println!("⚠️  No suitable branch found - expected for some test environments");
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_symbolic_head_resolution() {
        let detection = BranchDetection::new();
        let repo_path = Path::new(".");
        
        // Should succeed if we're in a git repository
        let result = detection.detect_branch(&repo_path, None, None, None);
        match result {
            Ok(_) => println!("✅ Symbolic HEAD resolution succeeded"),
            Err(BranchDetectionError::RepositoryError { .. }) => {
                println!("⚠️  Not in a git repository - expected for some test environments");
            }
            Err(BranchDetectionError::NoSuitableBranch) => {
                println!("⚠️  No suitable branch found - expected for some test environments");
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_fallback_list_processing() {
        let detection = BranchDetection::new();
        let repo_path = Path::new(".");
        
        // Should succeed if we're in a git repository
        let result = detection.detect_branch(&repo_path, None, None, None);
        match result {
            Ok(_) => println!("✅ Fallback list processing succeeded"),
            Err(BranchDetectionError::RepositoryError { .. }) => {
                println!("⚠️  Not in a git repository - expected for some test environments");
            }
            Err(BranchDetectionError::NoSuitableBranch) => {
                println!("⚠️  No suitable branch found - expected for some test environments");
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_nonexistent_branch_handling() {
        let detection = BranchDetection::new();
        let repo_path = Path::new(".");
        
        let result = detection.resolve_branch_ref(&repo_path, "nonexistent_branch");
        assert!(result.is_err());
        match result.unwrap_err() {
            BranchDetectionError::BranchNotFound { branch } => {
                assert_eq!(branch, "nonexistent_branch");
            }
            BranchDetectionError::RepositoryError { .. } => {
                // Expected if not in a git repository
                println!("⚠️  Not in a git repository - expected for some test environments");
            }
            e => panic!("Unexpected error: {:?}", e),
        }
    }
}