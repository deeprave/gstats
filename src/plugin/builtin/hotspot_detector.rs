//! Code Hotspot Detection
//! 
//! Combines complexity metrics with change frequency to identify problematic code areas.

use super::change_frequency::{FileChangeStats, TimeWindow};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Configuration for hotspot detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotConfig {
    /// Minimum complexity score to consider a file for hotspot analysis
    pub min_complexity: f64,
    /// Minimum change frequency to consider a file for hotspot analysis  
    pub min_frequency: f64,
    /// Weight for complexity in hotspot scoring (0.0 to 1.0)
    pub complexity_weight: f64,
    /// Weight for change frequency in hotspot scoring (0.0 to 1.0)
    pub frequency_weight: f64,
    /// Weight for recency in hotspot scoring (0.0 to 1.0)
    pub recency_weight: f64,
    /// Minimum hotspot score to classify as a hotspot
    pub hotspot_threshold: f64,
}

impl Default for HotspotConfig {
    fn default() -> Self {
        Self {
            min_complexity: 5.0,
            min_frequency: 0.1, // 0.1 changes per day
            complexity_weight: 0.4,
            frequency_weight: 0.4,
            recency_weight: 0.2,
            hotspot_threshold: 50.0,
        }
    }
}

/// File complexity metrics (simplified version for hotspot detection)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileComplexityMetrics {
    pub file_path: String,
    pub lines_of_code: usize,
    pub cyclomatic_complexity: f64,
    pub comment_ratio: f64,
    pub file_size_bytes: usize,
}

impl FileComplexityMetrics {
    pub fn new(file_path: String) -> Self {
        Self {
            file_path,
            lines_of_code: 0,
            cyclomatic_complexity: 1.0,
            comment_ratio: 0.0,
            file_size_bytes: 0,
        }
    }
    
    /// Calculate complexity score per line of code
    pub fn complexity_per_loc(&self) -> f64 {
        if self.lines_of_code == 0 {
            0.0
        } else {
            self.cyclomatic_complexity / self.lines_of_code as f64
        }
    }
    
    /// Calculate overall complexity score
    pub fn complexity_score(&self) -> f64 {
        // Combine multiple complexity factors
        let loc_factor = (self.lines_of_code as f64 / 100.0).min(10.0); // Cap at 1000 LOC = score 10
        let complexity_factor = self.cyclomatic_complexity;
        let comment_penalty = 1.0 - (self.comment_ratio * 0.5); // Good comments reduce complexity score
        
        (loc_factor + complexity_factor) * comment_penalty
    }
}

/// Code hotspot information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeHotspot {
    pub file_path: String,
    pub hotspot_score: f64,
    pub complexity_score: f64,
    pub frequency_score: f64,
    pub recency_weight: f64,
    pub change_stats: FileChangeStats,
    pub complexity_metrics: FileComplexityMetrics,
    pub priority: HotspotPriority,
    pub recommendations: Vec<String>,
}

/// Priority level for hotspots
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum HotspotPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl HotspotPriority {
    fn from_score(score: f64) -> Self {
        if score >= 100.0 {
            HotspotPriority::Critical
        } else if score >= 75.0 {
            HotspotPriority::High
        } else if score >= 50.0 {
            HotspotPriority::Medium
        } else {
            HotspotPriority::Low
        }
    }
}

impl CodeHotspot {
    /// Generate recommendations for addressing this hotspot
    pub fn generate_recommendations(&mut self) {
        let mut recommendations = Vec::new();
        
        // High complexity recommendations
        if self.complexity_score > 20.0 {
            recommendations.push("Consider breaking down large functions or classes".to_string());
        }
        if self.complexity_metrics.cyclomatic_complexity > 15.0 {
            recommendations.push("Reduce cyclomatic complexity by simplifying conditional logic".to_string());
        }
        if self.complexity_metrics.comment_ratio < 0.1 {
            recommendations.push("Add more documentation and comments".to_string());
        }
        
        // High change frequency recommendations
        if self.frequency_score > 1.0 { // More than 1 change per day
            recommendations.push("Investigate why this file changes so frequently".to_string());
            recommendations.push("Consider improving test coverage to prevent regressions".to_string());
        }
        
        // Combined complexity + frequency recommendations
        if self.complexity_score > 15.0 && self.frequency_score > 0.5 {
            recommendations.push("High priority for refactoring due to complexity and change frequency".to_string());
            recommendations.push("Consider creating a dedicated refactoring plan".to_string());
        }
        
        // Author-based recommendations
        if self.change_stats.author_count == 1 {
            recommendations.push("Consider code review from other team members".to_string());
        } else if self.change_stats.author_count > 5 {
            recommendations.push("Multiple authors suggest this is a central file - extra care needed for changes".to_string());
        }
        
        // Size-based recommendations
        if self.complexity_metrics.lines_of_code > 500 {
            recommendations.push("Consider splitting this large file into smaller modules".to_string());
        }
        
        self.recommendations = recommendations;
    }
}

/// Hotspot detector that combines complexity and change frequency analysis
pub struct HotspotDetector {
    config: HotspotConfig,
    time_window: TimeWindow,
}

impl HotspotDetector {
    /// Create a new hotspot detector
    pub fn new(config: HotspotConfig, time_window: TimeWindow) -> Self {
        Self { config, time_window }
    }
    
    /// Create with default configuration
    pub fn with_defaults(time_window: TimeWindow) -> Self {
        Self::new(HotspotConfig::default(), time_window)
    }
    
    /// Detect hotspots by combining complexity and change frequency data
    pub fn detect_hotspots(
        &self,
        complexity_metrics: &HashMap<String, FileComplexityMetrics>,
        change_stats: &HashMap<String, FileChangeStats>,
    ) -> Vec<CodeHotspot> {
        let mut hotspots = Vec::new();
        
        // Find files that exist in both datasets
        for (file_path, complexity) in complexity_metrics {
            if let Some(change_data) = change_stats.get(file_path) {
                let hotspot = self.analyze_file(file_path, complexity, change_data);
                
                // Only include files that meet minimum thresholds
                if hotspot.complexity_score >= self.config.min_complexity 
                   && hotspot.frequency_score >= self.config.min_frequency 
                   && hotspot.hotspot_score >= self.config.hotspot_threshold {
                    hotspots.push(hotspot);
                }
            }
        }
        
        // Sort by hotspot score (highest first)
        hotspots.sort_by(|a, b| b.hotspot_score.partial_cmp(&a.hotspot_score).unwrap_or(std::cmp::Ordering::Equal));
        
        hotspots
    }
    
    /// Analyze a single file to determine if it's a hotspot
    fn analyze_file(
        &self,
        file_path: &str,
        complexity: &FileComplexityMetrics,
        change_data: &FileChangeStats,
    ) -> CodeHotspot {
        let complexity_score = complexity.complexity_score();
        let frequency_score = change_data.frequency_score(self.time_window);
        let recency_weight = change_data.recency_weight();
        
        // Calculate weighted hotspot score
        let hotspot_score = (complexity_score * self.config.complexity_weight)
            + (frequency_score * 10.0 * self.config.frequency_weight) // Scale frequency to similar range as complexity
            + (recency_weight * 20.0 * self.config.recency_weight); // Scale recency weight
        
        let priority = HotspotPriority::from_score(hotspot_score);
        
        let mut hotspot = CodeHotspot {
            file_path: file_path.to_string(),
            hotspot_score,
            complexity_score,
            frequency_score,
            recency_weight,
            change_stats: change_data.clone(),
            complexity_metrics: complexity.clone(),
            priority,
            recommendations: Vec::new(),
        };
        
        hotspot.generate_recommendations();
        hotspot
    }
    
    /// Get top N hotspots
    pub fn get_top_hotspots(
        &self,
        complexity_metrics: &HashMap<String, FileComplexityMetrics>,
        change_stats: &HashMap<String, FileChangeStats>,
        n: usize,
    ) -> Vec<CodeHotspot> {
        let mut hotspots = self.detect_hotspots(complexity_metrics, change_stats);
        hotspots.truncate(n);
        hotspots
    }
    
    /// Get hotspots by priority level
    pub fn get_hotspots_by_priority(
        &self,
        complexity_metrics: &HashMap<String, FileComplexityMetrics>,
        change_stats: &HashMap<String, FileChangeStats>,
        min_priority: HotspotPriority,
    ) -> Vec<CodeHotspot> {
        let hotspots = self.detect_hotspots(complexity_metrics, change_stats);
        hotspots.into_iter()
            .filter(|h| h.priority >= min_priority)
            .collect()
    }
    
    /// Generate a summary report of hotspot analysis
    pub fn generate_summary(
        &self,
        complexity_metrics: &HashMap<String, FileComplexityMetrics>,
        change_stats: &HashMap<String, FileChangeStats>,
    ) -> HotspotSummary {
        let hotspots = self.detect_hotspots(complexity_metrics, change_stats);
        
        let mut priority_counts = HashMap::new();
        let mut total_score = 0.0;
        
        for hotspot in &hotspots {
            *priority_counts.entry(hotspot.priority).or_insert(0) += 1;
            total_score += hotspot.hotspot_score;
        }
        
        let average_score = if hotspots.is_empty() { 
            0.0 
        } else { 
            total_score / hotspots.len() as f64 
        };
        
        HotspotSummary {
            total_hotspots: hotspots.len(),
            critical_hotspots: priority_counts.get(&HotspotPriority::Critical).copied().unwrap_or(0),
            high_hotspots: priority_counts.get(&HotspotPriority::High).copied().unwrap_or(0),
            medium_hotspots: priority_counts.get(&HotspotPriority::Medium).copied().unwrap_or(0),
            low_hotspots: priority_counts.get(&HotspotPriority::Low).copied().unwrap_or(0),
            average_hotspot_score: average_score,
            max_hotspot_score: hotspots.first().map(|h| h.hotspot_score).unwrap_or(0.0),
            files_analyzed: complexity_metrics.len().max(change_stats.len()),
            time_window: self.time_window,
        }
    }
}

/// Summary of hotspot detection analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotSummary {
    pub total_hotspots: usize,
    pub critical_hotspots: usize,
    pub high_hotspots: usize,
    pub medium_hotspots: usize,
    pub low_hotspots: usize,
    pub average_hotspot_score: f64,
    pub max_hotspot_score: f64,
    pub files_analyzed: usize,
    pub time_window: TimeWindow,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_complexity_metrics() -> HashMap<String, FileComplexityMetrics> {
        let mut metrics = HashMap::new();
        
        let mut complex_file = FileComplexityMetrics::new("complex.rs".to_string());
        complex_file.lines_of_code = 500;
        complex_file.cyclomatic_complexity = 25.0;
        complex_file.comment_ratio = 0.05;
        complex_file.file_size_bytes = 15000;
        metrics.insert("complex.rs".to_string(), complex_file);
        
        let mut simple_file = FileComplexityMetrics::new("simple.rs".to_string());
        simple_file.lines_of_code = 50;
        simple_file.cyclomatic_complexity = 3.0;
        simple_file.comment_ratio = 0.2;
        simple_file.file_size_bytes = 1500;
        metrics.insert("simple.rs".to_string(), simple_file);
        
        metrics
    }
    
    fn create_test_change_stats() -> HashMap<String, FileChangeStats> {
        let mut stats = HashMap::new();
        
        let mut frequent_changes = super::super::change_frequency::FileChangeStats::new("complex.rs".to_string());
        frequent_changes.add_change(1000, "alice".to_string(), "abc123".to_string());
        frequent_changes.add_change(2000, "bob".to_string(), "def456".to_string());
        frequent_changes.add_change(3000, "charlie".to_string(), "ghi789".to_string());
        stats.insert("complex.rs".to_string(), frequent_changes);
        
        let mut rare_changes = super::super::change_frequency::FileChangeStats::new("simple.rs".to_string());
        rare_changes.add_change(1000, "alice".to_string(), "abc123".to_string());
        stats.insert("simple.rs".to_string(), rare_changes);
        
        stats
    }
    
    #[test]
    fn test_hotspot_config_default() {
        let config = HotspotConfig::default();
        assert_eq!(config.min_complexity, 5.0);
        assert_eq!(config.min_frequency, 0.1);
        assert_eq!(config.complexity_weight + config.frequency_weight + config.recency_weight, 1.0);
    }
    
    #[test]
    fn test_file_complexity_metrics() {
        let mut metrics = FileComplexityMetrics::new("test.rs".to_string());
        metrics.lines_of_code = 100;
        metrics.cyclomatic_complexity = 10.0;
        metrics.comment_ratio = 0.15;
        
        assert_eq!(metrics.complexity_per_loc(), 0.1);
        assert!(metrics.complexity_score() > 0.0);
    }
    
    #[test]
    fn test_hotspot_priority_from_score() {
        assert_eq!(HotspotPriority::from_score(25.0), HotspotPriority::Low);
        assert_eq!(HotspotPriority::from_score(60.0), HotspotPriority::Medium);
        assert_eq!(HotspotPriority::from_score(80.0), HotspotPriority::High);
        assert_eq!(HotspotPriority::from_score(150.0), HotspotPriority::Critical);
    }
    
    #[test]
    fn test_hotspot_detector_creation() {
        let detector = HotspotDetector::with_defaults(TimeWindow::Month);
        assert_eq!(detector.time_window, TimeWindow::Month);
        assert_eq!(detector.config.min_complexity, 5.0);
    }
    
    #[test]
    fn test_hotspot_detection() {
        // Use lower thresholds for testing
        let config = HotspotConfig {
            min_complexity: 5.0,
            min_frequency: 0.01, // Very low frequency threshold for testing
            complexity_weight: 0.4,
            frequency_weight: 0.4,
            recency_weight: 0.2,
            hotspot_threshold: 10.0, // Lower threshold for testing
        };
        let detector = HotspotDetector::new(config, TimeWindow::All);
        let complexity_metrics = create_test_complexity_metrics();
        let change_stats = create_test_change_stats();
        
        let hotspots = detector.detect_hotspots(&complexity_metrics, &change_stats);
        
        // Should detect at least the complex file as a hotspot
        assert!(!hotspots.is_empty());
        
        // Hotspots should be sorted by score
        for i in 1..hotspots.len() {
            assert!(hotspots[i-1].hotspot_score >= hotspots[i].hotspot_score);
        }
    }
    
    #[test]
    fn test_top_hotspots() {
        let detector = HotspotDetector::with_defaults(TimeWindow::All);
        let complexity_metrics = create_test_complexity_metrics();
        let change_stats = create_test_change_stats();
        
        let top_hotspots = detector.get_top_hotspots(&complexity_metrics, &change_stats, 1);
        assert!(top_hotspots.len() <= 1);
    }
    
    #[test]
    fn test_hotspot_summary() {
        let detector = HotspotDetector::with_defaults(TimeWindow::All);
        let complexity_metrics = create_test_complexity_metrics();
        let change_stats = create_test_change_stats();
        
        let summary = detector.generate_summary(&complexity_metrics, &change_stats);
        
        assert!(summary.files_analyzed > 0);
        assert!(summary.total_hotspots >= 0);
        assert_eq!(summary.time_window, TimeWindow::All);
    }
    
    #[test]
    fn test_hotspot_recommendations() {
        let detector = HotspotDetector::with_defaults(TimeWindow::All);
        let complexity_metrics = create_test_complexity_metrics();
        let change_stats = create_test_change_stats();
        
        let hotspots = detector.detect_hotspots(&complexity_metrics, &change_stats);
        
        for hotspot in &hotspots {
            // All hotspots should have some recommendations
            assert!(!hotspot.recommendations.is_empty());
        }
    }
}