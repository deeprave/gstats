//! Repository Scanner Module
//! 
//! Provides scanning capabilities for git repositories with plugin architecture support.
//! Features version compatibility, configurable scanning modes, and efficient filtering.
//! 
//! # Example Usage
//! 
//! ```no_run
//! use gstats::scanner::{QueryBuilder, ScannerConfig, get_api_version};
//! use std::time::{SystemTime, Duration};
//! 
//! // Check API compatibility
//! let version = get_api_version();
//! println!("Scanner API version: {}", version);
//! 
//! // Configure scanner
//! let config = ScannerConfig::builder()
//!     .max_threads(4)
//!     .performance_mode(true)
//!     .build()
//!     .unwrap();
//! 
//! // Build query parameters
//! let query = QueryBuilder::new()
//!     .since(SystemTime::now() - Duration::from_secs(86400 * 7))
//!     .include_path("src/")
//!     .author("developer@example.com")
//!     .limit(100)
//!     .build()
//!     .unwrap();
//! ```

pub mod version;
pub mod modes; 
pub mod messages;
pub mod config;
pub mod traits;
pub mod filters;
pub mod query;
pub mod async_engine;
pub mod async_traits;
pub mod plugin_scanner;
pub mod statistics;

// Re-export core types for easier access
pub use config::ScannerConfig;
pub use traits::{MessageProducer, CallbackMessageProducer};
pub use modes::ScanMode;
pub use version::{get_api_version, is_api_compatible};
pub use query::{QueryParams, QueryBuilder};
pub use async_engine::AsyncScannerEngineBuilder;
pub use plugin_scanner::PluginScannerBuilder;

use anyhow::Result;

// Module metadata
pub const MODULE_NAME: &str = "Repository Scanner";
pub const MODULE_VERSION: &str = "1.0.0";

/// Check if a given API version is compatible with the current implementation
/// 
/// # Arguments
/// * `version` - The API version to check (YYYYMMDD format)
/// 
/// # Returns
/// * `bool` - True if the version is compatible, false otherwise
/// 
/// # Example
/// ```
/// use gstats::scanner::{get_api_version, is_compatible_version};
/// 
/// let current = get_api_version();
/// assert!(is_compatible_version(current));
/// assert!(is_compatible_version(current - 30)); // Recent versions compatible
/// ```
pub fn is_compatible_version(version: i64) -> bool {
    is_api_compatible(version)
}

/// Validate scanner configuration for correctness
/// 
/// # Arguments
/// * `config` - The scanner configuration to validate
/// 
/// # Returns
/// * `Result<()>` - Ok if valid, Err with description if invalid
/// 
/// # Example
/// ```
/// use gstats::scanner::{ScannerConfig, validate_config};
/// 
/// let config = ScannerConfig::default();
/// assert!(validate_config(&config).is_ok());
/// ```
pub fn validate_config(config: &ScannerConfig) -> Result<()> {
    Ok(config.validate()?)
}

/// Validate query parameters for correctness
/// 
/// # Arguments
/// * `params` - The query parameters to validate
/// 
/// # Returns
/// * `Result<()>` - Ok if valid, Err with description if invalid
/// 
/// # Example
/// ```
/// use gstats::scanner::{QueryParams, validate_query_params};
/// 
/// let params = QueryParams::default();
/// assert!(validate_query_params(&params).is_ok());
/// ```
pub fn validate_query_params(params: &QueryParams) -> Result<()> {
    Ok(params.validate()?)
}
