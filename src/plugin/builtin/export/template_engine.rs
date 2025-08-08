//! Template engine for custom output formatting using Tera (Jinja2-like syntax)

use crate::plugin::{PluginResult, PluginError};
use std::collections::HashMap;
use std::path::Path;
use tera::{Tera, Context};

/// Template engine for custom output formatting using Tera (Jinja2-like syntax)
pub struct TemplateEngine {
    tera: Tera,
    template_path: Option<std::path::PathBuf>,
    pub template_vars: HashMap<String, String>,
}

impl TemplateEngine {
    pub fn new() -> Self {
        let mut tera = Tera::default();
        // Disable auto-escaping by default (users can use |safe or |escape filters as needed)
        tera.autoescape_on(vec![]);
        
        Self {
            tera,
            template_path: None,
            template_vars: HashMap::new(),
        }
    }
    
    pub fn load_template(&mut self, template_path: &Path) -> PluginResult<()> {
        let content = std::fs::read_to_string(template_path)
            .map_err(|e| PluginError::execution_failed(format!("Failed to read template file: {}", e)))?;
        
        // Add the template to Tera with a name based on the file path
        let template_name = template_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("template");
            
        self.tera.add_raw_template(template_name, &content)
            .map_err(|e| PluginError::configuration_error(format!("Template syntax error: {}", e)))?;
            
        self.template_path = Some(template_path.to_path_buf());
        
        // Register custom filters
        self.register_custom_filters();
        
        Ok(())
    }
    
    fn register_custom_filters(&mut self) {
        // Register a number formatting filter
        self.tera.register_filter("number_format", |value: &tera::Value, _: &HashMap<String, tera::Value>| {
            match value {
                tera::Value::Number(n) => {
                    if let Some(i) = n.as_u64() {
                        // Simple thousands separator formatting
                        let s = i.to_string();
                        let mut result = String::new();
                        let chars: Vec<char> = s.chars().collect();
                        for (pos, ch) in chars.iter().rev().enumerate() {
                            if pos > 0 && pos % 3 == 0 {
                                result.push(',');
                            }
                            result.push(*ch);
                        }
                        Ok(tera::Value::String(result.chars().rev().collect()))
                    } else if let Some(f) = n.as_f64() {
                        Ok(tera::Value::String(format!("{:.2}", f)))
                    } else {
                        Ok(value.clone())
                    }
                }
                _ => Ok(value.clone()),
            }
        });
        
        // Register a percentage filter
        self.tera.register_filter("percentage", |value: &tera::Value, args: &HashMap<String, tera::Value>| {
            let precision = args.get("precision")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as usize;
            
            match value {
                tera::Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        Ok(tera::Value::String(format!("{:.precision$}%", f, precision = precision)))
                    } else {
                        Ok(value.clone())
                    }
                }
                _ => Ok(value.clone()),
            }
        });
        
        // Register a simple date filter (just format timestamp)
        self.tera.register_filter("date", |value: &tera::Value, _: &HashMap<String, tera::Value>| {
            match value {
                tera::Value::String(s) => {
                    // For now, just return a simplified format since we're getting ISO timestamps
                    if let Ok(datetime) = chrono::DateTime::parse_from_rfc3339(s) {
                        Ok(tera::Value::String(datetime.format("%Y-%m-%d %H:%M UTC").to_string()))
                    } else {
                        Ok(value.clone())
                    }
                }
                _ => Ok(value.clone()),
            }
        });
    }
    
    pub fn add_template_var(&mut self, key: String, value: String) {
        self.template_vars.insert(key, value);
    }
    
    pub fn render(&self, data: &serde_json::Value) -> PluginResult<String> {
        // Create a Tera context from the JSON data
        let mut context = Context::new();
        
        // Add all data from the JSON value
        if let Some(obj) = data.as_object() {
            for (key, value) in obj {
                context.insert(key, value);
            }
        }
        
        // Add template variables
        for (key, value) in &self.template_vars {
            context.insert(key, value);
        }
        
        // Get the template name
        let template_name = self.template_path.as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("template");
        
        // Render the template
        self.tera.render(template_name, &context)
            .map_err(|e| PluginError::execution_failed(format!("Template rendering error: {}", e)))
    }
}
