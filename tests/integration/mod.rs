//! Integration Tests Module
//! 
//! Contains comprehensive integration tests for the gstats data export system.
//! These tests verify end-to-end functionality across the entire pipeline.

pub mod data_export_integration;
pub mod notification_export_pipeline;
pub mod export_formatting_tests;

// Re-export common test utilities
// pub use data_export_integration::*;
// pub use notification_export_pipeline::*;