//! Intermediate Data Format
//!
//! Provides common data transformation utilities for plugins to avoid
//! serialization concerns in individual plugins. Export plugins can
//! handle final format conversion.

use crate::scanner::messages::{ScanMessage, MessageData};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Intermediate data representation for plugin processing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IntermediateData {
    /// File information in a normalized format
    FileData {
        path: String,
        size: u64,
        lines: u32,
        metadata: HashMap<String, serde_json::Value>,
    },
    
    /// Commit information in a normalized format
    CommitData {
        hash: String,
        author: String,
        message: String,
        timestamp: i64,
        files_changed: Vec<FileChangeData>,
        metadata: HashMap<String, serde_json::Value>,
    },
    
    /// Change frequency data in a normalized format
    ChangeFrequencyData {
        file_path: String,
        change_count: u32,
        author_count: u32,
        frequency_metrics: FrequencyMetrics,
        authors: Vec<String>,
        metadata: HashMap<String, serde_json::Value>,
    },
    
    /// Metrics data in a normalized format
    MetricsData {
        file_count: u32,
        line_count: u64,
        complexity: f64,
        metrics: HashMap<String, f64>,
        metadata: HashMap<String, serde_json::Value>,
    },
    
    /// Generic key-value data for extensibility
    GenericData {
        data_type: String,
        fields: HashMap<String, serde_json::Value>,
    },
}

/// File change data for commits
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileChangeData {
    pub path: String,
    pub lines_added: usize,
    pub lines_removed: usize,
    pub change_type: ChangeType,
}

/// Type of file change
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed { from: String },
    Copied { from: String },
}

/// Frequency metrics for change frequency analysis
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrequencyMetrics {
    pub frequency_score: f64,
    pub recency_weight: f64,
    pub last_changed: i64,
    pub first_changed: i64,
    pub change_velocity: f64, // changes per day
}

/// Transformation utilities for converting between formats
pub struct DataTransformer;

impl DataTransformer {
    /// Convert a ScanMessage to IntermediateData
    pub fn from_scan_message(message: &ScanMessage) -> Option<IntermediateData> {
        match message.data() {
            MessageData::FileInfo { path, size, lines } => {
                Some(IntermediateData::FileData {
                    path: path.clone(),
                    size: *size,
                    lines: *lines,
                    metadata: HashMap::new(),
                })
            }
            
            MessageData::CommitInfo { hash, author, message: msg, timestamp, changed_files } => {
                let files_changed = changed_files.iter().map(|f| FileChangeData {
                    path: f.path.clone(),
                    lines_added: f.lines_added,
                    lines_removed: f.lines_removed,
                    change_type: ChangeType::Modified, // Default - could be enhanced
                }).collect();
                
                Some(IntermediateData::CommitData {
                    hash: hash.clone(),
                    author: author.clone(),
                    message: msg.clone(),
                    timestamp: *timestamp,
                    files_changed,
                    metadata: HashMap::new(),
                })
            }
            
            MessageData::ChangeFrequencyInfo { 
                file_path, change_count, author_count, last_changed, 
                first_changed, frequency_score, recency_weight, authors 
            } => {
                let frequency_metrics = FrequencyMetrics {
                    frequency_score: *frequency_score,
                    recency_weight: *recency_weight,
                    last_changed: *last_changed,
                    first_changed: *first_changed,
                    change_velocity: if *last_changed > *first_changed {
                        let days = (*last_changed - *first_changed) as f64 / 86400.0; // seconds to days
                        *change_count as f64 / days.max(1.0)
                    } else {
                        0.0
                    },
                };
                
                Some(IntermediateData::ChangeFrequencyData {
                    file_path: file_path.clone(),
                    change_count: *change_count,
                    author_count: *author_count,
                    frequency_metrics,
                    authors: authors.clone(),
                    metadata: HashMap::new(),
                })
            }
            
            MessageData::MetricInfo { file_count, line_count, complexity } => {
                let mut metrics = HashMap::new();
                metrics.insert("complexity".to_string(), *complexity);
                
                Some(IntermediateData::MetricsData {
                    file_count: *file_count,
                    line_count: *line_count,
                    complexity: *complexity,
                    metrics,
                    metadata: HashMap::new(),
                })
            }
            
            MessageData::DependencyInfo { name, version, license } => {
                let mut fields = HashMap::new();
                fields.insert("name".to_string(), serde_json::Value::String(name.clone()));
                fields.insert("version".to_string(), serde_json::Value::String(version.clone()));
                if let Some(license) = license {
                    fields.insert("license".to_string(), serde_json::Value::String(license.clone()));
                }
                
                Some(IntermediateData::GenericData {
                    data_type: "dependency".to_string(),
                    fields,
                })
            }
            
            MessageData::SecurityInfo { vulnerability, severity, location } => {
                let mut fields = HashMap::new();
                fields.insert("vulnerability".to_string(), serde_json::Value::String(vulnerability.clone()));
                fields.insert("severity".to_string(), serde_json::Value::String(severity.clone()));
                fields.insert("location".to_string(), serde_json::Value::String(location.clone()));
                
                Some(IntermediateData::GenericData {
                    data_type: "security".to_string(),
                    fields,
                })
            }
            
            MessageData::PerformanceInfo { function, execution_time, memory_usage } => {
                let mut fields = HashMap::new();
                fields.insert("function".to_string(), serde_json::Value::String(function.clone()));
                fields.insert("execution_time".to_string(), serde_json::Value::Number(
                    serde_json::Number::from_f64(*execution_time).unwrap_or_else(|| serde_json::Number::from(0))
                ));
                fields.insert("memory_usage".to_string(), serde_json::Value::Number(
                    serde_json::Number::from(*memory_usage)
                ));
                
                Some(IntermediateData::GenericData {
                    data_type: "performance".to_string(),
                    fields,
                })
            }
            
            MessageData::None => None,
        }
    }
    
    /// Add metadata to intermediate data
    pub fn add_metadata(mut data: IntermediateData, key: String, value: serde_json::Value) -> IntermediateData {
        match &mut data {
            IntermediateData::FileData { metadata, .. } |
            IntermediateData::CommitData { metadata, .. } |
            IntermediateData::ChangeFrequencyData { metadata, .. } |
            IntermediateData::MetricsData { metadata, .. } => {
                metadata.insert(key, value);
            }
            IntermediateData::GenericData { fields, .. } => {
                fields.insert(key, value);
            }
        }
        data
    }
    
    /// Extract common fields for export processing
    pub fn extract_common_fields(data: &IntermediateData) -> HashMap<String, serde_json::Value> {
        let mut fields = HashMap::new();
        
        match data {
            IntermediateData::FileData { path, size, lines, .. } => {
                fields.insert("type".to_string(), serde_json::Value::String("file".to_string()));
                fields.insert("path".to_string(), serde_json::Value::String(path.clone()));
                fields.insert("size".to_string(), serde_json::Value::Number((*size).into()));
                fields.insert("lines".to_string(), serde_json::Value::Number((*lines).into()));
            }
            
            IntermediateData::CommitData { hash, author, timestamp, .. } => {
                fields.insert("type".to_string(), serde_json::Value::String("commit".to_string()));
                fields.insert("hash".to_string(), serde_json::Value::String(hash.clone()));
                fields.insert("author".to_string(), serde_json::Value::String(author.clone()));
                fields.insert("timestamp".to_string(), serde_json::Value::Number((*timestamp).into()));
            }
            
            IntermediateData::ChangeFrequencyData { file_path, change_count, .. } => {
                fields.insert("type".to_string(), serde_json::Value::String("change_frequency".to_string()));
                fields.insert("file_path".to_string(), serde_json::Value::String(file_path.clone()));
                fields.insert("change_count".to_string(), serde_json::Value::Number((*change_count).into()));
            }
            
            IntermediateData::MetricsData { file_count, line_count, complexity, .. } => {
                fields.insert("type".to_string(), serde_json::Value::String("metrics".to_string()));
                fields.insert("file_count".to_string(), serde_json::Value::Number((*file_count).into()));
                fields.insert("line_count".to_string(), serde_json::Value::Number((*line_count).into()));
                fields.insert("complexity".to_string(), serde_json::Value::Number(
                    serde_json::Number::from_f64(*complexity).unwrap_or_else(|| serde_json::Number::from(0))
                ));
            }
            
            IntermediateData::GenericData { data_type, fields: data_fields } => {
                fields.insert("type".to_string(), serde_json::Value::String(data_type.clone()));
                fields.extend(data_fields.clone());
            }
        }
        
        fields
    }
    
    /// Convert intermediate data to JSON for export plugins
    pub fn to_json(data: &IntermediateData) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(data)
    }
    
    /// Convert intermediate data from JSON
    pub fn from_json(value: &serde_json::Value) -> serde_json::Result<IntermediateData> {
        serde_json::from_value(value.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::messages::{MessageHeader, FileChangeData as ScanFileChangeData};
    use crate::scanner::modes::ScanMode;

    #[test]
    fn test_file_data_transformation() {
        let header = MessageHeader::new(ScanMode::FILES, 0);
        let data = MessageData::FileInfo {
            path: "src/main.rs".to_string(),
            size: 1024,
            lines: 50,
        };
        let message = ScanMessage::new(header, data);
        
        let intermediate = DataTransformer::from_scan_message(&message).unwrap();
        
        match intermediate {
            IntermediateData::FileData { path, size, lines, .. } => {
                assert_eq!(path, "src/main.rs");
                assert_eq!(size, 1024);
                assert_eq!(lines, 50);
            }
            _ => panic!("Expected FileData"),
        }
    }

    #[test]
    fn test_commit_data_transformation() {
        let header = MessageHeader::new(ScanMode::HISTORY, 0);
        let data = MessageData::CommitInfo {
            hash: "abc123".to_string(),
            author: "John Doe".to_string(),
            message: "Fix bug".to_string(),
            timestamp: 1234567890,
            changed_files: vec![ScanFileChangeData {
                path: "src/lib.rs".to_string(),
                lines_added: 10,
                lines_removed: 5,
            }],
        };
        let message = ScanMessage::new(header, data);
        
        let intermediate = DataTransformer::from_scan_message(&message).unwrap();
        
        match intermediate {
            IntermediateData::CommitData { hash, author, files_changed, .. } => {
                assert_eq!(hash, "abc123");
                assert_eq!(author, "John Doe");
                assert_eq!(files_changed.len(), 1);
                assert_eq!(files_changed[0].path, "src/lib.rs");
            }
            _ => panic!("Expected CommitData"),
        }
    }

    #[test]
    fn test_metadata_addition() {
        let data = IntermediateData::FileData {
            path: "test.rs".to_string(),
            size: 100,
            lines: 10,
            metadata: HashMap::new(),
        };
        
        let data_with_metadata = DataTransformer::add_metadata(
            data,
            "plugin".to_string(),
            serde_json::Value::String("test_plugin".to_string()),
        );
        
        match data_with_metadata {
            IntermediateData::FileData { metadata, .. } => {
                assert_eq!(metadata.get("plugin").unwrap(), &serde_json::Value::String("test_plugin".to_string()));
            }
            _ => panic!("Expected FileData"),
        }
    }

    #[test]
    fn test_common_fields_extraction() {
        let data = IntermediateData::FileData {
            path: "test.rs".to_string(),
            size: 100,
            lines: 10,
            metadata: HashMap::new(),
        };
        
        let fields = DataTransformer::extract_common_fields(&data);
        
        assert_eq!(fields.get("type").unwrap(), &serde_json::Value::String("file".to_string()));
        assert_eq!(fields.get("path").unwrap(), &serde_json::Value::String("test.rs".to_string()));
        assert_eq!(fields.get("size").unwrap(), &serde_json::Value::Number(100.into()));
    }

    #[test]
    fn test_json_serialization() {
        let data = IntermediateData::FileData {
            path: "test.rs".to_string(),
            size: 100,
            lines: 10,
            metadata: HashMap::new(),
        };
        
        let json = DataTransformer::to_json(&data).unwrap();
        let roundtrip = DataTransformer::from_json(&json).unwrap();
        
        assert_eq!(data, roundtrip);
    }
}
