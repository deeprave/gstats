//! Global flag filtering utilities
//! 
//! This module provides utilities to filter out global CLI flags from plugin arguments,
//! allowing users to specify global flags after command names for better UX.

use std::collections::HashSet;

/// List of all global flags that should not be passed to plugins
pub fn get_global_flags() -> HashSet<&'static str> {
    let mut flags = HashSet::new();
    
    // Logging flags
    flags.insert("--verbose");
    flags.insert("-v");
    flags.insert("--quiet");
    flags.insert("-q");
    flags.insert("--debug");
    flags.insert("--log-format");
    flags.insert("--log-file");
    flags.insert("--log-file-level");
    
    // Color flags
    flags.insert("--color");
    flags.insert("--no-color");
    
    // Output format flags
    flags.insert("--compact");
    
    // Configuration flags
    flags.insert("--config-file");
    flags.insert("--config-name");
    
    // Repository flags
    flags.insert("--repo");
    flags.insert("-r");
    flags.insert("--repository");
    
    // Filter flags
    flags.insert("--since");
    flags.insert("-S");
    flags.insert("--until");
    flags.insert("-U");
    flags.insert("--include-path");
    flags.insert("-I");
    flags.insert("--exclude-path");
    flags.insert("-X");
    flags.insert("--include-file");
    flags.insert("-F");
    flags.insert("--exclude-file");
    flags.insert("-N");
    flags.insert("--author");
    flags.insert("-A");
    flags.insert("--exclude-author");
    flags.insert("-E");
    
    // Plugin management flags
    flags.insert("--plugins");
    flags.insert("--list-plugins");
    flags.insert("--plugin-info");
    flags.insert("--list-by-type");
    flags.insert("--list-formats");
    flags.insert("--export-config");
    
    flags
}

/// Filter out global flags from plugin arguments
/// 
/// This function removes global CLI flags from the plugin arguments list,
/// allowing users to specify global flags after command names while ensuring
/// plugins only receive their specific arguments.
/// 
/// # Arguments
/// 
/// * `plugin_args` - The original plugin arguments from CLI parsing
/// 
/// # Returns
/// 
/// A filtered vector containing only plugin-specific arguments
pub fn filter_global_flags(plugin_args: &[String]) -> Vec<String> {
    let global_flags = get_global_flags();
    let mut filtered_args = Vec::new();
    let mut i = 0;
    
    while i < plugin_args.len() {
        let arg = &plugin_args[i];
        
        if global_flags.contains(arg.as_str()) {
            // Skip this global flag
            // Check if it's a flag that takes a value
            if is_flag_with_value(arg) && i + 1 < plugin_args.len() {
                // Skip both the flag and its value
                i += 2;
            } else {
                // Skip just the flag
                i += 1;
            }
        } else {
            // Keep this argument
            filtered_args.push(arg.clone());
            i += 1;
        }
    }
    
    filtered_args
}

/// Check if a flag takes a value
fn is_flag_with_value(flag: &str) -> bool {
    matches!(flag, 
        "--log-format" | "--log-file" | "--log-file-level" |
        "--config-file" | "--config-name" |
        "--repo" | "-r" | "--repository" |
        "--since" | "-S" | "--until" | "-U" |
        "--include-path" | "-I" | "--exclude-path" | "-X" |
        "--include-file" | "-F" | "--exclude-file" | "-N" |
        "--author" | "-A" | "--exclude-author" | "-E" |
        "--plugins" | "--plugin-info" | "--list-by-type"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_global_flags() {
        let plugin_args = vec![
            "--output".to_string(),
            "test.json".to_string(),
            "--compact".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ];
        
        let filtered = filter_global_flags(&plugin_args);
        
        assert_eq!(filtered, vec![
            "--output".to_string(),
            "test.json".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ]);
    }
    
    #[test]
    fn test_filter_flags_with_values() {
        let plugin_args = vec![
            "--output".to_string(),
            "test.json".to_string(),
            "--since".to_string(),
            "1 week ago".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ];
        
        let filtered = filter_global_flags(&plugin_args);
        
        assert_eq!(filtered, vec![
            "--output".to_string(),
            "test.json".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ]);
    }
    
    #[test]
    fn test_no_global_flags() {
        let plugin_args = vec![
            "--output".to_string(),
            "test.json".to_string(),
            "--format".to_string(),
            "csv".to_string(),
        ];
        
        let filtered = filter_global_flags(&plugin_args);
        
        assert_eq!(filtered, plugin_args);
    }
    
    #[test]
    fn test_only_global_flags() {
        let plugin_args = vec![
            "--compact".to_string(),
            "--verbose".to_string(),
            "--since".to_string(),
            "1 week ago".to_string(),
        ];
        
        let filtered = filter_global_flags(&plugin_args);
        
        assert!(filtered.is_empty());
    }
}
