//! Built-in Plugin Implementations
//! 
//! Reference implementations of common plugins for git analytics.

pub mod commits;
pub mod metrics;
pub mod export;
pub mod debug;
pub mod utils;

// Re-export built-in plugins
pub use commits::CommitsPlugin;
pub use metrics::MetricsPlugin;
pub use export::ExportPlugin;
pub use debug::DebugPlugin;

/// Get all built-in plugins as descriptors for registration
pub(super) fn get_builtin_plugins() -> Vec<&'static str> {
    vec!["debug", "commits", "metrics", "export"]
}

/// Get the advertised functions for a builtin plugin (INTERNAL USE ONLY)
/// This provides metadata without creating plugin instances
pub(super) fn get_builtin_plugin_functions(name: &str) -> Vec<crate::plugin::traits::PluginFunction> {
    use crate::plugin::traits::PluginFunction;
    
    match name {
        "debug" => vec![
            PluginFunction {
                name: "debug".to_string(),
                aliases: vec!["inspect".to_string(), "stream".to_string()],
                description: "Debug and inspect git repository scan messages".to_string(),
                is_default: true,
            },
        ],
        "commits" => vec![
            PluginFunction {
                name: "commits".to_string(),
                aliases: vec!["history".to_string()],
                description: "Analyze commit history and patterns".to_string(),
                is_default: true,
            },
            PluginFunction {
                name: "authors".to_string(),
                aliases: vec!["contributors".to_string(), "committers".to_string()],
                description: "Analyze author contributions and statistics".to_string(),
                is_default: false,
            },
        ],
        "metrics" => vec![
            PluginFunction {
                name: "metrics".to_string(),
                aliases: vec!["stats".to_string(), "analysis".to_string()],
                description: "Analyze code quality metrics and statistics".to_string(),
                is_default: true,
            },
            PluginFunction {
                name: "complexity".to_string(),
                aliases: vec!["complex".to_string()],
                description: "Calculate code complexity metrics".to_string(),
                is_default: false,
            },
            PluginFunction {
                name: "hotspots".to_string(),
                aliases: vec!["hot".to_string()],
                description: "Identify code hotspots and problem areas".to_string(),
                is_default: false,
            },
        ],
        "export" => vec![
            PluginFunction {
                name: "export".to_string(),
                aliases: vec![],
                description: "Export data in various formats".to_string(),
                is_default: true,
            },
        ],
        _ => vec![],
    }
}

// Removed dead code functions: create_builtin_plugin, create_builtin_plugin_with_settings

/// Create a built-in plugin by name with all required dependencies (REQUIRED)
/// This is the correct way to instantiate plugins - they MUST have notification managers
pub(super) fn create_builtin_plugin_with_dependencies(
    name: &str, 
    settings: &crate::plugin::PluginSettings,
    notification_manager: std::sync::Arc<crate::notifications::AsyncNotificationManager<crate::notifications::events::PluginEvent>>
) -> Option<Box<dyn crate::plugin::Plugin>> {
    match name {
        "debug" => Some(Box::new(DebugPlugin::with_dependencies(settings.clone(), notification_manager))),
        "commits" => Some(Box::new(CommitsPlugin::with_dependencies(settings.clone(), notification_manager))),
        "metrics" => Some(Box::new(MetricsPlugin::with_dependencies(settings.clone(), notification_manager))),
        "export" => Some(Box::new(ExportPlugin::with_dependencies(settings.clone(), notification_manager))),
        _ => None,
    }
}
