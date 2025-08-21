//! Display module for colour management and visual enhancements
//! 
//! This module provides colour support, progress indicators, and visual formatting
//! for CLI output while maintaining terminal compatibility and accessibility.

pub mod colours;
pub mod config;
pub mod themes;
pub mod progress;
pub mod format;
pub mod table;

pub use colours::*;
pub use config::*;
pub use progress::*;
pub use format::*;
pub use table::*;