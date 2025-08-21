//! Plugin System Module
//! 
//! Provides a trait-based interface for plugin communication with async notifications.
//! Supports dynamic plugin loading, version compatibility, and real-time notifications.
//! 
//! # Example Usage
//! 
//! ```no_run
//! use gstats::plugin::{Plugin, PluginRegistry, PluginInfo};
//! 
//! // Plugin registry for managing plugins
//! let mut registry = PluginRegistry::new();
//! 
//! // Register a plugin (example with built-in plugin)
//! // let commits_plugin = builtin::CommitsPlugin::new();
//! // registry.register_plugin(Box::new(commits_plugin)).await?;
//! ```

pub mod traits;
pub mod error;
pub mod settings;
pub mod context;
pub mod registry;
pub mod notification;
pub mod compatibility;
pub mod discovery;
pub mod subscriber;
pub mod manager;
pub(crate) mod builtin;  // Make builtin module private to crate

// Expose export plugin publicly for use in applications and tests
pub use builtin::export::{ExportPlugin, ExportConfig, ExportFormat};
pub mod processors;
pub mod priority_queue;
pub mod data_export;
pub mod data_coordinator;

#[cfg(test)]
pub mod tests;

// Re-export core types for easier access
pub use traits::Plugin;
pub use error::{PluginError, PluginResult};
pub use context::{PluginContext, PluginRequest, PluginResponse, InvocationType};
pub use settings::PluginSettings;

// Plugin metadata and info
pub use traits::PluginInfo;

// Registry and management
#[allow(unused_imports)]
pub use registry::{PluginRegistry, SharedPluginRegistry};
pub use manager::PluginManager;
