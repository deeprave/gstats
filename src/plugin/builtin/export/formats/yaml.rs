//! YAML export format implementation

use crate::plugin::{PluginResult, PluginInfo};
use crate::scanner::messages::ScanMessage;
use crate::plugin::builtin::export::config::ExportConfig;

/// Export data as YAML
pub fn export_yaml(
    _config: &ExportConfig,
    _collected_data: &[ScanMessage],
    _data_to_export: &[&ScanMessage],
    _info: &PluginInfo,
) -> PluginResult<String> {
    // TODO: Implement YAML export
    Ok("# YAML export not yet implemented\nplaceholder: true".to_string())
}
