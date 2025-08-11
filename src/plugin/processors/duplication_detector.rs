//! Duplication Detector Processor
//! 
//! Event-driven processor that detects code duplication by analyzing
//! file content patterns and similarities. This processor can be used
//! by any plugin that needs duplication analysis.

use crate::scanner::async_engine::events::RepositoryEvent;
use crate::scanner::async_engine::processors::{EventProcessor, ProcessorStats};
use crate::scanner::async_engine::shared_state::{SharedProcessorState, RepositoryMetadata};
use crate::scanner::messages::{ScanMessage, MessageData, MessageHeader};
use crate::scanner::modes::ScanMode;
use crate::plugin::PluginResult;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use log::debug;
use serde::{Serialize, Deserialize};

/// Configuration for duplication detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicationConfig {
    /// Minimum similarity threshold (0.0 to 1.0)
    pub similarity_threshold: f64,
    /// Minimum block size in lines
    pub min_block_size: usize,
    /// Maximum number of duplicate groups to report
    pub max_groups: usize,
    /// Whether to ignore whitespace differences
    pub ignore_whitespace: bool,
    /// Whether to ignore comments
    pub ignore_comments: bool,
    /// File extensions to analyze
    pub analyzed_extensions: Vec<String>,
}

impl Default for DuplicationConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.8,
            min_block_size: 5,
            max_groups: 50,
            ignore_whitespace: true,
            ignore_comments: true,
            analyzed_extensions: vec![
                "rs".to_string(), "py".to_string(), "js".to_string(), "ts".to_string(),
                "java".to_string(), "c".to_string(), "cpp".to_string(), "h".to_string(),
                "cs".to_string(), "go".to_string(), "rb".to_string(), "php".to_string(),
            ],
        }
    }
}

/// A block of potentially duplicated code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content_hash: String,
    pub normalized_content: String,
    pub token_count: usize,
}

impl CodeBlock {
    pub fn new(
        file_path: String,
        start_line: usize,
        end_line: usize,
        content: &str,
        config: &DuplicationConfig,
    ) -> Self {
        let normalized_content = Self::normalize_content(content, config);
        let content_hash = Self::calculate_hash(&normalized_content);
        let token_count = Self::count_tokens(&normalized_content);

        Self {
            file_path,
            start_line,
            end_line,
            content_hash,
            normalized_content,
            token_count,
        }
    }

    fn normalize_content(content: &str, config: &DuplicationConfig) -> String {
        let mut normalized = content.to_string();

        if config.ignore_whitespace {
            normalized = normalized
                .lines()
                .map(|line| line.trim())
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
        }

        if config.ignore_comments {
            normalized = Self::remove_comments(&normalized);
        }

        normalized
    }

    fn remove_comments(content: &str) -> String {
        // Simple comment removal (would be more sophisticated in full implementation)
        content
            .lines()
            .map(|line| {
                // Remove single-line comments
                if let Some(pos) = line.find("//") {
                    line[..pos].trim_end()
                } else if let Some(pos) = line.find('#') {
                    line[..pos].trim_end()
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn calculate_hash(content: &str) -> String {
        // Simple hash calculation (would use proper hashing in full implementation)
        format!("{:x}", content.len() * 31 + content.chars().map(|c| c as usize).sum::<usize>())
    }

    fn count_tokens(content: &str) -> usize {
        content
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .count()
    }

    pub fn line_count(&self) -> usize {
        self.end_line - self.start_line + 1
    }

    pub fn similarity(&self, other: &CodeBlock) -> f64 {
        if self.content_hash == other.content_hash {
            return 1.0;
        }

        // Simple similarity calculation based on token overlap
        let self_tokens: std::collections::HashSet<&str> = self.normalized_content.split_whitespace().collect();
        let other_tokens: std::collections::HashSet<&str> = other.normalized_content.split_whitespace().collect();

        let intersection = self_tokens.intersection(&other_tokens).count();
        let union = self_tokens.union(&other_tokens).count();

        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }
}

/// A group of duplicate code blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub id: String,
    pub blocks: Vec<CodeBlock>,
    pub similarity_score: f64,
    pub total_lines: usize,
    pub total_tokens: usize,
    pub impact_score: f64,
}

impl DuplicateGroup {
    pub fn new(blocks: Vec<CodeBlock>) -> Self {
        if blocks.is_empty() {
            return Self {
                id: "empty".to_string(),
                blocks,
                similarity_score: 0.0,
                total_lines: 0,
                total_tokens: 0,
                impact_score: 0.0,
            };
        }

        let id = format!("dup_{}", blocks[0].content_hash);
        let total_lines = blocks.iter().map(|b| b.line_count()).sum();
        let total_tokens = blocks.iter().map(|b| b.token_count).sum();
        
        // Calculate average similarity between all pairs
        let mut similarity_sum = 0.0;
        let mut pair_count = 0;
        
        for i in 0..blocks.len() {
            for j in (i + 1)..blocks.len() {
                similarity_sum += blocks[i].similarity(&blocks[j]);
                pair_count += 1;
            }
        }
        
        let similarity_score = if pair_count > 0 {
            similarity_sum / pair_count as f64
        } else {
            1.0
        };

        // Calculate impact score based on duplication extent and complexity
        let impact_score = Self::calculate_impact_score(&blocks, total_lines, similarity_score);

        Self {
            id,
            blocks,
            similarity_score,
            total_lines,
            total_tokens,
            impact_score,
        }
    }

    fn calculate_impact_score(blocks: &[CodeBlock], total_lines: usize, similarity_score: f64) -> f64 {
        let duplication_factor = blocks.len() as f64;
        let size_factor = (total_lines as f64 / 10.0).min(10.0); // Cap at 10x
        let complexity_factor = blocks.iter()
            .map(|b| b.token_count as f64 / b.line_count().max(1) as f64)
            .sum::<f64>() / blocks.len() as f64;

        duplication_factor * size_factor * complexity_factor * similarity_score
    }

    pub fn get_involved_files(&self) -> Vec<String> {
        let mut files: Vec<String> = self.blocks.iter()
            .map(|b| b.file_path.clone())
            .collect();
        files.sort();
        files.dedup();
        files
    }

    pub fn get_file_count(&self) -> usize {
        self.get_involved_files().len()
    }
}

/// Summary of duplication analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicationSummary {
    pub total_files_analyzed: usize,
    pub total_lines_analyzed: usize,
    pub duplicate_groups: usize,
    pub total_duplicate_lines: usize,
    pub duplication_percentage: f64,
    pub average_similarity: f64,
    pub highest_impact_score: f64,
    pub files_with_duplicates: usize,
}

impl DuplicationSummary {
    pub fn new(
        total_files: usize,
        total_lines: usize,
        groups: &[DuplicateGroup],
    ) -> Self {
        let duplicate_groups = groups.len();
        let total_duplicate_lines = groups.iter().map(|g| g.total_lines).sum();
        let duplication_percentage = if total_lines > 0 {
            (total_duplicate_lines as f64 / total_lines as f64) * 100.0
        } else {
            0.0
        };

        let average_similarity = if duplicate_groups > 0 {
            groups.iter().map(|g| g.similarity_score).sum::<f64>() / duplicate_groups as f64
        } else {
            0.0
        };

        let highest_impact_score = groups.iter()
            .map(|g| g.impact_score)
            .fold(0.0, f64::max);

        let files_with_duplicates = groups.iter()
            .flat_map(|g| g.get_involved_files())
            .collect::<std::collections::HashSet<_>>()
            .len();

        Self {
            total_files_analyzed: total_files,
            total_lines_analyzed: total_lines,
            duplicate_groups,
            total_duplicate_lines,
            duplication_percentage,
            average_similarity,
            highest_impact_score,
            files_with_duplicates,
        }
    }
}

/// Duplication Detector Processor - can be used by any plugin
pub struct DuplicationDetectorProcessor {
    config: DuplicationConfig,
    file_contents: HashMap<String, String>,
    duplicate_groups: Vec<DuplicateGroup>,
    stats: ProcessorStats,
    shared_state: Option<Arc<SharedProcessorState>>,
}

impl DuplicationDetectorProcessor {
    pub fn new() -> Self {
        Self {
            config: DuplicationConfig::default(),
            file_contents: HashMap::new(),
            duplicate_groups: Vec::new(),
            stats: ProcessorStats::default(),
            shared_state: None,
        }
    }

    pub fn with_config(config: DuplicationConfig) -> Self {
        Self {
            config,
            file_contents: HashMap::new(),
            duplicate_groups: Vec::new(),
            stats: ProcessorStats::default(),
            shared_state: None,
        }
    }

    fn should_analyze_file(&self, file_path: &str) -> bool {
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        self.config.analyzed_extensions.contains(&extension)
    }

    /// Detect duplicates in the collected file contents
    pub fn detect_duplicates(&mut self) {
        self.duplicate_groups.clear();

        // Extract code blocks from all files
        let mut all_blocks = Vec::new();
        
        for (file_path, content) in &self.file_contents {
            if self.should_analyze_file(file_path) {
                let blocks = self.extract_code_blocks(file_path, content);
                all_blocks.extend(blocks);
            }
        }

        // Group similar blocks
        let groups = self.group_similar_blocks(all_blocks);
        
        // Filter groups by minimum size and similarity
        self.duplicate_groups = groups.into_iter()
            .filter(|g| g.blocks.len() >= 2)
            .filter(|g| g.similarity_score >= self.config.similarity_threshold)
            .filter(|g| g.total_lines >= self.config.min_block_size)
            .take(self.config.max_groups)
            .collect();

        // Sort by impact score
        self.duplicate_groups.sort_by(|a, b| b.impact_score.partial_cmp(&a.impact_score).unwrap());

        debug!("Detected {} duplicate groups", self.duplicate_groups.len());
    }

    fn extract_code_blocks(&self, file_path: &str, content: &str) -> Vec<CodeBlock> {
        let lines: Vec<&str> = content.lines().collect();
        let mut blocks = Vec::new();

        // Extract blocks of minimum size
        for start in 0..lines.len() {
            for end in (start + self.config.min_block_size - 1)..lines.len().min(start + 50) {
                let block_content = lines[start..=end].join("\n");
                let block = CodeBlock::new(
                    file_path.to_string(),
                    start + 1, // 1-based line numbers
                    end + 1,
                    &block_content,
                    &self.config,
                );
                blocks.push(block);
            }
        }

        blocks
    }

    fn group_similar_blocks(&self, blocks: Vec<CodeBlock>) -> Vec<DuplicateGroup> {
        let mut groups = Vec::new();
        let mut used_blocks = std::collections::HashSet::new();

        for (i, block) in blocks.iter().enumerate() {
            if used_blocks.contains(&i) {
                continue;
            }

            let mut similar_blocks = vec![block.clone()];
            used_blocks.insert(i);

            // Find similar blocks
            for (j, other_block) in blocks.iter().enumerate().skip(i + 1) {
                if used_blocks.contains(&j) {
                    continue;
                }

                if block.similarity(other_block) >= self.config.similarity_threshold {
                    similar_blocks.push(other_block.clone());
                    used_blocks.insert(j);
                }
            }

            if similar_blocks.len() >= 2 {
                groups.push(DuplicateGroup::new(similar_blocks));
            }
        }

        groups
    }

    /// Get the detected duplicate groups
    pub fn get_duplicate_groups(&self) -> &[DuplicateGroup] {
        &self.duplicate_groups
    }

    /// Generate duplication summary
    pub fn generate_summary(&self) -> DuplicationSummary {
        let total_files = self.file_contents.len();
        let total_lines = self.file_contents.values()
            .map(|content| content.lines().count())
            .sum();

        DuplicationSummary::new(total_files, total_lines, &self.duplicate_groups)
    }

    fn create_duplication_messages(&self) -> Vec<ScanMessage> {
        let mut messages = Vec::new();
        
        // Create messages for top duplicate groups
        for group in self.duplicate_groups.iter().take(10) {
            let header = MessageHeader::new(
                ScanMode::FILES,
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );

            // For now, use FileInfo - in a full implementation, we'd have a DuplicationInfo variant
            let data = MessageData::FileInfo {
                path: format!("duplicate_group_{}", group.id),
                size: group.total_lines as u64,
                lines: group.blocks.len() as u32,
            };

            messages.push(ScanMessage::new(header, data));
        }
        
        messages
    }
}

#[async_trait]
impl EventProcessor for DuplicationDetectorProcessor {
    fn supported_modes(&self) -> ScanMode {
        ScanMode::FILES
    }

    fn name(&self) -> &'static str {
        "duplication_detector"
    }

    fn set_shared_state(&mut self, shared_state: Arc<SharedProcessorState>) {
        self.shared_state = Some(shared_state);
    }

    fn shared_state(&self) -> Option<&Arc<SharedProcessorState>> {
        self.shared_state.as_ref()
    }

    async fn initialize(&mut self) -> PluginResult<()> {
        debug!("Initialized DuplicationDetectorProcessor with config: {:?}", self.config);
        Ok(())
    }

    async fn process_event(&mut self, event: &RepositoryEvent) -> PluginResult<Vec<ScanMessage>> {
        match event {
            RepositoryEvent::FileChanged { file_path, .. } => {
                if self.should_analyze_file(file_path) {
                    // In a full implementation, we would get the actual file content here
                    // For now, we'll store a placeholder
                    let placeholder_content = format!("// Content of {}\n// This would be actual file content", file_path);
                    self.file_contents.insert(file_path.clone(), placeholder_content);
                }
            }
            _ => {}
        }
        self.stats.events_processed += 1;
        Ok(vec![])
    }

    async fn finalize(&mut self) -> PluginResult<Vec<ScanMessage>> {
        // Perform duplication detection
        self.detect_duplicates();
        
        let messages = self.create_duplication_messages();
        self.stats.messages_generated = messages.len();
        
        let summary = self.generate_summary();
        debug!(
            "DuplicationDetectorProcessor finalized: {} files analyzed, {} duplicate groups found, {:.1}% duplication",
            summary.total_files_analyzed,
            summary.duplicate_groups,
            summary.duplication_percentage
        );
        
        Ok(messages)
    }

    fn get_stats(&self) -> ProcessorStats {
        self.stats.clone()
    }
}

impl Default for DuplicationDetectorProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_duplication_detector_processor_creation() {
        let processor = DuplicationDetectorProcessor::new();
        assert_eq!(processor.name(), "duplication_detector");
        assert_eq!(processor.supported_modes(), ScanMode::FILES);
        assert!(processor.file_contents.is_empty());
        assert!(processor.duplicate_groups.is_empty());
    }

    #[tokio::test]
    async fn test_code_block_creation() {
        let config = DuplicationConfig::default();
        let content = "fn main() {\n    println!(\"Hello\");\n}";
        
        let block = CodeBlock::new(
            "test.rs".to_string(),
            1,
            3,
            content,
            &config,
        );

        assert_eq!(block.file_path, "test.rs");
        assert_eq!(block.start_line, 1);
        assert_eq!(block.end_line, 3);
        assert_eq!(block.line_count(), 3);
        assert!(block.token_count > 0);
    }

    #[tokio::test]
    async fn test_code_block_similarity() {
        let config = DuplicationConfig::default();
        let content1 = "fn test() {\n    println!(\"test\");\n}";
        let content2 = "fn test() {\n    println!(\"test\");\n}";
        let content3 = "fn different() {\n    println!(\"different\");\n}";

        let block1 = CodeBlock::new("test1.rs".to_string(), 1, 3, content1, &config);
        let block2 = CodeBlock::new("test2.rs".to_string(), 1, 3, content2, &config);
        let block3 = CodeBlock::new("test3.rs".to_string(), 1, 3, content3, &config);

        assert_eq!(block1.similarity(&block2), 1.0); // Identical
        assert!(block1.similarity(&block3) < 1.0);   // Different
    }

    #[tokio::test]
    async fn test_duplicate_group_creation() {
        let config = DuplicationConfig::default();
        let content = "fn test() {\n    println!(\"test\");\n}";

        let block1 = CodeBlock::new("test1.rs".to_string(), 1, 3, content, &config);
        let block2 = CodeBlock::new("test2.rs".to_string(), 5, 7, content, &config);

        let group = DuplicateGroup::new(vec![block1, block2]);

        assert_eq!(group.blocks.len(), 2);
        assert_eq!(group.get_file_count(), 2);
        assert!(group.similarity_score > 0.8);
        assert!(group.impact_score > 0.0);
    }

    #[tokio::test]
    async fn test_should_analyze_file() {
        let processor = DuplicationDetectorProcessor::new();

        assert!(processor.should_analyze_file("main.rs"));
        assert!(processor.should_analyze_file("script.py"));
        assert!(processor.should_analyze_file("app.js"));
        assert!(!processor.should_analyze_file("image.png"));
        assert!(!processor.should_analyze_file("data.txt"));
    }

    #[tokio::test]
    async fn test_duplication_summary() {
        let config = DuplicationConfig::default();
        let content = "fn test() {\n    println!(\"test\");\n}";

        let block1 = CodeBlock::new("test1.rs".to_string(), 1, 3, content, &config);
        let block2 = CodeBlock::new("test2.rs".to_string(), 1, 3, content, &config);
        let group = DuplicateGroup::new(vec![block1, block2]);

        let summary = DuplicationSummary::new(10, 1000, &[group]);

        assert_eq!(summary.total_files_analyzed, 10);
        assert_eq!(summary.total_lines_analyzed, 1000);
        assert_eq!(summary.duplicate_groups, 1);
        assert!(summary.duplication_percentage > 0.0);
        assert_eq!(summary.files_with_duplicates, 2);
    }

    #[tokio::test]
    async fn test_file_processing() {
        let mut processor = DuplicationDetectorProcessor::new();
        processor.initialize().await.unwrap();

        let event = RepositoryEvent::FileChanged {
            file_path: "src/main.rs".to_string(),
            change_data: crate::scanner::async_engine::events::FileChangeData {
                change_type: crate::scanner::async_engine::events::ChangeType::Modified,
                old_path: Some("src/main.rs".to_string()),
                new_path: "src/main.rs".to_string(),
                insertions: 10,
                deletions: 2,
                is_binary: false,
            },
            commit_context: crate::scanner::async_engine::events::CommitInfo {
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
                insertions: 0,
                deletions: 0,
            },
        };

        let messages = processor.process_event(&event).await.unwrap();
        assert!(messages.is_empty()); // No messages during processing

        assert_eq!(processor.file_contents.len(), 1);
        assert!(processor.file_contents.contains_key("src/main.rs"));
    }
}
