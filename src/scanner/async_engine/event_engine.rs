use crate::scanner::async_engine::events::{
    RepositoryEvent, CommitInfo, FileChangeData, FileInfo, RepositoryStats, EventFilter, ChangeType
};
use crate::scanner::async_engine::repository::AsyncRepositoryHandle;
use crate::scanner::async_engine::error::{ScanResult, ScanError};
use crate::scanner::query::QueryParams;
use crate::scanner::modes::ScanMode;
use futures::stream::Stream;
use futures::stream;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tokio::fs;
use git2::{Repository, DiffOptions};
use log::{debug, info, warn};

/// Core event engine for single-pass repository traversal
pub struct RepositoryEventEngine {
    repository: Arc<AsyncRepositoryHandle>,
    filter: EventFilter,
}

impl RepositoryEventEngine {
    /// Create a new repository event engine
    pub fn new(repository: Arc<AsyncRepositoryHandle>, query_params: QueryParams, modes: ScanMode) -> Self {
        let filter = EventFilter::from_query_params(query_params, modes);
        Self {
            repository,
            filter,
        }
    }

    /// Scan repository and emit events in a single pass
    pub async fn scan_repository(&self) -> ScanResult<impl Stream<Item = RepositoryEvent>> {
        let start_time = Instant::now();
        let modes = self.filter.modes;
        
        debug!("Starting single-pass repository scan with modes: {:?}", modes);

        // Get repository statistics for initial event
        let (total_commits, total_files) = self.estimate_repository_size().await?;
        
        // Create event stream
        let events = stream::iter(self.generate_events(total_commits, total_files, start_time).await?);
        
        Ok(events)
    }

    /// Generate all events for the repository in a single pass
    async fn generate_events(
        &self, 
        total_commits: Option<usize>, 
        total_files: Option<usize>,
        start_time: Instant
    ) -> ScanResult<Vec<RepositoryEvent>> {
        let mut events = Vec::new();
        let mut stats = RepositoryStats {
            total_commits: 0,
            total_files: 0,
            total_changes: 0,
            scan_duration: start_time.elapsed(),
            events_emitted: 0,
        };

        // Emit repository started event
        events.push(RepositoryEvent::RepositoryStarted {
            total_commits,
            total_files,
            scan_modes: self.filter.modes,
        });

        // Single repository traversal - collect all data in one pass
        let repo_data = self.collect_repository_data().await?;

        // Process commits if HISTORY or CHANGE_FREQUENCY modes are active
        if self.filter.modes.contains(ScanMode::HISTORY) || 
           self.filter.modes.contains(ScanMode::CHANGE_FREQUENCY) {
            
            for (index, commit_info) in repo_data.commits.iter().enumerate() {
                if self.filter.should_include_commit(commit_info) {
                    // Emit commit discovered event
                    if self.filter.modes.contains(ScanMode::HISTORY) {
                        events.push(RepositoryEvent::CommitDiscovered {
                            commit: commit_info.clone(),
                            index,
                        });
                    }

                    // For now, we'll skip file change events since we need to restructure
                    // the data collection to properly track file changes per commit
                    // This will be implemented in a future iteration

                    stats.total_commits += 1;
                }
            }
        }

        // Process files if FILES or METRICS modes are active
        if self.filter.modes.contains(ScanMode::FILES) || 
           self.filter.modes.contains(ScanMode::METRICS) {
            
            for file_info in &repo_data.files {
                if self.filter.should_include_file(file_info) {
                    events.push(RepositoryEvent::FileScanned {
                        file_info: file_info.clone(),
                    });
                    stats.total_files += 1;
                }
            }
        }

        // Update final statistics
        stats.scan_duration = start_time.elapsed();
        stats.events_emitted = events.len();

        info!(
            "Single-pass scan completed: {} commits, {} files, {} changes, {} events in {:?}",
            stats.total_commits, stats.total_files, stats.total_changes, 
            stats.events_emitted, stats.scan_duration
        );

        // Emit repository completed event
        events.push(RepositoryEvent::RepositoryCompleted { stats });

        Ok(events)
    }

    /// Collect all repository data in a single pass
    async fn collect_repository_data(&self) -> ScanResult<RepositoryData> {
        let mut repo_data = RepositoryData {
            commits: Vec::new(),
            files: Vec::new(),
        };

        // Collect commit history if needed
        if self.filter.modes.contains(ScanMode::HISTORY) || 
           self.filter.modes.contains(ScanMode::CHANGE_FREQUENCY) {
            repo_data.commits = self.collect_commit_history().await?;
        }

        // Collect file information if needed
        if self.filter.modes.contains(ScanMode::FILES) || 
           self.filter.modes.contains(ScanMode::METRICS) {
            repo_data.files = self.collect_file_information().await?;
        }

        Ok(repo_data)
    }

    /// Collect commit history with file changes
    async fn collect_commit_history(&self) -> ScanResult<Vec<CommitInfoWithChanges>> {
        let repo_path = &self.repository.path();

        let repo = Repository::open(repo_path).map_err(|e| {
            ScanError::repository(format!("Failed to open repository: {}", e))
        })?;

        let mut revwalk = repo.revwalk().map_err(|e| {
            ScanError::repository(format!("Failed to create revwalk: {}", e))
        })?;

        revwalk.push_head().map_err(|e| {
            ScanError::repository(format!("Failed to push HEAD: {}", e))
        })?;

        let mut commits = Vec::new();

        for oid_result in revwalk {
            let oid = oid_result.map_err(|e| {
                ScanError::repository(format!("Failed to get commit OID: {}", e))
            })?;

            let commit = repo.find_commit(oid).map_err(|e| {
                ScanError::repository(format!("Failed to find commit: {}", e))
            })?;

            // Convert git2::Commit to our CommitInfo
            let commit_info = self.convert_git_commit(&repo, &commit).await?;
            commits.push(commit_info);

            // Apply limit if specified
            if let Some(limit) = self.filter.query_params.limit {
                if commits.len() >= limit {
                    break;
                }
            }
        }

        Ok(commits)
    }

    /// Convert git2::Commit to our CommitInfo with file changes
    async fn convert_git_commit(
        &self, 
        repo: &Repository, 
        commit: &git2::Commit<'_>
    ) -> ScanResult<CommitInfoWithChanges> {
        let author = commit.author();
        let committer = commit.committer();
        let timestamp = SystemTime::UNIX_EPOCH + 
            std::time::Duration::from_secs(commit.time().seconds() as u64);

        let mut file_changes = Vec::new();
        let mut insertions = 0;
        let mut deletions = 0;
        let mut changed_files = Vec::new();

        // Get diff for this commit
        if let Ok(tree) = commit.tree() {
            let parent_tree = if commit.parent_count() > 0 {
                commit.parent(0).ok().and_then(|p| p.tree().ok())
            } else {
                None
            };

            let mut diff_opts = DiffOptions::new();
            let diff = repo.diff_tree_to_tree(
                parent_tree.as_ref(),
                Some(&tree),
                Some(&mut diff_opts)
            ).map_err(|e| {
                ScanError::repository(format!("Failed to create diff: {}", e))
            })?;

            // Process diff to extract file changes
            diff.foreach(
                &mut |delta, _progress| {
                    if let Some(new_file) = delta.new_file().path() {
                        let file_path = new_file.to_string_lossy().to_string();
                        changed_files.push(file_path.clone());

                        let change_type = match delta.status() {
                            git2::Delta::Added => ChangeType::Added,
                            git2::Delta::Deleted => ChangeType::Deleted,
                            git2::Delta::Modified => ChangeType::Modified,
                            git2::Delta::Renamed => ChangeType::Renamed,
                            git2::Delta::Copied => ChangeType::Copied,
                            _ => ChangeType::Modified,
                        };

                        let old_path = delta.old_file().path()
                            .map(|p| p.to_string_lossy().to_string());

                        file_changes.push(FileChangeData {
                            change_type,
                            old_path,
                            new_path: file_path,
                            insertions: 0, // Will be updated in hunk callback
                            deletions: 0,  // Will be updated in hunk callback
                            is_binary: false, // Will be detected in hunk callback
                        });
                    }
                    true
                },
                None,
                Some(&mut |_delta, _hunk| true),
                Some(&mut |_delta, _hunk, line| {
                    match line.origin() {
                        '+' => insertions += 1,
                        '-' => deletions += 1,
                        _ => {}
                    }
                    true
                })
            ).map_err(|e| {
                ScanError::repository(format!("Failed to process diff: {}", e))
            })?;
        }

        Ok(CommitInfoWithChanges {
            hash: commit.id().to_string(),
            short_hash: commit.id().to_string()[..7].to_string(),
            author_name: author.name().unwrap_or("Unknown").to_string(),
            author_email: author.email().unwrap_or("unknown@example.com").to_string(),
            committer_name: committer.name().unwrap_or("Unknown").to_string(),
            committer_email: committer.email().unwrap_or("unknown@example.com").to_string(),
            timestamp,
            message: commit.message().unwrap_or("").to_string(),
            parent_hashes: (0..commit.parent_count())
                .filter_map(|i| commit.parent_id(i).ok().map(|id| id.to_string()))
                .collect(),
            changed_files,
            insertions,
            deletions,
        })
    }

    /// Collect file information from working directory
    async fn collect_file_information(&self) -> ScanResult<Vec<FileInfo>> {
        let repo_path = &self.repository.path();
        let mut files = Vec::new();

        let mut entries = fs::read_dir(repo_path).await.map_err(|e| {
            ScanError::stream(format!("Failed to read directory {}: {}", repo_path, e))
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            ScanError::stream(format!("Failed to read directory entry: {}", e))
        })? {
            let path = entry.path();
            
            // Skip .git directory
            if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
                continue;
            }

            if path.is_file() {
                let relative_path = path.strip_prefix(repo_path)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();

                let metadata = match fs::metadata(&path).await {
                    Ok(meta) => meta,
                    Err(e) => {
                        warn!("Failed to get metadata for {}: {}", path.display(), e);
                        continue;
                    }
                };

                let extension = path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|s| s.to_string());

                let is_binary = self.is_binary_file(&path).await.unwrap_or(false);
                let line_count = if !is_binary {
                    self.count_lines(&path).await.ok()
                } else {
                    None
                };

                let last_modified = metadata.modified().ok();

                files.push(FileInfo {
                    path: path.clone(),
                    relative_path,
                    size: metadata.len(),
                    extension,
                    is_binary,
                    line_count,
                    last_modified,
                });
            } else if path.is_dir() {
                // Recursively process subdirectories
                // For now, we'll skip this to keep it simple
                // In a full implementation, we'd want to recursively walk the directory tree
            }
        }

        Ok(files)
    }

    /// Check if a file is binary
    async fn is_binary_file(&self, path: &std::path::Path) -> ScanResult<bool> {
        let mut buffer = [0; 8192];
        let mut file = match fs::File::open(path).await {
            Ok(file) => file,
            Err(_) => return Ok(false), // Assume text if can't read
        };

        use tokio::io::AsyncReadExt;
        let bytes_read = match file.read(&mut buffer).await {
            Ok(n) => n,
            Err(_) => return Ok(false),
        };

        // Simple binary detection: look for null bytes
        Ok(buffer[..bytes_read].contains(&0))
    }

    /// Count lines in a text file
    async fn count_lines(&self, path: &std::path::Path) -> ScanResult<usize> {
        let content = fs::read_to_string(path).await.map_err(|e| {
            ScanError::stream(format!("Failed to read file {}: {}", path.display(), e))
        })?;

        Ok(content.lines().count())
    }

    /// Estimate repository size for progress reporting
    async fn estimate_repository_size(&self) -> ScanResult<(Option<usize>, Option<usize>)> {
        let repo_path = &self.repository.path();

        // Quick estimate of commits
        let total_commits = if self.filter.modes.contains(ScanMode::HISTORY) || 
                              self.filter.modes.contains(ScanMode::CHANGE_FREQUENCY) {
            match Repository::open(repo_path) {
                Ok(repo) => {
                    match repo.revwalk() {
                        Ok(mut revwalk) => {
                            if revwalk.push_head().is_ok() {
                                Some(revwalk.count())
                            } else {
                                None
                            }
                        }
                        Err(_) => None,
                    }
                }
                Err(_) => None,
            }
        } else {
            None
        };

        // Quick estimate of files (simplified - just count immediate files)
        let total_files = if self.filter.modes.contains(ScanMode::FILES) || 
                            self.filter.modes.contains(ScanMode::METRICS) {
            match fs::read_dir(repo_path).await {
                Ok(mut entries) => {
                    let mut count = 0;
                    while let Ok(Some(_)) = entries.next_entry().await {
                        count += 1;
                    }
                    Some(count)
                }
                Err(_) => None,
            }
        } else {
            None
        };

        Ok((total_commits, total_files))
    }
}

/// Internal data structure for collecting repository data
struct RepositoryData {
    commits: Vec<CommitInfoWithChanges>,
    files: Vec<FileInfo>,
}

/// Extended commit info with file changes
type CommitInfoWithChanges = CommitInfo;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::RepositoryHandle;
    use tempfile::TempDir;
    use std::fs;

    /// Helper function to create a test Git repository
    fn create_test_git_repo(temp_dir: &TempDir) {
        let repo = git2::Repository::init(temp_dir.path()).unwrap();
        
        // Configure user for commits
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        
        // Create an initial commit to make it a proper repository
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        ).unwrap();
    }

    #[tokio::test]
    async fn test_event_engine_creation() {
        let temp_dir = TempDir::new().unwrap();
        create_test_git_repo(&temp_dir);
        let repo_handle = RepositoryHandle::open(temp_dir.path()).unwrap();
        let async_repo_handle = Arc::new(AsyncRepositoryHandle::new(repo_handle));
        let query_params = QueryParams::default();
        let modes = ScanMode::FILES;

        let engine = RepositoryEventEngine::new(async_repo_handle, query_params, modes);
        assert_eq!(engine.filter.modes, modes);
    }

    #[tokio::test]
    async fn test_binary_file_detection() {
        let temp_dir = TempDir::new().unwrap();
        create_test_git_repo(&temp_dir);
        let repo_handle = RepositoryHandle::open(temp_dir.path()).unwrap();
        let async_repo_handle = Arc::new(AsyncRepositoryHandle::new(repo_handle));
        let engine = RepositoryEventEngine::new(async_repo_handle, QueryParams::default(), ScanMode::FILES);

        // Create a text file
        let text_file = temp_dir.path().join("test.txt");
        fs::write(&text_file, "Hello, world!").unwrap();

        // Create a binary file
        let binary_file = temp_dir.path().join("test.bin");
        fs::write(&binary_file, &[0, 1, 2, 3, 0, 255]).unwrap();

        assert!(!engine.is_binary_file(&text_file).await.unwrap());
        assert!(engine.is_binary_file(&binary_file).await.unwrap());
    }

    #[tokio::test]
    async fn test_line_counting() {
        let temp_dir = TempDir::new().unwrap();
        create_test_git_repo(&temp_dir);
        let repo_handle = RepositoryHandle::open(temp_dir.path()).unwrap();
        let async_repo_handle = Arc::new(AsyncRepositoryHandle::new(repo_handle));
        let engine = RepositoryEventEngine::new(async_repo_handle, QueryParams::default(), ScanMode::FILES);

        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "Line 1\nLine 2\nLine 3\n").unwrap();

        let line_count = engine.count_lines(&test_file).await.unwrap();
        assert_eq!(line_count, 3);
    }
}
