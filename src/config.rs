use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};
use toml::Value;
use log::{debug, info};
use crate::scanner::config::ScannerConfig;

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
        
        info!("No configuration file found, using empty configuration");
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
        
        if let Some(value) = self.config.get(section).and_then(|s| s.get(key)) {
            return Some(value);
        }
        
        self.config.get("base").and_then(|s| s.get(key))
    }
    
    /// Select configuration section for --config-name
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
        
        // Validate final configuration
        config.validate()
            .with_context(|| "Scanner configuration validation failed")?;
            
        Ok(config)
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
                // This is a direct key-value pair (e.g., in [base] section)
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
[base]
quiet = true
log-format = "json"

[module.commits]
since = "30d"
"#;
        
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, toml_content).unwrap();
        
        let manager = ConfigManager::load_from_file(temp_file.path().to_path_buf()).unwrap();
        
        assert_eq!(manager.get_value("base", "quiet").unwrap(), "true");
        assert_eq!(manager.get_value("base", "log-format").unwrap(), "json");
        assert_eq!(manager.get_value("module.commits", "since").unwrap(), "30d");
        assert_eq!(manager.config_file_path.as_ref().unwrap(), temp_file.path());
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
}
