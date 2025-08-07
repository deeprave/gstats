//! Built-in Plugin Implementations
//! 
//! Reference implementations of common plugins for git analytics.

pub mod commits;
pub mod metrics;
pub mod export;
pub mod change_frequency;
pub mod hotspot_detector;
pub mod duplication_detector;
pub mod debt_assessor;
pub mod complexity_calculator;

// Re-export built-in plugins
pub use commits::CommitsPlugin;
pub use metrics::MetricsPlugin;
pub use export::ExportPlugin;

/// Get all built-in plugins as descriptors for registration
pub fn get_builtin_plugins() -> Vec<&'static str> {
    vec!["commits", "metrics", "export"]
}

/// Create a built-in plugin by name
pub async fn create_builtin_plugin(name: &str) -> Option<Box<dyn crate::plugin::Plugin>> {
    match name {
        "commits" => Some(Box::new(CommitsPlugin::new())),
        "metrics" => Some(Box::new(MetricsPlugin::new())),
        "export" => Some(Box::new(ExportPlugin::new())),
        _ => None,
    }
}