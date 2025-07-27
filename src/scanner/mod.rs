//! Repository Scanner Module
//! 
//! Provides scanning capabilities for git repositories with plugin architecture support.
//! Features version compatibility, configurable scanning modes, and efficient filtering.

pub mod version;
pub mod modes; 
pub mod messages;
pub mod config;
pub mod traits;
pub mod filters;
pub mod query;

// Re-export core types for easier access
pub use config::ScannerConfig;
pub use traits::{Scanner, MessageProducer, ScanProcessor, ScanFilter, VersionCompatible, ScanAggregator};
pub use modes::get_supported_modes;
pub use version::{get_api_version, get_version_info, is_api_compatible};
pub use query::{QueryParams, QueryBuilder, DateRange, FilePathFilter, AuthorFilter, QueryValidationError};

// Module metadata
pub const MODULE_NAME: &str = "Repository Scanner";
pub const MODULE_VERSION: &str = "1.0.0";

/// Check if a given API version is compatible with the current implementation
/// 
/// # Arguments
/// * `version` - The API version to check (days since epoch)
/// 
/// # Returns
/// * `bool` - True if the version is compatible, false otherwise
pub fn is_compatible_version(version: i64) -> bool {
    let current_version = get_api_version();
    
    // Allow current version and up to 30 days backward compatibility
    version <= current_version && version >= (current_version - 30)
}
