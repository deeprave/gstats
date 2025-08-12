//! Hotspot Processor
//! 
//! Event-driven processor that identifies code hotspots by combining
//! complexity metrics with change frequency data. This processor can be used
//! by any plugin that needs hotspot analysis.

use crate::scanner::async_engine::events::RepositoryEvent;
use crate::scanner::async_engine::processors::{EventProcessor, ProcessorStats};
use crate::scanner::async_engine::shared_state::SharedProcessorState;
use crate::scanner::messages::{ScanMessage, MessageData, MessageHeader};
use crate::plugin::PluginResult;
use crate::plugin::processors::change_frequency::{FileChangeStats, TimeWindow};
use crate::plugin::processors::complexity::ComplexityMetrics;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use log::debug;
use serde::{Serialize, Deserialize};

/// Hotspot metrics combining complexity and change frequency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotMetrics {
    pub file_path: String,
    pub hotspot_score: f64,
    pub complexity_score: f64,
    pub frequency_score: f64,
    pub risk_level: RiskLevel,
    pub recommendations: Vec<String>,
    pub change_count: u32,
    pub author_count: u32,
    pub last_changed: i64,
    pub lines_of_code: u32,
    pub cyclomatic_complexity: f64,
}

impl HotspotMetrics {
    pub fn new(
        file_path: String,
        complexity_metrics: &ComplexityMetrics,
        change_stats: &FileChangeStats,
        time_window: TimeWindow,
    ) -> Self {
        let complexity_score = complexity_metrics.complexity_score();
        let frequency_score = change_stats.frequency_score(time_window);
        
        // Calculate hotspot score as weighted combination
        let complexity_weight = 0.6;
        let frequency_weight = 0.4;
        let hotspot_score = (complexity_score * complexity_weight) + (frequency_score * frequency_weight);
        
        let risk_level = Self::calculate_risk_level(hotspot_score);
        let recommendations = Self::generate_recommendations(&risk_level, complexity_metrics, change_stats);

        Self {
            file_path,
            hotspot_score,
            complexity_score,
            frequency_score,
            risk_level,
            recommendations,
            change_count: change_stats.change_count,
            author_count: change_stats.author_count as u32,
            last_changed: change_stats.last_changed,
            lines_of_code: complexity_metrics.lines_of_code,
            cyclomatic_complexity: complexity_metrics.cyclomatic_complexity,
        }
    }

    fn calculate_risk_level(hotspot_score: f64) -> RiskLevel {
        match hotspot_score {
            s if s < 5.0 => RiskLevel::Low,
            s if s < 15.0 => RiskLevel::Medium,
            s if s < 30.0 => RiskLevel::High,
            _ => RiskLevel::Critical,
        }
    }

    fn generate_recommendations(
        risk_level: &RiskLevel,
        complexity_metrics: &ComplexityMetrics,
        change_stats: &FileChangeStats,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        match risk_level {
            RiskLevel::Critical => {
                recommendations.push("URGENT: This file requires immediate attention".to_string());
                recommendations.push("Consider breaking this file into smaller modules".to_string());
                recommendations.push("Add comprehensive unit tests before refactoring".to_string());
            }
            RiskLevel::High => {
                recommendations.push("High priority: Schedule refactoring for this file".to_string());
                recommendations.push("Review and simplify complex functions".to_string());
            }
            RiskLevel::Medium => {
                recommendations.push("Monitor this file for further complexity growth".to_string());
                recommendations.push("Consider adding documentation for complex sections".to_string());
            }
            RiskLevel::Low => {
                recommendations.push("File is in good condition".to_string());
            }
        }

        // Complexity-specific recommendations
        if complexity_metrics.cyclomatic_complexity > 15.0 {
            recommendations.push("Reduce cyclomatic complexity by extracting methods".to_string());
        }
        if complexity_metrics.nesting_depth > 5 {
            recommendations.push("Reduce nesting depth using early returns or guard clauses".to_string());
        }
        if complexity_metrics.lines_of_code > 500 {
            recommendations.push("Consider splitting this large file into smaller modules".to_string());
        }

        // Change frequency-specific recommendations
        if change_stats.change_count > 20 {
            recommendations.push("High change frequency indicates potential design issues".to_string());
        }
        if change_stats.author_count > 5 {
            recommendations.push("Multiple authors suggest need for better documentation".to_string());
        }

        recommendations
    }
}

/// Risk level for hotspots
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
            RiskLevel::Critical => "critical",
        }
    }
}

/// Configuration for hotspot detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotConfig {
    pub complexity_threshold: f64,
    pub frequency_threshold: f64,
    pub complexity_weight: f64,
    pub frequency_weight: f64,
    pub max_hotspots: usize,
    pub time_window: TimeWindow,
}

impl Default for HotspotConfig {
    fn default() -> Self {
        Self {
            complexity_threshold: 10.0,
            frequency_threshold: 5.0,
            complexity_weight: 0.6,
            frequency_weight: 0.4,
            max_hotspots: 20,
            time_window: TimeWindow::Quarter,
        }
    }
}

/// Hotspot Processor - can be used by any plugin
/// 
/// This processor combines data from complexity and change frequency processors
/// to identify code hotspots that need attention.
pub struct HotspotProcessor {
    config: HotspotConfig,
    hotspots: HashMap<String, HotspotMetrics>,
    stats: ProcessorStats,
    shared_state: Option<Arc<SharedProcessorState>>,
}

impl HotspotProcessor {
    pub fn new() -> Self {
        Self {
            config: HotspotConfig::default(),
            hotspots: HashMap::new(),
            stats: ProcessorStats::default(),
            shared_state: None,
        }
    }

    pub fn with_config(config: HotspotConfig) -> Self {
        Self {
            config,
            hotspots: HashMap::new(),
            stats: ProcessorStats::default(),
            shared_state: None,
        }
    }

    /// Analyze hotspots using complexity and change frequency data
    /// 
    /// This method should be called during finalization after other processors
    /// have collected their data.
    pub fn analyze_hotspots(
        &mut self,
        complexity_metrics: &HashMap<String, ComplexityMetrics>,
        change_stats: &HashMap<String, FileChangeStats>,
    ) {
        self.hotspots.clear();

        // Find files that exist in both datasets
        for (file_path, complexity) in complexity_metrics {
            if let Some(changes) = change_stats.get(file_path) {
                // Only consider files that meet minimum thresholds
                if complexity.complexity_score() >= self.config.complexity_threshold
                    || changes.frequency_score(self.config.time_window) >= self.config.frequency_threshold
                {
                    let hotspot = HotspotMetrics::new(
                        file_path.clone(),
                        complexity,
                        changes,
                        self.config.time_window,
                    );
                    
                    self.hotspots.insert(file_path.clone(), hotspot);
                }
            }
        }

        debug!("Identified {} hotspots", self.hotspots.len());
    }

    /// Get the top hotspots sorted by score
    pub fn get_top_hotspots(&self, limit: usize) -> Vec<&HotspotMetrics> {
        let mut hotspots: Vec<&HotspotMetrics> = self.hotspots.values().collect();
        hotspots.sort_by(|a, b| b.hotspot_score.partial_cmp(&a.hotspot_score).unwrap());
        hotspots.into_iter().take(limit).collect()
    }

    /// Get hotspots by risk level
    pub fn get_hotspots_by_risk(&self, risk_level: RiskLevel) -> Vec<&HotspotMetrics> {
        self.hotspots
            .values()
            .filter(|h| h.risk_level == risk_level)
            .collect()
    }

    /// Generate hotspot summary statistics
    pub fn generate_summary(&self) -> HotspotSummary {
        let total_hotspots = self.hotspots.len();
        let critical_hotspots = self.get_hotspots_by_risk(RiskLevel::Critical).len();
        let high_hotspots = self.get_hotspots_by_risk(RiskLevel::High).len();
        let medium_hotspots = self.get_hotspots_by_risk(RiskLevel::Medium).len();
        let low_hotspots = self.get_hotspots_by_risk(RiskLevel::Low).len();

        let average_hotspot_score = if total_hotspots > 0 {
            self.hotspots.values().map(|h| h.hotspot_score).sum::<f64>() / total_hotspots as f64
        } else {
            0.0
        };

        let max_hotspot_score = self.hotspots
            .values()
            .map(|h| h.hotspot_score)
            .fold(0.0, f64::max);

        HotspotSummary {
            total_hotspots,
            critical_hotspots,
            high_hotspots,
            medium_hotspots,
            low_hotspots,
            average_hotspot_score,
            max_hotspot_score,
            time_window: self.config.time_window,
        }
    }

    fn create_hotspot_messages(&self) -> Vec<ScanMessage> {
        let mut messages = Vec::new();
        
        // Create messages for top hotspots
        let top_hotspots = self.get_top_hotspots(self.config.max_hotspots);
        
        for hotspot in top_hotspots {
            let header = MessageHeader::new(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );

            // For now, use MetricInfo - in a full implementation, we'd have a HotspotInfo variant
            let data = MessageData::MetricInfo {
                file_count: 1,
                line_count: hotspot.lines_of_code as u64,
                complexity: hotspot.hotspot_score,
            };

            messages.push(ScanMessage::new(header, data));
        }
        
        messages
    }
}

/// Summary of hotspot analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotSummary {
    pub total_hotspots: usize,
    pub critical_hotspots: usize,
    pub high_hotspots: usize,
    pub medium_hotspots: usize,
    pub low_hotspots: usize,
    pub average_hotspot_score: f64,
    pub max_hotspot_score: f64,
    pub time_window: TimeWindow,
}

#[async_trait]
impl EventProcessor for HotspotProcessor {

    fn name(&self) -> &'static str {
        "hotspot"
    }

    fn set_shared_state(&mut self, shared_state: Arc<SharedProcessorState>) {
        self.shared_state = Some(shared_state);
    }

    fn shared_state(&self) -> Option<&Arc<SharedProcessorState>> {
        self.shared_state.as_ref()
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        debug!("Initialized HotspotProcessor with config: {:?}", self.config);
        Ok(())
    }

    async fn process_event(&mut self, _event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
        // Hotspot processor doesn't process individual events
        // It analyzes data from other processors during finalization
        self.stats.events_processed += 1;
        Ok(vec![])
    }

    async fn finalize(&mut self) -> PluginResult<Vec<ScanMessage>> {
        // In a full implementation, this would get data from other processors
        // via shared state or dependency injection
        
        // For now, create empty analysis
        let messages = self.create_hotspot_messages();
        self.stats.messages_generated = messages.len();
        
        debug!("HotspotProcessor finalized with {} messages", messages.len());
        Ok(messages)
    }

    fn get_stats(&self) -> ProcessorStats {
        self.stats.clone()
    }
}

impl Default for HotspotProcessor {
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
    async fn test_hotspot_processor_creation() {
        let processor = HotspotProcessor::new();
        assert_eq!(processor.name(), "hotspot");
        // Processor no longer advertises supported modes
        assert!(processor.hotspots.is_empty());
    }

    #[tokio::test]
    async fn test_hotspot_metrics_creation() {
        let mut complexity = ComplexityMetrics::new("test.rs".to_string());
        complexity.cyclomatic_complexity = 15.0;
        complexity.lines_of_code = 200;

        let mut change_stats = FileChangeStats::new("test.rs".to_string());
        change_stats.change_count = 10;
        change_stats.author_count = 3;

        let hotspot = HotspotMetrics::new(
            "test.rs".to_string(),
            &complexity,
            &change_stats,
            TimeWindow::Month,
        );

        assert_eq!(hotspot.file_path, "test.rs");
        assert!(hotspot.hotspot_score > 0.0);
        assert!(!hotspot.recommendations.is_empty());
    }

    #[tokio::test]
    async fn test_risk_level_calculation() {
        let mut complexity = ComplexityMetrics::new("test.rs".to_string());
        let mut change_stats = FileChangeStats::new("test.rs".to_string());

        // Low risk
        complexity.cyclomatic_complexity = 2.0;
        change_stats.change_count = 1;
        let hotspot = HotspotMetrics::new("test.rs".to_string(), &complexity, &change_stats, TimeWindow::Month);
        assert_eq!(hotspot.risk_level, RiskLevel::Low);

        // High risk
        complexity.cyclomatic_complexity = 25.0;
        change_stats.change_count = 20;
        // Set recent change to get recency boost
        change_stats.last_changed = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64 - (3 * 24 * 60 * 60); // 3 days ago
        let hotspot = HotspotMetrics::new("test.rs".to_string(), &complexity, &change_stats, TimeWindow::Month);
        assert!(matches!(hotspot.risk_level, RiskLevel::High | RiskLevel::Critical));
    }

    #[tokio::test]
    async fn test_hotspot_analysis() {
        let mut processor = HotspotProcessor::new();
        
        let mut complexity_metrics = HashMap::new();
        let mut complexity = ComplexityMetrics::new("test.rs".to_string());
        complexity.cyclomatic_complexity = 15.0;
        complexity_metrics.insert("test.rs".to_string(), complexity);

        let mut change_stats = HashMap::new();
        let mut stats = FileChangeStats::new("test.rs".to_string());
        stats.change_count = 10;
        change_stats.insert("test.rs".to_string(), stats);

        processor.analyze_hotspots(&complexity_metrics, &change_stats);
        
        assert_eq!(processor.hotspots.len(), 1);
        assert!(processor.hotspots.contains_key("test.rs"));
    }

    #[tokio::test]
    async fn test_hotspot_summary() {
        let mut processor = HotspotProcessor::new();
        
        // Add some test hotspots
        let mut complexity = ComplexityMetrics::new("test1.rs".to_string());
        complexity.cyclomatic_complexity = 25.0; // High complexity
        let mut change_stats = FileChangeStats::new("test1.rs".to_string());
        change_stats.change_count = 15;
        
        let hotspot = HotspotMetrics::new("test1.rs".to_string(), &complexity, &change_stats, TimeWindow::Month);
        processor.hotspots.insert("test1.rs".to_string(), hotspot);

        let summary = processor.generate_summary();
        assert_eq!(summary.total_hotspots, 1);
        assert!(summary.average_hotspot_score > 0.0);
    }
}
