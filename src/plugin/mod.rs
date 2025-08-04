//! Plugin System Module
//! 
//! Provides a trait-based interface for plugin communication with async notifications.
//! Supports dynamic plugin loading, version compatibility, and real-time notifications.
//! 
//! # Example Usage
//! 
//! ```no_run
//! use gstats::plugin::{Plugin, PluginRegistry, PluginInfo};
//! use gstats::scanner::modes::ScanMode;
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
pub mod context;
pub mod registry;
pub mod notification;
pub mod compatibility;
pub mod discovery;
pub mod executor;
pub mod builtin;

#[cfg(test)]
pub mod tests;

// Re-export core types for easier access
pub use traits::{Plugin, ScannerPlugin, NotificationPlugin, PluginLifecycle, PluginFunction};
pub use error::{PluginError, PluginResult};
pub use context::{PluginContext, PluginRequest, PluginResponse, InvocationType};

// Plugin metadata and info
pub use traits::{PluginInfo, PluginDependency, PluginCapability};

// Registry and management
pub use registry::{PluginRegistry, SharedPluginRegistry};
pub use notification::AsyncNotificationManager;
pub use compatibility::VersionCompatibilityChecker;
pub use discovery::PluginDiscovery;
pub use executor::{PluginExecutor, PluginMessageProcessor, PluginStreamExt};