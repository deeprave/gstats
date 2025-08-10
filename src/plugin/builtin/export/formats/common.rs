//! Common utilities for export formats

use crate::scanner::messages::{ScanMessage, MessageData};
use std::collections::HashMap;

/// Check if the data is primarily commit data (for authors reports)
pub fn is_commit_data(data: &[&ScanMessage]) -> bool {
    data.iter()
        .any(|msg| matches!(msg.data, MessageData::CommitInfo { .. }))
}

// Removed unused utility functions: escape_html, escape_xml, format_timestamp - no current usage found

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
