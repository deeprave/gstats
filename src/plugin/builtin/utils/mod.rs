//! Utility modules for builtin plugins
//! 
//! This module contains various utility functions and types used by the builtin plugins
//! for analysis, detection, and processing of git repository data.
//! 
//! NOTE: These utilities are being migrated to event-driven processors in the scanner module.
//! They will eventually be moved to their appropriate plugin modules.

pub mod complexity_calculator;
pub mod debt_assessor;
pub mod duplication_detector;
pub mod format_detection;
pub mod hotspot_detector;

// Re-export main types and functions for convenience
// Removed unused wildcard exports - these utilities are only used internally
// pub use complexity_calculator::*;
// pub use debt_assessor::*;
// pub use duplication_detector::*;
// pub use format_detection::*;
// pub use hotspot_detector::*;
