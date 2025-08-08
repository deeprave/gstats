//! Code Duplication Detection
//! 
//! Detects duplicate and similar code patterns using token-based analysis.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use serde::{Serialize, Deserialize};
use regex::Regex;
use log::{debug, info};

/// Configuration for duplication detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicationConfig {
    /// Minimum number of tokens to consider for duplication
    pub min_tokens: usize,
    /// Minimum similarity percentage to consider as duplicate (0.0 to 1.0)
    pub similarity_threshold: f64,
    /// Maximum number of lines to skip between tokens for similarity
    pub max_line_gap: usize,
    /// Whether to ignore whitespace and comments in comparison
    pub ignore_whitespace: bool,
    /// File extensions to include in analysis
    pub included_extensions: Vec<String>,
    /// Maximum file size to analyze (in bytes)
    pub max_file_size: usize,
}

impl Default for DuplicationConfig {
    fn default() -> Self {
        Self {
            min_tokens: 50,           // Minimum 50 tokens for meaningful duplication
            similarity_threshold: 0.8, // 80% similarity threshold
            max_line_gap: 5,          // Allow up to 5 lines gap
            ignore_whitespace: true,
            included_extensions: vec![
                "rs".to_string(), "py".to_string(), "js".to_string(), "ts".to_string(),
                "java".to_string(), "c".to_string(), "cpp".to_string(), "go".to_string(),
                "rb".to_string(), "php".to_string(), "cs".to_string()
            ],
            max_file_size: 1_048_576, // 1MB max file size
        }
    }
}

/// A tokenized code block for comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub tokens: Vec<String>,
    pub hash: u64,
    pub raw_content: String,
}

impl CodeBlock {
    /// Create a new code block
    pub fn new(file_path: String, start_line: usize, end_line: usize, tokens: Vec<String>, raw_content: String) -> Self {
        let hash = Self::calculate_hash(&tokens);
        Self {
            file_path,
            start_line,
            end_line,
            tokens,
            hash,
            raw_content,
        }
    }
    
    /// Calculate hash for the token sequence
    fn calculate_hash(tokens: &[String]) -> u64 {
        let mut hasher = DefaultHasher::new();
        for token in tokens {
            token.hash(&mut hasher);
        }
        hasher.finish()
    }
    
    /// Calculate similarity with another code block
    pub fn similarity(&self, other: &CodeBlock) -> f64 {
        if self.tokens.len() == 0 || other.tokens.len() == 0 {
            return 0.0;
        }
        
        // Use Jaccard similarity: |intersection| / |union|
        let tokens1: HashSet<&String> = self.tokens.iter().collect();
        let tokens2: HashSet<&String> = other.tokens.iter().collect();
        
        let intersection_size = tokens1.intersection(&tokens2).count();
        let union_size = tokens1.union(&tokens2).count();
        
        intersection_size as f64 / union_size as f64
    }
    
    /// Get the size of this block in lines
    pub fn line_count(&self) -> usize {
        if self.end_line >= self.start_line {
            self.end_line - self.start_line + 1
        } else {
            1
        }
    }
    
    /// Get the size of this block in tokens
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }
}

/// Information about detected duplicate code
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
    /// Create a new duplicate group
    pub fn new(id: String, blocks: Vec<CodeBlock>) -> Self {
        let similarity_score = Self::calculate_group_similarity(&blocks);
        let total_lines = blocks.iter().map(|b| b.line_count()).sum();
        let total_tokens = blocks.iter().map(|b| b.token_count()).sum();
        let impact_score = Self::calculate_impact_score(&blocks, similarity_score);
        
        Self {
            id,
            blocks,
            similarity_score,
            total_lines,
            total_tokens,
            impact_score,
        }
    }
    
    /// Calculate average similarity within the group
    fn calculate_group_similarity(blocks: &[CodeBlock]) -> f64 {
        if blocks.len() < 2 {
            return 1.0;
        }
        
        let mut total_similarity = 0.0;
        let mut comparisons = 0;
        
        for i in 0..blocks.len() {
            for j in (i + 1)..blocks.len() {
                total_similarity += blocks[i].similarity(&blocks[j]);
                comparisons += 1;
            }
        }
        
        if comparisons > 0 {
            total_similarity / comparisons as f64
        } else {
            1.0
        }
    }
    
    /// Calculate impact score based on size and frequency
    fn calculate_impact_score(blocks: &[CodeBlock], similarity: f64) -> f64 {
        let avg_lines = blocks.iter().map(|b| b.line_count()).sum::<usize>() as f64 / blocks.len() as f64;
        let frequency = blocks.len() as f64;
        
        // Impact = average_size * frequency * similarity
        avg_lines * frequency * similarity
    }
    
    /// Get files involved in this duplication group
    pub fn get_involved_files(&self) -> HashSet<String> {
        self.blocks.iter().map(|b| b.file_path.clone()).collect()
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

/// Code duplication detector
pub struct DuplicationDetector {
    config: DuplicationConfig,
    tokenizer: CodeTokenizer,
}

impl DuplicationDetector {
    /// Create a new duplication detector
    pub fn new(config: DuplicationConfig) -> Self {
        Self {
            tokenizer: CodeTokenizer::new(),
            config,
        }
    }
    
    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(DuplicationConfig::default())
    }
    
    /// Analyze files for code duplication
    pub fn detect_duplicates(&self, file_contents: &HashMap<String, String>) -> Vec<DuplicateGroup> {
        info!("Starting duplication detection on {} files", file_contents.len());
        
        // Step 1: Tokenize all files and create code blocks
        let mut all_blocks = Vec::new();
        
        for (file_path, content) in file_contents {
            if !self.should_analyze_file(file_path, content) {
                continue;
            }
            
            let blocks = self.tokenize_file(file_path, content);
            all_blocks.extend(blocks);
        }
        
        info!("Generated {} code blocks for analysis", all_blocks.len());
        
        // Step 2: Group similar blocks
        let duplicate_groups = self.find_duplicate_groups(all_blocks);
        
        info!("Found {} duplicate groups", duplicate_groups.len());
        
        duplicate_groups
    }
    
    /// Check if file should be analyzed
    fn should_analyze_file(&self, file_path: &str, content: &str) -> bool {
        // Check file extension
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
            
        if !self.config.included_extensions.contains(&extension.to_string()) {
            return false;
        }
        
        // Check file size
        if content.len() > self.config.max_file_size {
            debug!("Skipping large file: {} ({} bytes)", file_path, content.len());
            return false;
        }
        
        true
    }
    
    /// Tokenize a file into code blocks
    fn tokenize_file(&self, file_path: &str, content: &str) -> Vec<CodeBlock> {
        let lines: Vec<&str> = content.lines().collect();
        let mut blocks = Vec::new();
        
        // Create overlapping windows of code
        let window_size = self.config.min_tokens * 2; // Lines per window
        let step_size = self.config.min_tokens / 2;   // Overlap
        
        for start in (0..lines.len()).step_by(step_size) {
            let end = std::cmp::min(start + window_size, lines.len());
            
            if end - start < self.config.min_tokens / 4 {
                break; // Not enough content for meaningful analysis
            }
            
            let window_content = lines[start..end].join("\n");
            let tokens = self.tokenizer.tokenize(&window_content);
            
            if tokens.len() >= self.config.min_tokens {
                let block = CodeBlock::new(
                    file_path.to_string(),
                    start + 1, // 1-based line numbers
                    end,
                    tokens,
                    window_content,
                );
                blocks.push(block);
            }
        }
        
        debug!("Tokenized {} into {} blocks", file_path, blocks.len());
        blocks
    }
    
    /// Find groups of duplicate blocks
    fn find_duplicate_groups(&self, blocks: Vec<CodeBlock>) -> Vec<DuplicateGroup> {
        let mut groups = Vec::new();
        let mut processed = HashSet::new();
        
        for i in 0..blocks.len() {
            if processed.contains(&i) {
                continue;
            }
            
            let mut group_blocks = vec![blocks[i].clone()];
            processed.insert(i);
            
            // Find similar blocks
            for j in (i + 1)..blocks.len() {
                if processed.contains(&j) {
                    continue;
                }
                
                let similarity = blocks[i].similarity(&blocks[j]);
                if similarity >= self.config.similarity_threshold {
                    group_blocks.push(blocks[j].clone());
                    processed.insert(j);
                }
            }
            
            // Only create group if we have actual duplicates
            if group_blocks.len() > 1 {
                let group_id = format!("dup_{}", groups.len() + 1);
                let group = DuplicateGroup::new(group_id, group_blocks);
                groups.push(group);
            }
        }
        
        // Sort groups by impact score (highest first)
        groups.sort_by(|a, b| b.impact_score.partial_cmp(&a.impact_score).unwrap_or(std::cmp::Ordering::Equal));
        
        groups
    }
    
    /// Generate analysis summary
    pub fn generate_summary(&self, file_contents: &HashMap<String, String>, duplicate_groups: &[DuplicateGroup]) -> DuplicationSummary {
        let total_files_analyzed = file_contents.iter()
            .filter(|(path, content)| self.should_analyze_file(path, content))
            .count();
            
        let total_lines_analyzed: usize = file_contents.iter()
            .filter(|(path, content)| self.should_analyze_file(path, content))
            .map(|(_, content)| content.lines().count())
            .sum();
            
        let total_duplicate_lines: usize = duplicate_groups.iter()
            .map(|g| g.total_lines)
            .sum();
            
        let duplication_percentage = if total_lines_analyzed > 0 {
            (total_duplicate_lines as f64 / total_lines_analyzed as f64) * 100.0
        } else {
            0.0
        };
        
        let average_similarity = if duplicate_groups.is_empty() {
            0.0
        } else {
            duplicate_groups.iter().map(|g| g.similarity_score).sum::<f64>() / duplicate_groups.len() as f64
        };
        
        let highest_impact_score = duplicate_groups.iter()
            .map(|g| g.impact_score)
            .fold(0.0_f64, |max, score| max.max(score));
            
        let files_with_duplicates = duplicate_groups.iter()
            .flat_map(|g| g.get_involved_files())
            .collect::<HashSet<_>>()
            .len();
        
        DuplicationSummary {
            total_files_analyzed,
            total_lines_analyzed,
            duplicate_groups: duplicate_groups.len(),
            total_duplicate_lines,
            duplication_percentage,
            average_similarity,
            highest_impact_score,
            files_with_duplicates,
        }
    }
}

/// Simple code tokenizer
pub struct CodeTokenizer {
    // Regex patterns for different token types
    identifier_pattern: Regex,
    keyword_pattern: Regex,
    operator_pattern: Regex,
    literal_pattern: Regex,
}

impl CodeTokenizer {
    pub fn new() -> Self {
        Self {
            identifier_pattern: Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]*").unwrap(),
            keyword_pattern: Regex::new(r"\b(if|else|for|while|function|class|def|return|import|export|var|let|const|struct|enum|impl|trait|fn|pub|use|mod)\b").unwrap(),
            operator_pattern: Regex::new(r"[+\-*/=<>!&|^%~]+|==|!=|<=|>=|&&|\|\||<<|>>|\+=|-=|\*=|/=").unwrap(),
            literal_pattern: Regex::new(r#"(\d+\.?\d*|"[^"]*"|'[^']*')"#).unwrap(),
        }
    }
    
    /// Tokenize code into meaningful tokens
    pub fn tokenize(&self, code: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        
        // Remove comments and normalize whitespace
        let cleaned_code = self.clean_code(code);
        
        // Extract tokens using regex patterns
        for line in cleaned_code.lines() {
            // Keywords
            for mat in self.keyword_pattern.find_iter(line) {
                tokens.push(mat.as_str().to_string());
            }
            
            // Operators
            for mat in self.operator_pattern.find_iter(line) {
                tokens.push(mat.as_str().to_string());
            }
            
            // Literals (simplified to just "LITERAL" to focus on structure)
            for _mat in self.literal_pattern.find_iter(line) {
                tokens.push("LITERAL".to_string());
            }
            
            // Identifiers (simplified to "ID" to focus on structure)
            for _mat in self.identifier_pattern.find_iter(line) {
                tokens.push("ID".to_string());
            }
            
            // Structural tokens
            for ch in line.chars() {
                match ch {
                    '{' | '}' | '(' | ')' | '[' | ']' | ';' | ',' | '.' => {
                        tokens.push(ch.to_string());
                    }
                    _ => {}
                }
            }
        }
        
        tokens
    }
    
    /// Clean code by removing comments and normalizing whitespace
    fn clean_code(&self, code: &str) -> String {
        let mut cleaned = String::new();
        let mut in_string = false;
        let mut in_line_comment = false;
        let mut in_block_comment = false;
        let mut chars = code.chars().peekable();
        
        while let Some(ch) = chars.next() {
            if in_line_comment {
                if ch == '\n' {
                    in_line_comment = false;
                    cleaned.push(ch);
                }
                continue;
            }
            
            if in_block_comment {
                if ch == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    in_block_comment = false;
                }
                continue;
            }
            
            if in_string {
                if ch == '"' || ch == '\'' {
                    in_string = false;
                }
                cleaned.push(ch);
                continue;
            }
            
            match ch {
                '"' | '\'' => {
                    in_string = true;
                    cleaned.push(ch);
                }
                '/' if chars.peek() == Some(&'/') => {
                    in_line_comment = true;
                    chars.next();
                }
                '/' if chars.peek() == Some(&'*') => {
                    in_block_comment = true;
                    chars.next();
                }
                _ => {
                    cleaned.push(ch);
                }
            }
        }
        
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_duplication_config_default() {
        let config = DuplicationConfig::default();
        assert_eq!(config.min_tokens, 50);
        assert_eq!(config.similarity_threshold, 0.8);
        assert!(config.ignore_whitespace);
        assert!(config.included_extensions.contains(&"rs".to_string()));
    }
    
    #[test]
    fn test_code_tokenizer() {
        let tokenizer = CodeTokenizer::new();
        let code = r#"
        function test() {
            if (x == 5) {
                return "hello";
            }
        }
        "#;
        
        let tokens = tokenizer.tokenize(code);
        assert!(!tokens.is_empty());
        assert!(tokens.contains(&"function".to_string()));
        assert!(tokens.contains(&"if".to_string()));
        assert!(tokens.contains(&"==".to_string()));
        assert!(tokens.contains(&"return".to_string()));
    }
    
    #[test]
    fn test_code_block_similarity() {
        let tokens1 = vec!["function".to_string(), "test".to_string(), "{".to_string()];
        let tokens2 = vec!["function".to_string(), "demo".to_string(), "{".to_string()];
        let tokens3 = vec!["if".to_string(), "else".to_string(), "return".to_string()];
        
        let block1 = CodeBlock::new("file1.rs".to_string(), 1, 3, tokens1, "content1".to_string());
        let block2 = CodeBlock::new("file2.rs".to_string(), 1, 3, tokens2, "content2".to_string());
        let block3 = CodeBlock::new("file3.rs".to_string(), 1, 3, tokens3, "content3".to_string());
        
        let similarity_12 = block1.similarity(&block2);
        let similarity_13 = block1.similarity(&block3);
        
        assert!(similarity_12 > similarity_13); // block1 and block2 should be more similar
        assert!(similarity_12 > 0.0);
        assert!(similarity_13 >= 0.0);
    }
    
    #[test]
    fn test_duplicate_group_creation() {
        let tokens = vec!["test".to_string(), "function".to_string()];
        let block1 = CodeBlock::new("file1.rs".to_string(), 1, 2, tokens.clone(), "content".to_string());
        let block2 = CodeBlock::new("file2.rs".to_string(), 5, 6, tokens, "content".to_string());
        
        let group = DuplicateGroup::new("test_group".to_string(), vec![block1, block2]);
        
        assert_eq!(group.id, "test_group");
        assert_eq!(group.blocks.len(), 2);
        assert!(group.similarity_score > 0.0);
        assert_eq!(group.total_lines, 4); // 2 lines per block
    }
    
    #[test]
    fn test_duplication_detector_creation() {
        let detector = DuplicationDetector::with_defaults();
        assert_eq!(detector.config.min_tokens, 50);
        assert_eq!(detector.config.similarity_threshold, 0.8);
    }
    
    #[test]
    fn test_should_analyze_file() {
        let detector = DuplicationDetector::with_defaults();
        
        assert!(detector.should_analyze_file("test.rs", "fn main() {}"));
        assert!(detector.should_analyze_file("test.py", "def test(): pass"));
        assert!(!detector.should_analyze_file("test.txt", "plain text"));
        
        // Test large file
        let large_content = "x".repeat(2_000_000);
        assert!(!detector.should_analyze_file("large.rs", &large_content));
    }
    
    #[test]
    fn test_clean_code() {
        let tokenizer = CodeTokenizer::new();
        let code = r#"
        // This is a comment
        function test() { /* block comment */ 
            return "hello"; // another comment
        }
        "#;
        
        let cleaned = tokenizer.clean_code(code);
        assert!(!cleaned.contains("This is a comment"));
        assert!(!cleaned.contains("block comment"));
        assert!(!cleaned.contains("another comment"));
        assert!(cleaned.contains("function test()"));
        assert!(cleaned.contains("return \"hello\";"));
    }
}