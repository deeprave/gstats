//! Enhanced Technical Debt Assessment
//! 
//! Combines complexity, change frequency, and duplication metrics to provide
//! a comprehensive technical debt assessment for each file in the codebase.

use super::change_frequency::{FileChangeStats, TimeWindow};
use super::hotspot_detector::FileComplexityMetrics;
use super::duplication_detector::DuplicateGroup;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Configuration for technical debt assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtConfig {
    /// Weight for complexity score in debt calculation (0.0 to 1.0)
    pub complexity_weight: f64,
    /// Weight for change frequency in debt calculation (0.0 to 1.0)
    pub frequency_weight: f64,
    /// Weight for duplication impact in debt calculation (0.0 to 1.0)
    pub duplication_weight: f64,
    /// Weight for file size factor in debt calculation (0.0 to 1.0)
    pub size_weight: f64,
    /// Weight for code age factor in debt calculation (0.0 to 1.0)
    pub age_weight: f64,
    /// Minimum debt score to classify as having debt
    pub debt_threshold: f64,
    /// Time window for change frequency analysis
    pub time_window: TimeWindow,
}

impl Default for DebtConfig {
    fn default() -> Self {
        Self {
            complexity_weight: 0.3,
            frequency_weight: 0.25,
            duplication_weight: 0.25,
            size_weight: 0.1,
            age_weight: 0.1,
            debt_threshold: 20.0,
            time_window: TimeWindow::Month,
        }
    }
}

impl DebtConfig {
    /// Validate that weights sum to approximately 1.0
    pub fn validate(&self) -> Result<(), String> {
        let total = self.complexity_weight + self.frequency_weight + self.duplication_weight + 
                   self.size_weight + self.age_weight;
        
        if (total - 1.0).abs() > 0.01 {
            return Err(format!("Debt weights must sum to 1.0, got {:.3}", total));
        }
        
        // Check individual weight ranges
        let weights = [
            ("complexity", self.complexity_weight),
            ("frequency", self.frequency_weight),
            ("duplication", self.duplication_weight),
            ("size", self.size_weight),
            ("age", self.age_weight),
        ];
        
        for (name, weight) in &weights {
            if *weight < 0.0 || *weight > 1.0 {
                return Err(format!("{} weight must be between 0.0 and 1.0, got {:.3}", name, weight));
            }
        }
        
        Ok(())
    }
}

/// Technical debt level classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DebtLevel {
    Minimal,
    Low,
    Medium,
    High,
    Critical,
}

impl DebtLevel {
    fn from_score(score: f64) -> Self {
        if score >= 80.0 {
            DebtLevel::Critical
        } else if score >= 60.0 {
            DebtLevel::High
        } else if score >= 40.0 {
            DebtLevel::Medium
        } else if score >= 20.0 {
            DebtLevel::Low
        } else {
            DebtLevel::Minimal
        }
    }
    
    pub fn as_str(&self) -> &'static str {
        match self {
            DebtLevel::Minimal => "Minimal",
            DebtLevel::Low => "Low",
            DebtLevel::Medium => "Medium",
            DebtLevel::High => "High",
            DebtLevel::Critical => "Critical",
        }
    }
}

/// File-specific technical debt assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDebtAssessment {
    pub file_path: String,
    pub debt_score: f64,
    pub debt_level: DebtLevel,
    pub complexity_score: f64,
    pub frequency_score: f64,
    pub duplication_score: f64,
    pub size_score: f64,
    pub age_score: f64,
    pub recommendations: Vec<String>,
    pub priority_actions: Vec<String>,
    pub estimated_hours: f64,
}

impl FileDebtAssessment {
    /// Generate recommendations based on debt factors
    pub fn generate_recommendations(&mut self) {
        let mut recommendations = Vec::new();
        let mut priority_actions = Vec::new();
        let mut estimated_hours = 0.0;
        
        // Complexity-based recommendations
        if self.complexity_score > 15.0 {
            recommendations.push("Reduce cyclomatic complexity by breaking down large functions".to_string());
            if self.complexity_score > 25.0 {
                priority_actions.push("URGENT: Refactor high-complexity functions immediately".to_string());
                estimated_hours += 8.0;
            } else {
                estimated_hours += 4.0;
            }
        }
        
        // Frequency-based recommendations
        if self.frequency_score > 1.0 {
            recommendations.push("Investigate frequent changes - may indicate unstable code".to_string());
            recommendations.push("Improve test coverage to prevent regression issues".to_string());
            estimated_hours += 2.0;
        }
        
        // Duplication-based recommendations
        if self.duplication_score > 10.0 {
            recommendations.push("Extract common code patterns into reusable functions".to_string());
            if self.duplication_score > 20.0 {
                priority_actions.push("HIGH: Eliminate significant code duplication".to_string());
                estimated_hours += 6.0;
            } else {
                estimated_hours += 3.0;
            }
        }
        
        // Size-based recommendations
        if self.size_score > 15.0 {
            recommendations.push("Consider splitting large file into smaller, focused modules".to_string());
            estimated_hours += 4.0;
        }
        
        // Age-based recommendations  
        if self.age_score > 15.0 {
            recommendations.push("Review and modernize old code patterns".to_string());
            recommendations.push("Update documentation and add missing comments".to_string());
            estimated_hours += 2.0;
        }
        
        // Combined factor recommendations
        if self.debt_level == DebtLevel::Critical {
            priority_actions.push("CRITICAL: Schedule immediate refactoring sprint for this file".to_string());
            estimated_hours += 4.0;
        } else if self.debt_level == DebtLevel::High {
            priority_actions.push("Schedule refactoring in next sprint".to_string());
        }
        
        self.recommendations = recommendations;
        self.priority_actions = priority_actions;
        self.estimated_hours = estimated_hours;
    }
}

/// Technical debt assessor that combines all metrics
pub struct DebtAssessor {
    config: DebtConfig,
}

impl DebtAssessor {
    /// Create a new debt assessor
    pub fn new(config: DebtConfig) -> Result<Self, String> {
        config.validate()?;
        Ok(Self { config })
    }
    
    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self { 
            config: DebtConfig::default() 
        }
    }
    
    /// Assess technical debt for all files
    pub fn assess_debt(
        &self,
        complexity_metrics: &HashMap<String, FileComplexityMetrics>,
        change_stats: &HashMap<String, FileChangeStats>,
        duplication_groups: &[DuplicateGroup],
    ) -> Vec<FileDebtAssessment> {
        // Build duplication impact map
        let duplication_map = self.build_duplication_impact_map(duplication_groups);
        
        let mut assessments = Vec::new();
        
        // Assess debt for each file that has complexity metrics
        for (file_path, complexity) in complexity_metrics {
            let assessment = self.assess_file_debt(
                file_path,
                complexity,
                change_stats.get(file_path),
                duplication_map.get(file_path).copied().unwrap_or(0.0),
            );
            
            // Only include files that meet the debt threshold
            if assessment.debt_score >= self.config.debt_threshold {
                assessments.push(assessment);
            }
        }
        
        // Sort by debt score (highest first)
        assessments.sort_by(|a, b| b.debt_score.partial_cmp(&a.debt_score).unwrap_or(std::cmp::Ordering::Equal));
        
        assessments
    }
    
    /// Build a map of file paths to duplication impact scores
    fn build_duplication_impact_map(&self, duplication_groups: &[DuplicateGroup]) -> HashMap<String, f64> {
        let mut impact_map = HashMap::new();
        
        for group in duplication_groups {
            let impact_per_file = group.impact_score / group.blocks.len() as f64;
            
            for block in &group.blocks {
                let current_impact = impact_map.get(&block.file_path).copied().unwrap_or(0.0);
                impact_map.insert(block.file_path.clone(), current_impact + impact_per_file);
            }
        }
        
        impact_map
    }
    
    /// Assess technical debt for a single file
    fn assess_file_debt(
        &self,
        file_path: &str,
        complexity: &FileComplexityMetrics,
        change_stats: Option<&FileChangeStats>,
        duplication_impact: f64,
    ) -> FileDebtAssessment {
        let complexity_score = self.calculate_complexity_score(complexity);
        let frequency_score = self.calculate_frequency_score(change_stats);
        let duplication_score = self.calculate_duplication_score(duplication_impact);
        let size_score = self.calculate_size_score(complexity);
        let age_score = self.calculate_age_score(change_stats);
        
        // Calculate weighted debt score
        let debt_score = (complexity_score * self.config.complexity_weight) +
                        (frequency_score * self.config.frequency_weight) +
                        (duplication_score * self.config.duplication_weight) +
                        (size_score * self.config.size_weight) +
                        (age_score * self.config.age_weight);
        
        let debt_level = DebtLevel::from_score(debt_score);
        
        let mut assessment = FileDebtAssessment {
            file_path: file_path.to_string(),
            debt_score,
            debt_level,
            complexity_score,
            frequency_score,
            duplication_score,
            size_score,
            age_score,
            recommendations: Vec::new(),
            priority_actions: Vec::new(),
            estimated_hours: 0.0,
        };
        
        assessment.generate_recommendations();
        assessment
    }
    
    /// Calculate normalized complexity score (0-100 scale)
    fn calculate_complexity_score(&self, complexity: &FileComplexityMetrics) -> f64 {
        let cyclomatic_factor = (complexity.cyclomatic_complexity / 10.0).min(10.0); // Cap at complexity 100 = score 100
        let comment_penalty = 1.0 - (complexity.comment_ratio * 0.3); // Good comments reduce score
        
        (cyclomatic_factor * 10.0 * comment_penalty).min(100.0)
    }
    
    /// Calculate normalized frequency score (0-100 scale)
    fn calculate_frequency_score(&self, change_stats: Option<&FileChangeStats>) -> f64 {
        match change_stats {
            Some(stats) => {
                let frequency = stats.frequency_score(self.config.time_window);
                let recency = stats.recency_weight();
                
                // Scale frequency (changes per day) to 0-100
                let freq_score = (frequency * 20.0).min(50.0); // 2.5 changes/day = 50 points
                let recency_score = recency * 50.0; // Recent changes add up to 50 points
                
                (freq_score + recency_score).min(100.0)
            }
            None => 0.0, // No change history = no frequency debt
        }
    }
    
    /// Calculate normalized duplication score (0-100 scale)
    fn calculate_duplication_score(&self, duplication_impact: f64) -> f64 {
        // Scale duplication impact to 0-100
        (duplication_impact / 10.0).min(100.0) // Impact of 1000 = score 100
    }
    
    /// Calculate normalized size score (0-100 scale)
    fn calculate_size_score(&self, complexity: &FileComplexityMetrics) -> f64 {
        let loc_factor = (complexity.lines_of_code as f64 / 20.0).min(50.0); // 1000 LOC = 50 points
        let size_factor = (complexity.file_size_bytes as f64 / 2000.0).min(50.0); // 100KB = 50 points
        
        (loc_factor + size_factor).min(100.0)
    }
    
    /// Calculate normalized age score (0-100 scale)
    fn calculate_age_score(&self, change_stats: Option<&FileChangeStats>) -> f64 {
        match change_stats {
            Some(stats) => {
                let first_change = stats.first_changed;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                
                let age_days = (now - first_change as u64) / 86400; // Convert to days
                
                // Files older than 2 years start accumulating age debt
                if age_days > 730 {
                    let age_years = age_days as f64 / 365.0;
                    ((age_years - 2.0) * 10.0).min(100.0) // 12 years old = 100 points
                } else {
                    0.0
                }
            }
            None => 0.0,
        }
    }
    
    /// Generate summary statistics for debt assessment
    pub fn generate_summary(
        &self,
        assessments: &[FileDebtAssessment],
    ) -> DebtSummary {
        if assessments.is_empty() {
            return DebtSummary::default();
        }
        
        let mut level_counts = HashMap::new();
        let mut total_debt_score = 0.0;
        let mut total_estimated_hours = 0.0;
        
        for assessment in assessments {
            *level_counts.entry(assessment.debt_level).or_insert(0) += 1;
            total_debt_score += assessment.debt_score;
            total_estimated_hours += assessment.estimated_hours;
        }
        
        let average_debt_score = total_debt_score / assessments.len() as f64;
        let max_debt_score = assessments.first().map(|a| a.debt_score).unwrap_or(0.0);
        
        DebtSummary {
            total_files_with_debt: assessments.len(),
            critical_debt_files: level_counts.get(&DebtLevel::Critical).copied().unwrap_or(0),
            high_debt_files: level_counts.get(&DebtLevel::High).copied().unwrap_or(0),
            medium_debt_files: level_counts.get(&DebtLevel::Medium).copied().unwrap_or(0),
            low_debt_files: level_counts.get(&DebtLevel::Low).copied().unwrap_or(0),
            minimal_debt_files: level_counts.get(&DebtLevel::Minimal).copied().unwrap_or(0),
            average_debt_score,
            max_debt_score,
            total_estimated_hours,
            config: self.config.clone(),
        }
    }
}

/// Summary of technical debt assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtSummary {
    pub total_files_with_debt: usize,
    pub critical_debt_files: usize,
    pub high_debt_files: usize,
    pub medium_debt_files: usize,
    pub low_debt_files: usize,
    pub minimal_debt_files: usize,
    pub average_debt_score: f64,
    pub max_debt_score: f64,
    pub total_estimated_hours: f64,
    pub config: DebtConfig,
}

impl Default for DebtSummary {
    fn default() -> Self {
        Self {
            total_files_with_debt: 0,
            critical_debt_files: 0,
            high_debt_files: 0,
            medium_debt_files: 0,
            low_debt_files: 0,
            minimal_debt_files: 0,
            average_debt_score: 0.0,
            max_debt_score: 0.0,
            total_estimated_hours: 0.0,
            config: DebtConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::change_frequency::FileChangeStats;
    use super::super::duplication_detector::{CodeBlock, DuplicateGroup};
    
    fn create_test_complexity_metrics() -> HashMap<String, FileComplexityMetrics> {
        let mut metrics = HashMap::new();
        
        let mut high_debt_file = FileComplexityMetrics::new("high_debt.rs".to_string());
        high_debt_file.lines_of_code = 800;
        high_debt_file.cyclomatic_complexity = 35.0;
        high_debt_file.comment_ratio = 0.02;
        high_debt_file.file_size_bytes = 25000;
        metrics.insert("high_debt.rs".to_string(), high_debt_file);
        
        let mut low_debt_file = FileComplexityMetrics::new("low_debt.rs".to_string());
        low_debt_file.lines_of_code = 100;
        low_debt_file.cyclomatic_complexity = 5.0;
        low_debt_file.comment_ratio = 0.15;
        low_debt_file.file_size_bytes = 3000;
        metrics.insert("low_debt.rs".to_string(), low_debt_file);
        
        metrics
    }
    
    fn create_test_change_stats() -> HashMap<String, FileChangeStats> {
        let mut stats = HashMap::new();
        
        let mut frequent_changes = FileChangeStats::new("high_debt.rs".to_string());
        // Add many recent changes
        for i in 0..10 {
            frequent_changes.add_change(1000000 + (i * 86400), format!("author{}", i), format!("commit{}", i));
        }
        stats.insert("high_debt.rs".to_string(), frequent_changes);
        
        let mut rare_changes = FileChangeStats::new("low_debt.rs".to_string());
        rare_changes.add_change(500000, "author1".to_string(), "commit1".to_string());
        stats.insert("low_debt.rs".to_string(), rare_changes);
        
        stats
    }
    
    fn create_test_duplication_groups() -> Vec<DuplicateGroup> {
        let block1 = CodeBlock {
            file_path: "high_debt.rs".to_string(),
            start_line: 10,
            end_line: 30,
            tokens: vec!["token1".to_string(), "token2".to_string()],
            hash: 12345,
            raw_content: "duplicate code".to_string(),
        };
        
        let block2 = CodeBlock {
            file_path: "other.rs".to_string(),
            start_line: 50,
            end_line: 70,
            tokens: vec!["token1".to_string(), "token2".to_string()],
            hash: 12345,
            raw_content: "duplicate code".to_string(),
        };
        
        vec![DuplicateGroup {
            id: "group1".to_string(),
            blocks: vec![block1, block2],
            similarity_score: 0.95,
            total_lines: 40,
            total_tokens: 4,
            impact_score: 50.0,
        }]
    }
    
    #[test]
    fn test_debt_config_default() {
        let config = DebtConfig::default();
        assert!(config.validate().is_ok());
        
        let total_weight = config.complexity_weight + config.frequency_weight + 
                          config.duplication_weight + config.size_weight + config.age_weight;
        assert!((total_weight - 1.0).abs() < 0.01);
    }
    
    #[test]
    fn test_debt_config_validation() {
        let mut config = DebtConfig::default();
        config.complexity_weight = 0.5;
        config.frequency_weight = 0.6; // Total will be > 1.0
        
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_debt_level_from_score() {
        assert_eq!(DebtLevel::from_score(10.0), DebtLevel::Minimal);
        assert_eq!(DebtLevel::from_score(30.0), DebtLevel::Low);
        assert_eq!(DebtLevel::from_score(50.0), DebtLevel::Medium);
        assert_eq!(DebtLevel::from_score(70.0), DebtLevel::High);
        assert_eq!(DebtLevel::from_score(90.0), DebtLevel::Critical);
    }
    
    #[test]
    fn test_debt_assessor_creation() {
        let assessor = DebtAssessor::with_defaults();
        assert_eq!(assessor.config.complexity_weight, 0.3);
        
        let config = DebtConfig::default();
        let assessor2 = DebtAssessor::new(config).unwrap();
        assert_eq!(assessor2.config.complexity_weight, 0.3);
    }
    
    #[test]
    fn test_debt_assessment() {
        let assessor = DebtAssessor::with_defaults();
        let complexity_metrics = create_test_complexity_metrics();
        let change_stats = create_test_change_stats();
        let duplication_groups = create_test_duplication_groups();
        
        let assessments = assessor.assess_debt(&complexity_metrics, &change_stats, &duplication_groups);
        
        // Should have assessments for files with significant debt
        assert!(!assessments.is_empty());
        
        // Assessments should be sorted by debt score
        for i in 1..assessments.len() {
            assert!(assessments[i-1].debt_score >= assessments[i].debt_score);
        }
        
        // High debt file should have higher score than low debt file
        let high_debt = assessments.iter().find(|a| a.file_path == "high_debt.rs");
        assert!(high_debt.is_some());
        let high_debt = high_debt.unwrap();
        assert!(high_debt.debt_score > 20.0); // Should exceed threshold
    }
    
    #[test]
    fn test_debt_recommendations() {
        let assessor = DebtAssessor::with_defaults();
        let complexity_metrics = create_test_complexity_metrics();
        let change_stats = create_test_change_stats();
        let duplication_groups = create_test_duplication_groups();
        
        let assessments = assessor.assess_debt(&complexity_metrics, &change_stats, &duplication_groups);
        
        for assessment in &assessments {
            // All assessments should have recommendations
            assert!(!assessment.recommendations.is_empty() || !assessment.priority_actions.is_empty());
            // Should have estimated hours
            assert!(assessment.estimated_hours > 0.0);
        }
    }
    
    #[test]
    fn test_debt_summary() {
        let assessor = DebtAssessor::with_defaults();
        let complexity_metrics = create_test_complexity_metrics();
        let change_stats = create_test_change_stats();
        let duplication_groups = create_test_duplication_groups();
        
        let assessments = assessor.assess_debt(&complexity_metrics, &change_stats, &duplication_groups);
        let summary = assessor.generate_summary(&assessments);
        
        assert!(summary.total_files_with_debt > 0);
        assert!(summary.average_debt_score > 0.0);
        assert!(summary.max_debt_score >= summary.average_debt_score);
        assert!(summary.total_estimated_hours > 0.0);
    }
    
    #[test]
    fn test_duplication_impact_map() {
        let duplication_groups = create_test_duplication_groups();
        let assessor = DebtAssessor::with_defaults();
        
        let impact_map = assessor.build_duplication_impact_map(&duplication_groups);
        
        // Should have impact for high_debt.rs
        assert!(impact_map.contains_key("high_debt.rs"));
        assert!(impact_map.get("high_debt.rs").unwrap() > &0.0);
    }
    
    #[test]
    fn test_score_calculations() {
        let assessor = DebtAssessor::with_defaults();
        let complexity_metrics = create_test_complexity_metrics();
        
        let high_debt_file = complexity_metrics.get("high_debt.rs").unwrap();
        let complexity_score = assessor.calculate_complexity_score(high_debt_file);
        let size_score = assessor.calculate_size_score(high_debt_file);
        
        // High complexity file should have high scores
        assert!(complexity_score > 20.0);
        assert!(size_score > 10.0);
        
        // Scores should be normalized to 0-100 range
        assert!(complexity_score <= 100.0);
        assert!(size_score <= 100.0);
    }
}