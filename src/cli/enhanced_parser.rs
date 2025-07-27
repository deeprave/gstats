//! Enhanced argument parsing for CLI arguments that support both multiple flags and comma-separated values

/// Parse a vector of strings that may contain comma-separated values
/// 
/// This function takes a Vec<String> from clap's ArgAction::Append and processes it to:
/// - Split any comma-separated values into individual items
/// - Trim whitespace from each item
/// - Remove empty items
/// - Return a flattened vector of all values
/// 
/// # Examples
/// 
/// ```
/// use gstats::cli::enhanced_parser::parse_comma_separated;
/// 
/// // Multiple flags: -I src/ -I tests/
/// let input = vec!["src/".to_string(), "tests/".to_string()];
/// let result = parse_comma_separated(input);
/// assert_eq!(result, vec!["src/", "tests/"]);
/// 
/// // Comma-separated: -I "src/,tests/"
/// let input = vec!["src/,tests/".to_string()];
/// let result = parse_comma_separated(input);
/// assert_eq!(result, vec!["src/", "tests/"]);
/// 
/// // Mixed: -I "src/,tests/" -I lib/
/// let input = vec!["src/,tests/".to_string(), "lib/".to_string()];
/// let result = parse_comma_separated(input);
/// assert_eq!(result, vec!["src/", "tests/", "lib/"]);
/// ```
pub fn parse_comma_separated(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .flat_map(|item| {
            item.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<String>>()
        })
        .collect()
}

/// Apply enhanced parsing to CLI arguments that support comma-separated values
/// 
/// This struct provides methods to apply the enhanced parsing to specific CLI argument fields
pub struct EnhancedParser;

impl EnhancedParser {
    /// Parse include/exclude path arguments
    pub fn parse_paths(paths: Vec<String>) -> Vec<String> {
        parse_comma_separated(paths)
    }

    /// Parse include/exclude file pattern arguments
    pub fn parse_file_patterns(patterns: Vec<String>) -> Vec<String> {
        parse_comma_separated(patterns)
    }

    /// Parse include/exclude author arguments
    pub fn parse_authors(authors: Vec<String>) -> Vec<String> {
        parse_comma_separated(authors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_comma_separated_single_values() {
        let input = vec!["src/".to_string(), "tests/".to_string()];
        let result = parse_comma_separated(input);
        assert_eq!(result, vec!["src/", "tests/"]);
    }

    #[test]
    fn test_parse_comma_separated_comma_values() {
        let input = vec!["src/,tests/".to_string()];
        let result = parse_comma_separated(input);
        assert_eq!(result, vec!["src/", "tests/"]);
    }

    #[test]
    fn test_parse_comma_separated_mixed() {
        let input = vec!["src/,tests/".to_string(), "lib/".to_string()];
        let result = parse_comma_separated(input);
        assert_eq!(result, vec!["src/", "tests/", "lib/"]);
    }

    #[test]
    fn test_parse_comma_separated_with_spaces() {
        let input = vec!["src/ , tests/ ".to_string(), " lib/ ".to_string()];
        let result = parse_comma_separated(input);
        assert_eq!(result, vec!["src/", "tests/", "lib/"]);
    }

    #[test]
    fn test_parse_comma_separated_empty_values() {
        let input = vec!["src/,,tests/".to_string(), "".to_string()];
        let result = parse_comma_separated(input);
        assert_eq!(result, vec!["src/", "tests/"]);
    }

    #[test]
    fn test_parse_comma_separated_complex() {
        let input = vec![
            "path1,path2".to_string(),
            "path3".to_string(),
            "path4,path5,path6".to_string(),
            " path7 , path8 ".to_string(),
        ];
        let result = parse_comma_separated(input);
        assert_eq!(result, vec!["path1", "path2", "path3", "path4", "path5", "path6", "path7", "path8"]);
    }

    #[test]
    fn test_enhanced_parser_paths() {
        let input = vec!["src/,lib/".to_string(), "tests/".to_string()];
        let result = EnhancedParser::parse_paths(input);
        assert_eq!(result, vec!["src/", "lib/", "tests/"]);
    }

    #[test]
    fn test_enhanced_parser_file_patterns() {
        let input = vec!["*.rs,*.toml".to_string(), "*.py".to_string()];
        let result = EnhancedParser::parse_file_patterns(input);
        assert_eq!(result, vec!["*.rs", "*.toml", "*.py"]);
    }

    #[test]
    fn test_enhanced_parser_authors() {
        let input = vec!["john@example.com,jane@example.com".to_string(), "bob@example.com".to_string()];
        let result = EnhancedParser::parse_authors(input);
        assert_eq!(result, vec!["john@example.com", "jane@example.com", "bob@example.com"]);
    }
}
