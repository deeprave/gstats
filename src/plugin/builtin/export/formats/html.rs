//! HTML export format implementation

use crate::plugin::{PluginResult, PluginInfo};
use crate::scanner::messages::ScanMessage;
use crate::plugin::builtin::export::config::ExportConfig;

/// Export data as HTML
pub fn export_html(
    _config: &ExportConfig,
    _collected_data: &[ScanMessage],
    _data_to_export: &[&ScanMessage],
    _info: &PluginInfo,
) -> PluginResult<String> {
    // TODO: Implement HTML export
    Ok("<!DOCTYPE html>\n<html>\n<head><title>Git Analytics Report</title></head>\n<body>\n<h1>HTML export not yet implemented</h1>\n</body>\n</html>".to_string())
}
