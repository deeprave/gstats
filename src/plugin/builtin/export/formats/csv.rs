//! CSV export format implementation

use crate::plugin::{PluginResult, PluginError};
use crate::scanner::messages::ScanMessage;
use crate::plugin::builtin::export::config::ExportConfig;
use super::common::{is_commit_data, generate_authors_summary};

/// Export data as CSV
pub fn export_csv(
    config: &ExportConfig,
    collected_data: &[ScanMessage],
    data_to_export: &[&ScanMessage],
) -> PluginResult<String> {
    let mut csv_content = String::new();
    let delimiter = &config.csv_delimiter;
    let quote_char = &config.csv_quote_char;

    if is_commit_data(data_to_export) {
        // Generate authors summary for CSV
        export_authors_csv_summary(&mut csv_content, collected_data, config, delimiter, quote_char)?;
    } else {
        // Regular data export for non-author reports
        csv_content.push_str(&format!("timestamp{}scan_mode{}data_json\n", delimiter, delimiter));

        for message in data_to_export {
            let timestamp = message.header.timestamp;
            let scan_mode = format!("{:?}", message.header.scan_mode);
            let data_json = serde_json::to_string(&message.data)
                .map_err(|e| PluginError::execution_failed(format!("JSON serialization failed: {}", e)))?;

            // Escape CSV values based on quote character
            let escaped_json = if quote_char == "\"" {
                data_json.replace('"', "\"\"")
            } else {
                data_json.replace(quote_char, &format!("{}{}", quote_char, quote_char))
            };
            
            csv_content.push_str(&format!(
                "{}{}{}{}{}{}{}\n", 
                timestamp, delimiter, 
                scan_mode, delimiter,
                quote_char, escaped_json, quote_char
            ));
        }
    }

    Ok(csv_content)
}

/// Generate authors summary for CSV export
fn export_authors_csv_summary(
    csv_content: &mut String,
    collected_data: &[ScanMessage],
    config: &ExportConfig,
    delimiter: &str,
    quote_char: &str,
) -> PluginResult<()> {
    let sorted_authors = generate_authors_summary(collected_data);
    
    // Determine how many recent commits to show (respecting output limit)
    let recent_limit = if config.output_all {
        usize::MAX
    } else {
        3.min(config.max_entries.unwrap_or(3)) // Default to 3 recent commits per author
    };
    
    // CSV header for authors summary
    csv_content.push_str(&format!("author{}total_commits{}lines_added{}lines_removed{}recent_activity\n", 
        delimiter, delimiter, delimiter, delimiter));
    
    // CSV rows - one per author
    for (author, (commits, lines_added, lines_removed, recent_commits)) in sorted_authors {
        // Escape author name if needed
        let escaped_author = if author.contains(delimiter) || author.contains(quote_char) {
            if quote_char == "\"" {
                format!("\"{}\"", author.replace('"', "\"\""))
            } else {
                format!("{}{}{}", quote_char, author.replace(quote_char, &format!("{}{}", quote_char, quote_char)), quote_char)
            }
        } else {
            author.clone()
        };
        
        // Format recent activity as a summary string (not individual commits to keep CSV clean)
        let recent_activity = if recent_commits.is_empty() {
            "No recent activity".to_string()
        } else {
            let latest_commit = &recent_commits[0];
            let datetime = chrono::DateTime::from_timestamp(latest_commit.2 as i64, 0)
                .unwrap_or_else(|| chrono::Utc::now());
            let formatted_date = datetime.format("%Y-%m-%d").to_string();
            
            format!("Last: {} ({} commits shown)", formatted_date, recent_commits.len().min(recent_limit))
        };
        
        // Escape recent activity if needed
        let escaped_activity = if recent_activity.contains(delimiter) || recent_activity.contains(quote_char) {
            if quote_char == "\"" {
                format!("\"{}\"", recent_activity.replace('"', "\"\""))
            } else {
                format!("{}{}{}", quote_char, recent_activity.replace(quote_char, &format!("{}{}", quote_char, quote_char)), quote_char)
            }
        } else {
            recent_activity
        };
        
        csv_content.push_str(&format!(
            "{}{}{}{}{}{}{}{}{}\n", 
            escaped_author, delimiter,
            commits, delimiter,
            lines_added, delimiter,
            lines_removed, delimiter,
            escaped_activity
        ));
    }
    
    Ok(())
}
