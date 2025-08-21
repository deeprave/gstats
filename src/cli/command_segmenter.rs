//! Command Line Segmentation for Plugin Argument Parsing
//! 
//! This module handles the segmentation of command line arguments by plugin boundaries.
//! It splits the command line into global arguments and plugin-specific argument segments.

use crate::cli::plugin_handler::PluginHandler;
use crate::plugin::error::PluginResult;
use anyhow::Result;
use std::collections::HashMap;

/// Represents a segment of command line arguments for a specific plugin
#[derive(Debug, Clone)]
pub struct PluginArgumentSegment {
    /// Plugin or function name
    pub plugin_name: String,
    /// Function name (if using plugin:function syntax)
    pub function_name: Option<String>,
    /// Arguments for this plugin
    pub args: Vec<String>,
}

/// Represents the segmented command line arguments
#[derive(Debug, Clone)]
pub struct SegmentedArgs {
    /// Global arguments (before any plugin)
    pub global_args: Vec<String>,
    /// Plugin-specific argument segments
    pub plugin_segments: Vec<PluginArgumentSegment>,
}

/// Command line segmenter that splits arguments by plugin boundaries
pub struct CommandSegmenter {
    plugin_handler: PluginHandler,
    known_plugins: HashMap<String, Vec<String>>, // plugin -> functions
}

impl CommandSegmenter {
    /// Create a new command segmenter
    pub async fn new(plugin_handler: PluginHandler) -> PluginResult<Self> {
        let mut segmenter = Self {
            plugin_handler,
            known_plugins: HashMap::new(),
        };
        
        // Build known plugins and functions map
        segmenter.build_plugin_map().await?;
        
        Ok(segmenter)
    }
    
    /// Build the map of known plugins and their functions
    async fn build_plugin_map(&mut self) -> PluginResult<()> {
        // Get function mappings from plugin handler
        let function_mappings = self.plugin_handler.get_function_mappings();
        
        for mapping in function_mappings {
            let plugin_entry = self.known_plugins.entry(mapping.plugin_name.clone())
                .or_insert_with(Vec::new);
            
            // Add the function name
            plugin_entry.push(mapping.function_name.clone());
            
            // Add any aliases
            for alias in &mapping.aliases {
                plugin_entry.push(alias.clone());
            }
        }
        
        Ok(())
    }
    
    /// Segment command line arguments by plugin boundaries
    /// 
    /// Example input: ["--verbose", "debug", "--output", "file.json", "commits", "--since", "1week"]
    /// Output: 
    /// - global_args: ["--verbose"]
    /// - plugin_segments: [
    ///     { plugin: "debug", args: ["--output", "file.json"] },
    ///     { plugin: "commits", args: ["--since", "1week"] }
    ///   ]
    pub fn segment_arguments(&self, args: &[String]) -> Result<SegmentedArgs> {
        let mut global_args = Vec::new();
        let mut plugin_segments = Vec::new();
        let mut current_plugin: Option<String> = None;
        let mut current_function: Option<String> = None;
        let mut current_args = Vec::new();
        
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            
            // Check if this is a plugin or function
            if let Some((plugin, function)) = self.resolve_plugin_or_function(arg) {
                // Save previous plugin segment if any
                if let Some(plugin_name) = current_plugin.take() {
                    plugin_segments.push(PluginArgumentSegment {
                        plugin_name,
                        function_name: current_function.take(),
                        args: current_args.drain(..).collect(),
                    });
                }
                
                // Start new plugin segment
                current_plugin = Some(plugin);
                current_function = function;
            } else if current_plugin.is_some() {
                // We're in a plugin context, add to current args
                current_args.push(arg.clone());
            } else {
                // We're in global context
                global_args.push(arg.clone());
            }
            
            i += 1;
        }
        
        // Save final plugin segment if any
        if let Some(plugin_name) = current_plugin {
            plugin_segments.push(PluginArgumentSegment {
                plugin_name,
                function_name: current_function,
                args: current_args,
            });
        }
        
        Ok(SegmentedArgs {
            global_args,
            plugin_segments,
        })
    }
    
    /// Resolve if an argument is a plugin name or function
    /// Returns (plugin_name, function_name) if found
    fn resolve_plugin_or_function(&self, arg: &str) -> Option<(String, Option<String>)> {
        // Handle plugin:function syntax
        if arg.contains(':') {
            let parts: Vec<&str> = arg.splitn(2, ':').collect();
            if parts.len() == 2 {
                let plugin = parts[0].to_string();
                let function = parts[1].to_string();
                
                // Check if this plugin:function combination is valid
                if let Some(functions) = self.known_plugins.get(&plugin) {
                    if functions.contains(&function) {
                        return Some((plugin, Some(function)));
                    }
                }
            }
        }
        
        // Check if it's a direct plugin name
        if self.known_plugins.contains_key(arg) {
            return Some((arg.to_string(), None));
        }
        
        // Check if it's a function name (search across all plugins)
        for (plugin_name, functions) in &self.known_plugins {
            if functions.contains(&arg.to_string()) {
                return Some((plugin_name.clone(), Some(arg.to_string())));
            }
        }
        
        None
    }
    
    // Removed is_help_request and get_plugin_help_with_colors methods
    // Plugins handle their own help through parse_plugin_arguments()
    
    /// Extract the plugin handler from the segmenter
    pub fn extract_plugin_handler(self) -> PluginHandler {
        self.plugin_handler
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;
    
    async fn create_test_segmenter() -> CommandSegmenter {
        let plugin_handler = PluginHandler::new().unwrap();
        let mut plugin_handler = plugin_handler;
        plugin_handler.build_command_mappings().await.unwrap();
        CommandSegmenter::new(plugin_handler).await.unwrap()
    }
    
    #[tokio::test]
    async fn test_segment_global_only() {
        let segmenter = create_test_segmenter().await;
        let args = vec!["--verbose".to_string(), "--repo".to_string(), "/path".to_string()];
        
        let result = segmenter.segment_arguments(&args).unwrap();
        
        assert_eq!(result.global_args, vec!["--verbose", "--repo", "/path"]);
        assert!(result.plugin_segments.is_empty());
    }
    
    #[tokio::test]
    async fn test_segment_single_plugin() {
        let segmenter = create_test_segmenter().await;
        let args = vec![
            "--verbose".to_string(),
            "debug".to_string(),
            "--output".to_string(),
            "file.json".to_string()
        ];
        
        let result = segmenter.segment_arguments(&args).unwrap();
        
        assert_eq!(result.global_args, vec!["--verbose"]);
        assert_eq!(result.plugin_segments.len(), 1);
        assert_eq!(result.plugin_segments[0].plugin_name, "debug");
        assert_eq!(result.plugin_segments[0].args, vec!["--output", "file.json"]);
    }
    
    #[tokio::test]
    async fn test_segment_multiple_plugins() {
        let segmenter = create_test_segmenter().await;
        let args = vec![
            "--verbose".to_string(),
            "debug".to_string(),
            "--output".to_string(),
            "file.json".to_string(),
            "commits".to_string(),
            "--since".to_string(),
            "1week".to_string()
        ];
        
        let result = segmenter.segment_arguments(&args).unwrap();
        
        assert_eq!(result.global_args, vec!["--verbose"]);
        assert_eq!(result.plugin_segments.len(), 2);
        
        assert_eq!(result.plugin_segments[0].plugin_name, "debug");
        assert_eq!(result.plugin_segments[0].args, vec!["--output", "file.json"]);
        
        assert_eq!(result.plugin_segments[1].plugin_name, "commits");
        assert_eq!(result.plugin_segments[1].args, vec!["--since", "1week"]);
    }
    
    #[tokio::test]
    async fn test_segment_plugin_function_syntax() {
        let segmenter = create_test_segmenter().await;
        let args = vec![
            "--verbose".to_string(),
            "commits:authors".to_string(),
            "--since".to_string(),
            "1week".to_string()
        ];
        
        let result = segmenter.segment_arguments(&args).unwrap();
        
        assert_eq!(result.global_args, vec!["--verbose"]);
        assert_eq!(result.plugin_segments.len(), 1);
        assert_eq!(result.plugin_segments[0].plugin_name, "commits");
        assert_eq!(result.plugin_segments[0].function_name, Some("authors".to_string()));
        assert_eq!(result.plugin_segments[0].args, vec!["--since", "1week"]);
    }
    
    #[tokio::test]
    async fn test_is_help_request() {
        let segmenter = create_test_segmenter().await;
        
        let help_segment = PluginArgumentSegment {
            plugin_name: "debug".to_string(),
            function_name: None,
            args: vec!["--help".to_string()],
        };
        
        let normal_segment = PluginArgumentSegment {
            plugin_name: "debug".to_string(),
            function_name: None,
            args: vec!["--output".to_string(), "file.json".to_string()],
        };
        
        assert!(segmenter.is_help_request(&help_segment));
        assert!(!segmenter.is_help_request(&normal_segment));
    }
}