//! Smart Suggestion Engine for "Did You Mean?" Functionality
//! 
//! Provides intelligent suggestions for misspelled commands, plugins, and functions
//! using string similarity algorithms.

use std::collections::HashMap;

/// Configuration for suggestion generation
#[derive(Debug, Clone)]
pub struct SuggestionConfig {
    /// Maximum edit distance for suggestions (lower = more strict)
    pub max_edit_distance: usize,
    /// Minimum similarity threshold (0.0 - 1.0, higher = more strict)
    pub min_similarity: f64,
    /// Maximum number of suggestions to return
    pub max_suggestions: usize,
}

impl Default for SuggestionConfig {
    fn default() -> Self {
        Self {
            max_edit_distance: 2,
            min_similarity: 0.6,
            max_suggestions: 3,
        }
    }
}

/// Smart suggestion engine using multiple string similarity algorithms
pub struct SuggestionEngine {
    config: SuggestionConfig,
    commands: Vec<String>,
    plugins: Vec<String>,
    functions: Vec<String>,
}

impl SuggestionEngine {
    /// Create a new suggestion engine
    pub fn new(config: SuggestionConfig) -> Self {
        Self {
            config,
            commands: Vec::new(),
            plugins: Vec::new(),
            functions: Vec::new(),
        }
    }

    /// Update available commands for suggestions
    pub fn update_commands(&mut self, plugins: &[String], functions: &[String]) {
        self.plugins = plugins.to_vec();
        self.functions = functions.to_vec();
        
        // Combine all possible commands
        self.commands.clear();
        self.commands.extend_from_slice(plugins);
        self.commands.extend_from_slice(functions);
        
        // Sort for consistent results
        self.commands.sort();
        self.commands.dedup();
    }

    /// Get suggestions for an unknown command
    pub fn suggest(&self, input: &str) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();
        
        // Generate suggestions from all available commands
        for candidate in &self.commands {
            // Skip exact matches - if they typed it exactly, the command exists
            if input.eq_ignore_ascii_case(candidate) {
                continue;
            }
            
            let similarity = self.calculate_similarity(input, candidate);
            let edit_distance = levenshtein_distance(input, candidate);
            
            if similarity >= self.config.min_similarity && edit_distance <= self.config.max_edit_distance {
                let suggestion_type = if self.plugins.contains(candidate) {
                    SuggestionType::Plugin
                } else {
                    SuggestionType::Function
                };
                
                suggestions.push(Suggestion {
                    text: candidate.clone(),
                    similarity,
                    edit_distance,
                    suggestion_type,
                });
            }
        }
        
        // Sort by similarity (highest first), then by edit distance (lowest first)
        suggestions.sort_by(|a, b| {
            b.similarity.partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.edit_distance.cmp(&b.edit_distance))
        });
        
        // Limit results
        suggestions.truncate(self.config.max_suggestions);
        suggestions
    }

    /// Calculate similarity using multiple algorithms (average score)
    fn calculate_similarity(&self, a: &str, b: &str) -> f64 {
        let jaro_winkler = jaro_winkler_similarity(a, b);
        let dice = dice_coefficient(a, b);
        let normalized_levenshtein = normalized_levenshtein_similarity(a, b);
        
        // Weighted average - Jaro-Winkler is best for typos, others for context
        jaro_winkler * 0.5 + dice * 0.3 + normalized_levenshtein * 0.2
    }
}

/// Type of suggestion
#[derive(Debug, Clone, PartialEq)]
pub enum SuggestionType {
    Plugin,
    Function,
}

/// A single suggestion with metadata
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub text: String,
    pub similarity: f64,
    pub edit_distance: usize,
    pub suggestion_type: SuggestionType,
}

impl Suggestion {
    /// Get a formatted suggestion message
    pub fn format_message(&self) -> String {
        match self.suggestion_type {
            SuggestionType::Plugin => format!("Did you mean the plugin '{}'?", self.text),
            SuggestionType::Function => format!("Did you mean the function '{}'?", self.text),
        }
    }
}

/// Calculate Levenshtein distance between two strings
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();
    
    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }
    
    let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];
    
    // Initialize first row and column
    for i in 0..=a_len {
        matrix[i][0] = i;
    }
    for j in 0..=b_len {
        matrix[0][j] = j;
    }
    
    // Fill the matrix
    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            matrix[i][j] = std::cmp::min(
                std::cmp::min(
                    matrix[i - 1][j] + 1,     // deletion
                    matrix[i][j - 1] + 1      // insertion
                ),
                matrix[i - 1][j - 1] + cost   // substitution
            );
        }
    }
    
    matrix[a_len][b_len]
}

/// Calculate normalized Levenshtein similarity (0.0 - 1.0)
pub fn normalized_levenshtein_similarity(a: &str, b: &str) -> f64 {
    let max_len = std::cmp::max(a.len(), b.len());
    if max_len == 0 {
        return 1.0;
    }
    
    let distance = levenshtein_distance(a, b);
    1.0 - (distance as f64 / max_len as f64)
}

/// Calculate Jaro-Winkler similarity
pub fn jaro_winkler_similarity(a: &str, b: &str) -> f64 {
    let jaro = jaro_similarity(a, b);
    if jaro < 0.7 {
        return jaro;
    }
    
    // Calculate common prefix (up to 4 characters)
    let prefix_len = a.chars()
        .zip(b.chars())
        .take(4)
        .take_while(|(a_char, b_char)| a_char == b_char)
        .count();
    
    jaro + (0.1 * prefix_len as f64 * (1.0 - jaro))
}

/// Calculate Jaro similarity
pub fn jaro_similarity(a: &str, b: &str) -> f64 {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();
    
    if a_len == 0 && b_len == 0 {
        return 1.0;
    }
    if a_len == 0 || b_len == 0 {
        return 0.0;
    }
    
    let match_window = std::cmp::max(a_len, b_len) / 2;
    if match_window == 0 {
        return if a_chars[0] == b_chars[0] { 1.0 } else { 0.0 };
    }
    let match_window = match_window - 1;
    
    let mut a_matches = vec![false; a_len];
    let mut b_matches = vec![false; b_len];
    
    let mut matches = 0;
    
    // Identify matches
    for i in 0..a_len {
        let start = if i >= match_window { i - match_window } else { 0 };
        let end = std::cmp::min(i + match_window + 1, b_len);
        
        for j in start..end {
            if b_matches[j] || a_chars[i] != b_chars[j] {
                continue;
            }
            a_matches[i] = true;
            b_matches[j] = true;
            matches += 1;
            break;
        }
    }
    
    if matches == 0 {
        return 0.0;
    }
    
    // Count transpositions
    let mut transpositions = 0;
    let mut k = 0;
    for i in 0..a_len {
        if !a_matches[i] {
            continue;
        }
        while !b_matches[k] {
            k += 1;
        }
        if a_chars[i] != b_chars[k] {
            transpositions += 1;
        }
        k += 1;
    }
    
    let m = matches as f64;
    (m / a_len as f64 + m / b_len as f64 + (m - transpositions as f64 / 2.0) / m) / 3.0
}

/// Calculate Dice coefficient (Sørensen-Dice index)
pub fn dice_coefficient(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    
    let a_bigrams = get_bigrams(a);
    let b_bigrams = get_bigrams(b);
    
    if a_bigrams.is_empty() && b_bigrams.is_empty() {
        return 1.0;
    }
    if a_bigrams.is_empty() || b_bigrams.is_empty() {
        return 0.0;
    }
    
    let mut intersection = 0;
    let mut a_counts = HashMap::new();
    let mut b_counts = HashMap::new();
    
    for bigram in &a_bigrams {
        *a_counts.entry(bigram).or_insert(0) += 1;
    }
    for bigram in &b_bigrams {
        *b_counts.entry(bigram).or_insert(0) += 1;
    }
    
    for (bigram, count) in &a_counts {
        if let Some(b_count) = b_counts.get(bigram) {
            intersection += std::cmp::min(*count, *b_count);
        }
    }
    
    (2.0 * intersection as f64) / (a_bigrams.len() + b_bigrams.len()) as f64
}

/// Extract bigrams from a string
fn get_bigrams(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 2 {
        return vec![];
    }
    
    chars.windows(2)
        .map(|window| window.iter().collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("", "abc"), 3);
        assert_eq!(levenshtein_distance("abc", ""), 3);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
        assert_eq!(levenshtein_distance("abc", "ab"), 1);
        assert_eq!(levenshtein_distance("abc", "abcd"), 1);
        assert_eq!(levenshtein_distance("abc", "axc"), 1);
        assert_eq!(levenshtein_distance("commits", "comits"), 1);
        assert_eq!(levenshtein_distance("metrics", "metriks"), 1);
    }

    #[test]
    fn test_jaro_winkler_similarity() {
        assert!((jaro_winkler_similarity("", "") - 1.0).abs() < 0.01);
        assert!((jaro_winkler_similarity("abc", "abc") - 1.0).abs() < 0.01);
        assert!(jaro_winkler_similarity("", "abc") < 0.01);
        
        // Test common typos
        assert!(jaro_winkler_similarity("commits", "comits") > 0.8);
        assert!(jaro_winkler_similarity("metrics", "metriks") > 0.8);
        assert!(jaro_winkler_similarity("export", "exprt") > 0.7);
    }

    #[test]
    fn test_dice_coefficient() {
        assert!((dice_coefficient("", "") - 1.0).abs() < 0.01);
        assert!((dice_coefficient("abc", "abc") - 1.0).abs() < 0.01);
        assert!(dice_coefficient("", "abc") < 0.01);
        
        // Test bigram overlap
        assert!(dice_coefficient("commits", "commit") > 0.8);
        assert!(dice_coefficient("metrics", "metric") > 0.8);
    }

    #[test]
    fn test_suggestion_engine() {
        let config = SuggestionConfig::default();
        let mut engine = SuggestionEngine::new(config);
        
        let plugins = vec!["commits".to_string(), "metrics".to_string(), "export".to_string()];
        let functions = vec!["analyze".to_string(), "complexity".to_string()];
        engine.update_commands(&plugins, &functions);
        
        // Test typo suggestions
        let suggestions = engine.suggest("comits");
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].text, "commits");
        assert_eq!(suggestions[0].suggestion_type, SuggestionType::Plugin);
        
        let suggestions = engine.suggest("metriks");
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].text, "metrics");
        
        // Test partial matches
        let suggestions = engine.suggest("analyz");
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.text == "analyze"));
    }

    #[test]
    fn test_suggestion_formatting() {
        let plugin_suggestion = Suggestion {
            text: "commits".to_string(),
            similarity: 0.9,
            edit_distance: 1,
            suggestion_type: SuggestionType::Plugin,
        };
        
        assert!(plugin_suggestion.format_message().contains("plugin"));
        assert!(plugin_suggestion.format_message().contains("commits"));
        
        let function_suggestion = Suggestion {
            text: "analyze".to_string(),
            similarity: 0.9,
            edit_distance: 1,
            suggestion_type: SuggestionType::Function,
        };
        
        assert!(function_suggestion.format_message().contains("function"));
        assert!(function_suggestion.format_message().contains("analyze"));
    }

    #[test]
    fn test_edge_cases() {
        let config = SuggestionConfig {
            max_edit_distance: 1,
            min_similarity: 0.8,
            max_suggestions: 2,
        };
        let mut engine = SuggestionEngine::new(config);
        
        let plugins = vec!["test".to_string()];
        let functions = vec!["function".to_string()];
        engine.update_commands(&plugins, &functions);
        
        // Test with empty input
        let suggestions = engine.suggest("");
        assert!(suggestions.is_empty());
        
        // Test with exact match (shouldn't suggest itself)
        let suggestions = engine.suggest("test");
        assert!(suggestions.is_empty());
        
        // Test with very different string
        let suggestions = engine.suggest("completely_different");
        assert!(suggestions.is_empty());
    }
}