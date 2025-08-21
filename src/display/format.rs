//! Output formatting modes and traits for compact display
//!
//! This module provides the foundation for compact output formatting, enabling
//! concise one-line summaries suitable for CI/CD integration and quick scanning.

/// Output formatting modes
#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    /// Current tabular format with detailed information
    Standard,
    /// New one-line format for quick scanning and automation
    Compact,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Standard
    }
}

/// Trait for types that can be formatted compactly
pub trait CompactFormat {
    /// Convert the type to a compact one-line format
    /// 
    /// The output should be:
    /// - Single line (no newlines except at the end)
    /// - Include essential information only
    /// - Suitable for CI/CD parsing
    /// - Human readable for quick scanning
    fn to_compact_format(&self) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_default() {
        assert_eq!(OutputFormat::default(), OutputFormat::Standard);
    }

    #[test]
    fn test_output_format_equality() {
        assert_eq!(OutputFormat::Standard, OutputFormat::Standard);
        assert_eq!(OutputFormat::Compact, OutputFormat::Compact);
        assert_ne!(OutputFormat::Standard, OutputFormat::Compact);
    }

    // RED PHASE: These tests will fail until we implement CompactFormat for test types
    
    #[derive(Debug)]
    struct TestStatistics {
        name: String,
        count: usize,
        score: f64,
    }

    impl CompactFormat for TestStatistics {
        fn to_compact_format(&self) -> String {
            // Replace newlines with spaces for single-line output
            let clean_name = self.name.replace('\n', " ").replace('\r', " ");
            format!("{}: {} items (score: {:.1})", clean_name, self.count, self.score)
        }
    }

    #[test]
    fn test_compact_format_trait() {
        let stats = TestStatistics {
            name: "Test".to_string(),
            count: 42,
            score: 95.5,
        };
        
        let compact = stats.to_compact_format();
        assert_eq!(compact, "Test: 42 items (score: 95.5)");
        assert!(!compact.contains('\n'), "Compact format should not contain newlines");
    }

    #[test]
    fn test_compact_format_single_line() {
        let stats = TestStatistics {
            name: "MultiLine\nTest".to_string(),
            count: 5,
            score: 88.2,
        };
        
        let compact = stats.to_compact_format();
        // Should handle embedded newlines appropriately
        assert!(compact.lines().count() <= 1, "Compact format should be single line");
    }
}