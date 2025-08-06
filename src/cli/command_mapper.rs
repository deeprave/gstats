//! Command Mapping for Plugin Function Resolution
//! 
//! Maps CLI commands to plugin functions with ambiguity detection.

use std::collections::HashMap;
use anyhow::{Result, bail};
use log::debug;
use crate::plugin::traits::PluginFunction;
use super::suggestion::{SuggestionEngine, SuggestionConfig};
use super::contextual_help::{ContextualHelp, HelpLevel, create_help_context};
use super::help_formatter::HelpFormatter;

/// Resolution result for a command
#[derive(Debug, Clone)]
pub enum CommandResolution {
    /// Function found in single plugin
    Function {
        plugin_name: String,
        function_name: String,
        is_default: bool,
    },
    /// Direct plugin invocation
    DirectPlugin {
        plugin_name: String,
        default_function: Option<String>,
    },
    /// Plugin and function explicitly specified
    Explicit {
        plugin_name: String,
        function_name: String,
    },
}

/// Ambiguity report for diagnostics
#[derive(Debug, Clone)]
pub struct AmbiguityReport {
    pub function_name: String,
    pub providers: Vec<(String, bool)>, // (plugin_name, is_default)
}

impl std::fmt::Display for AmbiguityReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let providers: Vec<String> = self.providers.iter()
            .map(|(plugin, is_default)| {
                if *is_default {
                    format!("{}*", plugin)
                } else {
                    plugin.clone()
                }
            })
            .collect();
        
        write!(f, "Function '{}' provided by: {} (use plugin:function syntax to disambiguate)", 
               self.function_name, providers.join(", "))
    }
}

/// Maps commands to plugins and their functions
pub struct CommandMapper {
    /// Function name -> Vec<(plugin_name, is_default)>
    function_map: HashMap<String, Vec<(String, bool)>>,
    
    /// Plugin name -> Plugin functions
    plugin_map: HashMap<String, Vec<PluginFunction>>,
    
    /// Alias to canonical function name mapping
    alias_map: HashMap<String, String>,
    
    /// Smart suggestion engine for "did you mean?" functionality
    suggestion_engine: SuggestionEngine,
    
    /// Contextual help system for progressive help discovery
    contextual_help: ContextualHelp,
}

impl CommandMapper {
    /// Create a new command mapper
    pub fn new() -> Self {
        Self {
            function_map: HashMap::new(),
            plugin_map: HashMap::new(),
            alias_map: HashMap::new(),
            suggestion_engine: SuggestionEngine::new(SuggestionConfig::default()),
            contextual_help: ContextualHelp::new(),
        }
    }
    
    /// Get the number of registered plugins
    pub fn plugin_count(&self) -> usize {
        self.plugin_map.len()
    }
    
    /// Get all plugin mappings for display
    pub fn get_all_mappings(&self) -> &HashMap<String, Vec<PluginFunction>> {
        &self.plugin_map
    }
    
    /// Register a plugin and its functions
    pub fn register_plugin(&mut self, plugin_name: &str, functions: Vec<PluginFunction>) {
        debug!("Registering plugin '{}' with {} functions", plugin_name, functions.len());
        
        // Store plugin functions
        self.plugin_map.insert(plugin_name.to_string(), functions.clone());
        
        // Build function and alias mappings
        for func in functions {
            // Map primary function name
            self.function_map
                .entry(func.name.clone())
                .or_insert_with(Vec::new)
                .push((plugin_name.to_string(), func.is_default));
            
            // Map aliases to canonical name
            for alias in &func.aliases {
                self.alias_map.insert(alias.clone(), func.name.clone());
                
                // Also add alias to function map
                self.function_map
                    .entry(alias.clone())
                    .or_insert_with(Vec::new)
                    .push((plugin_name.to_string(), func.is_default));
            }
            
            if func.is_default {
                debug!("  - Function '{}' (default)", func.name);
            } else {
                debug!("  - Function '{}'", func.name);
            }
            
            if !func.aliases.is_empty() {
                debug!("    Aliases: {:?}", func.aliases);
            }
        }
        
        // Update suggestion engine with current commands
        self.update_suggestion_engine();
    }
    
    /// Update the suggestion engine with current available commands
    fn update_suggestion_engine(&mut self) {
        let plugins: Vec<String> = self.plugin_map.keys().cloned().collect();
        let functions: Vec<String> = self.function_map.keys().cloned().collect();
        self.suggestion_engine.update_commands(&plugins, &functions);
    }
    
    /// Resolve a command string to plugin and function
    pub async fn resolve_command(&self, input: &str) -> Result<CommandResolution> {
        self.resolve_command_with_colors(input, false, false).await
    }

    /// Resolve a command string to plugin and function with color control
    pub async fn resolve_command_with_colors(&self, input: &str, no_color: bool, color: bool) -> Result<CommandResolution> {
        debug!("Resolving command: '{}'", input);
        
        // Check for explicit plugin:function syntax
        if let Some(colon_pos) = input.find(':') {
            let plugin_name = &input[..colon_pos];
            let function_name = &input[colon_pos + 1..];
            
            debug!("Explicit syntax: plugin='{}', function='{}'", plugin_name, function_name);
            
            // Verify plugin exists
            if !self.plugin_map.contains_key(plugin_name) {
                bail!("Unknown plugin: '{}'", plugin_name);
            }
            
            // Verify function exists in plugin
            let plugin_functions = &self.plugin_map[plugin_name];
            let canonical_name = self.alias_map.get(function_name)
                .map(|s| s.as_str())
                .unwrap_or(function_name);
            
            let has_function = plugin_functions.iter()
                .any(|f| f.name == canonical_name || f.aliases.contains(&function_name.to_string()));
            
            if !has_function {
                let available: Vec<String> = plugin_functions.iter()
                    .map(|f| f.name.clone())
                    .collect();
                bail!(
                    "Plugin '{}' does not provide function '{}'. Available functions: {:?}",
                    plugin_name, function_name, available
                );
            }
            
            return Ok(CommandResolution::Explicit {
                plugin_name: plugin_name.to_string(),
                function_name: canonical_name.to_string(),
            });
        }
        
        // Check if it's a function name (with possible alias resolution)
        let canonical_name = self.alias_map.get(input)
            .map(|s| s.as_str())
            .unwrap_or(input);
        
        if let Some(providers) = self.function_map.get(canonical_name) {
            debug!("Function '{}' found in {} plugin(s)", canonical_name, providers.len());
            
            if providers.len() == 1 {
                let (plugin_name, is_default) = &providers[0];
                return Ok(CommandResolution::Function {
                    plugin_name: plugin_name.clone(),
                    function_name: canonical_name.to_string(),
                    is_default: *is_default,
                });
            } else {
                // Multiple providers - ambiguous
                let plugin_names: Vec<String> = providers.iter()
                    .map(|(name, _)| name.clone())
                    .collect();
                bail!(
                    "Ambiguous function '{}'. Multiple plugins provide this function: {:?}. Use 'plugin:function' syntax.",
                    input, plugin_names
                );
            }
        }
        
        // Check if it's a direct plugin name
        if self.plugin_map.contains_key(input) {
            debug!("Direct plugin invocation: '{}'", input);
            
            let default_function = self.plugin_map[input].iter()
                .find(|f| f.is_default)
                .map(|f| f.name.clone());
            
            return Ok(CommandResolution::DirectPlugin {
                plugin_name: input.to_string(),
                default_function,
            });
        }
        
        // Not found - generate smart suggestions
        let available_plugins: Vec<String> = self.plugin_map.keys()
            .cloned()
            .collect();
        
        // Sort the lists for better readability
        let mut sorted_plugins = available_plugins;
        sorted_plugins.sort();
        
        // Use colored formatter for the error message
        let formatter = HelpFormatter::from_color_flags(no_color, color);
        let suggestion_texts: Vec<String> = self.suggestion_engine.suggest(input)
            .iter()
            .map(|s| s.text.clone())
            .collect();
        
        let error_msg = formatter.format_invalid_command(input, &suggestion_texts).await;
        
        // Add contextual help for common commands if no suggestions
        let final_error = if suggestion_texts.is_empty() && sorted_plugins.len() <= 3 {
            let help_context = create_help_context(None, Some(format!("Unknown command '{}'", input)));
            let contextual = self.contextual_help.get_contextual_help(&help_context);
            if !contextual.trim().is_empty() {
                format!("{}\n\n{}", error_msg, contextual)
            } else {
                error_msg
            }
        } else {
            error_msg
        };
        
        bail!("{}", final_error);
    }
    
    /// Detect all ambiguous function names
    pub fn detect_ambiguities(&self) -> Vec<AmbiguityReport> {
        let mut reports = Vec::new();
        
        for (function_name, providers) in &self.function_map {
            if providers.len() > 1 {
                reports.push(AmbiguityReport {
                    function_name: function_name.clone(),
                    providers: providers.clone(),
                });
            }
        }
        
        reports.sort_by(|a, b| a.function_name.cmp(&b.function_name));
        reports
    }
    
    /// Get all registered plugins
    pub fn registered_plugins(&self) -> Vec<&str> {
        self.plugin_map.keys().map(|s| s.as_str()).collect()
    }
    
    /// Get all available functions
    pub fn available_functions(&self) -> Vec<&str> {
        self.function_map.keys().map(|s| s.as_str()).collect()
    }
    
    /// Get contextual help for a command or error
    pub fn get_contextual_help(&self, command: Option<String>, error: Option<String>) -> String {
        let context = create_help_context(command, error);
        self.contextual_help.get_contextual_help(&context)
    }
    
    /// Get command-specific help with progressive detail
    pub fn get_command_help(&self, command: &str, level: HelpLevel) -> Option<String> {
        self.contextual_help.get_command_help(command, level)
    }
}

impl Default for CommandMapper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_functions() -> Vec<PluginFunction> {
        vec![
            PluginFunction {
                name: "analyze".to_string(),
                aliases: vec!["analysis".to_string()],
                description: "Analyze repository".to_string(),
                is_default: true,
            },
            PluginFunction {
                name: "complexity".to_string(),
                aliases: vec!["complex".to_string()],
                description: "Analyze complexity".to_string(),
                is_default: false,
            },
        ]
    }
    
    #[tokio::test]
    async fn test_single_function_resolution() {
        let mut mapper = CommandMapper::new();
        mapper.register_plugin("metrics", create_test_functions());
        
        // Direct function name
        let result = mapper.resolve_command("complexity").await.unwrap();
        match result {
            CommandResolution::Function { plugin_name, function_name, .. } => {
                assert_eq!(plugin_name, "metrics");
                assert_eq!(function_name, "complexity");
            }
            _ => panic!("Expected Function resolution"),
        }
        
        // Via alias
        let result = mapper.resolve_command("complex").await.unwrap();
        match result {
            CommandResolution::Function { plugin_name, function_name, .. } => {
                assert_eq!(plugin_name, "metrics");
                assert_eq!(function_name, "complexity");
            }
            _ => panic!("Expected Function resolution"),
        }
    }
    
    #[tokio::test]
    async fn test_explicit_syntax() {
        let mut mapper = CommandMapper::new();
        mapper.register_plugin("metrics", create_test_functions());
        
        let result = mapper.resolve_command("metrics:complexity").await.unwrap();
        match result {
            CommandResolution::Explicit { plugin_name, function_name } => {
                assert_eq!(plugin_name, "metrics");
                assert_eq!(function_name, "complexity");
            }
            _ => panic!("Expected Explicit resolution"),
        }
    }
    
    #[tokio::test]
    async fn test_direct_plugin_invocation() {
        let mut mapper = CommandMapper::new();
        mapper.register_plugin("metrics", create_test_functions());
        
        let result = mapper.resolve_command("metrics").await.unwrap();
        match result {
            CommandResolution::DirectPlugin { plugin_name, default_function } => {
                assert_eq!(plugin_name, "metrics");
                assert_eq!(default_function, Some("analyze".to_string()));
            }
            _ => panic!("Expected DirectPlugin resolution"),
        }
    }
    
    #[tokio::test]
    async fn test_ambiguous_function_error() {
        let mut mapper = CommandMapper::new();
        mapper.register_plugin("metrics", create_test_functions());
        mapper.register_plugin("analyze", vec![
            PluginFunction {
                name: "complexity".to_string(),
                aliases: vec![],
                description: "Different complexity analysis".to_string(),
                is_default: false,
            }
        ]);
        
        let result = mapper.resolve_command("complexity").await;
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Ambiguous function"));
        assert!(error.contains("metrics"));
        assert!(error.contains("analyze"));
    }
    
    #[tokio::test]
    async fn test_unknown_command_error() {
        let mapper = CommandMapper::new();
        let result = mapper.resolve_command("unknown").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown command"));
    }
    
    #[test]
    fn test_ambiguity_detection() {
        let mut mapper = CommandMapper::new();
        mapper.register_plugin("plugin1", vec![
            PluginFunction {
                name: "shared".to_string(),
                aliases: vec![],
                description: "Shared function".to_string(),
                is_default: false,
            }
        ]);
        mapper.register_plugin("plugin2", vec![
            PluginFunction {
                name: "shared".to_string(),
                aliases: vec![],
                description: "Another shared function".to_string(),
                is_default: false,
            }
        ]);
        
        let ambiguities = mapper.detect_ambiguities();
        assert_eq!(ambiguities.len(), 1);
        assert_eq!(ambiguities[0].function_name, "shared");
        assert_eq!(ambiguities[0].providers.len(), 2);
    }
}