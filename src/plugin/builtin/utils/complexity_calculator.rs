//! Cyclomatic Complexity Calculator
//! 
//! Calculates cyclomatic complexity for various programming languages.

use std::fs;
use std::path::Path;

/// Language-specific cyclomatic complexity calculator
pub struct ComplexityCalculator {
    /// Supported file extensions and their complexity patterns
    language_patterns: std::collections::HashMap<String, Vec<&'static str>>,
}

impl Default for ComplexityCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl ComplexityCalculator {
    /// Create a new complexity calculator with default language patterns
    pub fn new() -> Self {
        let mut language_patterns = std::collections::HashMap::new();
        
        // Rust patterns
        language_patterns.insert("rs".to_string(), vec![
            "if ", "else if ", "while ", "for ", "loop ", "match ", "?", "&&", "||"
        ]);
        
        // Python patterns
        language_patterns.insert("py".to_string(), vec![
            "if ", "elif ", "while ", "for ", "try:", "except", "and ", "or ", "with "
        ]);
        
        // JavaScript/TypeScript patterns
        language_patterns.insert("js".to_string(), vec![
            "if ", "else if ", "while ", "for ", "switch ", "case ", "try ", "catch ", "&&", "||", "?"
        ]);
        language_patterns.insert("ts".to_string(), vec![
            "if ", "else if ", "while ", "for ", "switch ", "case ", "try ", "catch ", "&&", "||", "?"
        ]);
        
        // Java patterns
        language_patterns.insert("java".to_string(), vec![
            "if ", "else if ", "while ", "for ", "do ", "switch ", "case ", "try ", "catch ", "&&", "||", "?"
        ]);
        
        // C/C++ patterns
        language_patterns.insert("c".to_string(), vec![
            "if ", "else if ", "while ", "for ", "do ", "switch ", "case ", "&&", "||", "?"
        ]);
        language_patterns.insert("cpp".to_string(), vec![
            "if ", "else if ", "while ", "for ", "do ", "switch ", "case ", "try ", "catch ", "&&", "||", "?"
        ]);
        
        // Go patterns
        language_patterns.insert("go".to_string(), vec![
            "if ", "for ", "switch ", "case ", "select ", "&&", "||"
        ]);
        
        Self {
            language_patterns,
        }
    }
    
    /// Calculate cyclomatic complexity for a file
    pub fn calculate_complexity(&self, file_path: &str) -> Result<usize, Box<dyn std::error::Error>> {
        let path = Path::new(file_path);
        
        // Get file extension
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();
            
        // If we don't support this language, return default complexity of 1
        let patterns = match self.language_patterns.get(&extension) {
            Some(patterns) => patterns,
            None => return Ok(1),
        };
        
        // Read file content
        let content = fs::read_to_string(file_path)?;
        
        // Calculate complexity
        self.calculate_complexity_from_content(&content, patterns)
    }
    
    /// Calculate complexity from file content and language patterns
    fn calculate_complexity_from_content(&self, content: &str, patterns: &[&str]) -> Result<usize, Box<dyn std::error::Error>> {
        // Start with base complexity of 1
        let mut complexity = 1;
        
        // Remove comments and strings to avoid false positives
        let mut cleaned_content = self.remove_comments_and_strings(content);
        
        // Sort patterns by length (longest first) to avoid substring conflicts
        let mut sorted_patterns: Vec<&str> = patterns.to_vec();
        sorted_patterns.sort_by_key(|p| std::cmp::Reverse(p.len()));
        
        // Count occurrences of complexity patterns, processing longer patterns first
        for pattern in sorted_patterns {
            let count = cleaned_content.matches(pattern).count();
            complexity += count;
            
            // Replace all occurrences of this pattern with spaces to avoid double-counting
            let replacement = " ".repeat(pattern.len());
            cleaned_content = cleaned_content.replace(pattern, &replacement);
        }
        
        Ok(complexity)
    }
    
    /// Remove comments and string literals to avoid counting patterns within them
    fn remove_comments_and_strings(&self, content: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = content.chars().collect();
        let mut i = 0;
        
        while i < chars.len() {
            match chars[i] {
                // Handle single-line comments (//)
                '/' if i + 1 < chars.len() && chars[i + 1] == '/' => {
                    // Skip until end of line
                    while i < chars.len() && chars[i] != '\n' {
                        i += 1;
                    }
                    if i < chars.len() {
                        result.push(chars[i]); // Keep the newline
                        i += 1;
                    }
                }
                // Handle multi-line comments (/* */)
                '/' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                    i += 2; // Skip /*
                    // Skip until */
                    while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                        if chars[i] == '\n' {
                            result.push('\n'); // Preserve line breaks for line-based analysis
                        }
                        i += 1;
                    }
                    if i + 1 < chars.len() {
                        i += 2; // Skip */
                    }
                }
                // Handle Python comments (#)
                '#' => {
                    // Skip until end of line
                    while i < chars.len() && chars[i] != '\n' {
                        i += 1;
                    }
                    if i < chars.len() {
                        result.push(chars[i]); // Keep the newline
                        i += 1;
                    }
                }
                // Handle string literals (")
                '"' => {
                    i += 1; // Skip opening quote
                    // Skip until closing quote, handling escaped quotes
                    while i < chars.len() && chars[i] != '"' {
                        if chars[i] == '\\' && i + 1 < chars.len() {
                            i += 2; // Skip escaped character
                        } else {
                            if chars[i] == '\n' {
                                result.push('\n'); // Preserve line breaks
                            }
                            i += 1;
                        }
                    }
                    if i < chars.len() {
                        i += 1; // Skip closing quote
                    }
                }
                // Handle string literals (')
                '\'' => {
                    i += 1; // Skip opening quote
                    // Skip until closing quote, handling escaped quotes
                    while i < chars.len() && chars[i] != '\'' {
                        if chars[i] == '\\' && i + 1 < chars.len() {
                            i += 2; // Skip escaped character
                        } else {
                            if chars[i] == '\n' {
                                result.push('\n'); // Preserve line breaks
                            }
                            i += 1;
                        }
                    }
                    if i < chars.len() {
                        i += 1; // Skip closing quote
                    }
                }
                // Regular character
                _ => {
                    result.push(chars[i]);
                    i += 1;
                }
            }
        }
        
        result
    }
    
    /// Get supported file extensions
    pub fn supported_extensions(&self) -> Vec<&str> {
        self.language_patterns.keys().map(|s| s.as_str()).collect()
    }
    
    /// Check if a file extension is supported
    pub fn supports_extension(&self, extension: &str) -> bool {
        self.language_patterns.contains_key(&extension.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_complexity_calculator_creation() {
        let calculator = ComplexityCalculator::new();
        assert!(calculator.supports_extension("rs"));
        assert!(calculator.supports_extension("py"));
        assert!(calculator.supports_extension("js"));
        assert!(!calculator.supports_extension("unknown"));
    }
    
    #[test]
    fn test_rust_complexity_simple() {
        let calculator = ComplexityCalculator::new();
        let content = "fn main() {\n    println!(\"Hello, world!\");\n}";
        let patterns = calculator.language_patterns.get("rs").unwrap();
        let complexity = calculator.calculate_complexity_from_content(content, patterns).unwrap();
        assert_eq!(complexity, 1); // Base complexity only
    }
    
    #[test]
    fn test_rust_complexity_with_conditionals() {
        let calculator = ComplexityCalculator::new();
        let content = r#"
fn test_function(x: i32) -> i32 {
    if x > 0 {
        if x > 10 {
            return x * 2;
        } else if x > 5 {
            return x + 1;
        }
    }
    x
}
"#;
        let patterns = calculator.language_patterns.get("rs").unwrap();
        let complexity = calculator.calculate_complexity_from_content(content, patterns).unwrap();
        println!("Calculated complexity: {}", complexity);
        // Should be: 1 (base) + 1 (if x > 0) + 1 (if x > 10) + 1 (else if x > 5) = 4
        assert_eq!(complexity, 4);
    }
    
    #[test]
    fn test_python_complexity() {
        let calculator = ComplexityCalculator::new();
        let content = r#"
def test_function(x):
    if x > 0:
        for i in range(x):
            if i % 2 == 0:
                print(i)
    elif x < 0:
        while x < 0:
            x += 1
    return x
"#;
        let patterns = calculator.language_patterns.get("py").unwrap();
        let complexity = calculator.calculate_complexity_from_content(content, patterns).unwrap();
        // Should be: 1 (base) + 1 (if x > 0) + 1 (for i) + 1 (if i % 2) + 1 (elif x < 0) + 1 (while x < 0) = 6
        assert_eq!(complexity, 6);
    }
    
    #[test]
    fn test_comment_removal() {
        let calculator = ComplexityCalculator::new();
        let content = r#"
// This if should be ignored
/* 
 * This if should also be ignored
 * if (true) { ... }
 */
fn test() {
    if true { // This if should count
        println!("test");
    }
}
"#;
        let cleaned = calculator.remove_comments_and_strings(content);
        assert!(!cleaned.contains("This if should be ignored"));
        assert!(!cleaned.contains("This if should also be ignored"));
        assert!(cleaned.contains("if true"));
    }
    
    #[test]
    fn test_string_removal() {
        let calculator = ComplexityCalculator::new();
        let content = r#"
fn test() {
    let msg = "This if should be ignored";
    let msg2 = 'This if should also be ignored';
    if true {
        println!("{}", msg);
    }
}
"#;
        let cleaned = calculator.remove_comments_and_strings(content);
        assert!(!cleaned.contains("This if should be ignored"));
        assert!(!cleaned.contains("This if should also be ignored"));
        assert!(cleaned.contains("if true"));
    }
    
    #[test]
    fn test_file_complexity_calculation() -> Result<(), Box<dyn std::error::Error>> {
        let calculator = ComplexityCalculator::new();
        
        // Create a temporary Rust file
        let temp_file = NamedTempFile::new()?;
        let content = r#"
fn main() {
    let x = 5;
    if x > 0 {
        println!("Positive");
        if x > 10 {
            println!("Large");
        }
    } else if x < 0 {
        println!("Negative");
    }
}
"#;
        fs::write(temp_file.path(), content)?;
        
        // Rename to .rs extension for the test
        let rust_path = temp_file.path().with_extension("rs");
        fs::copy(temp_file.path(), &rust_path)?;
        
        let complexity = calculator.calculate_complexity(rust_path.to_str().unwrap())?;
        
        // Clean up
        fs::remove_file(&rust_path)?;
        
        // Should be: 1 (base) + 1 (if x > 0) + 1 (if x > 10) + 1 (else if x < 0) = 4
        assert_eq!(complexity, 4);
        
        Ok(())
    }
    
    #[test]
    fn test_unsupported_extension() -> Result<(), Box<dyn std::error::Error>> {
        let calculator = ComplexityCalculator::new();
        
        // Create a temporary file with unsupported extension
        let temp_file = NamedTempFile::new()?;
        let content = "Some content with if statements that shouldn't be counted";
        fs::write(temp_file.path(), content)?;
        
        let unknown_path = temp_file.path().with_extension("unknown");
        fs::copy(temp_file.path(), &unknown_path)?;
        
        let complexity = calculator.calculate_complexity(unknown_path.to_str().unwrap())?;
        
        // Clean up
        fs::remove_file(&unknown_path)?;
        
        // Should return default complexity of 1 for unsupported extensions
        assert_eq!(complexity, 1);
        
        Ok(())
    }
}