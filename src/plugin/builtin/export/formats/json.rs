//! JSON export format implementation

use crate::plugin::{PluginResult, PluginError, PluginInfo};
use crate::scanner::messages::ScanMessage;
use crate::plugin::builtin::export::config::ExportConfig;
use super::common::{is_commit_data, generate_authors_summary};
use std::collections::HashMap;
use serde_json::json;

/// Export data as JSON
pub fn export_json(
    config: &ExportConfig,
    collected_data: &[ScanMessage],
    data_to_export: &[&ScanMessage],
    info: &PluginInfo,
) -> PluginResult<String> {
    let mut export_data = HashMap::new();

    if config.include_metadata {
        export_data.insert("metadata", json!({
            "export_timestamp": chrono::Utc::now().to_rfc3339(),
            "total_entries": collected_data.len(),
            "exported_entries": data_to_export.len(),
            "format": "json",
            "plugin_version": info.version,
        }));
    }

    if is_commit_data(data_to_export) {
        // Generate authors summary for JSON
        let authors_summary = generate_authors_json_summary(collected_data, config);
        export_data.insert("authors", authors_summary);
    } else {
        // Regular data export for non-author reports
        let data: Vec<serde_json::Value> = data_to_export.iter()
            .map(|msg| json!({
                "header": {
                    "scan_mode": format!("{:?}", msg.header.scan_mode),
                    "timestamp": msg.header.timestamp,
                },
                "data": msg.data,
            }))
            .collect();

        export_data.insert("scan_results", json!(data));
    }

    serde_json::to_string_pretty(&export_data)
        .map_err(|e| PluginError::execution_failed(format!("JSON serialization failed: {}", e)))
}

/// Generate authors summary for JSON export
fn generate_authors_json_summary(collected_data: &[ScanMessage], config: &ExportConfig) -> serde_json::Value {
    let sorted_authors = generate_authors_summary(collected_data);
    
    // Determine how many recent commits to show (respecting output limit)
    let recent_limit = if config.output_all {
        usize::MAX
    } else {
        3.min(config.max_entries.unwrap_or(3)) // Default to 3 recent commits per author
    };
    
    let authors_json: Vec<serde_json::Value> = sorted_authors.into_iter().map(|(author, (commits, lines_added, lines_removed, recent_commits))| {
        let recent_commits_json: Vec<serde_json::Value> = recent_commits.iter().take(recent_limit).map(|(hash, message, timestamp, files)| {
            // Only include first 8 characters of hash for brevity
            let short_hash = if hash.len() > 8 { &hash[..8] } else { hash };
            
            // Format timestamp as human-readable date
            let datetime = chrono::DateTime::from_timestamp(*timestamp as i64, 0)
                .unwrap_or_else(|| chrono::Utc::now());
            let formatted_date = datetime.format("%Y-%m-%d %H:%M").to_string();
            
            json!({
                "hash": short_hash,
                "message": message,
                "date": formatted_date,
                "files_changed": files
            })
        }).collect();
        
        json!({
            "name": author,
            "total_commits": commits,
            "lines_added": lines_added,
            "lines_removed": lines_removed,
            "recent_commits": recent_commits_json
        })
    }).collect();
    
    json!(authors_json)
}
