//! Plugin-Specific CLI Argument Handling
//!
//! This module provides functionality to parse and manage plugin-specific
//! command line arguments. It supports the pattern where users can specify
//! plugin configurations directly on the command line.
//!
//! # Argument Format
//!
//! Plugin-specific arguments follow the pattern:
//! ```bash
//! gstats --plugin debug:verbose,full-commit-message,message-index /path/to/repo
//! gstats --plugin debug:raw-data --plugin export:json /path/to/repo
//! ```
//!
//! The format is: `--plugin <plugin_name>:<arg1>,<arg2>,<arg3>`

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::plugin::error::{PluginError, PluginResult};

/// Plugin-specific argument configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginArguments {
    /// Plugin name this configuration applies to
    pub plugin_name: String,
    
    /// Parsed arguments as key-value pairs
    pub arguments: HashMap<String, PluginArgValue>,
    
    /// Raw argument string for debugging
    pub raw_args: String,
}

/// Value types for plugin arguments
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginArgValue {
    /// Boolean flag (present or not)
    Flag(bool),
    
    /// String value
    String(String),
    
    /// Numeric value
    Number(i64),
    
    /// Multiple values (for repeated arguments)
    Multiple(Vec<String>),
}

/// Parser for plugin-specific CLI arguments
#[derive(Debug)]
#[allow(dead_code)]
pub struct PluginArgumentParser {
    /// Parsed plugin configurations indexed by plugin name
    configurations: HashMap<String, PluginArguments>,
}

#[allow(dead_code)]
impl PluginArgumentParser {
    /// Create a new parser
    pub fn new() -> Self {
        Self {
            configurations: HashMap::new(),
        }
    }
    
    /// Parse plugin argument string
    /// 
    /// Format: "plugin_name:arg1,arg2=value,arg3"
    /// 
    /// # Arguments
    /// * `plugin_arg` - The plugin argument string (e.g., "debug:verbose,raw-data")
    /// 
    /// # Returns
    /// * `Ok(())` if parsing succeeded
    /// * `Err(PluginError)` if parsing failed
    pub fn parse_plugin_argument(&mut self, plugin_arg: &str) -> PluginResult<()> {
        let parts: Vec<&str> = plugin_arg.splitn(2, ':').collect();
        
        if parts.len() != 2 {
            return Err(PluginError::InvalidArgument {
                arg: plugin_arg.to_string(),
                reason: "Plugin argument must be in format 'plugin_name:args'".to_string(),
            });
        }
        
        let plugin_name = parts[0].trim().to_string();
        let args_string = parts[1].trim();
        
        if plugin_name.is_empty() {
            return Err(PluginError::InvalidArgument {
                arg: plugin_arg.to_string(),
                reason: "Plugin name cannot be empty".to_string(),
            });
        }
        
        let arguments = self.parse_argument_list(args_string)?;
        
        let plugin_args = PluginArguments {
            plugin_name: plugin_name.clone(),
            arguments,
            raw_args: args_string.to_string(),
        };
        
        // Store or merge with existing configuration
        if let Some(existing) = self.configurations.get_mut(&plugin_name) {
            // Merge arguments (new ones override existing)
            existing.arguments.extend(plugin_args.arguments);
            existing.raw_args.push_str(&format!(", {}", args_string));
        } else {
            self.configurations.insert(plugin_name, plugin_args);
        }
        
        Ok(())
    }
    
    /// Parse a comma-separated list of arguments
    /// 
    /// Supports formats:
    /// - `flag` -> Flag(true)
    /// - `key=value` -> String(value) or Number(value)
    /// - `key=value1,value2` -> Multiple([value1, value2])
    fn parse_argument_list(&self, args_string: &str) -> PluginResult<HashMap<String, PluginArgValue>> {
        let mut arguments = HashMap::new();
        
        if args_string.is_empty() {
            return Ok(arguments);
        }
        
        for arg in args_string.split(',') {
            let arg = arg.trim();
            if arg.is_empty() {
                continue;
            }
            
            if let Some(eq_pos) = arg.find('=') {
                // Key=value format
                let key = arg[..eq_pos].trim().to_string();
                let value = arg[eq_pos + 1..].trim();
                
                if key.is_empty() {
                    return Err(PluginError::InvalidArgument {
                        arg: arg.to_string(),
                        reason: "Argument key cannot be empty".to_string(),
                    });
                }
                
                // Try to parse as number first, then string
                let parsed_value = if let Ok(num) = value.parse::<i64>() {
                    PluginArgValue::Number(num)
                } else {
                    PluginArgValue::String(value.to_string())
                };
                
                arguments.insert(key, parsed_value);
            } else {
                // Flag format
                arguments.insert(arg.to_string(), PluginArgValue::Flag(true));
            }
        }
        
        Ok(arguments)
    }
    
    /// Get arguments for a specific plugin
    pub fn get_plugin_arguments(&self, plugin_name: &str) -> Option<&PluginArguments> {
        self.configurations.get(plugin_name)
    }
    
    /// Get all plugin configurations
    pub fn get_all_configurations(&self) -> &HashMap<String, PluginArguments> {
        &self.configurations
    }
    
    /// Check if a plugin has any arguments configured
    pub fn has_plugin_arguments(&self, plugin_name: &str) -> bool {
        self.configurations.contains_key(plugin_name)
    }
    
    /// Convert plugin arguments to string array format for plugin consumption
    /// 
    /// This converts the parsed arguments back to a format that plugins
    /// can consume using their own argument parsing logic.
    pub fn to_string_args(&self, plugin_name: &str) -> Vec<String> {
        if let Some(config) = self.configurations.get(plugin_name) {
            let mut args = Vec::new();
            
            for (key, value) in &config.arguments {
                match value {
                    PluginArgValue::Flag(true) => {
                        args.push(format!("--{}", key));
                    }
                    PluginArgValue::String(s) => {
                        args.push(format!("--{}", key));
                        args.push(s.clone());
                    }
                    PluginArgValue::Number(n) => {
                        args.push(format!("--{}", key));
                        args.push(n.to_string());
                    }
                    PluginArgValue::Multiple(values) => {
                        for value in values {
                            args.push(format!("--{}", key));
                            args.push(value.clone());
                        }
                    }
                    PluginArgValue::Flag(false) => {
                        // Skip false flags
                    }
                }
            }
            
            args
        } else {
            Vec::new()
        }
    }
    
    /// Clear all configurations
    pub fn clear(&mut self) {
        self.configurations.clear();
    }
    
    /// Get the list of plugins that have arguments configured
    pub fn get_configured_plugins(&self) -> Vec<&str> {
        self.configurations.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for PluginArgumentParser {
    fn default() -> Self {
        Self::new()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_simple_plugin_argument() {
        let mut parser = PluginArgumentParser::new();
        parser.parse_plugin_argument("debug:verbose").unwrap();
        
        let config = parser.get_plugin_arguments("debug").unwrap();
        assert_eq!(config.plugin_name, "debug");
        assert_eq!(config.arguments.len(), 1);
        assert_eq!(config.arguments.get("verbose"), Some(&PluginArgValue::Flag(true)));
    }
    
    #[test]
    fn test_parse_multiple_flags() {
        let mut parser = PluginArgumentParser::new();
        parser.parse_plugin_argument("debug:verbose,raw-data,message-index").unwrap();
        
        let config = parser.get_plugin_arguments("debug").unwrap();
        assert_eq!(config.arguments.len(), 3);
        assert_eq!(config.arguments.get("verbose"), Some(&PluginArgValue::Flag(true)));
        assert_eq!(config.arguments.get("raw-data"), Some(&PluginArgValue::Flag(true)));
        assert_eq!(config.arguments.get("message-index"), Some(&PluginArgValue::Flag(true)));
    }
    
    #[test]
    fn test_parse_key_value_arguments() {
        let mut parser = PluginArgumentParser::new();
        parser.parse_plugin_argument("export:format=json,output=/tmp/output.json").unwrap();
        
        let config = parser.get_plugin_arguments("export").unwrap();
        assert_eq!(config.arguments.len(), 2);
        assert_eq!(config.arguments.get("format"), Some(&PluginArgValue::String("json".to_string())));
        assert_eq!(config.arguments.get("output"), Some(&PluginArgValue::String("/tmp/output.json".to_string())));
    }
    
    #[test]
    fn test_parse_numeric_arguments() {
        let mut parser = PluginArgumentParser::new();
        parser.parse_plugin_argument("debug:limit=100,timeout=5000").unwrap();
        
        let config = parser.get_plugin_arguments("debug").unwrap();
        assert_eq!(config.arguments.len(), 2);
        assert_eq!(config.arguments.get("limit"), Some(&PluginArgValue::Number(100)));
        assert_eq!(config.arguments.get("timeout"), Some(&PluginArgValue::Number(5000)));
    }
    
    #[test]
    fn test_parse_mixed_arguments() {
        let mut parser = PluginArgumentParser::new();
        parser.parse_plugin_argument("debug:verbose,limit=50,format=json").unwrap();
        
        let config = parser.get_plugin_arguments("debug").unwrap();
        assert_eq!(config.arguments.len(), 3);
        assert_eq!(config.arguments.get("verbose"), Some(&PluginArgValue::Flag(true)));
        assert_eq!(config.arguments.get("limit"), Some(&PluginArgValue::Number(50)));
        assert_eq!(config.arguments.get("format"), Some(&PluginArgValue::String("json".to_string())));
    }
    
    #[test]
    fn test_multiple_plugin_configurations() {
        let mut parser = PluginArgumentParser::new();
        parser.parse_plugin_argument("debug:verbose").unwrap();
        parser.parse_plugin_argument("export:format=json").unwrap();
        
        assert!(parser.has_plugin_arguments("debug"));
        assert!(parser.has_plugin_arguments("export"));
        assert!(!parser.has_plugin_arguments("nonexistent"));
        
        let debug_config = parser.get_plugin_arguments("debug").unwrap();
        assert_eq!(debug_config.plugin_name, "debug");
        
        let export_config = parser.get_plugin_arguments("export").unwrap();
        assert_eq!(export_config.plugin_name, "export");
    }
    
    #[test]
    fn test_merge_plugin_arguments() {
        let mut parser = PluginArgumentParser::new();
        parser.parse_plugin_argument("debug:verbose").unwrap();
        parser.parse_plugin_argument("debug:raw-data,limit=100").unwrap();
        
        let config = parser.get_plugin_arguments("debug").unwrap();
        assert_eq!(config.arguments.len(), 3);
        assert_eq!(config.arguments.get("verbose"), Some(&PluginArgValue::Flag(true)));
        assert_eq!(config.arguments.get("raw-data"), Some(&PluginArgValue::Flag(true)));
        assert_eq!(config.arguments.get("limit"), Some(&PluginArgValue::Number(100)));
    }
    
    #[test]
    fn test_to_string_args() {
        let mut parser = PluginArgumentParser::new();
        parser.parse_plugin_argument("debug:verbose,limit=100,format=json").unwrap();
        
        let args = parser.to_string_args("debug");
        
        // The order might vary due to HashMap, so check individual elements
        assert!(args.contains(&"--verbose".to_string()));
        assert!(args.contains(&"--limit".to_string()));
        assert!(args.contains(&"100".to_string()));
        assert!(args.contains(&"--format".to_string()));
        assert!(args.contains(&"json".to_string()));
    }
    
    #[test]
    fn test_invalid_plugin_argument_format() {
        let mut parser = PluginArgumentParser::new();
        
        // Missing colon
        let result = parser.parse_plugin_argument("debug-verbose");
        assert!(result.is_err());
        
        // Empty plugin name
        let result = parser.parse_plugin_argument(":verbose");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_empty_arguments() {
        let mut parser = PluginArgumentParser::new();
        parser.parse_plugin_argument("debug:").unwrap();
        
        let config = parser.get_plugin_arguments("debug").unwrap();
        assert_eq!(config.arguments.len(), 0);
    }
    
    
    #[test]
    fn test_configured_plugins_list() {
        let mut parser = PluginArgumentParser::new();
        parser.parse_plugin_argument("debug:verbose").unwrap();
        parser.parse_plugin_argument("export:format=json").unwrap();
        
        let mut plugins = parser.get_configured_plugins();
        plugins.sort(); // For consistent testing
        
        assert_eq!(plugins, vec!["debug", "export"]);
    }
}