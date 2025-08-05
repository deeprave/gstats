//! Change Frequency Analysis
//! 
//! Analyzes git commit history to track file change frequency and identify code hotspots.

use crate::git::RepositoryHandle;
use anyhow::{Result, Context};
use git2::{Commit, DiffOptions};
use std::collections::HashMap;
use std::time::{SystemTime, Duration, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use log::{debug, info};

/// Time window for change frequency analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeWindow {
    /// Last 7 days
    Week,
    /// Last 30 days
    Month,
    /// Last 90 days
    Quarter,
    /// Last 365 days
    Year,
    /// All time
    All,
}

impl TimeWindow {
    /// Get the duration for this time window
    pub fn duration(&self) -> Option<Duration> {
        match self {
            TimeWindow::Week => Some(Duration::from_secs(7 * 24 * 60 * 60)),
            TimeWindow::Month => Some(Duration::from_secs(30 * 24 * 60 * 60)),
            TimeWindow::Quarter => Some(Duration::from_secs(90 * 24 * 60 * 60)),
            TimeWindow::Year => Some(Duration::from_secs(365 * 24 * 60 * 60)),
            TimeWindow::All => None,
        }
    }
    
    /// Get the cutoff timestamp for this time window
    pub fn cutoff_timestamp(&self) -> Option<i64> {
        self.duration().map(|duration| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            now - duration.as_secs() as i64
        })
    }
}

/// File change frequency statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeStats {
    /// File path relative to repository root
    pub file_path: String,
    /// Total number of commits that modified this file
    pub change_count: usize,
    /// Number of unique authors who modified this file
    pub author_count: usize,
    /// Timestamp of most recent change
    pub last_changed: i64,
    /// Timestamp of first change in the analysis window
    pub first_changed: i64,
    /// Average time between changes in days
    pub average_change_interval: f64,
    /// Authors who have modified this file
    pub authors: Vec<String>,
    /// Recent commit hashes that modified this file
    pub recent_commits: Vec<String>,
}

impl FileChangeStats {
    /// Create new file change statistics
    pub fn new(file_path: String) -> Self {
        Self {
            file_path,
            change_count: 0,
            author_count: 0,
            last_changed: 0,
            first_changed: i64::MAX,
            average_change_interval: 0.0,
            authors: Vec::new(),
            recent_commits: Vec::new(),
        }
    }
    
    /// Add a change record to the statistics
    pub fn add_change(&mut self, timestamp: i64, author: String, commit_hash: String) {
        self.change_count += 1;
        self.last_changed = self.last_changed.max(timestamp);
        self.first_changed = self.first_changed.min(timestamp);
        
        // Add author if not already present
        if !self.authors.contains(&author) {
            self.authors.push(author);
            self.author_count = self.authors.len();
        }
        
        // Add commit hash, keeping only recent ones (last 10)
        self.recent_commits.push(commit_hash);
        if self.recent_commits.len() > 10 {
            self.recent_commits.remove(0);
        }
        
        // Calculate average change interval
        if self.change_count > 1 {
            let time_span = self.last_changed - self.first_changed;
            self.average_change_interval = time_span as f64 / (self.change_count - 1) as f64 / (24.0 * 60.0 * 60.0); // Convert to days
        }
    }
    
    /// Calculate change frequency score (changes per day)
    pub fn frequency_score(&self, window: TimeWindow) -> f64 {
        if self.change_count == 0 {
            return 0.0;
        }
        
        let window_days = match window.duration() {
            Some(duration) => duration.as_secs() as f64 / (24.0 * 60.0 * 60.0),
            None => {
                // For "All" time window, use actual time span
                if self.change_count > 1 {
                    (self.last_changed - self.first_changed) as f64 / (24.0 * 60.0 * 60.0)
                } else {
                    1.0 // Single change, assume 1 day
                }
            }
        };
        
        self.change_count as f64 / window_days.max(1.0)
    }
    
    /// Calculate recency weight (recent changes are weighted higher)
    pub fn recency_weight(&self) -> f64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        
        let days_since_last_change = (now - self.last_changed) as f64 / (24.0 * 60.0 * 60.0);
        
        // Exponential decay: more recent changes get higher weight
        (-days_since_last_change / 30.0_f64).exp() // 30-day half-life
    }
}

/// Change frequency analyzer for git repositories
pub struct ChangeFrequencyAnalyzer {
    repository: RepositoryHandle,
    time_window: TimeWindow,
    file_stats: HashMap<String, FileChangeStats>,
    total_commits_analyzed: usize,
}

impl ChangeFrequencyAnalyzer {
    /// Create a new change frequency analyzer
    pub fn new(repository: RepositoryHandle, time_window: TimeWindow) -> Self {
        Self {
            repository,
            time_window,
            file_stats: HashMap::new(),
            total_commits_analyzed: 0,
        }
    }
    
    /// Analyze change frequency across the repository
    pub fn analyze(&mut self) -> Result<()> {
        info!("Starting change frequency analysis with time window: {:?}", self.time_window);
        
        let cutoff_timestamp = self.time_window.cutoff_timestamp();
        
        // Clone repository handle to avoid borrow conflicts
        let repository = self.repository.clone();
        let repo = repository.repository();
        
        // Walk through all commits
        let mut revwalk = repo.revwalk()
            .context("Failed to create repository walker")?;
        
        revwalk.push_head()
            .context("Failed to push HEAD to revwalk")?;
        
        for oid_result in revwalk {
            let oid = oid_result.context("Failed to get commit OID")?;
            let commit = repo.find_commit(oid)
                .context("Failed to find commit")?;
            
            let commit_time = commit.time().seconds();
            
            // Skip commits outside our time window
            if let Some(cutoff) = cutoff_timestamp {
                if commit_time < cutoff {
                    continue;
                }
            }
            
            self.analyze_commit_internal(&commit, repo)
                .with_context(|| format!("Failed to analyze commit {}", oid))?;
            
            self.total_commits_analyzed += 1;
            
            // Log progress every 1000 commits
            if self.total_commits_analyzed % 1000 == 0 {
                debug!("Analyzed {} commits", self.total_commits_analyzed);
            }
        }
        
        info!("Change frequency analysis complete. Analyzed {} commits, tracking {} files", 
              self.total_commits_analyzed, self.file_stats.len());
        
        Ok(())
    }
    
    /// Analyze a single commit for file changes (internal version with repo reference)
    fn analyze_commit_internal(&mut self, commit: &Commit, repo: &git2::Repository) -> Result<()> {
        let commit_time = commit.time().seconds();
        let author = commit.author().name().unwrap_or("Unknown").to_string();
        let commit_hash = commit.id().to_string();
        
        // Get the tree for this commit
        let tree = commit.tree()
            .context("Failed to get commit tree")?;
        
        // Compare with parent(s) to find changed files
        let parents: Vec<_> = commit.parents().collect();
        
        if parents.is_empty() {
            // Initial commit - all files are "changed"
            self.analyze_tree_files(&tree, commit_time, &author, &commit_hash)?;
        } else {
            // Compare with first parent (handles merges by only looking at first parent)
            let parent = &parents[0];
            let parent_tree = parent.tree()
                .context("Failed to get parent tree")?;
            
            let mut diff_options = DiffOptions::new();
            let diff = repo.diff_tree_to_tree(
                Some(&parent_tree),
                Some(&tree),
                Some(&mut diff_options)
            ).context("Failed to create diff")?;
            
            // Process each changed file
            diff.foreach(
                &mut |delta, _progress| {
                    if let Some(new_file) = delta.new_file().path() {
                        let file_path = new_file.to_string_lossy().to_string();
                        
                        // Add or update file statistics
                        let stats = self.file_stats.entry(file_path.clone())
                            .or_insert_with(|| FileChangeStats::new(file_path));
                        
                        stats.add_change(commit_time, author.clone(), commit_hash.clone());
                    }
                    true
                },
                None,
                None,
                None,
            ).context("Failed to process diff")?;
        }
        
        Ok(())
    }
    
    /// Analyze all files in a tree (for initial commit)
    fn analyze_tree_files(&mut self, tree: &git2::Tree, commit_time: i64, author: &str, commit_hash: &str) -> Result<()> {
        tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
            if let Some(name) = entry.name() {
                let file_path = if root.is_empty() {
                    name.to_string()
                } else {
                    format!("{}/{}", root, name)
                };
                
                // Only process files, not directories
                if entry.kind() == Some(git2::ObjectType::Blob) {
                    let stats = self.file_stats.entry(file_path.clone())
                        .or_insert_with(|| FileChangeStats::new(file_path));
                    
                    stats.add_change(commit_time, author.to_string(), commit_hash.to_string());
                }
            }
            git2::TreeWalkResult::Ok
        })?;
        
        Ok(())
    }
    
    /// Get file change statistics
    pub fn get_file_stats(&self) -> &HashMap<String, FileChangeStats> {
        &self.file_stats
    }
    
    /// Get files sorted by change frequency (most frequently changed first)
    pub fn get_files_by_frequency(&self) -> Vec<(&String, &FileChangeStats)> {
        let mut files: Vec<_> = self.file_stats.iter().collect();
        files.sort_by(|a, b| {
            let freq_a = a.1.frequency_score(self.time_window);
            let freq_b = b.1.frequency_score(self.time_window);
            freq_b.partial_cmp(&freq_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        files
    }
    
    /// Get top N most frequently changed files
    pub fn get_top_changed_files(&self, n: usize) -> Vec<(&String, &FileChangeStats)> {
        let mut files = self.get_files_by_frequency();
        files.truncate(n);
        files
    }
    
    /// Get statistics summary
    pub fn get_summary(&self) -> ChangeFrequencySummary {
        let total_files = self.file_stats.len();
        let total_changes: usize = self.file_stats.values().map(|s| s.change_count).sum();
        
        let mut frequency_scores: Vec<_> = self.file_stats.values()
            .map(|s| s.frequency_score(self.time_window))
            .collect();
        frequency_scores.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        
        let avg_frequency = if frequency_scores.is_empty() {
            0.0
        } else {
            frequency_scores.iter().sum::<f64>() / frequency_scores.len() as f64
        };
        
        let median_frequency = if frequency_scores.is_empty() {
            0.0
        } else {
            let mid = frequency_scores.len() / 2;
            if frequency_scores.len() % 2 == 0 {
                (frequency_scores[mid - 1] + frequency_scores[mid]) / 2.0
            } else {
                frequency_scores[mid]
            }
        };
        
        ChangeFrequencySummary {
            time_window: self.time_window,
            total_files,
            total_changes,
            total_commits_analyzed: self.total_commits_analyzed,
            average_frequency: avg_frequency,
            median_frequency,
            max_frequency: frequency_scores.first().copied().unwrap_or(0.0),
        }
    }
}

/// Summary statistics for change frequency analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeFrequencySummary {
    pub time_window: TimeWindow,
    pub total_files: usize,
    pub total_changes: usize,
    pub total_commits_analyzed: usize,
    pub average_frequency: f64,
    pub median_frequency: f64,
    pub max_frequency: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git;
    
    fn get_test_repository() -> RepositoryHandle {
        // Use current repository for testing
        git::resolve_repository_handle(None).unwrap()
    }
    
    #[test]
    fn test_time_window_durations() {
        assert_eq!(TimeWindow::Week.duration().unwrap().as_secs(), 7 * 24 * 60 * 60);
        assert_eq!(TimeWindow::Month.duration().unwrap().as_secs(), 30 * 24 * 60 * 60);
        assert_eq!(TimeWindow::Quarter.duration().unwrap().as_secs(), 90 * 24 * 60 * 60);
        assert_eq!(TimeWindow::Year.duration().unwrap().as_secs(), 365 * 24 * 60 * 60);
        assert!(TimeWindow::All.duration().is_none());
    }
    
    #[test]
    fn test_file_change_stats_creation() {
        let mut stats = FileChangeStats::new("test.rs".to_string());
        assert_eq!(stats.file_path, "test.rs");
        assert_eq!(stats.change_count, 0);
        assert_eq!(stats.author_count, 0);
        
        stats.add_change(1000, "alice".to_string(), "abc123".to_string());
        assert_eq!(stats.change_count, 1);
        assert_eq!(stats.author_count, 1);
        assert_eq!(stats.last_changed, 1000);
        assert_eq!(stats.first_changed, 1000);
    }
    
    #[test]
    fn test_file_change_stats_multiple_changes() {
        let mut stats = FileChangeStats::new("test.rs".to_string());
        
        stats.add_change(1000, "alice".to_string(), "abc123".to_string());
        stats.add_change(2000, "bob".to_string(), "def456".to_string());
        stats.add_change(3000, "alice".to_string(), "ghi789".to_string());
        
        assert_eq!(stats.change_count, 3);
        assert_eq!(stats.author_count, 2); // alice and bob
        assert_eq!(stats.last_changed, 3000);
        assert_eq!(stats.first_changed, 1000);
        assert!(stats.average_change_interval > 0.0);
    }
    
    #[test]
    fn test_frequency_score_calculation() {
        let mut stats = FileChangeStats::new("test.rs".to_string());
        
        // Add 7 changes over 7 days (1 change per day)
        for i in 0..7 {
            stats.add_change(i * 24 * 60 * 60, "author".to_string(), format!("commit{}", i));
        }
        
        let score = stats.frequency_score(TimeWindow::Week);
        assert!(score > 0.9 && score < 1.1); // Should be approximately 1 change per day
    }
    
    #[test]
    fn test_change_frequency_analyzer_creation() {
        let repo = get_test_repository();
        let analyzer = ChangeFrequencyAnalyzer::new(repo, TimeWindow::Month);
        
        assert_eq!(analyzer.time_window, TimeWindow::Month);
        assert_eq!(analyzer.file_stats.len(), 0);
        assert_eq!(analyzer.total_commits_analyzed, 0);
    }
    
    #[tokio::test]
    async fn test_change_frequency_analysis() {
        let repo = get_test_repository();
        let mut analyzer = ChangeFrequencyAnalyzer::new(repo, TimeWindow::All);
        
        // Run analysis
        let result = analyzer.analyze();
        assert!(result.is_ok(), "Analysis should succeed: {:?}", result);
        
        // Should have found some files and commits
        assert!(analyzer.total_commits_analyzed > 0);
        assert!(analyzer.file_stats.len() > 0);
        
        // Get summary
        let summary = analyzer.get_summary();
        assert!(summary.total_files > 0);
        assert!(summary.total_changes > 0);
        assert!(summary.total_commits_analyzed > 0);
    }
    
    #[tokio::test]
    async fn test_top_changed_files() {
        let repo = get_test_repository();
        let mut analyzer = ChangeFrequencyAnalyzer::new(repo, TimeWindow::All);
        
        analyzer.analyze().unwrap();
        
        let top_files = analyzer.get_top_changed_files(5);
        assert!(top_files.len() <= 5);
        
        // Verify files are sorted by frequency
        for i in 1..top_files.len() {
            let freq_prev = top_files[i-1].1.frequency_score(TimeWindow::All);
            let freq_curr = top_files[i].1.frequency_score(TimeWindow::All);
            assert!(freq_prev >= freq_curr, "Files should be sorted by frequency");
        }
    }
}