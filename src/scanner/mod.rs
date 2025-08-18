//! Repository Scanner Module
//! 
//! Provides scanning capabilities for git repositories with plugin architecture support.
//! Features version compatibility, configurable scanning modes, and efficient filtering.
//! 
//! # Example Usage
//! 
//! ```rust,no_run
//! use gstats::scanner::{ScannerConfig, get_api_version};
//! use gstats::scanner::query::{QueryParams, DateRange, FilePathFilter, AuthorFilter};
//! use std::time::{SystemTime, Duration};
//! 
//! // Check API compatibility
//! let version = get_api_version();
//! println!("Scanner API version: {}", version);
//! 
//! // Configure scanner
//! let config = ScannerConfig::builder()
//!     .with_max_memory(128 * 1024 * 1024)
//!     .with_queue_size(2000)
//!     .build()
//!     .unwrap();
//! 
//! // Build query parameters  
//! let query = QueryParams {
//!     date_range: Some(DateRange::from(SystemTime::now() - Duration::from_secs(86400 * 7))),
//!     file_paths: FilePathFilter::default(),
//!     limit: Some(100),
//!     authors: AuthorFilter::default(),
//!     ..Default::default()
//! };
//! ```

pub mod version;
 
pub mod messages;
pub mod config;
pub mod traits;
pub mod filters;
pub mod query;
pub mod async_engine;
pub mod async_traits;
pub mod branch_detection;
pub mod statistics;

#[cfg(test)]
mod tests;

// Re-export core types for easier access
pub use config::ScannerConfig;
pub use traits::MessageProducer;
pub use version::{get_api_version, is_api_compatible};
pub use query::QueryParams;
pub use async_engine::AsyncScannerEngineBuilder;

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
