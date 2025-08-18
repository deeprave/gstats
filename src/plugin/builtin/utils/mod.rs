//! Utility modules for builtin plugins
//! 
//! This module contains various utility functions and types used by the builtin plugins
//! for analysis, detection, and processing of git repository data.
//! 
//! NOTE: These utilities are being migrated to event-driven processors in the scanner module.
//! They will eventually be moved to their appropriate plugin modules.

pub mod complexity_calculator;
pub mod duplication_detector;
pub mod format_detection;
pub mod hotspot_detector;

// Re-export main types and functions for convenience
