use crate::scanner::async_engine::events::{CommitInfo, FileInfo};
use crate::scanner::modes::ScanMode;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use log::{debug, info, warn};

/// Shared state container for cross-processor communication
#[derive(Debug, Clone)]
pub struct SharedProcessorState {
    inner: Arc<RwLock<SharedStateInner>>,
}

/// Internal shared state data
#[derive(Debug, Default)]
struct SharedStateInner {
    /// Repository-level metadata
    repository_metadata: RepositoryMetadata,
    
    /// Commit cache for efficient lookups
    commit_cache: HashMap<String, Arc<CommitInfo>>,
    
    /// File cache for efficient lookups
    file_cache: HashMap<String, Arc<FileInfo>>,
    
    /// Cross-processor data exchange
    processor_data: HashMap<String, ProcessorSharedData>,
    
    /// Performance metrics
    cache_stats: CacheStats,
}

/// Repository-level metadata shared across processors
#[derive(Debug, Clone)]
pub struct RepositoryMetadata {
    pub total_commits: Option<usize>,
    pub total_files: Option<usize>,
    pub scan_start_time: Option<SystemTime>,
    pub active_modes: ScanMode,
    pub repository_path: Option<String>,
}

impl Default for RepositoryMetadata {
    fn default() -> Self {
        Self {
            total_commits: None,
            total_files: None,
            scan_start_time: None,
            active_modes: ScanMode::empty(),
            repository_path: None,
        }
    }
}

/// Data that processors can share with each other
#[derive(Debug, Clone)]
pub enum ProcessorSharedData {
    /// File change frequency data
    FileChangeFrequency {
        file_path: String,
        change_count: usize,
        last_change: SystemTime,
        authors: Vec<String>,
    },
    /// File complexity metrics
    FileComplexity {
        file_path: String,
        complexity_score: f64,
        lines_of_code: usize,
        cyclomatic_complexity: usize,
    },
    /// Commit impact metrics
    CommitImpact {
        commit_hash: String,
        files_changed: usize,
        lines_added: usize,
        lines_removed: usize,
        impact_score: f64,
    },
    /// Custom processor data
    Custom {
        processor_name: String,
        data_type: String,
        data: serde_json::Value,
    },
}

/// Cache performance statistics
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub commit_cache_hits: usize,
    pub commit_cache_misses: usize,
    pub file_cache_hits: usize,
    pub file_cache_misses: usize,
    pub total_cache_size: usize,
}

impl SharedProcessorState {
    /// Create a new shared state container
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(SharedStateInner::default())),
        }
    }

    /// Initialize shared state with repository metadata
    pub fn initialize(&self, metadata: RepositoryMetadata) -> Result<(), String> {
        match self.inner.write() {
            Ok(mut state) => {
                state.repository_metadata = metadata;
                info!("Initialized shared state with repository metadata");
                Ok(())
            }
            Err(e) => {
                warn!("Failed to initialize shared state: {}", e);
                Err(format!("Failed to acquire write lock: {}", e))
            }
        }
    }

    /// Get repository metadata
    pub fn get_repository_metadata(&self) -> Result<RepositoryMetadata, String> {
        match self.inner.read() {
            Ok(state) => Ok(state.repository_metadata.clone()),
            Err(e) => Err(format!("Failed to acquire read lock: {}", e)),
        }
    }

    /// Cache a commit for efficient lookups
    pub fn cache_commit(&self, commit: CommitInfo) -> Result<Arc<CommitInfo>, String> {
        match self.inner.write() {
            Ok(mut state) => {
                let commit_hash = commit.hash.clone();
                let commit_arc = Arc::new(commit);
                state.commit_cache.insert(commit_hash.clone(), commit_arc.clone());
                state.cache_stats.total_cache_size += 1;
                debug!("Cached commit: {}", commit_hash);
                Ok(commit_arc)
            }
            Err(e) => Err(format!("Failed to acquire write lock: {}", e)),
        }
    }

    /// Get a cached commit
    pub fn get_cached_commit(&self, commit_hash: &str) -> Result<Option<Arc<CommitInfo>>, String> {
        match self.inner.write() {
            Ok(mut state) => {
                if let Some(commit) = state.commit_cache.get(commit_hash).cloned() {
                    state.cache_stats.commit_cache_hits += 1;
                    debug!("Cache hit for commit: {}", commit_hash);
                    Ok(Some(commit))
                } else {
                    state.cache_stats.commit_cache_misses += 1;
                    debug!("Cache miss for commit: {}", commit_hash);
                    Ok(None)
                }
            }
            Err(e) => Err(format!("Failed to acquire write lock: {}", e)),
        }
    }

    /// Cache a file for efficient lookups
    pub fn cache_file(&self, file: FileInfo) -> Result<Arc<FileInfo>, String> {
        match self.inner.write() {
            Ok(mut state) => {
                let file_path = file.relative_path.clone();
                let file_arc = Arc::new(file);
                state.file_cache.insert(file_path.clone(), file_arc.clone());
                state.cache_stats.total_cache_size += 1;
                debug!("Cached file: {}", file_path);
                Ok(file_arc)
            }
            Err(e) => Err(format!("Failed to acquire write lock: {}", e)),
        }
    }

    /// Get a cached file
    pub fn get_cached_file(&self, file_path: &str) -> Result<Option<Arc<FileInfo>>, String> {
        match self.inner.write() {
            Ok(mut state) => {
                if let Some(file) = state.file_cache.get(file_path).cloned() {
                    state.cache_stats.file_cache_hits += 1;
                    debug!("Cache hit for file: {}", file_path);
                    Ok(Some(file))
                } else {
                    state.cache_stats.file_cache_misses += 1;
                    debug!("Cache miss for file: {}", file_path);
                    Ok(None)
                }
            }
            Err(e) => Err(format!("Failed to acquire write lock: {}", e)),
        }
    }

    /// Share data between processors
    pub fn share_processor_data(&self, key: String, data: ProcessorSharedData) -> Result<(), String> {
        match self.inner.write() {
            Ok(mut state) => {
                state.processor_data.insert(key.clone(), data);
                debug!("Shared processor data with key: {}", key);
                Ok(())
            }
            Err(e) => Err(format!("Failed to acquire write lock: {}", e)),
        }
    }

    /// Get shared processor data
    pub fn get_processor_data(&self, key: &str) -> Result<Option<ProcessorSharedData>, String> {
        match self.inner.read() {
            Ok(state) => {
                let data = state.processor_data.get(key).cloned();
                if data.is_some() {
                    debug!("Retrieved shared processor data for key: {}", key);
                } else {
                    debug!("No shared processor data found for key: {}", key);
                }
                Ok(data)
            }
            Err(e) => Err(format!("Failed to acquire read lock: {}", e)),
        }
    }

    /// Get all shared processor data keys
    pub fn get_processor_data_keys(&self) -> Result<Vec<String>, String> {
        match self.inner.read() {
            Ok(state) => Ok(state.processor_data.keys().cloned().collect()),
            Err(e) => Err(format!("Failed to acquire read lock: {}", e)),
        }
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> Result<CacheStats, String> {
        match self.inner.read() {
            Ok(state) => Ok(state.cache_stats.clone()),
            Err(e) => Err(format!("Failed to acquire read lock: {}", e)),
        }
    }

    /// Clear all cached data (useful for memory management)
    pub fn clear_cache(&self) -> Result<(), String> {
        match self.inner.write() {
            Ok(mut state) => {
                let commit_count = state.commit_cache.len();
                let file_count = state.file_cache.len();
                let data_count = state.processor_data.len();
                
                state.commit_cache.clear();
                state.file_cache.clear();
                state.processor_data.clear();
                state.cache_stats = CacheStats::default();
                
                info!(
                    "Cleared shared state cache: {} commits, {} files, {} processor data entries",
                    commit_count, file_count, data_count
                );
                Ok(())
            }
            Err(e) => Err(format!("Failed to acquire write lock: {}", e)),
        }
    }

    /// Get memory usage estimate in bytes
    pub fn estimate_memory_usage(&self) -> Result<usize, String> {
        match self.inner.read() {
            Ok(state) => {
                let mut total_size = 0;
                
                // Estimate commit cache size
                total_size += state.commit_cache.len() * std::mem::size_of::<CommitInfo>();
                
                // Estimate file cache size
                total_size += state.file_cache.len() * std::mem::size_of::<FileInfo>();
                
                // Estimate processor data size (rough approximation)
                total_size += state.processor_data.len() * 1024; // Assume 1KB per entry
                
                debug!("Estimated shared state memory usage: {} bytes", total_size);
                Ok(total_size)
            }
            Err(e) => Err(format!("Failed to acquire read lock: {}", e)),
        }
    }

    /// Check if memory usage is concerning (over threshold)
    pub fn is_memory_usage_concerning(&self, threshold_bytes: usize) -> Result<bool, String> {
        let usage = self.estimate_memory_usage()?;
        let concerning = usage > threshold_bytes;
        
        if concerning {
            warn!(
                "Shared state memory usage ({} bytes) exceeds threshold ({} bytes)",
                usage, threshold_bytes
            );
        }
        
        Ok(concerning)
    }
}

impl Default for SharedProcessorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper trait for processors to access shared state
pub trait SharedStateAccess {
    /// Get access to shared state
    fn shared_state(&self) -> &SharedProcessorState;

    /// Cache a commit in shared state
    fn cache_commit(&self, commit: CommitInfo) -> Result<Arc<CommitInfo>, String> {
        self.shared_state().cache_commit(commit)
    }

    /// Get a cached commit from shared state
    fn get_cached_commit(&self, commit_hash: &str) -> Result<Option<Arc<CommitInfo>>, String> {
        self.shared_state().get_cached_commit(commit_hash)
    }

    /// Cache a file in shared state
    fn cache_file(&self, file: FileInfo) -> Result<Arc<FileInfo>, String> {
        self.shared_state().cache_file(file)
    }

    /// Get a cached file from shared state
    fn get_cached_file(&self, file_path: &str) -> Result<Option<Arc<FileInfo>>, String> {
        self.shared_state().get_cached_file(file_path)
    }

    /// Share data with other processors
    fn share_data(&self, key: String, data: ProcessorSharedData) -> Result<(), String> {
        self.shared_state().share_processor_data(key, data)
    }

    /// Get shared data from other processors
    fn get_shared_data(&self, key: &str) -> Result<Option<ProcessorSharedData>, String> {
        self.shared_state().get_processor_data(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn test_shared_state_creation() {
        let state = SharedProcessorState::new();
        let metadata = state.get_repository_metadata().unwrap();
        assert_eq!(metadata.total_commits, None);
        assert_eq!(metadata.total_files, None);
    }

    #[test]
    fn test_repository_metadata_initialization() {
        let state = SharedProcessorState::new();
        let metadata = RepositoryMetadata {
            total_commits: Some(100),
            total_files: Some(50),
            scan_start_time: Some(SystemTime::now()),
            active_modes: ScanMode::HISTORY | ScanMode::FILES,
            repository_path: Some("/test/repo".to_string()),
        };

        state.initialize(metadata.clone()).unwrap();
        let retrieved = state.get_repository_metadata().unwrap();
        
        assert_eq!(retrieved.total_commits, Some(100));
        assert_eq!(retrieved.total_files, Some(50));
        assert_eq!(retrieved.active_modes, ScanMode::HISTORY | ScanMode::FILES);
    }

    #[test]
    fn test_commit_caching() {
        let state = SharedProcessorState::new();
        let commit = CommitInfo {
            hash: "abc123".to_string(),
            short_hash: "abc123".to_string(),
            author_name: "Test Author".to_string(),
            author_email: "test@example.com".to_string(),
            committer_name: "Test Author".to_string(),
            committer_email: "test@example.com".to_string(),
            timestamp: SystemTime::now(),
            message: "Test commit".to_string(),
            parent_hashes: vec![],
            changed_files: vec![],
            insertions: 10,
            deletions: 5,
        };

        // Cache the commit
        let cached = state.cache_commit(commit.clone()).unwrap();
        assert_eq!(cached.hash, "abc123");

        // Retrieve the cached commit
        let retrieved = state.get_cached_commit("abc123").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().hash, "abc123");

        // Test cache miss
        let missing = state.get_cached_commit("nonexistent").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_processor_data_sharing() {
        let state = SharedProcessorState::new();
        let data = ProcessorSharedData::FileChangeFrequency {
            file_path: "test.rs".to_string(),
            change_count: 5,
            last_change: SystemTime::now(),
            authors: vec!["author1".to_string(), "author2".to_string()],
        };

        // Share the data
        state.share_processor_data("test_key".to_string(), data).unwrap();

        // Retrieve the data
        let retrieved = state.get_processor_data("test_key").unwrap();
        assert!(retrieved.is_some());

        if let Some(ProcessorSharedData::FileChangeFrequency { file_path, change_count, .. }) = retrieved {
            assert_eq!(file_path, "test.rs");
            assert_eq!(change_count, 5);
        } else {
            panic!("Retrieved data has wrong type");
        }
    }

    #[test]
    fn test_cache_stats() {
        let state = SharedProcessorState::new();
        
        // Initial stats should be zero
        let stats = state.get_cache_stats().unwrap();
        assert_eq!(stats.commit_cache_hits, 0);
        assert_eq!(stats.commit_cache_misses, 0);

        // Create a test commit
        let commit = CommitInfo {
            hash: "test123".to_string(),
            short_hash: "test123".to_string(),
            author_name: "Test".to_string(),
            author_email: "test@example.com".to_string(),
            committer_name: "Test".to_string(),
            committer_email: "test@example.com".to_string(),
            timestamp: SystemTime::now(),
            message: "Test".to_string(),
            parent_hashes: vec![],
            changed_files: vec![],
            insertions: 0,
            deletions: 0,
        };

        // Cache and retrieve to generate stats
        state.cache_commit(commit).unwrap();
        state.get_cached_commit("test123").unwrap(); // Hit
        state.get_cached_commit("missing").unwrap(); // Miss

        let updated_stats = state.get_cache_stats().unwrap();
        assert_eq!(updated_stats.commit_cache_hits, 1);
        assert_eq!(updated_stats.commit_cache_misses, 1);
    }

    #[test]
    fn test_memory_usage_estimation() {
        let state = SharedProcessorState::new();
        
        // Initial memory usage should be minimal
        let initial_usage = state.estimate_memory_usage().unwrap();
        // Memory usage is always non-negative (usize type)

        // Add some data and check usage increases
        let commit = CommitInfo {
            hash: "memory_test".to_string(),
            short_hash: "memory_test".to_string(),
            author_name: "Test".to_string(),
            author_email: "test@example.com".to_string(),
            committer_name: "Test".to_string(),
            committer_email: "test@example.com".to_string(),
            timestamp: SystemTime::now(),
            message: "Memory test".to_string(),
            parent_hashes: vec![],
            changed_files: vec![],
            insertions: 0,
            deletions: 0,
        };

        state.cache_commit(commit).unwrap();
        let updated_usage = state.estimate_memory_usage().unwrap();
        assert!(updated_usage > initial_usage);
    }

    #[test]
    fn test_cache_clearing() {
        let state = SharedProcessorState::new();
        
        // Add some data
        let commit = CommitInfo {
            hash: "clear_test".to_string(),
            short_hash: "clear_test".to_string(),
            author_name: "Test".to_string(),
            author_email: "test@example.com".to_string(),
            committer_name: "Test".to_string(),
            committer_email: "test@example.com".to_string(),
            timestamp: SystemTime::now(),
            message: "Clear test".to_string(),
            parent_hashes: vec![],
            changed_files: vec![],
            insertions: 0,
            deletions: 0,
        };

        state.cache_commit(commit).unwrap();
        
        // Verify data exists
        let cached = state.get_cached_commit("clear_test").unwrap();
        assert!(cached.is_some());

        // Clear cache
        state.clear_cache().unwrap();

        // Verify data is gone
        let cleared = state.get_cached_commit("clear_test").unwrap();
        assert!(cleared.is_none());

        // Verify stats are reset
        let stats = state.get_cache_stats().unwrap();
        assert_eq!(stats.total_cache_size, 0);
    }
}
