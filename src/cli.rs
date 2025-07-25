// Command line interface module for gstats
// Handles argument parsing and CLI structure

use clap::Parser;
use anyhow::Result;

/// Git Repository Analytics Tool
#[derive(Parser, Debug)]
#[command(name = "gstats")]
#[command(about = "A fast, local-first git analytics tool providing code complexity trends, contributor statistics, performance metrics, and native macOS widgets")]
#[command(version)]
pub struct Args {
    /// Path to git repository (defaults to current directory if it's a git repository)
    pub repository: Option<String>,
}

/// Parse command line arguments
pub fn parse_args() -> Args {
    Args::parse()
}

/// Main CLI logic - processes parsed arguments and coordinates repository operations
pub fn run(args: Args) -> Result<()> {
    let repo_path = match args.repository {
        Some(path) => path,
        None => ".".to_string(), // Default to current directory
    };

    // For now, just print what we would analyze
    // This will be replaced with actual git analysis in future iterations
    println!("Analyzing git repository at: {}", repo_path);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_parsing_with_repository() {
        // Test that Args can be created with repository path
        let args = Args {
            repository: Some("/path/to/repo".to_string()),
        };
        assert_eq!(args.repository, Some("/path/to/repo".to_string()));
    }

    #[test]
    fn test_args_parsing_without_repository() {
        // Test that Args can be created without repository path
        let args = Args {
            repository: None,
        };
        assert_eq!(args.repository, None);
    }

    #[test]
    fn test_cli_run_with_path() {
        // Test that run function handles repository path correctly
        let args = Args {
            repository: Some("/some/path".to_string()),
        };
        
        // Should not panic or error for basic path handling
        // Note: This doesn't validate git repository, just CLI processing
        let result = run(args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_run_without_path() {
        // Test that run function handles no repository path correctly
        let args = Args {
            repository: None,
        };
        
        // Should not panic or error for basic path handling
        let result = run(args);
        assert!(result.is_ok());
    }
}
