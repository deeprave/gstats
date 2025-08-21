//! Export Format Implementations
//! 
//! This module contains format-specific implementations for exporting plugin data.
//! All formats work with `PluginDataExport` as the common data structure.

pub mod console;
pub mod json;
pub mod csv;
pub mod xml;
pub mod yaml;
pub mod html;
pub mod markdown;
pub mod template;

use crate::plugin::PluginResult;
use crate::plugin::data_export::PluginDataExport;
use std::sync::Arc;

/// Trait for format-specific exporters
pub trait FormatExporter {
    /// Format the plugin data for export
    fn format_data(&self, data: &[Arc<PluginDataExport>]) -> PluginResult<String>;
}

