//! Common utilities for export formats

use crate::scanner::messages::{ScanMessage, MessageData};
use std::collections::HashMap;

/// Check if the data is primarily commit data (for authors reports)
pub fn is_commit_data(data: &[&ScanMessage]) -> bool {
    data.iter()
        .any(|msg| matches!(msg.data, MessageData::CommitInfo { .. }))
}

/// Escape HTML characters
pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Escape XML characters
pub fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Format timestamp as human-readable date
pub fn format_timestamp(timestamp: u64) -> String {
    use std::time::{UNIX_EPOCH, Duration};
    
    if let Some(datetime) = UNIX_EPOCH.checked_add(Duration::from_secs(timestamp)) {
        // Format as YYYY-MM-DD HH:MM
        let since_epoch = datetime.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let days = since_epoch / 86400;
        let hours = (since_epoch % 86400) / 3600;
        let minutes = (since_epoch % 3600) / 60;
        
        // Simple date calculation (approximate)
        let year = 1970 + (days / 365);
        let day_of_year = days % 365;
        let month = (day_of_year / 30) + 1;
        let day = (day_of_year % 30) + 1;
        
        format!("{:04}-{:02}-{:02} {:02}:{:02}", year, month.min(12), day.min(31), hours, minutes)
    } else {
        format!("{}", timestamp)
    }
}

/// Generate authors summary data structure
pub fn generate_authors_summary(collected_data: &[ScanMessage]) -> Vec<(String, (u32, u32, u32, Vec<(String, String, u64, u32)>))> {
    let mut author_stats: HashMap<String, (u32, u32, u32, Vec<(String, String, u64, u32)>)> = HashMap::new();
    
    // Process ALL data for accurate statistics
    for message in collected_data {
        if let MessageData::CommitInfo { hash, author, message: commit_msg, timestamp, changed_files } = &message.data {
            let total_files = changed_files.len() as u32;
            let total_added: u32 = changed_files.iter().map(|f| f.lines_added as u32).sum();
            let total_removed: u32 = changed_files.iter().map(|f| f.lines_removed as u32).sum();
            
            let entry = author_stats.entry(author.clone()).or_insert((0, 0, 0, Vec::new()));
            entry.0 += 1; // commit count
            entry.1 += total_added; // lines added
            entry.2 += total_removed; // lines removed
            
            // Store commit summary (first line only)
            let first_line = commit_msg.lines().next().unwrap_or("").trim().to_string();
            entry.3.push((hash.clone(), first_line, *timestamp as u64, total_files));
        }
    }
    
    // Sort authors by commit count (descending)
    let mut sorted_authors: Vec<_> = author_stats.into_iter().collect();
    sorted_authors.sort_by(|a, b| b.1.0.cmp(&a.1.0));
    
    sorted_authors
}
