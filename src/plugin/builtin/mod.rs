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
pub fn get_builtin_plugins() -> Vec<&'static str> {
    vec!["debug", "commits", "metrics", "export"]
}

/// Create a built-in plugin by name
pub async fn create_builtin_plugin(name: &str) -> Option<Box<dyn crate::plugin::Plugin>> {
    match name {
        "debug" => Some(Box::new(DebugPlugin::new())),
        "commits" => Some(Box::new(CommitsPlugin::new())),
        "metrics" => Some(Box::new(MetricsPlugin::new())),
        "export" => Some(Box::new(ExportPlugin::new())),
        _ => None,
    }
}
