//! Markdown export format implementation

use crate::plugin::{PluginResult, PluginInfo};
use crate::scanner::messages::ScanMessage;
use crate::plugin::builtin::export::config::ExportConfig;

/// Export data as Markdown
pub fn export_markdown(
    _config: &ExportConfig,
    _collected_data: &[ScanMessage],
    _data_to_export: &[&ScanMessage],
    _info: &PluginInfo,
) -> PluginResult<String> {
    // TODO: Implement Markdown export
    Ok("# Git Analytics Report\n\n*Markdown export not yet implemented*".to_string())
}
