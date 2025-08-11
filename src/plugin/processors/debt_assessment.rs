//! Debt Assessment Processor
//! 
//! Event-driven processor that assesses technical debt by combining
//! complexity, change frequency, and duplication metrics. This processor
//! can be used by any plugin that needs technical debt analysis.

use crate::scanner::async_engine::events::RepositoryEvent;
use crate::scanner::async_engine::processors::{EventProcessor, ProcessorStats};
use crate::scanner::async_engine::shared_state::{SharedProcessorState, RepositoryMetadata};
use crate::scanner::messages::{ScanMessage, MessageData, MessageHeader};
use crate::scanner::modes::ScanMode;
use crate::plugin::PluginResult;
use crate::plugin::processors::change_frequency::{FileChangeStats, TimeWindow};
use crate::plugin::processors::complexity::ComplexityMetrics;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use log::debug;
use serde::{Serialize, Deserialize};

/// Configuration for technical debt assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtConfig {
    /// Weight for complexity score in debt calculation (0.0 to 1.0)
    pub complexity_weight: f64,
    /// Weight for change frequency in debt calculation (0.0 to 1.0)
    pub frequency_weight: f64,
    /// Weight for duplication impact in debt calculation (0.0 to 1.0)
    pub duplication_weight: f64,
    /// Weight for file size in debt calculation (0.0 to 1.0)
    pub size_weight: f64,
    /// Weight for file age in debt calculation (0.0 to 1.0)
    pub age_weight: f64,
    /// Minimum debt score threshold for reporting
    pub debt_threshold: f64,
    /// Maximum number of files to report
    pub max_files: usize,
    /// Time window for change frequency analysis
    pub time_window: TimeWindow,
}

impl Default for DebtConfig {
    fn default() -> Self {
        Self {
            complexity_weight: 0.3,
            frequency_weight: 0.25,
            duplication_weight: 0.2,
            size_weight: 0.15,
            age_weight: 0.1,
            debt_threshold: 50.0,
            max_files: 20,
            time_window: TimeWindow::Quarter,
        }
    }
}

/// File-level technical debt assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDebtAssessment {
    pub file_path: String,
    pub debt_score: f64,
    pub debt_level: DebtLevel,
    pub component_scores: DebtComponentScores,
    pub recommendations: Vec<String>,
    pub priority_actions: Vec<String>,
    pub estimated_hours: f64,
}

/// Component scores that make up the debt assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtComponentScores {
    pub complexity_score: f64,
    pub frequency_score: f64,
    pub duplication_score: f64,
    pub size_score: f64,
    pub age_score: f64,
}

/// Technical debt level classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebtLevel {
    Minimal,
    Low,
    Medium,
    High,
    Critical,
}

impl DebtLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            DebtLevel::Minimal => "minimal",
            DebtLevel::Low => "low",
            DebtLevel::Medium => "medium",
            DebtLevel::High => "high",
            DebtLevel::Critical => "critical",
        }
    }

    pub fn from_score(score: f64) -> Self {
        match score {
            s if s < 20.0 => DebtLevel::Minimal,
            s if s < 40.0 => DebtLevel::Low,
            s if s < 60.0 => DebtLevel::Medium,
            s if s < 80.0 => DebtLevel::High,
            _ => DebtLevel::Critical,
        }
    }
}

impl FileDebtAssessment {
    pub fn new(
        file_path: String,
        complexity_metrics: &ComplexityMetrics,
        change_stats: Option<&FileChangeStats>,
        duplication_impact: f64,
        config: &DebtConfig,
    ) -> Self {
        let component_scores = Self::calculate_component_scores(
            complexity_metrics,
            change_stats,
            duplication_impact,
            config,
        );

        let debt_score = Self::calculate_debt_score(&component_scores, config);
        let debt_level = DebtLevel::from_score(debt_score);
        let recommendations = Self::generate_recommendations(&debt_level, complexity_metrics, change_stats);
        let priority_actions = Self::generate_priority_actions(&debt_level, &component_scores);
        let estimated_hours = Self::estimate_refactoring_hours(&debt_level, complexity_metrics);

        Self {
            file_path,
            debt_score,
            debt_level,
            component_scores,
            recommendations,
            priority_actions,
            estimated_hours,
        }
    }

    fn calculate_component_scores(
        complexity_metrics: &ComplexityMetrics,
        change_stats: Option<&FileChangeStats>,
        duplication_impact: f64,
        config: &DebtConfig,
    ) -> DebtComponentScores {
        let complexity_score = Self::normalize_complexity_score(complexity_metrics);
        let frequency_score = Self::normalize_frequency_score(change_stats, config.time_window);
        let duplication_score = Self::normalize_duplication_score(duplication_impact);
        let size_score = Self::normalize_size_score(complexity_metrics);
        let age_score = Self::normalize_age_score(change_stats);

        DebtComponentScores {
            complexity_score,
            frequency_score,
            duplication_score,
            size_score,
            age_score,
        }
    }

    fn calculate_debt_score(scores: &DebtComponentScores, config: &DebtConfig) -> f64 {
        (scores.complexity_score * config.complexity_weight) +
        (scores.frequency_score * config.frequency_weight) +
        (scores.duplication_score * config.duplication_weight) +
        (scores.size_score * config.size_weight) +
        (scores.age_score * config.age_weight)
    }

    fn normalize_complexity_score(metrics: &ComplexityMetrics) -> f64 {
        // Normalize complexity to 0-100 scale
        let complexity = metrics.complexity_score();
        (complexity * 5.0).min(100.0) // Scale factor of 5, cap at 100
    }

    fn normalize_frequency_score(change_stats: Option<&FileChangeStats>, time_window: TimeWindow) -> f64 {
        match change_stats {
            Some(stats) => {
                let frequency = stats.frequency_score(time_window);
                (frequency * 2.0).min(100.0) // Scale factor of 2, cap at 100
            }
            None => 0.0,
        }
    }

    fn normalize_duplication_score(duplication_impact: f64) -> f64 {
        (duplication_impact * 10.0).min(100.0) // Scale factor of 10, cap at 100
    }

    fn normalize_size_score(metrics: &ComplexityMetrics) -> f64 {
        // Normalize file size to 0-100 scale
        let size_factor = (metrics.lines_of_code as f64 / 10.0).min(100.0);
        size_factor
    }

    fn normalize_age_score(change_stats: Option<&FileChangeStats>) -> f64 {
        match change_stats {
            Some(stats) => {
                if stats.last_changed == 0 {
                    return 50.0; // Default for unknown age
                }

                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                let days_since_change = (now - stats.last_changed) / (24 * 60 * 60);
                
                // Older files get higher age scores (more debt)
                match days_since_change {
                    0..=30 => 10.0,      // Very recent
                    31..=90 => 25.0,     // Recent
                    91..=365 => 50.0,    // Moderate age
                    366..=730 => 75.0,   // Old
                    _ => 100.0,          // Very old
                }
            }
            None => 50.0, // Default for unknown age
        }
    }

    fn generate_recommendations(
        debt_level: &DebtLevel,
        complexity_metrics: &ComplexityMetrics,
        change_stats: Option<&FileChangeStats>,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        match debt_level {
            DebtLevel::Critical => {
                recommendations.push("CRITICAL: This file requires immediate refactoring".to_string());
                recommendations.push("Stop adding new features until debt is addressed".to_string());
                recommendations.push("Consider complete rewrite if refactoring is too complex".to_string());
            }
            DebtLevel::High => {
                recommendations.push("High priority: Schedule dedicated refactoring time".to_string());
                recommendations.push("Break down into smaller, more focused modules".to_string());
                recommendations.push("Add comprehensive test coverage before refactoring".to_string());
            }
            DebtLevel::Medium => {
                recommendations.push("Medium priority: Include in next sprint planning".to_string());
                recommendations.push("Refactor during related feature development".to_string());
                recommendations.push("Improve code documentation and comments".to_string());
            }
            DebtLevel::Low => {
                recommendations.push("Low priority: Address during maintenance cycles".to_string());
                recommendations.push("Monitor for increasing complexity trends".to_string());
            }
            DebtLevel::Minimal => {
                recommendations.push("File is in good condition".to_string());
                recommendations.push("Maintain current quality standards".to_string());
            }
        }

        // Specific recommendations based on metrics
        if complexity_metrics.cyclomatic_complexity > 20.0 {
            recommendations.push("Extract complex methods into smaller functions".to_string());
        }
        if complexity_metrics.lines_of_code > 500 {
            recommendations.push("Split large file into multiple focused modules".to_string());
        }
        if complexity_metrics.nesting_depth > 6 {
            recommendations.push("Reduce nesting depth using early returns and guard clauses".to_string());
        }

        if let Some(stats) = change_stats {
            if stats.change_count > 30 {
                recommendations.push("High change frequency suggests design instability".to_string());
            }
            if stats.author_count > 8 {
                recommendations.push("Many contributors suggest need for better documentation".to_string());
            }
        }

        recommendations
    }

    fn generate_priority_actions(debt_level: &DebtLevel, scores: &DebtComponentScores) -> Vec<String> {
        let mut actions = Vec::new();

        match debt_level {
            DebtLevel::Critical | DebtLevel::High => {
                if scores.complexity_score > 70.0 {
                    actions.push("Reduce cyclomatic complexity immediately".to_string());
                }
                if scores.duplication_score > 60.0 {
                    actions.push("Eliminate code duplication".to_string());
                }
                if scores.size_score > 80.0 {
                    actions.push("Break down large file into smaller modules".to_string());
                }
            }
            DebtLevel::Medium => {
                if scores.complexity_score > 50.0 {
                    actions.push("Simplify complex functions".to_string());
                }
                if scores.frequency_score > 60.0 {
                    actions.push("Stabilize frequently changing code".to_string());
                }
            }
            DebtLevel::Low | DebtLevel::Minimal => {
                actions.push("Continue monitoring".to_string());
            }
        }

        actions
    }

    fn estimate_refactoring_hours(debt_level: &DebtLevel, metrics: &ComplexityMetrics) -> f64 {
        let base_hours = match debt_level {
            DebtLevel::Critical => 40.0,
            DebtLevel::High => 24.0,
            DebtLevel::Medium => 12.0,
            DebtLevel::Low => 4.0,
            DebtLevel::Minimal => 1.0,
        };

        // Adjust based on file size
        let size_multiplier = (metrics.lines_of_code as f64 / 200.0).max(0.5).min(3.0);
        
        base_hours * size_multiplier
    }
}

/// Technical debt assessor processor
pub struct DebtAssessmentProcessor {
    config: DebtConfig,
    debt_assessments: HashMap<String, FileDebtAssessment>,
    stats: ProcessorStats,
    shared_state: Option<Arc<SharedProcessorState>>,
}

impl DebtAssessmentProcessor {
    pub fn new() -> Self {
        Self {
            config: DebtConfig::default(),
            debt_assessments: HashMap::new(),
            stats: ProcessorStats::default(),
            shared_state: None,
        }
    }

    pub fn with_config(config: DebtConfig) -> Self {
        Self {
            config,
            debt_assessments: HashMap::new(),
            stats: ProcessorStats::default(),
            shared_state: None,
        }
    }

    /// Assess technical debt using complexity and change frequency data
    pub fn assess_debt(
        &mut self,
        complexity_metrics: &HashMap<String, ComplexityMetrics>,
        change_stats: &HashMap<String, FileChangeStats>,
        duplication_impacts: &HashMap<String, f64>,
    ) {
        self.debt_assessments.clear();

        for (file_path, complexity) in complexity_metrics {
            let change_data = change_stats.get(file_path);
            let duplication_impact = duplication_impacts.get(file_path).copied().unwrap_or(0.0);

            let assessment = FileDebtAssessment::new(
                file_path.clone(),
                complexity,
                change_data,
                duplication_impact,
                &self.config,
            );

            // Only include files that meet the debt threshold
            if assessment.debt_score >= self.config.debt_threshold {
                self.debt_assessments.insert(file_path.clone(), assessment);
            }
        }

        debug!("Assessed {} files with technical debt", self.debt_assessments.len());
    }

    /// Get debt assessments sorted by score
    pub fn get_top_debt_files(&self, limit: usize) -> Vec<&FileDebtAssessment> {
        let mut assessments: Vec<&FileDebtAssessment> = self.debt_assessments.values().collect();
        assessments.sort_by(|a, b| b.debt_score.partial_cmp(&a.debt_score).unwrap());
        assessments.into_iter().take(limit).collect()
    }

    /// Get assessments by debt level
    pub fn get_assessments_by_level(&self, debt_level: DebtLevel) -> Vec<&FileDebtAssessment> {
        self.debt_assessments
            .values()
            .filter(|a| a.debt_level == debt_level)
            .collect()
    }

    /// Generate debt summary statistics
    pub fn generate_summary(&self) -> DebtSummary {
        let total_files_with_debt = self.debt_assessments.len();
        let critical_debt_files = self.get_assessments_by_level(DebtLevel::Critical).len();
        let high_debt_files = self.get_assessments_by_level(DebtLevel::High).len();
        let medium_debt_files = self.get_assessments_by_level(DebtLevel::Medium).len();
        let low_debt_files = self.get_assessments_by_level(DebtLevel::Low).len();
        let minimal_debt_files = self.get_assessments_by_level(DebtLevel::Minimal).len();

        let average_debt_score = if total_files_with_debt > 0 {
            self.debt_assessments.values().map(|a| a.debt_score).sum::<f64>() / total_files_with_debt as f64
        } else {
            0.0
        };

        let max_debt_score = self.debt_assessments
            .values()
            .map(|a| a.debt_score)
            .fold(0.0, f64::max);

        let total_estimated_hours = self.debt_assessments
            .values()
            .map(|a| a.estimated_hours)
            .sum();

        DebtSummary {
            total_files_with_debt,
            critical_debt_files,
            high_debt_files,
            medium_debt_files,
            low_debt_files,
            minimal_debt_files,
            average_debt_score,
            max_debt_score,
            total_estimated_hours,
            config: self.config.clone(),
        }
    }

    fn create_debt_messages(&self) -> Vec<ScanMessage> {
        let mut messages = Vec::new();
        
        // Create messages for top debt files
        let top_debt_files = self.get_top_debt_files(self.config.max_files);
        
        for assessment in top_debt_files {
            let header = MessageHeader::new(
                ScanMode::METRICS,
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );

            // For now, use MetricInfo - in a full implementation, we'd have a DebtInfo variant
            let data = MessageData::MetricInfo {
                file_count: 1,
                line_count: assessment.component_scores.size_score as u64,
                complexity: assessment.debt_score,
            };

            messages.push(ScanMessage::new(header, data));
        }
        
        messages
    }
}

/// Summary of technical debt analysis
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

#[async_trait]
impl EventProcessor for DebtAssessmentProcessor {
    fn supported_modes(&self) -> ScanMode {
        ScanMode::METRICS | ScanMode::CHANGE_FREQUENCY
    }

    fn name(&self) -> &'static str {
        "debt_assessment"
    }

    fn set_shared_state(&mut self, shared_state: Arc<SharedProcessorState>) {
        self.shared_state = Some(shared_state);
    }

    fn shared_state(&self) -> Option<&Arc<SharedProcessorState>> {
        self.shared_state.as_ref()
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        debug!("Initialized DebtAssessmentProcessor with config: {:?}", self.config);
        Ok(())
    }

    async fn process_event(&mut self, _event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
        // Debt assessment processor doesn't process individual events
        // It analyzes data from other processors during finalization
        self.stats.events_processed += 1;
        Ok(vec![])
    }

    async fn finalize(&mut self) -> PluginResult<Vec<ScanMessage>> {
        // In a full implementation, this would get data from other processors
        // via shared state or dependency injection
        
        let messages = self.create_debt_messages();
        self.stats.messages_generated = messages.len();
        
        debug!("DebtAssessmentProcessor finalized with {} messages", messages.len());
        Ok(messages)
    }

    fn get_stats(&self) -> ProcessorStats {
        self.stats.clone()
    }
}

impl Default for DebtAssessmentProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::processors::complexity::ComplexityMetrics;
    use crate::plugin::processors::change_frequency::FileChangeStats;

    #[tokio::test]
    async fn test_debt_assessment_processor_creation() {
        let processor = DebtAssessmentProcessor::new();
        assert_eq!(processor.name(), "debt_assessment");
        assert_eq!(processor.supported_modes(), ScanMode::METRICS | ScanMode::CHANGE_FREQUENCY);
        assert!(processor.debt_assessments.is_empty());
    }

    #[tokio::test]
    async fn test_debt_level_from_score() {
        assert_eq!(DebtLevel::from_score(10.0), DebtLevel::Minimal);
        assert_eq!(DebtLevel::from_score(30.0), DebtLevel::Low);
        assert_eq!(DebtLevel::from_score(50.0), DebtLevel::Medium);
        assert_eq!(DebtLevel::from_score(70.0), DebtLevel::High);
        assert_eq!(DebtLevel::from_score(90.0), DebtLevel::Critical);
    }

    #[tokio::test]
    async fn test_file_debt_assessment() {
        let mut complexity = ComplexityMetrics::new("test.rs".to_string());
        complexity.cyclomatic_complexity = 20.0;
        complexity.lines_of_code = 300;

        let mut change_stats = FileChangeStats::new("test.rs".to_string());
        change_stats.change_count = 15;
        change_stats.author_count = 4;

        let config = DebtConfig::default();
        let assessment = FileDebtAssessment::new(
            "test.rs".to_string(),
            &complexity,
            Some(&change_stats),
            5.0, // duplication impact
            &config,
        );

        assert_eq!(assessment.file_path, "test.rs");
        assert!(assessment.debt_score > 0.0);
        assert!(!assessment.recommendations.is_empty());
        assert!(!assessment.priority_actions.is_empty());
        assert!(assessment.estimated_hours > 0.0);
    }

    #[tokio::test]
    async fn test_debt_assessment() {
        let mut processor = DebtAssessmentProcessor::new();
        
        let mut complexity_metrics = HashMap::new();
        let mut complexity = ComplexityMetrics::new("test.rs".to_string());
        complexity.cyclomatic_complexity = 25.0;
        complexity.lines_of_code = 400;
        complexity_metrics.insert("test.rs".to_string(), complexity);

        let mut change_stats = HashMap::new();
        let mut stats = FileChangeStats::new("test.rs".to_string());
        stats.change_count = 20;
        change_stats.insert("test.rs".to_string(), stats);

        let mut duplication_impacts = HashMap::new();
        duplication_impacts.insert("test.rs".to_string(), 8.0);

        processor.assess_debt(&complexity_metrics, &change_stats, &duplication_impacts);
        
        assert_eq!(processor.debt_assessments.len(), 1);
        assert!(processor.debt_assessments.contains_key("test.rs"));
    }

    #[tokio::test]
    async fn test_debt_summary() {
        let mut processor = DebtAssessmentProcessor::new();
        
        // Add a test assessment
        let mut complexity = ComplexityMetrics::new("test.rs".to_string());
        complexity.cyclomatic_complexity = 20.0;
        complexity.lines_of_code = 300;
        
        let change_stats = FileChangeStats::new("test.rs".to_string());
        let config = DebtConfig::default();
        
        let assessment = FileDebtAssessment::new(
            "test.rs".to_string(),
            &complexity,
            Some(&change_stats),
            5.0,
            &config,
        );
        
        processor.debt_assessments.insert("test.rs".to_string(), assessment);

        let summary = processor.generate_summary();
        assert_eq!(summary.total_files_with_debt, 1);
        assert!(summary.average_debt_score > 0.0);
        assert!(summary.total_estimated_hours > 0.0);
    }
}
