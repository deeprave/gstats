use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};
use toml::Value;
use log::{debug, info};
use crate::scanner::config::ScannerConfig;
use crate::display::{ColourConfig, ColourTheme, ColourPalette};

/// Configuration storage - section_name -> key -> value
pub type Configuration = HashMap<String, HashMap<String, String>>;

/// Configuration manager
pub struct ConfigManager {
    config: Configuration,
    _config_file_path: Option<PathBuf>,
    selected_section: Option<String>,
}

impl ConfigManager {
    /// Create a new ConfigManager from a Configuration (primarily for testing)
    #[allow(dead_code)]
    pub fn from_config(config: Configuration) -> Self {
        Self {
            config,
            _config_file_path: None,
            selected_section: None,
        }
    }
    /// Load configuration using discovery hierarchy
    pub fn load() -> Result<Self> {
        debug!("Starting configuration discovery");
        
        // Try discovery hierarchy
        let config_paths = discover_config_files()?;
        
        for path in config_paths {
            debug!("Attempting to load config from: {}", path.display());
            if path.exists() {
                info!("Loading configuration from: {}", path.display());
                return Self::load_from_file(path);
            }
        }
        
        debug!("No configuration file found, using empty configuration");
        Ok(Self {
            config: Configuration::new(),
            _config_file_path: None,
            selected_section: None,
        })
    }
    
    /// Load configuration from explicit file path
    pub fn load_from_file(path: PathBuf) -> Result<Self> {
        debug!("Loading configuration from file: {}", path.display());
        
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        
        let config = parse_toml_config(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        
        info!("Successfully loaded configuration from: {}", path.display());
        Ok(Self {
            config,
            _config_file_path: Some(path),
            selected_section: None,
        })
    }
    
    /// Get value from configuration with section fallback
    pub fn get_value(&self, section: &str, key: &str) -> Option<&String> {
        // Priority: selected_section -> specified section -> base
        if let Some(selected) = &self.selected_section {
            if let Some(value) = self.config.get(selected).and_then(|s| s.get(key)) {
                return Some(value);
            }
        }
        
        // First try direct key access within the section
        if let Some(value) = self.config.get(section).and_then(|s| s.get(key)) {
            return Some(value);
        }
        
        // Then try flattened key access (section.key with value key)
        let flattened_key = format!("{}.{}", section, key);
        if let Some(value) = self.config.get(&flattened_key).and_then(|s| s.get("value")) {
            return Some(value);
        }
        
        self.config.get("base").and_then(|s| s.get(key))
    }
    
    /// Get value from root-level configuration (for sectionless keys)
    pub fn get_value_root(&self, key: &str) -> Option<&String> {
        // First try as root-level key (stored as single-value section)
        if let Some(value) = self.config.get(key).and_then(|s| s.get("value")) {
            return Some(value);
        }
        
        // Fallback to base section for compatibility
        self.config.get("base").and_then(|s| s.get(key))
    }
    
    /// Get boolean value from root-level configuration
    pub fn get_bool_root(&self, key: &str) -> Result<Option<bool>> {
        if let Some(value) = self.get_value_root(key) {
            match value.to_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => Ok(Some(true)),
                "false" | "0" | "no" | "off" => Ok(Some(false)),
                _ => Err(anyhow::anyhow!("Invalid boolean value '{}' for key '{}'", value, key)),
            }
        } else {
            Ok(None)
        }
    }
    
    /// Select configuration section for --config-name
    #[cfg(test)]
    pub fn select_section(&mut self, section: String) {
        debug!("Selecting configuration section: {}", section);
        self.selected_section = Some(section);
    }
    
    /// Get boolean value with type conversion
    pub fn get_bool(&self, section: &str, key: &str) -> Result<Option<bool>> {
        match self.get_value(section, key) {
            Some(value) => match value.to_lowercase().as_str() {
                "true" => Ok(Some(true)),
                "false" => Ok(Some(false)),
                _ => Err(anyhow::anyhow!("Invalid boolean value for {}.{}: {}", section, key, value)),
            },
            None => Ok(None),
        }
    }
    
    /// Get log level value with type conversion
    pub fn get_log_level(&self, section: &str, key: &str) -> Result<Option<log::LevelFilter>> {
        match self.get_value(section, key) {
            Some(value) => Ok(Some(crate::logging::parse_log_level(value)?)),
            None => Ok(None),
        }
    }
    
    /// Get path value with type conversion
    pub fn get_path(&self, section: &str, key: &str) -> Option<PathBuf> {
        self.get_value(section, key).map(PathBuf::from)
    }
    
    /// Get scanner configuration from config file
    pub fn get_scanner_config(&self) -> Result<ScannerConfig> {
        let mut config = ScannerConfig::default();
        
        // Check for scanner configuration values
        if let Some(max_memory_str) = self.get_value("scanner", "max-memory") {
            let max_memory_bytes = crate::cli::memory_parser::parse_memory_size(max_memory_str)
                .with_context(|| format!("Invalid max-memory value in config: {}", max_memory_str))?;
            config.max_memory_bytes = max_memory_bytes;
        }
        
        if let Some(queue_size_str) = self.get_value("scanner", "queue-size") {
            let queue_size = queue_size_str.parse::<usize>()
                .with_context(|| format!("Invalid queue-size value in config: {}", queue_size_str))?;
            config.queue_size = queue_size;
        }
        
        if let Some(max_threads_str) = self.get_value("scanner", "max-threads") {
            let max_threads = max_threads_str.parse::<usize>()
                .with_context(|| format!("Invalid max-threads value in config: {}", max_threads_str))?;
            config.max_threads = Some(max_threads);
        }
        
        // Handle performance-mode preset
        if let Some(_performance_mode_str) = self.get_value("scanner", "performance-mode") {
            let performance_mode = self.get_bool("scanner", "performance-mode")?
                .unwrap_or(false);
                
            if performance_mode {
                // Apply performance mode presets (match CLI converter)
                config.max_memory_bytes = 256 * 1024 * 1024; // 256MB
                config.queue_size = 5000;
            }
        }
        
        // GS-75 Phase 4: Branch Configuration Support
        // Handle default-branch setting
        if let Some(default_branch) = self.get_value("scanner", "default-branch") {
            config.default_branch = Some(default_branch.clone());
        }
        
        // Handle branch-fallbacks array
        if let Some(fallbacks_str) = self.get_value("scanner", "branch-fallbacks") {
            let fallbacks = self.parse_toml_array(fallbacks_str);
            // Allow empty arrays to override defaults
            config.branch_fallbacks = fallbacks;
        }
        
        // Handle default-remote setting
        if let Some(default_remote) = self.get_value("scanner", "default-remote") {
            config.default_remote = Some(default_remote.clone());
        }
        
        // Validate final configuration
        config.validate()
            .with_context(|| "Scanner configuration validation failed")?;
            
        Ok(config)
    }
    
    /// Get colour configuration from config file
    pub fn get_colour_config(&self) -> Result<ColourConfig> {
        let mut config = ColourConfig::default();
        
        // Check if colours are enabled - now as root-level "color" key
        if let Some(enabled) = self.get_bool_root("color")? {
            config.set_enabled(enabled);
            // If color = false, it means we force disable colors (don't respect NO_COLOR)
            if !enabled {
                config.set_respect_no_color(false);
            }
        }
        
        // Check theme setting - now as root-level "theme" key
        if let Some(theme_str) = self.get_value_root("theme") {
            let theme = match theme_str.to_lowercase().as_str() {
                "auto" => ColourTheme::Auto,
                "light" => ColourTheme::Light,
                "dark" => ColourTheme::Dark,
                "custom" => {
                    // Try to load custom palette from base.colors
                    let palette = self.get_custom_colour_palette()?;
                    ColourTheme::Custom(palette)
                }
                _ => {
                    debug!("Unknown theme '{}', falling back to Auto", theme_str);
                    ColourTheme::Auto
                }
            };
            config.set_theme(theme);
        }
        
        Ok(config)
    }
    
    /// Get custom colour palette from config file
    fn get_custom_colour_palette(&self) -> Result<ColourPalette> {
        let mut palette = ColourPalette::default();
        
        // Read colors from inline table format: colors = { error = "red", warning = "yellow" }
        // This gets parsed as a "colors" section
        if let Some(error_color) = self.get_value("colors", "error") {
            palette.error = error_color.clone();
        }
        
        if let Some(warning_color) = self.get_value("colors", "warning") {
            palette.warning = warning_color.clone();
        }
        
        if let Some(info_color) = self.get_value("colors", "info") {
            palette.info = info_color.clone();
        }
        
        if let Some(debug_color) = self.get_value("colors", "debug") {
            palette.debug = debug_color.clone();
        }
        
        if let Some(success_color) = self.get_value("colors", "success") {
            palette.success = success_color.clone();
        }
        
        if let Some(highlight_color) = self.get_value("colors", "highlight") {
            palette.highlight = highlight_color.clone();
        }
        
        // Validate that all colors are parseable
        for (name, color) in [
            ("error", &palette.error),
            ("warning", &palette.warning),
            ("info", &palette.info),
            ("debug", &palette.debug),
            ("success", &palette.success),
            ("highlight", &palette.highlight),
        ] {
            if ColourPalette::parse_color(color).is_none() {
                return Err(anyhow::anyhow!(
                    "Invalid color '{}' for base.colors.{}", 
                    color, name
                ));
            }
        }
        
        Ok(palette)
    }
    
    /// Get plugin directory from configuration
    pub fn get_plugins_directory(&self) -> Option<&str> {
        self.get_value("plugins", "directory").map(|s| s.as_str())
    }
    
    /// Get list of plugins to explicitly load (bypasses discovery)
    pub fn get_plugins_load(&self) -> Vec<String> {
        self.get_value("plugins", "plugin-load")
            .map(|s| self.parse_toml_array(s))
            .unwrap_or_else(Vec::new)
    }
    
    /// Get list of plugins to exclude by name or path
    pub fn get_plugins_exclude(&self) -> Vec<String> {
        self.get_value("plugins", "plugin-exclude")
            .map(|s| self.parse_toml_array(s))
            .unwrap_or_else(Vec::new)
    }
    
    /// Parse TOML array string back to Vec<String>
    fn parse_toml_array(&self, toml_string: &str) -> Vec<String> {
        // Handle TOML array format stored as string: ["item1", "item2", "item3"]
        // or direct array format: item1, item2, item3
        
        // Try parsing as TOML value first (for ["item1", "item2"] format)
        if let Ok(Value::Array(arr)) = toml_string.parse::<Value>() {
            return arr.into_iter()
                .filter_map(|v| match v {
                    Value::String(s) => Some(s),
                    _ => None,
                })
                .collect();
        }
        
        // Try parsing the string as JSON array (fallback since TOML arrays look like JSON arrays)
        if toml_string.starts_with('[') && toml_string.ends_with(']') {
            if let Ok(arr) = serde_json::from_str::<Vec<String>>(toml_string) {
                return arr;
            }
        }
        
        // Fallback to comma-separated parsing
        if !toml_string.is_empty() {
            return toml_string
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        
        // Final fallback to empty vector
        Vec::new()
    }

    /// Export complete configuration with all available options and their current values
    /// This includes default values for options not explicitly set
    pub fn export_complete_config(&self) -> Result<String> {
        let mut output = String::new();
        
        // Add header comment
        output.push_str("# Complete gstats configuration file\n");
        output.push_str("# Generated with all available configuration options and their current values\n");
        output.push_str("# This file can be used as-is without any changes\n\n");
        
        // Root-level configuration keys (no [base] section)
        if let Some(quiet) = self.get_value_root("quiet") {
            output.push_str(&format!("quiet = {}\n", quiet));
        } else {
            output.push_str("# quiet = false\n");
        }
        
        if let Some(log_format) = self.get_value_root("log-format") {
            output.push_str(&format!("log-format = \"{}\"\n", log_format));
        } else {
            output.push_str("# log-format = \"text\"\n");
        }
        
        if let Some(log_file) = self.get_value_root("log-file") {
            output.push_str(&format!("log-file = \"{}\"\n", log_file));
        } else {
            output.push_str("# log-file = \"/path/to/log/file\"\n");
        }
        
        if let Some(log_level) = self.get_value_root("log-level") {
            output.push_str(&format!("log-level = \"{}\"\n", log_level));
        } else {
            output.push_str("# log-level = \"info\"\n");
        }
        
        // Color configuration as root-level keys
        if let Some(color) = self.get_value_root("color") {
            output.push_str(&format!("color = {}\n", color));
        } else {
            output.push_str("# color = true\n");
        }
        
        if let Some(theme) = self.get_value_root("theme") {
            output.push_str(&format!("theme = \"{}\"\n", theme));
        } else {
            output.push_str("# theme = \"auto\"  # Options: auto, light, dark, custom\n");
        }
        
        // Colors as inline table format at root level
        let color_keys = [
            ("error", "red"),
            ("warning", "yellow"),
            ("info", "blue"),
            ("debug", "bright_black"),
            ("success", "green"),
            ("highlight", "cyan"),
        ];
        
        let mut colors = Vec::new();
        let mut has_custom_colors = false;
        
        for (key, default) in &color_keys {
            if let Some(custom_color) = self.get_value("colors", key) {
                colors.push(format!("{} = \"{}\"", key, custom_color));
                has_custom_colors = true;
            } else {
                colors.push(format!("{} = \"{}\"", key, default));
            }
        }
        
        if has_custom_colors {
            output.push_str(&format!("colors = {{ {} }}\n", colors.join(", ")));
        } else {
            output.push_str(&format!("# colors = {{ {} }}\n", colors.join(", ")));
        }
        
        output.push('\n');
        
        // Scanner configuration section
        output.push_str("[scanner]\n");
        if let Some(max_memory) = self.get_value("scanner", "max-memory") {
            output.push_str(&format!("max-memory = \"{}\"\n", max_memory));
        } else {
            output.push_str("# max-memory = \"64MB\"\n");
        }
        
        if let Some(queue_size) = self.get_value("scanner", "queue-size") {
            output.push_str(&format!("queue-size = {}\n", queue_size));
        } else {
            output.push_str("# queue-size = 1000\n");
        }
        
        if let Some(max_threads) = self.get_value("scanner", "max-threads") {
            output.push_str(&format!("max-threads = {}\n", max_threads));
        } else {
            output.push_str("# max-threads = 4\n");
        }
        
        if let Some(performance_mode) = self.get_value("scanner", "performance-mode") {
            output.push_str(&format!("performance-mode = {}\n", performance_mode));
        } else {
            output.push_str("# performance-mode = false\n");
        }
        
        // GS-75 Phase 4: Branch configuration settings
        if let Some(default_branch) = self.get_value("scanner", "default-branch") {
            output.push_str(&format!("default-branch = \"{}\"\n", default_branch));
        } else {
            output.push_str("# default-branch = \"develop\"\n");
        }
        
        if let Some(fallbacks) = self.get_value("scanner", "branch-fallbacks") {
            output.push_str(&format!("branch-fallbacks = {}\n", fallbacks));
        } else {
            output.push_str("# branch-fallbacks = [\"main\", \"master\", \"develop\", \"trunk\"]\n");
        }
        
        if let Some(default_remote) = self.get_value("scanner", "default-remote") {
            output.push_str(&format!("default-remote = \"{}\"\n", default_remote));
        } else {
            output.push_str("# default-remote = \"origin\"\n");
        }
        
        output.push('\n');
        
        // Plugins configuration section
        output.push_str("[plugins]\n");
        if let Some(directory) = self.get_plugins_directory() {
            output.push_str(&format!("directory = \"{}\"\n", directory));
        } else {
            output.push_str("# directory = \"plugins\"\n");
        }
        
        let plugin_load = self.get_plugins_load();
        if !plugin_load.is_empty() {
            output.push_str(&format!("plugin-load = {:?}\n", plugin_load));
        } else {
            output.push_str("# plugin-load = [\"plugin1\", \"plugin2\"]\n");
        }
        
        let plugin_exclude = self.get_plugins_exclude();
        if !plugin_exclude.is_empty() {
            output.push_str(&format!("plugin-exclude = {:?}\n", plugin_exclude));
        } else {
            output.push_str("# plugin-exclude = [\"unwanted-plugin\"]\n");
        }
        output.push('\n');
        
        // Module-specific configurations (example modules)
        output.push_str("# Module-specific configurations\n");
        output.push_str("# [module.commits]\n");
        output.push_str("# since = \"30d\"\n");
        output.push_str("# per-day = true\n");
        output.push_str("# format = \"json\"\n\n");
        
        output.push_str("# [module.metrics]\n");
        output.push_str("# complexity-threshold = 10\n");
        output.push_str("# include-tests = false\n\n");
        
        output.push_str("# [module.export]\n");
        output.push_str("# default-format = \"json\"\n");
        output.push_str("# output-file = \"gstats-report\"\n");
        
        Ok(output)
    }
}

/// Discover configuration files in order of precedence
fn discover_config_files() -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    
    // 1. Environment variable $GSTATS_CONFIG
    if let Ok(env_path) = env::var("GSTATS_CONFIG") {
        paths.push(PathBuf::from(env_path));
    }
    
    // 2. XDG config directory
    if let Some(config_dir) = dirs::config_dir() {
        paths.push(config_dir.join("gstats").join("config.toml"));
    }
    
    // 3. Home directory
    if let Some(home_dir) = dirs::home_dir() {
        paths.push(home_dir.join(".gstats.toml"));
    }
    
    // 4. Project local
    paths.push(PathBuf::from("./.gstats.toml"));
    
    debug!("Config discovery paths: {:?}", paths);
    Ok(paths)
}

/// Parse TOML content to string-based configuration
fn parse_toml_config(content: &str) -> Result<Configuration> {
    let toml_value: Value = content.parse()
        .context("Failed to parse TOML content")?;
    
    let mut config = Configuration::new();
    
    if let Value::Table(table) = toml_value {
        flatten_toml_table(&table, String::new(), &mut config);
    }
    
    debug!("Parsed configuration: {:?}", config);
    Ok(config)
}

/// Recursively flatten TOML tables into section.subsection format
fn flatten_toml_table(table: &toml::Table, prefix: String, config: &mut Configuration) {
    for (key, value) in table {
        let section_name = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };
        
        match value {
            Value::Table(subtable) => {
                // This is a nested table - check if it contains only key-value pairs
                if subtable.values().all(|v| !matches!(v, Value::Table(_))) {
                    // This is a leaf table (configuration section)
                    let mut section_map = HashMap::new();
                    for (subkey, subvalue) in subtable {
                        section_map.insert(subkey.clone(), toml_value_to_string(subvalue));
                    }
                    config.insert(section_name, section_map);
                } else {
                    // This table contains other tables - continue flattening
                    flatten_toml_table(subtable, section_name, config);
                }
            }
            _ => {
                // This is a direct key-value pair - store as single-value section
                let mut section_map = HashMap::new();
                section_map.insert("value".to_string(), toml_value_to_string(value));
                config.insert(section_name, section_map);
            }
        }
    }
}

/// Convert TOML Value to string representation
fn toml_value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Array(_) | Value::Table(_) => {
            // For complex types, use TOML representation
            value.to_string()
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn test_toml_value_to_string_conversion() {
        assert_eq!(toml_value_to_string(&Value::String("test".to_string())), "test");
        assert_eq!(toml_value_to_string(&Value::Integer(42)), "42");
        assert_eq!(toml_value_to_string(&Value::Float(3.14)), "3.14");
        assert_eq!(toml_value_to_string(&Value::Boolean(true)), "true");
        assert_eq!(toml_value_to_string(&Value::Boolean(false)), "false");
    }

    #[test]
    fn test_parse_toml_config() {
        let toml_content = r#"
[base]
quiet = true
log-format = "json"
log-file = "/tmp/gstats.log"

[module.commits]
since = "30d"
per-day = true
format = "json"
"#;
        
        let config = parse_toml_config(toml_content).unwrap();
        
        assert!(config.contains_key("base"), "Config should contain 'base' section");
        assert_eq!(config.get("base").unwrap().get("quiet").unwrap(), "true");
        assert_eq!(config.get("base").unwrap().get("log-format").unwrap(), "json");
        assert_eq!(config.get("base").unwrap().get("log-file").unwrap(), "/tmp/gstats.log");
        
        assert!(config.contains_key("module.commits"), "Config should contain 'module.commits' section");
        assert_eq!(config.get("module.commits").unwrap().get("since").unwrap(), "30d");
        assert_eq!(config.get("module.commits").unwrap().get("per-day").unwrap(), "true");
        assert_eq!(config.get("module.commits").unwrap().get("format").unwrap(), "json");
    }

    #[test]
    fn test_plugins_config_parsing() {
        let toml_content = r#"
[plugins]
directory = "/custom/plugins"
plugin-load = ["plugin1", "plugin2"]
plugin-exclude = ["plugin3", "plugin4"]
"#;
        
        let config = parse_toml_config(toml_content).unwrap();
        let manager = ConfigManager::from_config(config);
        
        // Test plugin directory
        assert_eq!(manager.get_plugins_directory(), Some("/custom/plugins"));
        
        // Test plugin-load list
        let plugin_load = manager.get_plugins_load();
        assert_eq!(plugin_load.len(), 2);
        assert!(plugin_load.contains(&"plugin1".to_string()));
        assert!(plugin_load.contains(&"plugin2".to_string()));
        
        // Test plugin-exclude list
        let plugin_exclude = manager.get_plugins_exclude();
        assert_eq!(plugin_exclude.len(), 2);
        assert!(plugin_exclude.contains(&"plugin3".to_string()));
        assert!(plugin_exclude.contains(&"plugin4".to_string()));
    }

    #[test]
    fn test_plugins_config_defaults() {
        let manager = ConfigManager::from_config(Configuration::new());
        
        assert_eq!(manager.get_plugins_directory(), None);
        assert!(manager.get_plugins_load().is_empty());
        assert!(manager.get_plugins_exclude().is_empty());
    }

    #[test]
    fn test_config_manager_value_retrieval() {
        let mut config = Configuration::new();
        
        let mut base_section = HashMap::new();
        base_section.insert("quiet".to_string(), "true".to_string());
        base_section.insert("log-format".to_string(), "text".to_string());
        config.insert("base".to_string(), base_section);
        
        let mut module_section = HashMap::new();
        module_section.insert("since".to_string(), "30d".to_string());
        module_section.insert("log-format".to_string(), "json".to_string()); // Override base
        config.insert("module.commits".to_string(), module_section);
        
        let manager = ConfigManager::from_config(config);
        
        assert_eq!(manager.get_value("module.commits", "quiet").unwrap(), "true");
        assert_eq!(manager.get_value("module.commits", "log-format").unwrap(), "json");
        assert_eq!(manager.get_value("module.commits", "since").unwrap(), "30d");
        assert!(manager.get_value("module.commits", "missing").is_none());
    }

    #[test]
    fn test_config_manager_section_selection() {
        let mut config = Configuration::new();
        
        let mut base_section = HashMap::new();
        base_section.insert("format".to_string(), "text".to_string());
        config.insert("base".to_string(), base_section);
        
        let mut selected_section = HashMap::new();
        selected_section.insert("format".to_string(), "json".to_string());
        config.insert("special".to_string(), selected_section);
        
        let mut manager = ConfigManager {
            config,
            _config_file_path: None,
            selected_section: None,
        };
        
        assert_eq!(manager.get_value("base", "format").unwrap(), "text");
        
        manager.select_section("special".to_string());
        assert_eq!(manager.get_value("base", "format").unwrap(), "json");
    }

    #[test]
    fn test_config_manager_type_conversion() {
        let mut config = Configuration::new();
        
        let mut base_section = HashMap::new();
        base_section.insert("debug".to_string(), "true".to_string());
        base_section.insert("quiet".to_string(), "false".to_string());
        base_section.insert("invalid-bool".to_string(), "maybe".to_string());
        base_section.insert("log-level".to_string(), "info".to_string());
        base_section.insert("invalid-level".to_string(), "invalid".to_string());
        base_section.insert("path".to_string(), "/tmp/test".to_string());
        config.insert("base".to_string(), base_section);
        
        let manager = ConfigManager::from_config(config);
        
        assert_eq!(manager.get_bool("base", "debug").unwrap().unwrap(), true);
        assert_eq!(manager.get_bool("base", "quiet").unwrap().unwrap(), false);
        assert!(manager.get_bool("base", "invalid-bool").is_err());
        assert!(manager.get_bool("base", "missing").unwrap().is_none());
        
        assert_eq!(manager.get_log_level("base", "log-level").unwrap().unwrap(), log::LevelFilter::Info);
        assert!(manager.get_log_level("base", "invalid-level").is_err());
        assert!(manager.get_log_level("base", "missing").unwrap().is_none());
        
        assert_eq!(manager.get_path("base", "path").unwrap(), PathBuf::from("/tmp/test"));
        assert!(manager.get_path("base", "missing").is_none());
    }

    #[test] 
    fn test_config_file_loading() {
        let toml_content = r#"
quiet = true
log-format = "json"

[module.commits]
since = "30d"
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        
        assert_eq!(manager.get_value_root("quiet").unwrap(), "true");
        assert_eq!(manager.get_value_root("log-format").unwrap(), "json");
        assert_eq!(manager.get_value("module.commits", "since").unwrap(), "30d");
        assert_eq!(manager._config_file_path.as_ref().unwrap(), temp_file.path());
    }

    #[test]
    fn test_scanner_config_default() {
        let manager = ConfigManager {
            config: Configuration::new(),
            _config_file_path: None,
            selected_section: None,
        };
        
        let scanner_config = manager.get_scanner_config().unwrap();
        assert_eq!(scanner_config.max_memory_bytes, 64 * 1024 * 1024); // Default 64MB
        assert_eq!(scanner_config.queue_size, 1000); // Default 1000
        assert!(scanner_config.max_threads.is_none()); // Default None
    }

    #[test]
    fn test_scanner_config_from_toml() {
        let toml_content = r#"
[scanner]
max-memory = "128MB"
queue-size = 2000
max-threads = 8
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let scanner_config = manager.get_scanner_config().unwrap();
        
        assert_eq!(scanner_config.max_memory_bytes, 128 * 1024 * 1024); // 128MB
        assert_eq!(scanner_config.queue_size, 2000);
        assert_eq!(scanner_config.max_threads, Some(8));
    }

    #[test]
    fn test_scanner_config_performance_mode() {
        let toml_content = r#"
[scanner]
performance-mode = true
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let scanner_config = manager.get_scanner_config().unwrap();
        
        // Performance mode presets
        assert_eq!(scanner_config.max_memory_bytes, 256 * 1024 * 1024); // 256MB
        assert_eq!(scanner_config.queue_size, 5000);
    }

    #[test]
    fn test_scanner_config_invalid_values() {
        let toml_content = r#"
[scanner]
max-memory = "invalid"
queue-size = 0
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        
        // Should fail due to invalid memory value
        assert!(manager.get_scanner_config().is_err());
    }

    #[test]
    fn test_scanner_config_mixed_units() {
        let toml_content = r#"
[scanner]
max-memory = "1GB"
queue-size = 3000
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let scanner_config = manager.get_scanner_config().unwrap();
        
        assert_eq!(scanner_config.max_memory_bytes, 1024 * 1024 * 1024); // 1GB
        assert_eq!(scanner_config.queue_size, 3000);
    }

    // GS-75 Phase 4: Branch Configuration Tests (RED)
    #[test]
    fn test_scanner_config_branch_settings_from_toml() {
        let toml_content = r#"
[scanner]
default-branch = "develop"
branch-fallbacks = ["main", "master", "develop", "trunk"]
default-remote = "upstream"
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let scanner_config = manager.get_scanner_config().unwrap();
        
        // These should fail until branch configuration parsing is implemented
        assert_eq!(scanner_config.default_branch, Some("develop".to_string()));
        assert_eq!(scanner_config.branch_fallbacks, vec!["main", "master", "develop", "trunk"]);
        assert_eq!(scanner_config.default_remote, Some("upstream".to_string()));
    }

    #[test]
    fn test_scanner_config_branch_settings_defaults() {
        let manager = ConfigManager {
            config: Configuration::new(),
            _config_file_path: None,
            selected_section: None,
        };
        
        let scanner_config = manager.get_scanner_config().unwrap();
        
        // These should use defaults when not specified in config
        assert_eq!(scanner_config.default_branch, None);
        assert_eq!(scanner_config.branch_fallbacks, vec!["main", "master", "develop", "trunk"]);
        assert_eq!(scanner_config.default_remote, None);
    }

    #[test]
    fn test_scanner_config_partial_branch_settings() {
        let toml_content = r#"
[scanner]
default-branch = "develop"
# Leave fallbacks and remote as defaults
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let scanner_config = manager.get_scanner_config().unwrap();
        
        // Only default-branch should be set, others should use defaults
        assert_eq!(scanner_config.default_branch, Some("develop".to_string()));
        assert_eq!(scanner_config.branch_fallbacks, vec!["main", "master", "develop", "trunk"]);
        assert_eq!(scanner_config.default_remote, None);
    }

    #[test]
    fn test_scanner_config_empty_branch_fallbacks() {
        let toml_content = r#"
[scanner]
branch-fallbacks = []
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let scanner_config = manager.get_scanner_config().unwrap();
        
        // Should handle empty branch fallbacks array
        assert_eq!(scanner_config.branch_fallbacks, Vec::<String>::new());
    }

    #[test]
    fn test_scanner_config_mixed_branch_and_performance() {
        let toml_content = r#"
[scanner]
default-branch = "develop"
default-remote = "upstream"
performance-mode = true
max-threads = 8
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let scanner_config = manager.get_scanner_config().unwrap();
        
        // Should handle mixed branch and performance settings
        assert_eq!(scanner_config.default_branch, Some("develop".to_string()));
        assert_eq!(scanner_config.default_remote, Some("upstream".to_string()));
        assert_eq!(scanner_config.max_threads, Some(8));
        // Performance mode presets
        assert_eq!(scanner_config.max_memory_bytes, 256 * 1024 * 1024); // 256MB
        assert_eq!(scanner_config.queue_size, 5000);
    }

    #[test]
    fn test_export_complete_config_with_branch_settings() {
        let toml_content = r#"
[scanner]
default-branch = "develop"
branch-fallbacks = ["develop", "main"]
default-remote = "upstream"
max-memory = "128MB"
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let exported = manager.export_complete_config().unwrap();
        
        // Check that branch settings are present in export
        assert!(exported.contains("default-branch = \"develop\""));
        assert!(exported.contains("branch-fallbacks = [\"develop\", \"main\"]"));
        assert!(exported.contains("default-remote = \"upstream\""));
        assert!(exported.contains("max-memory = \"128MB\""));
        
        // Should not have commented defaults for set values
        assert!(!exported.contains("# default-branch = "));
        assert!(!exported.contains("# default-remote = "));
    }

    #[test]
    fn test_colour_config_default() {
        let manager = ConfigManager {
            config: Configuration::new(),
            _config_file_path: None,
            selected_section: None,
        };
        
        let colour_config = manager.get_colour_config().unwrap();
        assert!(colour_config.enabled); // Default is enabled
        assert_eq!(colour_config.theme, ColourTheme::Auto);
        assert!(colour_config.respect_no_color); // Default respects NO_COLOR
    }

    #[test]
    fn test_colour_config_from_toml() {
        let toml_content = r#"
color = true
theme = "dark"
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let colour_config = manager.get_colour_config().unwrap();
        
        assert!(colour_config.enabled);
        assert_eq!(colour_config.theme, ColourTheme::Dark);
    }

    #[test]
    fn test_colour_config_disabled() {
        let toml_content = r#"
color = false
theme = "light"
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let colour_config = manager.get_colour_config().unwrap();
        
        assert!(!colour_config.enabled);
        assert_eq!(colour_config.theme, ColourTheme::Light);
    }

    #[test]
    fn test_colour_config_custom_theme() {
        let toml_content = r#"
theme = "custom"
colors = { error = "bright_red", warning = "bright_yellow", info = "bright_blue", debug = "white", success = "bright_green", highlight = "bright_cyan" }
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let colour_config = manager.get_colour_config().unwrap();
        
        if let ColourTheme::Custom(palette) = colour_config.theme {
            assert_eq!(palette.error, "bright_red");
            assert_eq!(palette.warning, "bright_yellow");
            assert_eq!(palette.info, "bright_blue");
            assert_eq!(palette.debug, "white");
            assert_eq!(palette.success, "bright_green");
            assert_eq!(palette.highlight, "bright_cyan");
        } else {
            panic!("Expected Custom theme, got {:?}", colour_config.theme);
        }
    }

    #[test]
    fn test_colour_config_invalid_theme() {
        let toml_content = r#"
[base]
theme = "unknown"
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let colour_config = manager.get_colour_config().unwrap();
        
        // Unknown theme should fall back to Auto
        assert_eq!(colour_config.theme, ColourTheme::Auto);
    }

    #[test]
    fn test_colour_config_invalid_custom_color() {
        let toml_content = r#"
theme = "custom"
colors = { error = "invalid_color", warning = "yellow" }
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let result = manager.get_colour_config();
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid color 'invalid_color'"));
    }

    #[test]
    fn test_colour_config_partial_custom_palette() {
        let toml_content = r#"
theme = "custom"
colors = { error = "bright_red", success = "bright_green" }
# Other colors should use defaults
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let colour_config = manager.get_colour_config().unwrap();
        
        if let ColourTheme::Custom(palette) = colour_config.theme {
            // Custom colors
            assert_eq!(palette.error, "bright_red");
            assert_eq!(palette.success, "bright_green");
            // Default colors should remain
            assert_eq!(palette.warning, "yellow");
            assert_eq!(palette.info, "blue");
            assert_eq!(palette.debug, "bright_black");
            assert_eq!(palette.highlight, "cyan");
        } else {
            panic!("Expected Custom theme");
        }
    }

    #[test]
    fn test_colour_config_no_color_inversion() {
        let toml_content = r#"
[base]
color = false
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let colour_config = manager.get_colour_config().unwrap();
        
        // color = false should set respect_no_color = false
        assert!(!colour_config.respect_no_color);
    }
    
    #[test]
    fn test_export_complete_config_default() {
        let manager = ConfigManager {
            config: Configuration::new(),
            _config_file_path: None,
            selected_section: None,
        };
        
        let exported = manager.export_complete_config().unwrap();
        
        // Check that it contains expected sections (no [base] section, just [scanner] and [plugins])
        assert!(exported.contains("[scanner]"));
        assert!(exported.contains("[plugins]"));
        
        // Check that it contains default values as comments
        assert!(exported.contains("# quiet = false"));
        assert!(exported.contains("# max-memory = \"64MB\""));
        assert!(exported.contains("# color = true"));
        assert!(exported.contains("# theme = \"auto\""));
        assert!(exported.contains("# directory = \"plugins\""));
        assert!(exported.contains("# plugin-load = [\"plugin1\", \"plugin2\"]"));
        assert!(exported.contains("# plugin-exclude = [\"unwanted-plugin\"]"));
    }
    
    #[test]
    fn test_export_complete_config_with_values() {
        let toml_content = r#"
quiet = true
log-format = "json"
color = false
theme = "dark"

[scanner]
max-memory = "128MB"
queue-size = 2000
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let exported = manager.export_complete_config().unwrap();
        
        // Check that actual values are present (not commented)
        assert!(exported.contains("quiet = true"));
        assert!(exported.contains("log-format = \"json\""));
        assert!(exported.contains("max-memory = \"128MB\""));
        assert!(exported.contains("queue-size = 2000"));
        assert!(exported.contains("color = false"));
        assert!(exported.contains("theme = \"dark\""));
    }
    
    #[test]
    fn test_export_complete_config_with_custom_colors() {
        let toml_content = r#"
theme = "custom"
colors = { error = "bright_red", warning = "bright_yellow" }
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let exported = manager.export_complete_config().unwrap();
        
        // Check that custom colors are present in inline table format
        assert!(exported.contains("theme = \"custom\""));
        assert!(exported.contains("colors = {"));
        assert!(exported.contains("error = \"bright_red\""));
        assert!(exported.contains("warning = \"bright_yellow\""));
        // Default colors should be included in the inline format
        assert!(exported.contains("info = \"blue\""));
    }

    #[test]
    fn test_export_complete_config_with_plugins() {
        let toml_content = r#"
[plugins]
directory = "/custom/plugins"
plugin-load = ["custom-plugin1", "custom-plugin2"]
plugin-exclude = ["bad-plugin"]
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        let exported = manager.export_complete_config().unwrap();
        
        // Check that plugins section is present with actual values
        assert!(exported.contains("[plugins]"));
        assert!(exported.contains("directory = \"/custom/plugins\""));
        assert!(exported.contains("plugin-load = [\"custom-plugin1\", \"custom-plugin2\"]"));
        assert!(exported.contains("plugin-exclude = [\"bad-plugin\"]"));
        
        // Verify no commented defaults are present when values are set
        assert!(!exported.contains("# directory = \"plugins\""));
        assert!(!exported.contains("# plugin-load = [\"plugin1\", \"plugin2\"]"));
        assert!(!exported.contains("# plugin-exclude = [\"unwanted-plugin\"]"));
    }
}
