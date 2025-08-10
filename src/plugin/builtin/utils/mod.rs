//! Utility modules for builtin plugins
//! 
//! This module contains various utility functions and types used by the builtin plugins
//! for analysis, detection, and processing of git repository data.

pub mod change_frequency;
pub mod complexity_calculator;
pub mod debt_assessor;
pub mod duplication_detector;
pub mod format_detection;
pub mod hotspot_detector;

// Re-export main types and functions for convenience
// Removed unused wildcard exports - these utilities are only used internally
// pub use change_frequency::*;
// pub use complexity_calculator::*;
// pub use debt_assessor::*;
// pub use duplication_detector::*;
// pub use format_detection::*;
// pub use hotspot_detector::*;
